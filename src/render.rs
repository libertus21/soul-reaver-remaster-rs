//! wgpu fullscreen-quad renderer (`dds-texture-viewer` + `bc7-cpu-decode`)
//! plus the flat-shaded mesh pipeline (`models-3d`: [`show_mesh`]),
//! the textured mesh pipeline (`mesh-textures`: [`show_mesh_textured`]),
//! and the textured mesh gallery (`mesh-batch`: [`show_mesh_gallery`]).
//!
//! One job: take RGBA8 image bytes, upload them to a GPU texture, and draw
//! them as a stretched fullscreen quad in a winit window until the user
//! closes it. Single upload path: [`show_rgba8`]. BC7 textures arrive
//! already CPU-decoded to RGBA8, so no compressed GPU format (and no
//! `TEXTURE_COMPRESSION_BC` feature) is ever requested. Deliberately
//! deferred (see design): sRGB correctness, resize sophistication beyond
//! re-configuring the surface, mip selection.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::dds::{self, DecodedImage, LoadedTexture};
use crate::gallery::{Direction, step_index};

/// Trivial blit shader: one fullscreen triangle, no vertex buffers.
const SHADER_SRC: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(positions[i], 0.0, 1.0);
    out.uv = uvs[i];
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv);
}
"#;

/// GPU state for one viewer window. Owns the window handle so the
/// surface outlives nothing it depends on.
struct State<'window> {
    window: Arc<Window>,
    surface: wgpu::Surface<'window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

/// The single texture/sampler bind-group layout. Created once per
/// window and reused for every upload, so the gallery's per-step
/// re-upload always matches the pipeline layout.
fn make_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("dds bind group layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float {
                        filterable: true,
                    },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Upload RGBA8 bytes as a fresh texture + bind group under `layout`.
/// Shared by the single-image window and the gallery's per-step
/// re-upload (the pipeline is reused in both cases).
fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> wgpu::BindGroup {
    let tex_size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("dds texture"),
        size: tex_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    // RGBA8 packs 4 bytes per texel.
    let bytes_per_row = Some(4 * width);
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row,
            rows_per_image: Some(height),
        },
        tex_size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("dds bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    })
}

/// GPU core shared by the blit window and the mesh windows: surface +
/// device + queue + configured surface. Extracted verbatim from the
/// blit setup so all windows open devices the same way (adapter's own
/// limits, no special features); pipelines/uploads stay per-window.
struct GpuCore<'window> {
    surface: wgpu::Surface<'window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

async fn open_core(window: Arc<Window>, label: &str) -> Result<GpuCore<'_>, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let surface = instance
        .create_surface(window.clone())
        .map_err(|e| format!("cannot create GPU surface: {e}"))?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no usable GPU adapter: {e}"))?;
    // Request the adapter's own limits (not `Limits::default()`): this
    // viewer needs nothing special, so take whatever the GPU offers.
    // `Limits::default()` can exceed what low-capability / software
    // adapters allow and fail device creation there.
    // No special features are requested: RGBA8 rendering works on any
    // adapter, including ones without BC compression support.
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some(label),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("cannot open GPU device: {e}"))?;

    let win_size = window.inner_size();
    let (cfg_w, cfg_h) = (win_size.width.max(1), win_size.height.max(1));
    let config = surface
        .get_default_config(&adapter, cfg_w, cfg_h)
        .ok_or_else(|| "GPU adapter does not support this window surface".to_string())?;
    surface.configure(&device, &config);
    Ok(GpuCore { surface, device, queue, config })
}

impl<'window> State<'window> {
    async fn new(
        window: Arc<Window>,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<Self, String> {
        let expected_len = (width as usize) * (height as usize) * 4;
        assert_eq!(
            pixels.len(),
            expected_len,
            "upload buffer must hold exactly one Rgba8Unorm image ({width}x{height})"
        );

        let core = open_core(window.clone(), "dds viewer").await?;
        let GpuCore { surface, device, queue, config } = core;

        let bind_group_layout = make_bind_group_layout(&device);
        let bind_group = upload_texture(&device, &queue, &bind_group_layout, width, height, pixels);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("dds blit"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });
        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("dds pipeline layout"),
                bind_group_layouts: &[Some(&bind_group_layout)],
                immediate_size: 0,
            });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("dds blit"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group_layout,
            bind_group,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn set_image(&mut self, width: u32, height: u32, pixels: &[u8]) {
        self.bind_group = upload_texture(
            &self.device,
            &self.queue,
            &self.bind_group_layout,
            width,
            height,
            pixels,
        );
    }

    fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("dds blit"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("dds blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(frame);
    }
}

struct App<'a> {
    width: u32,
    height: u32,
    pixels: &'a [u8],
    state: Option<State<'a>>,
    init_error: Option<String>,
}

