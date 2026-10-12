//! wgpu fullscreen-quad renderer (`dds-texture-viewer` + `bc7-cpu-decode`)
//! plus the flat-shaded mesh pipeline (`models-3d`: [`show_mesh`]).
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

/// GPU core shared by the blit window and the mesh window: surface +
/// device + queue + configured surface. Extracted verbatim from the
/// blit setup so both windows open devices the same way (adapter's own
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
        // RGBA8 packs 4 bytes per texel.
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

    /// Swap the shown image (gallery navigation): upload a new texture +
    /// bind group, reusing device/queue/pipeline. The window keeps the
    /// FIRST shown texture's size (per-texture resize is a non-goal).
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
            // Occluded/minimized/timeout/validation: skip this frame.
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return,
            // Surface changed or lost: re-configure and try next frame.
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
/// until the user closes it. Returns `Err` with a displayable message
/// when no window/GPU is available (e.g. headless environments).
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

/// Decode one gallery entry to RGBA8 through the EXISTING path
/// (`load_texture` → CPU BC7 or `image` RGBA8). No new decoder.
fn decode_entry(path: &Path) -> Result<DecodedImage, dds::ViewerError> {
    match dds::load_texture(path)? {
        LoadedTexture::Rgba8(image) => Ok(image),
        LoadedTexture::Bc7(compressed) => Ok(dds::decode_bc7_to_rgba8(&compressed)),
    }
}

/// File name for titles and skip lines (falls back to the full path when
/// there is no file name).
fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Window title AND console line for one shown entry: `<index>/<total>
/// <filename>` with a 1-based index. Both carry the same content so
/// playtest notes can quote either.
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

/// Step the gallery in `dir`, skipping loud over undecodable entries.
/// The currently shown texture stays until a decodable replacement
/// exists; clamping at the ends stays put (no wrap, no blank window).
/// Free function (not a method) so the window-event borrow of
/// `state` and the cursor fields never alias.
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

/// Open a gallery window over `entries` (sorted `.DDS` paths from
/// [`crate::gallery::collect_dds_entries`]) and block until the user
/// exits. Shows the first DECODABLE entry; undecodable entries are
/// skipped loud (`warning: skipping <filename>: <error>`) and browsing
/// continues. Zero successes is `Err` (all-bad dir exits 1, no window).
/// ArrowRight/ArrowLeft step (clamped, auto-repeat ignored), Esc and
/// window close exit cleanly.
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

// --- `models-3d` mesh path (NEW pipeline; the blit above is untouched) ---

/// Mesh shader: MVP uniform, world-space varying, derivative flat shading.
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
    // abs(): culling is OFF so back faces show with flipped normals.
    let d = abs(dot(n, l));
    let base = vec3<f32>(0.55, 0.75, 0.9);
    return vec4<f32>(base * (0.25 + 0.75 * d), 1.0);
}
"#;

/// Center + uniformly scale positions into a unit cube (max extent maps
/// to 1.0). File units/orientation are UNKNOWN, so framing is normalized
/// instead of assumed. Degenerate (empty, non-finite, zero-extent) input
/// is `Err`, never a divide-by-zero.
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

/// Column-major `mat4` multiply: `C = A * B`.
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

/// Y-rotation by `angle` radians, then a fixed camera (translate `-2.2`
/// in Z) under a 45-degree perspective: hand-rolled, no new crates.
///
/// WebGPU/DirectX NDC convention: z maps into [0, 1]. The `proj` row
/// carrying the perspective divide must produce a POSITIVE `clip.w`
/// for points in front of the camera (view-space z is negative there);
/// the `z` row must produce a `clip.z / clip.w` inside [0, 1]. The
/// signs below are the ones that satisfy both — flipping either back
/// (the pre-fix `far / (far - near)` / `1.0` pair) sends every vertex
/// behind the camera and the rasterizer discards the whole mesh.
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
            // Culling OFF: file winding is carried as observed (spec R6).
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
        // Slow auto-rotation (~0.6 rad/s about Y).
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
        // Continuous rotation needs per-frame wakeups.
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

/// Open a window showing one auto-fitted `.SRM` mesh as a flat-shaded
/// auto-rotating solid and block until the user closes it (Esc or window
/// close exits cleanly). Returns `Err` with a displayable message when
/// the mesh is degenerate or no window/GPU is available (headless).
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

#[cfg(test)]
mod mesh_tests {
    use super::{fit_positions, mvp_for};

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

    /// Regression: a mesh at the origin (post auto-fit) must land in
    /// front of the camera, i.e. with `clip.w > 0` and `clip.z / clip.w`
    /// inside the WebGPU [0, 1] NDC range. The pre-fix projection matrix
    /// produced `clip.w = -2.2` for this exact point, which the
    /// rasterizer interprets as "behind the camera" and discards — the
    /// symptom was a fully black window despite a successful parse.
    #[test]
    fn origin_lands_in_front_of_camera() {
        let m = mvp_for(0.0, 1.0);
        // Apply m to (0, 0, 0, 1) in column-major order:
        // clip.i = m[0][i]*0 + m[1][i]*0 + m[2][i]*0 + m[3][i]*1 = m[3][i].
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
}