impl<'a> ApplicationHandler for App<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title("Soul Reaver DDS viewer")
                .with_inner_size(PhysicalSize::new(self.width, self.height)),
        ) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.init_error = Some(format!("cannot create window: {e}"));
                event_loop.exit();
                return;
            }
        };
        match pollster::block_on(State::new(window, self.width, self.height, self.pixels))
        {
            Ok(state) => self.state = Some(state),
            Err(e) => {
                self.init_error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };
        if window_id != state.window.id() {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => state.render(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

/// Open a window showing `pixels` (RGBA8, `width` x `height`) and block
/// until the user closes it.
pub fn show_rgba8(width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    show(width, height, pixels)
}

fn show(width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("cannot show an empty (0x0) image".to_string());
    }
    let event_loop =
        EventLoop::new().map_err(|e| format!("cannot create window event loop: {e}"))?;
    let mut app = App {
        width,
        height,
        pixels,
        state: None,
        init_error: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| format!("window event loop failed: {e}"))?;
    match app.init_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Decode one gallery entry to RGBA8 through the EXISTING path.
fn decode_entry(path: &Path) -> Result<DecodedImage, dds::ViewerError> {
    match dds::load_texture(path)? {
        LoadedTexture::Rgba8(image) => Ok(image),
        LoadedTexture::Bc7(compressed) => Ok(dds::decode_bc7_to_rgba8(&compressed)),
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn entry_label(index: usize, total: usize, path: &Path) -> String {
    format!("{}/{} {}", index + 1, total, display_name(path))
}

struct GalleryApp<'a> {
    entries: &'a [PathBuf],
    index: usize,
    current: DecodedImage,
    state: Option<State<'a>>,
    init_error: Option<String>,
}

fn gallery_step(
    entries: &[PathBuf],
    index: &mut usize,
    current: &mut DecodedImage,
    state: &mut State<'_>,
    dir: Direction,
) {
    let total = entries.len();
    let mut next = *index;
    loop {
        let candidate = step_index(next, total, dir);
        if candidate == next {
            break;
        }
        next = candidate;
        match decode_entry(&entries[next]) {
            Ok(image) => {
                let label = entry_label(next, total, &entries[next]);
                state.set_image(image.width, image.height, &image.rgba8);
                state.window.set_title(&label);
                println!("{label}");
                *current = image;
                *index = next;
                state.window.request_redraw();
                break;
            }
            Err(e) => {
                eprintln!("warning: skipping {}: {e}", display_name(&entries[next]));
            }
        }
    }
}

impl<'a> ApplicationHandler for GalleryApp<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
        let label = entry_label(self.index, self.entries.len(), &self.entries[self.index]);
        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title(&label)
                .with_inner_size(PhysicalSize::new(self.current.width, self.current.height)),
        ) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.init_error = Some(format!("cannot create window: {e}"));
                event_loop.exit();
                return;
            }
        };
        match pollster::block_on(State::new(
            window,
            self.current.width,
            self.current.height,
            &self.current.rgba8,
        )) {
            Ok(state) => {
                println!("{label}");
                self.state = Some(state);
            }
            Err(e) => {
                self.init_error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };
        if window_id != state.window.id() {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => state.render(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed || event.repeat {
                    return;
                }
                match event.logical_key {
                    Key::Named(NamedKey::ArrowRight) => gallery_step(
                        self.entries,
                        &mut self.index,
                        &mut self.current,
                        state,
                        Direction::Next,
                    ),
                    Key::Named(NamedKey::ArrowLeft) => gallery_step(
                        self.entries,
                        &mut self.index,
                        &mut self.current,
                        state,
                        Direction::Prev,
                    ),
                    Key::Named(NamedKey::Escape) => event_loop.exit(),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

pub fn show_gallery(entries: &[PathBuf]) -> Result<(), String> {
    if entries.is_empty() {
        return Err("no .DDS files to show".to_string());
    }
    let mut first: Option<(usize, DecodedImage)> = None;
    for (index, path) in entries.iter().enumerate() {
        match decode_entry(path) {
            Ok(image) => {
                first = Some((index, image));
                break;
            }
            Err(e) => {
                eprintln!("warning: skipping {}: {e}", display_name(path));
            }
        }
    }
    let Some((index, current)) = first else {
        let dir = entries[0]
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".".to_string());
        return Err(format!(
            "no decodable .DDS file in directory '{dir}' ({} entries, all skipped)",
            entries.len()
        ));
    };
    let event_loop =
        EventLoop::new().map_err(|e| format!("cannot create window event loop: {e}"))?;
    let mut app = GalleryApp {
        entries,
        index,
        current,
        state: None,
        init_error: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| format!("window event loop failed: {e}"))?;
    match app.init_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

// --- `models-3d` mesh path (flat-shaded pipeline) ---

const MESH_SHADER_SRC: &str = r#"
struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
};

@group(0) @binding(0) var<uniform> mvp: mat4x4<f32>;

@vertex
fn vs(@location(0) pos: vec3<f32>) -> VOut {
    var out: VOut;
    out.clip = mvp * vec4<f32>(pos, 1.0);
    out.world = pos;
    return out;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let n = normalize(cross(dpdx(in.world), dpdy(in.world)));
    let l = normalize(vec3<f32>(0.4, 0.8, 0.6));
    let d = abs(dot(n, l));
    let base = vec3<f32>(0.55, 0.75, 0.9);
    return vec4<f32>(base * (0.25 + 0.75 * d), 1.0);
}
"#;

pub fn fit_positions(positions: &[[f32; 3]]) -> Result<Vec<[f32; 3]>, String> {
    if positions.is_empty() {
        return Err("cannot frame an empty mesh (0 vertices)".to_string());
    }
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in positions {
        if !p.iter().all(|c| c.is_finite()) {
            return Err("cannot frame a mesh with non-finite coordinates".to_string());
        }
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    let extent = (0..3).map(|i| max[i] - min[i]).fold(0.0f32, f32::max);
    if !(extent > 0.0) || !extent.is_finite() {
        return Err("cannot frame a degenerate mesh (all positions identical)".to_string());
    }
    let center = [
        (min[0] + max[0]) / 2.0,
        (min[1] + max[1]) / 2.0,
        (min[2] + max[2]) / 2.0,
    ];
    let scale = 1.0 / extent;
    Ok(positions
        .iter()
        .map(|p| {
            [
                (p[0] - center[0]) * scale,
                (p[1] - center[1]) * scale,
                (p[2] - center[2]) * scale,
            ]
        })
        .collect())
}

fn mul4(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            out[col][row] = a[0][row] * b[col][0]
                + a[1][row] * b[col][1]
                + a[2][row] * b[col][2]
                + a[3][row] * b[col][3];
        }
    }
    out
}

fn mvp_for(angle: f32, aspect: f32) -> [[f32; 4]; 4] {
    let (s, c) = angle.sin_cos();
    let rot = [
        [c, 0.0, -s, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [s, 0.0, c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let view = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, -2.2, 1.0],
    ];
    let f = 1.0 / (std::f32::consts::FRAC_PI_8).tan();
    let (near, far) = (0.1, 100.0);
    let proj = [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), -1.0],
        [0.0, 0.0, -far * near / (far - near), 0.0],
    ];
    mul4(&proj, &mul4(&view, &rot))
}

fn mat_bytes(m: &[[f32; 4]; 4]) -> [u8; 64] {
    let mut out = [0u8; 64];
    for col in 0..4 {
        for row in 0..4 {
            out[(col * 4 + row) * 4..][..4].copy_from_slice(&m[col][row].to_le_bytes());
        }
    }
    out
}

struct MeshState<'window> {
    window: Arc<Window>,
    surface: wgpu::Surface<'window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    depth_view: wgpu::TextureView,
    start: std::time::Instant,
}

impl<'window> MeshState<'window> {
    fn make_depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("srm depth"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        tex.create_view(&wgpu::TextureViewDescriptor::default())
    }

    async fn new(
        window: Arc<Window>,
        vertices: &[[f32; 3]],
        indices: &[u16],
    ) -> Result<Self, String> {
        let mut vbytes = Vec::with_capacity(vertices.len() * 12);
        for v in vertices {
            for c in v {
                vbytes.extend_from_slice(&c.to_le_bytes());
            }
        }
        let mut ibytes = Vec::with_capacity(indices.len() * 2);
        for i in indices {
            ibytes.extend_from_slice(&i.to_le_bytes());
        }
        let core = open_core(window.clone(), "srm mesh").await?;
        let GpuCore { surface, device, queue, config } = core;
        let depth_view = Self::make_depth(&device, config.width, config.height);
        fn upload(device: &wgpu::Device, queue: &wgpu::Queue, label: &str, bytes: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes.len() as u64,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buf, 0, bytes);
            buf
        }
        let vertex_buffer = upload(&device, &queue, "srm vertices", &vbytes, wgpu::BufferUsages::VERTEX);
        let index_buffer = upload(&device, &queue, "srm indices", &ibytes, wgpu::BufferUsages::INDEX);
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("srm mvp"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("srm uniform layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("srm uniform"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("srm mesh"),
            source: wgpu::ShaderSource::Wgsl(MESH_SHADER_SRC.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("srm pipeline layout"),
            bind_group_layouts: &[Some(&uniform_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("srm mesh"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 12,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    }],
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            uniform_buffer,
            uniform_bind_group,
            depth_view,
            start: std::time::Instant::now(),
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = Self::make_depth(&self.device, size.width, size.height);
        }
    }

    fn render(&mut self) {
        let aspect = self.config.width.max(1) as f32 / self.config.height.max(1) as f32;
        let mvp = mvp_for(self.start.elapsed().as_secs_f32() * 0.6, aspect);
        self.queue.write_buffer(&self.uniform_buffer, 0, &mat_bytes(&mvp));
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("srm mesh"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("srm mesh"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                multiview_mask: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..self.index_count, 0, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(frame);
    }
}

struct MeshApp {
    title: String,
    vertices: Vec<[f32; 3]>,
    indices: Vec<u16>,
    state: Option<MeshState<'static>>,
    init_error: Option<String>,
}

impl ApplicationHandler for MeshApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);
        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title(&self.title)
                .with_inner_size(PhysicalSize::new(800, 600)),
        ) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.init_error = Some(format!("cannot create window: {e}"));
                event_loop.exit();
                return;
            }
        };
        let vertices = std::mem::take(&mut self.vertices);
        let indices = std::mem::take(&mut self.indices);
        match pollster::block_on(MeshState::new(window, &vertices, &indices)) {
            Ok(state) => {
                self.state = Some(state);
                self.vertices = vertices;
                self.indices = indices;
            }
            Err(e) => {
                self.vertices = vertices;
                self.indices = indices;
                self.init_error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };
        if window_id != state.window.id() {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => state.render(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed || event.repeat {
                    return;
                }
                if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

pub fn show_mesh(positions: &[[f32; 3]], indices: &[u16], title: &str) -> Result<(), String> {
    let fitted = fit_positions(positions)?;
    if indices.is_empty() {
        return Err("cannot show a mesh with 0 indices".to_string());
    }
    if indices.len() % 3 != 0 {
        return Err(format!(
            "cannot show a mesh with {} indices (one triangle is 3 indices)",
            indices.len()
        ));
    }
    let event_loop =
        EventLoop::new().map_err(|e| format!("cannot create window event loop: {e}"))?;
    let mut app = MeshApp {
        title: title.to_string(),
        vertices: fitted,
        indices: indices.to_vec(),
        state: None,
        init_error: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| format!("window event loop failed: {e}"))?;
    match app.init_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

// --- `mesh-textures` + `mesh-batch` paths ---

const MESH_TEXTURED_SHADER_SRC: &str = r#"
struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct Uniforms {
    mvp: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var tex_d: texture_2d<f32>;
@group(1) @binding(1) var tex_n: texture_2d<f32>;
@group(1) @binding(2) var tex_s: texture_2d<f32>;
@group(1) @binding(3) var samp: sampler;

@vertex
fn vs(@location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>) -> VOut {
    var out: VOut;
    out.clip = u.mvp * vec4<f32>(pos, 1.0);
    out.world = pos;
    out.uv = uv;
    return out;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let geo_n = normalize(cross(dpdx(in.world), dpdy(in.world)));

    let dp1 = dpdx(in.world);
    let dp2 = dpdy(in.world);
    let duv1 = dpdx(in.uv);
    let duv2 = dpdy(in.uv);
    let T = normalize(dp1 * duv2.y - dp2 * duv1.y + vec3<f32>(1e-6, 0.0, 0.0));
    let B = normalize(cross(geo_n, T));
    let tbn = mat3x3<f32>(T, B, geo_n);

    let nm_sample = textureSample(tex_n, samp, in.uv).xyz * 2.0 - 1.0;
    let n = normalize(tbn * normalize(nm_sample));

    let d = textureSample(tex_d, samp, in.uv).rgb;
    let s = textureSample(tex_s, samp, in.uv).rgb;

    let light_dir = normalize(vec3<f32>(0.4, 0.8, 0.6));
    let view_dir = normalize(vec3<f32>(0.0, 0.0, 1.0));
    let half_v = normalize(light_dir + view_dir);

    let ndl = max(dot(n, light_dir), 0.0);
    let ndh = max(dot(n, half_v), 0.0);
    let spec_intensity = s.r;
    let spec = pow(ndh, 32.0) * spec_intensity;

    let lit = d * (0.20 + 0.80 * ndl) + vec3<f32>(spec);
    return vec4<f32>(lit, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TVertex {
    pos: [f32; 3],
    uv: [f32; 2],
    _pad: f32,
}

fn upload_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    img: &DecodedImage,
    label: &str,
) -> wgpu::Texture {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: img.width,
            height: img.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &img.rgba8,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * img.width),
            rows_per_image: Some(img.height),
        },
        wgpu::Extent3d {
            width: img.width,
            height: img.height,
            depth_or_array_layers: 1,
        },
    );
    tex
}

/// Validate the shape contract shared by `show_mesh_textured` and
/// `set_mesh`. Pure; testable without a GPU.
fn validate_entry_shape(entry: &crate::mesh_entry::MeshEntry) -> Result<(), String> {
    let positions = &entry.positions;
    let uvs = &entry.uvs;
    let material_indices = &entry.material_indices;
    let indices = &entry.indices;
    let materials = &entry.materials;
    let textures = &entry.textures;

    if positions.len() != uvs.len() || positions.len() != material_indices.len() {
        return Err(format!(
            "mesh arrays disagree: {} positions, {} uvs, {} material indices",
            positions.len(),
            uvs.len(),
            material_indices.len(),
        ));
    }
    if materials.len() != textures.len() {
        return Err(format!(
            "materials/textures disagree: {} materials, {} textures",
            materials.len(),
            textures.len(),
        ));
    }
    if indices.is_empty() || indices.len() % 3 != 0 {
        return Err(format!(
            "cannot show a mesh with {} indices (one triangle is 3 indices)",
            indices.len()
        ));
    }
    for (i, &mi) in material_indices.iter().enumerate() {
        if (mi as usize) == 0 || (mi as usize) > materials.len() {
            return Err(format!(
                "vertex {i} references material index {mi} but only {} materials exist (indices are 1-based)",
                materials.len()
            ));
        }
    }
    Ok(())
}

struct TexturedMeshState<'window> {
    window: Arc<Window>,
    surface: wgpu::Surface<'window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    depth_view: wgpu::TextureView,
    start: std::time::Instant,
    vertex_buffer: wgpu::Buffer,
    index_buffers: Vec<(u32, wgpu::Buffer, u32)>,
    texture_bind_groups: Vec<wgpu::BindGroup>,
}

impl<'window> TexturedMeshState<'window> {
    fn make_depth(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("srm textured depth"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        tex.create_view(&wgpu::TextureViewDescriptor::default())
    }

    async fn new(
        window: Arc<Window>,
        entry: &crate::mesh_entry::MeshEntry,
    ) -> Result<Self, String> {
        let core = open_core(window.clone(), "srm textured").await?;
        let GpuCore { surface, device, queue, config } = core;

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("srm textured mvp"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("srm textured uniform layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("srm textured uniform"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("srm textured tex layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("srm textured sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("srm textured"),
            source: wgpu::ShaderSource::Wgsl(MESH_TEXTURED_SHADER_SRC.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("srm textured pipeline layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("srm textured"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 12,
                            shader_location: 1,
                        },
                    ],
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let depth_view = Self::make_depth(&device, config.width, config.height);
        let dummy = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dummy"),
            size: 16,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let mut state = Self {
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            texture_layout,
            sampler,
            depth_view,
            start: std::time::Instant::now(),
            vertex_buffer: dummy,
            index_buffers: Vec::new(),
            texture_bind_groups: Vec::new(),
        };
        state.set_mesh(entry)?;
        Ok(state)
    }

    fn set_mesh(&mut self, entry: &crate::mesh_entry::MeshEntry) -> Result<(), String> {
        validate_entry_shape(entry)?;

        let fitted = fit_positions(&entry.positions)?;
        let vertices: Vec<TVertex> = fitted
            .iter()
            .zip(entry.uvs.iter())
            .map(|(p, uv)| TVertex {
                pos: *p,
                uv: [uv[0] as f32 / 255.0, uv[1] as f32 / 255.0],
                _pad: 0.0,
            })
            .collect();
        let vbytes: &[u8] = bytemuck::cast_slice(&vertices);
        let vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("srm textured vertices"),
            size: vbytes.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&vertex_buffer, 0, vbytes);

        let mut ranges: Vec<(usize, Vec<u16>)> =
            (0..entry.materials.len()).map(|i| (i + 1, Vec::new())).collect();
        for tri in entry.indices.chunks_exact(3) {
            let m = entry.material_indices[tri[0] as usize];
            if let Some((_, v)) = ranges.iter_mut().find(|(idx, _)| *idx == m as usize) {
                v.extend_from_slice(tri);
            }
        }
        let mut index_buffers = Vec::with_capacity(ranges.len());
        for (mat_idx, tri_indices) in &ranges {
            if tri_indices.is_empty() {
                continue;
            }
            let ibytes: Vec<u8> = tri_indices.iter().flat_map(|i| i.to_le_bytes()).collect();
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("srm textured indices"),
                size: ibytes.len() as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue.write_buffer(&buf, 0, &ibytes);
            index_buffers.push((*mat_idx as u32, buf, tri_indices.len() as u32));
        }

        let mut texture_bind_groups = Vec::with_capacity(entry.textures.len());
        for (i, t) in entry.textures.iter().enumerate() {
            let d = upload_rgba_texture(&self.device, &self.queue, &t.diffuse, "diffuse");
            let n = upload_rgba_texture(&self.device, &self.queue, &t.normal, "normal");
            let s = upload_rgba_texture(&self.device, &self.queue, &t.specular, "specular");
            let dv = d.create_view(&wgpu::TextureViewDescriptor::default());
            let nv = n.create_view(&wgpu::TextureViewDescriptor::default());
            let sv = s.create_view(&wgpu::TextureViewDescriptor::default());
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("mat {i} textures")),
                layout: &self.texture_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&dv) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&nv) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&sv) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            texture_bind_groups.push(bg);
        }

        self.vertex_buffer = vertex_buffer;
        self.index_buffers = index_buffers;
        self.texture_bind_groups = texture_bind_groups;
        Ok(())
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = Self::make_depth(&self.device, size.width, size.height);
        }
    }

    fn render(&mut self) {
        let aspect = self.config.width.max(1) as f32 / self.config.height.max(1) as f32;
        let mvp = mvp_for(self.start.elapsed().as_secs_f32() * 0.6, aspect);
        self.queue.write_buffer(&self.uniform_buffer, 0, &mat_bytes(&mvp));

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("srm textured"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("srm textured"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                multiview_mask: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            for (mat_idx, ibuf, count) in &self.index_buffers {
                let bg_idx = (*mat_idx as usize) - 1;
                if bg_idx < self.texture_bind_groups.len() {
                    pass.set_bind_group(1, &self.texture_bind_groups[bg_idx], &[]);
                    pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint16);
                    pass.draw_indexed(0..*count, 0, 0..1);
                }
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(frame);
    }
}

struct TexturedMeshApp {
    entry: crate::mesh_entry::MeshEntry,
    state: Option<TexturedMeshState<'static>>,
    init_error: Option<String>,
}

impl ApplicationHandler for TexturedMeshApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);
        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title(&self.entry.title)
                .with_inner_size(PhysicalSize::new(800, 600)),
        ) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.init_error = Some(format!("cannot create window: {e}"));
                event_loop.exit();
                return;
            }
        };
        match pollster::block_on(TexturedMeshState::new(window, &self.entry)) {
            Ok(state) => self.state = Some(state),
            Err(e) => {
                self.init_error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else { return };
        if window_id != state.window.id() { return; }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => state.render(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed || event.repeat { return; }
                if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

pub fn show_mesh_textured(
    positions: &[[f32; 3]],
    uvs: &[[u8; 2]],
    indices: &[u16],
    material_indices: &[u8],
    materials: &[crate::srm::Material],
    textures: &[crate::textures_object::MaterialTextures],
    title: &str,
) -> Result<(), String> {
    let entry = crate::mesh_entry::MeshEntry {
        srm_path: std::path::PathBuf::from(title),
        title: title.to_string(),
        positions: positions.to_vec(),
        uvs: uvs.to_vec(),
        indices: indices.to_vec(),
        material_indices: material_indices.to_vec(),
        materials: materials.to_vec(),
        textures: textures.to_vec(),
    };
    validate_entry_shape(&entry)?;
    let event_loop =
        EventLoop::new().map_err(|e| format!("cannot create window event loop: {e}"))?;
    let mut app = TexturedMeshApp { entry, state: None, init_error: None };
    event_loop
        .run_app(&mut app)
        .map_err(|e| format!("window event loop failed: {e}"))?;
    match app.init_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

// --- `mesh-batch` gallery ---

fn mesh_gallery_step(
    entries: &[PathBuf],
    tex_dir: &Path,
    index: &mut usize,
    current: &mut crate::mesh_entry::MeshEntry,
    state: &mut TexturedMeshState<'_>,
    dir: Direction,
) {
    let total = entries.len();
    let mut next = *index;
    loop {
        let candidate = step_index(next, total, dir);
        if candidate == next {
            break;
        }
        next = candidate;
        match crate::mesh_entry::load(&entries[next], tex_dir) {
            Ok(entry) => match state.set_mesh(&entry) {
                Ok(()) => {
                    let label = format!("{}/{} {}", next + 1, total, entry.title);
                    state.window.set_title(&label);
                    println!("{label}");
                    *current = entry;
                    *index = next;
                    state.window.request_redraw();
                    break;
                }
                Err(e) => {
                    eprintln!("warning: skipping {}: {e}", entries[next].display());
                }
            },
            Err(e) => {
                eprintln!("warning: skipping {}: {e}", entries[next].display());
            }
        }
    }
}

struct MeshGalleryApp<'a> {
    entries: &'a [PathBuf],
    tex_dir: &'a Path,
    index: usize,
    current: crate::mesh_entry::MeshEntry,
    state: Option<TexturedMeshState<'static>>,
    init_error: Option<String>,
}

impl<'a> ApplicationHandler for MeshGalleryApp<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);
        let total = self.entries.len();
        let label = format!("{}/{} {}", self.index + 1, total, self.current.title);
        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title(&label)
                .with_inner_size(PhysicalSize::new(800, 600)),
        ) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.init_error = Some(format!("cannot create window: {e}"));
                event_loop.exit();
                return;
            }
        };
        match pollster::block_on(TexturedMeshState::new(window, &self.current)) {
            Ok(state) => {
                println!("{label}");
                self.state = Some(state);
            }
            Err(e) => {
                self.init_error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else { return };
        if window_id != state.window.id() { return; }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => state.render(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed || event.repeat { return; }
                match event.logical_key {
                    Key::Named(NamedKey::ArrowRight) => mesh_gallery_step(
                        self.entries,
                        self.tex_dir,
                        &mut self.index,
                        &mut self.current,
                        state,
                        Direction::Next,
                    ),
                    Key::Named(NamedKey::ArrowLeft) => mesh_gallery_step(
                        self.entries,
                        self.tex_dir,
                        &mut self.index,
                        &mut self.current,
                        state,
                        Direction::Prev,
                    ),
                    Key::Named(NamedKey::Escape) => event_loop.exit(),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

pub fn show_mesh_gallery(entries: &[PathBuf], tex_dir: &Path) -> Result<(), String> {
    if entries.is_empty() {
        return Err("no .SRM files to show".to_string());
    }
    let mut first: Option<(usize, crate::mesh_entry::MeshEntry)> = None;
    for (index, path) in entries.iter().enumerate() {
        match crate::mesh_entry::load(path, tex_dir) {
            Ok(entry) => {
                first = Some((index, entry));
                break;
            }
            Err(e) => {
                eprintln!("warning: skipping {}: {e}", path.display());
            }
        }
    }
    let Some((index, current)) = first else {
        let dir = entries[0]
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".".to_string());
        return Err(format!(
            "no loadable .SRM in directory '{dir}' ({} entries, all skipped)",
            entries.len()
        ));
    };
    let event_loop =
        EventLoop::new().map_err(|e| format!("cannot create window event loop: {e}"))?;
    let mut app = MeshGalleryApp {
        entries,
        tex_dir,
        index,
        current,
        state: None,
        init_error: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| format!("window event loop failed: {e}"))?;
    match app.init_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod mesh_tests {
    use super::{fit_positions, mvp_for, validate_entry_shape};

    #[test]
    fn autofit_centers_and_scales_to_unit_extent() {
        let fitted = fit_positions(&[[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 2.0, 0.0]])
            .expect("non-degenerate mesh must fit");
        assert_eq!(fitted[0], [-0.5, -0.25, 0.0]);
        assert_eq!(fitted[1], [0.5, -0.25, 0.0]);
        assert_eq!(fitted[2], [-0.5, 0.25, 0.0]);
    }

    #[test]
    fn autofit_rejects_degenerate_input() {
        assert!(fit_positions(&[]).is_err());
        assert!(fit_positions(&[[1.0, 1.0, 1.0]; 3]).is_err());
        assert!(fit_positions(&[[f32::INFINITY, 0.0, 0.0], [0.0, 0.0, 0.0]]).is_err());
    }

    #[test]
    fn origin_lands_in_front_of_camera() {
        let m = mvp_for(0.0, 1.0);
        let clip_z = m[3][2];
        let clip_w = m[3][3];
        assert!(
            clip_w > 0.0,
            "clip.w must be positive for a point in front of the camera, got {clip_w}"
        );
        let ndc_z = clip_z / clip_w;
        assert!(
            (0.0..=1.0).contains(&ndc_z),
            "NDC z must lie in [0, 1] for WebGPU, got {ndc_z}"
        );
    }

    #[test]
    fn validate_entry_shape_rejects_mismatched_arrays() {
        let entry = crate::mesh_entry::MeshEntry {
            srm_path: std::path::PathBuf::from("x.srm"),
            title: "x.srm".to_string(),
            positions: vec![[0.0; 3]; 3],
            uvs: vec![[0, 0]; 2], // wrong length
            indices: vec![0, 1, 2],
            material_indices: vec![1, 1, 1],
            materials: Vec::new(),
            textures: Vec::new(),
        };
        let e = validate_entry_shape(&entry).unwrap_err();
        assert!(e.contains("mesh arrays disagree"));
    }

    #[test]
    fn validate_entry_shape_rejects_out_of_range_material() {
        let entry = crate::mesh_entry::MeshEntry {
            srm_path: std::path::PathBuf::from("x.srm"),
            title: "x.srm".to_string(),
            positions: vec![[0.0; 3]; 3],
            uvs: vec![[0, 0]; 3],
            indices: vec![0, 1, 2],
            material_indices: vec![2, 2, 2], // but only 1 material
            materials: vec![crate::srm::Material { name: "M".into(), flags: 0x07 }],
            textures: vec![],
        };
        let e = validate_entry_shape(&entry).unwrap_err();
        // Fails first on the materials/textures length mismatch.
        assert!(e.contains("materials/textures disagree") || e.contains("references material index"));
    }

    #[test]
    fn validate_entry_shape_rejects_bad_indices() {
        let entry = crate::mesh_entry::MeshEntry {
            srm_path: std::path::PathBuf::from("x.srm"),
            title: "x.srm".to_string(),
            positions: vec![[0.0; 3]; 3],
            uvs: vec![[0, 0]; 3],
            indices: vec![0, 1], // not multiple of 3
            material_indices: vec![1, 1, 1],
            materials: Vec::new(),
            textures: Vec::new(),
        };
        let e = validate_entry_shape(&entry).unwrap_err();
        assert!(e.contains("indices") || e.contains("materials/textures"));
    }
}