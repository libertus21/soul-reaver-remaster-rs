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
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::cli_out::outln;
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
pub(crate) struct GpuCore<'window> {
    pub(crate) surface: wgpu::Surface<'window>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
}

pub(crate) async fn open_core(window: Arc<Window>, label: &str) -> Result<GpuCore<'_>, String> {
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
                outln!("{label}");
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
                outln!("{label}");
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

pub(crate) fn fit_bounds(positions: &[[f32; 3]]) -> Result<([f32; 3], f32), String> {
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
    Ok((center, 1.0 / extent))
}

pub fn fit_positions(positions: &[[f32; 3]]) -> Result<Vec<[f32; 3]>, String> {
    let (center, scale) = fit_bounds(positions)?;
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

/// Map a world-space point through the same auto-fit applied to `positions`.
fn fit_point(positions: &[[f32; 3]], point: [f32; 3]) -> Result<[f32; 3], String> {
    let (center, scale) = fit_bounds(positions)?;
    Ok([
        (point[0] - center[0]) * scale,
        (point[1] - center[1]) * scale,
        (point[2] - center[2]) * scale,
    ])
}

/// Fitted fly eye for `spawn`, aimed at the fitted scene centre (the origin).
fn initial_fly_camera(positions: &[[f32; 3]], spawn: Option<[f32; 3]>) -> Result<FlyCamera, String> {
    let eye = match spawn {
        Some(point) => fit_point(positions, point)?,
        None => [0.0; 3],
    };
    Ok(FlyCamera::looking_at(eye, [0.0; 3]))
}

/// Remap a vector between the classic Y-up camera frame and the Z-up world.
///
/// The map is the involution `[x, y, z] -> [x, z, y]`, which sends the classic
/// camera up `+Y` to the world up `+Z` (see [`crate::engine::axis`]). It is its
/// own inverse, so applying it once converts either way.
fn map3(v: [f32; 3], z_up: bool) -> [f32; 3] {
    if z_up { [v[0], v[2], v[1]] } else { v }
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

// --- interactive orbit camera (mesh/level windows) ---

const ORBIT_SENSITIVITY: f32 = 0.008;
const ZOOM_SENSITIVITY: f32 = 0.15;
const PAN_SENSITIVITY: f32 = 0.0015;
const AUTO_ROTATE_SPEED: f32 = 0.6;
/// Historical eye distance: reproduces the fixed auto-fit framing exactly.
const FIT_DISTANCE: f32 = 2.2;
/// 0.005 is ~0.6% of the ~0.87 scene radius, so the eye can sit inside.
const MIN_DISTANCE: f32 = 0.005;
/// Finite ceiling: zoom-out can never send the eye to infinity.
const MAX_DISTANCE: f32 = 20.0;
/// Just short of the pole, so the orbit basis never degenerates.
const PITCH_LIMIT: f32 = 1.553_343;

/// Pure orbit/zoom/pan state. `fit_positions` normalises meshes to a unit
/// extent, so `new` frames them exactly as the historical fixed view did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitCamera {
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: [f32; 3],
    auto_rotate: bool,
    /// When set, the camera treats world `+Z` as up (the shipped level data),
    /// mapping its classic Y-up basis through [`map3`].
    z_up: bool,
}

impl OrbitCamera {
    pub const fn new() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            distance: FIT_DISTANCE,
            target: [0.0; 3],
            auto_rotate: true,
            z_up: false,
        }
    }

    /// Return this camera with a Z-up world basis.
    pub fn z_up(mut self) -> Self {
        self.z_up = true;
        self
    }

    /// Reset the orbit state, preserving the world up axis.
    pub fn reset(&mut self) {
        let z_up = self.z_up;
        *self = Self::new();
        self.z_up = z_up;
    }

    fn interact(&mut self) { self.auto_rotate = false; }

    pub fn apply_orbit(&mut self, dx: f32, dy: f32) {
        self.interact();
        self.yaw -= dx * ORBIT_SENSITIVITY;
        self.pitch = (self.pitch + dy * ORBIT_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    pub fn apply_zoom(&mut self, notches: f32) {
        self.interact();
        let rate = (-notches * ZOOM_SENSITIVITY).exp();
        self.distance = (self.distance * rate).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub fn apply_pan(&mut self, dx: f32, dy: f32) {
        self.interact();
        let (right, up) = self.basis();
        let step = self.distance * PAN_SENSITIVITY;
        for axis in 0..3 {
            self.target[axis] += (-right[axis] * dx + up[axis] * dy) * step;
        }
    }

    pub fn tick(&mut self, seconds: f32) {
        if self.auto_rotate {
            self.yaw += seconds * AUTO_ROTATE_SPEED;
        }
    }

    /// World-space camera right and up; both unit length.
    fn basis(&self) -> ([f32; 3], [f32; 3]) {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        (
            map3([cy, 0.0, -sy], self.z_up),
            map3([-sy * sp, cp, -cy * sp], self.z_up),
        )
    }

    /// Near/far scale with distance, so interior geometry survives any zoom.
    pub(crate) fn mvp(&self, aspect: f32) -> [[f32; 4]; 4] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let target = map3(self.target, self.z_up);
        let eye = map3(
            [
                target[0] + self.distance * cp * sy,
                target[1] + self.distance * sp,
                target[2] + self.distance * cp * cy,
            ],
            self.z_up,
        );
        let forward = map3([-cp * sy, -sp, -cp * cy], self.z_up);
        let near = (self.distance * 0.02).max(1e-3);
        view_proj(eye, self.basis(), forward, near, self.distance + 4.0, aspect)
    }
}

/// Shared view/projection: `right`/`up` are unit, `forward` is the view direction.
pub(crate) fn view_proj(
    eye: [f32; 3],
    (right, up): ([f32; 3], [f32; 3]),
    forward: [f32; 3],
    near: f32,
    far: f32,
    aspect: f32,
) -> [[f32; 4]; 4] {
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let view = [
        [right[0], up[0], -forward[0], 0.0],
        [right[1], up[1], -forward[1], 0.0],
        [right[2], up[2], -forward[2], 0.0],
        [-dot(right, eye), -dot(up, eye), dot(forward, eye), 1.0],
    ];
    let f = 1.0 / std::f32::consts::FRAC_PI_8.tan();
    let proj = [
        [f / aspect.max(1e-6), 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), -1.0],
        [0.0, 0.0, -far * near / (far - near), 0.0],
    ];
    mul4(&proj, &view)
}

// --- fly camera ---

const FLY_LOOK_SENSITIVITY: f32 = 0.003;
const FLY_SPEED: f32 = 0.8;
const FLY_FAST_MULTIPLIER: f32 = 3.0;
/// Frame-time ceiling: a stalled frame cannot teleport the eye through walls.
const FLY_MAX_STEP: f32 = 0.1;
const FLY_NEAR: f32 = 0.01;
const FLY_FAR: f32 = 100.0;

/// Pure first-person fly state: a world position plus yaw/pitch in radians.
/// Movement is relative to the view basis; no animation or collision is claimed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlyCamera {
    position: [f32; 3],
    yaw: f32,
    pitch: f32,
    /// When set, the camera treats world `+Z` as up (the shipped level data).
    z_up: bool,
}

impl FlyCamera {
    pub const fn new(position: [f32; 3]) -> Self {
        Self { position, yaw: 0.0, pitch: 0.0, z_up: false }
    }

    /// Return this camera with a Z-up world basis.
    pub fn z_up(mut self) -> Self {
        self.z_up = true;
        self
    }

    /// Start at `position` facing `target` in the classic Y-up frame.
    pub fn looking_at(position: [f32; 3], target: [f32; 3]) -> Self {
        Self::looking_at_with_up(position, target, false)
    }

    /// Start at `position` facing `target` under the chosen world up axis.
    ///
    /// The fitted scene is centred on the origin, so this aims the eye at the
    /// geometry instead of blindly down one axis: a spawn on the scene edge
    /// would otherwise look out into the void. The direction is solved in the
    /// classic Y-up frame and the basis is mapped to the world. If the direction
    /// is degenerate the camera keeps its default yaw/pitch.
    pub fn looking_at_with_up(position: [f32; 3], target: [f32; 3], z_up: bool) -> Self {
        let dir = map3(
            [
                target[0] - position[0],
                target[1] - position[1],
                target[2] - position[2],
            ],
            z_up,
        );
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        let (yaw, pitch) = if !(len > 1e-6) || !len.is_finite() {
            (0.0, 0.0)
        } else {
            let unit = [dir[0] / len, dir[1] / len, dir[2] / len];
            (
                (-unit[0]).atan2(-unit[2]),
                (-unit[1]).asin().clamp(-PITCH_LIMIT, PITCH_LIMIT),
            )
        };
        Self { position, yaw, pitch, z_up }
    }

    pub fn look(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * FLY_LOOK_SENSITIVITY;
        self.pitch =
            (self.pitch - dy * FLY_LOOK_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Move by `forward`/`strafe` in view space and `rise` along world up.
    pub fn fly(&mut self, forward: f32, strafe: f32, rise: f32, seconds: f32, fast: bool) {
        let step = FLY_SPEED
            * if fast { FLY_FAST_MULTIPLIER } else { 1.0 }
            * seconds.clamp(0.0, FLY_MAX_STEP);
        let (front, right, _) = self.basis();
        for axis in 0..3 {
            self.position[axis] += step * (front[axis] * forward + right[axis] * strafe);
        }
        let up_axis = if self.z_up { 2 } else { 1 };
        self.position[up_axis] += step * rise;
    }

    /// View direction, right and up; all unit length.
    fn basis(&self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        (
            map3([-cp * sy, -sp, -cp * cy], self.z_up),
            map3([cy, 0.0, -sy], self.z_up),
            map3([-sy * sp, cp, -cy * sp], self.z_up),
        )
    }

    pub(crate) fn mvp(&self, aspect: f32) -> [[f32; 4]; 4] {
        let (forward, right, up) = self.basis();
        view_proj(self.position, (right, up), forward, FLY_NEAR, FLY_FAR, aspect)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CameraMode {
    #[default]
    Orbit,
    Fly,
}

impl CameraMode {
    fn toggle(&mut self) -> bool {
        *self = match *self { Self::Orbit => Self::Fly, Self::Fly => Self::Orbit };
        *self == Self::Fly
    }
}

/// Held movement keys; press/release tracking keeps movement frame-stable.
#[derive(Default)]
struct FlyKeys {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    fast: bool,
}

impl FlyKeys {
    fn update(&mut self, event: &KeyEvent) {
        let held = event.state == ElementState::Pressed;
        match &event.logical_key {
            Key::Character(text) => match text.to_ascii_lowercase().as_str() {
                "w" => self.forward = held,
                "s" => self.back = held,
                "a" => self.left = held,
                "d" => self.right = held,
                "e" => self.up = held,
                "c" => self.down = held,
                "q" => self.down = held,
                _ => {}
            },
            Key::Named(NamedKey::Space) => self.up = held,
            Key::Named(NamedKey::Shift) => self.fast = held,
            _ => {}
        }
    }

    fn axes(&self) -> (f32, f32, f32) {
        let axis = |on: bool, off: bool| f32::from(on as u8) - f32::from(off as u8);
        (axis(self.forward, self.back), axis(self.right, self.left), axis(self.up, self.down))
    }
}

fn next_fly_mvp(
    fly: &mut FlyCamera,
    keys: &FlyKeys,
    last_frame: &mut std::time::Instant,
    aspect: f32,
) -> [u8; 64] {
    let now = std::time::Instant::now();
    let dt = now.duration_since(*last_frame).as_secs_f32();
    *last_frame = now;
    let (forward, strafe, rise) = keys.axes();
    fly.fly(forward, strafe, rise, dt, keys.fast);
    mat_bytes(&fly.mvp(aspect))
}

pub(crate) fn set_fly_cursor(window: &Window, fly: bool) {
    let grab = if fly {
        window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
    } else {
        window.set_cursor_grab(CursorGrabMode::None)
    };
    let _ = grab;
    window.set_cursor_visible(!fly);
}

fn next_mvp(camera: &mut OrbitCamera, last_frame: &mut std::time::Instant, aspect: f32) -> [u8; 64] {
    let now = std::time::Instant::now();
    let dt = now.duration_since(*last_frame).as_secs_f32();
    *last_frame = now;
    camera.tick(dt);
    mat_bytes(&camera.mvp(aspect))
}

#[derive(Default)]
struct CameraInput {
    drag: Option<MouseButton>,
    last: Option<PhysicalPosition<f64>>,
    mode: CameraMode,
}

impl CameraInput {
    fn toggle_mode(&mut self) -> bool { self.mode.toggle() }

    fn handle(
        &mut self,
        camera: &mut OrbitCamera,
        fly: Option<&mut FlyCamera>,
        event: &WindowEvent,
    ) -> bool {
        match event {
            WindowEvent::MouseInput { state, button, .. } => {
                if self.mode == CameraMode::Orbit {
                    if *state == ElementState::Pressed {
                        self.drag = Some(*button);
                    } else if self.drag == Some(*button) {
                        self.drag = None;
                    }
                }
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(prev) = self.last {
                    let (dx, dy) = ((position.x - prev.x) as f32, (position.y - prev.y) as f32);
                    if let Some(fly) = fly.filter(|_| self.mode == CameraMode::Fly) {
                        fly.look(dx, dy);
                    } else if let (Some(button), CameraMode::Orbit) = (self.drag, self.mode) {
                        if button == MouseButton::Left { camera.apply_orbit(dx, dy); }
                        else { camera.apply_pan(dx, dy); }
                    }
                }
                self.last = Some(*position);
                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.mode == CameraMode::Orbit {
                    let notches = match delta {
                        MouseScrollDelta::LineDelta(_, y) => *y,
                        MouseScrollDelta::PixelDelta(p) => (p.y / 50.0) as f32,
                    };
                    camera.apply_zoom(notches);
                }
                true
            }
            _ => false,
        }
    }
}

pub(crate) fn mat_bytes(m: &[[f32; 4]; 4]) -> [u8; 64] {
    let mut out = [0u8; 64];
    for col in 0..4 {
        for row in 0..4 {
            out[(col * 4 + row) * 4..][..4].copy_from_slice(&m[col][row].to_le_bytes());
        }
    }
    out
}

/// Camera state and input handling shared by every interactive mesh window:
/// orbit, zoom, pan, reset, and the optional first-person fly mode.
struct CameraRig {
    camera: OrbitCamera,
    fly: FlyCamera,
    start: FlyCamera,
    input: CameraInput,
    keys: FlyKeys,
    last_frame: std::time::Instant,
}

impl CameraRig {
    fn new(positions: &[[f32; 3]], spawn: Option<[f32; 3]>) -> Result<Self, String> {
        Ok(Self::from_start(initial_fly_camera(positions, spawn)?))
    }

    fn from_start(start: FlyCamera) -> Self {
        Self {
            camera: OrbitCamera::new(),
            fly: start,
            start,
            input: CameraInput::default(),
            keys: FlyKeys::default(),
            last_frame: std::time::Instant::now(),
        }
    }

    fn reset(&mut self) {
        self.camera.reset();
        self.fly = self.start;
        self.input.mode = CameraMode::Orbit;
        self.keys = FlyKeys::default();
    }

    fn handle_input(&mut self, event: &WindowEvent) -> bool {
        self.input.handle(&mut self.camera, Some(&mut self.fly), event)
    }

    /// Track held keys and apply the shared window keys; `true` means exit.
    fn handle_key(&mut self, window: &Window, event: &KeyEvent) -> bool {
        self.keys.update(event);
        if event.state != ElementState::Pressed || event.repeat {
            return false;
        }
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => true,
            Key::Character(text) if text.eq_ignore_ascii_case("f") => {
                let flying = self.input.toggle_mode();
                set_fly_cursor(window, flying);
                false
            }
            Key::Character(text) if text.eq_ignore_ascii_case("r") => {
                self.reset();
                set_fly_cursor(window, false);
                false
            }
            _ => false,
        }
    }

    fn mvp(&mut self, aspect: f32) -> [u8; 64] {
        match self.input.mode {
            CameraMode::Orbit => next_mvp(&mut self.camera, &mut self.last_frame, aspect),
            CameraMode::Fly => {
                next_fly_mvp(&mut self.fly, &self.keys, &mut self.last_frame, aspect)
            }
        }
    }
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
    rig: CameraRig,
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
        indices: &[u32],
        start: FlyCamera,
    ) -> Result<Self, String> {
        let mut vbytes = Vec::with_capacity(vertices.len() * 12);
        for v in vertices {
            for c in v {
                vbytes.extend_from_slice(&c.to_le_bytes());
            }
        }
        let mut ibytes = Vec::with_capacity(indices.len() * 4);
        for i in indices {
            ibytes.extend_from_slice(&i.to_le_bytes());
        }
        let core = open_core(window.clone(), "srm mesh").await?;
        let GpuCore { surface, device, queue, config } = core;
        let depth_view = Self::make_depth(&device, config.width, config.height);
        // Pad every upload to COPY_BUFFER_ALIGNMENT (4 bytes); zero
        // padding is inert and the GPU never reads past the real size.
        fn upload(device: &wgpu::Device, queue: &wgpu::Queue, label: &str, bytes: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
            let mut padded = bytes.to_vec();
            while padded.len() % 4 != 0 {
                padded.push(0);
            }
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: padded.len() as u64,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buf, 0, &padded);
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
            rig: CameraRig::from_start(start),
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
        let mvp = self.rig.mvp(aspect);
        self.queue.write_buffer(&self.uniform_buffer, 0, &mvp);
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
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.index_count, 0, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(frame);
    }
}

struct MeshApp {
    title: String,
    vertices: Vec<[f32; 3]>,
    indices: Vec<u32>,
    start: FlyCamera,
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
        match pollster::block_on(MeshState::new(window, &vertices, &indices, self.start)) {
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
        if state.rig.handle_input(&event) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => state.render(),
            WindowEvent::Focused(false) => state.rig.keys = FlyKeys::default(),
            WindowEvent::KeyboardInput { event, .. } => {
                if state.rig.handle_key(&state.window, &event) {
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

pub fn show_mesh(
    positions: &[[f32; 3]],
    indices: &[u32],
    title: &str,
    spawn: Option<[f32; 3]>,
) -> Result<(), String> {
    let fitted = fit_positions(positions)?;
    let start = initial_fly_camera(positions, spawn)?;
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
        start,
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

pub(crate) const MESH_TEXTURED_SHADER_SRC: &str = r#"
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
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct TVertex {
    pub(crate) pos: [f32; 3],
    pub(crate) uv: [f32; 2],
    pub(crate) _pad: f32,
}

pub(crate) fn upload_rgba_texture(
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

/// Uniform bind-group layout for the textured mesh pipeline.
///
/// Shared by the viewer and the engine backend so both bind the same shader.
pub(crate) fn textured_uniform_layout(
    device: &wgpu::Device,
    label: &str,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
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
    })
}

/// Diffuse/normal/specular/sampler bind-group layout for the textured mesh
/// pipeline. Shared by the viewer and the engine backend.
pub(crate) fn textured_texture_layout(
    device: &wgpu::Device,
    label: &str,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
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
    })
}

/// Build the textured mesh pipeline (shader, vertex layout, depth state).
///
/// The viewer and the engine backend call this one function with their own
/// bind-group layouts. The pipeline shape - shader source, vertex stride,
/// cull mode, depth test, blend - is therefore identical by construction.
pub(crate) fn build_textured_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
    label: &str,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(MESH_TEXTURED_SHADER_SRC.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(uniform_layout), Some(texture_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
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
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
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
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// Create the shared `Depth24Plus` depth attachment.
pub(crate) fn make_depth_view(device: &wgpu::Device, width: u32, height: u32, label: &str) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
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
    rig: CameraRig,
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
        spawn: Option<[f32; 3]>,
    ) -> Result<Self, String> {
        let core = open_core(window.clone(), "srm textured").await?;
        let GpuCore { surface, device, queue, config } = core;

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("srm textured mvp"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_layout = textured_uniform_layout(&device, "srm textured uniform layout");
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("srm textured uniform"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let texture_layout = textured_texture_layout(&device, "srm textured tex layout");
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("srm textured sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let pipeline = build_textured_pipeline(
            &device,
            config.format,
            &uniform_layout,
            &texture_layout,
            "srm textured",
        );

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
            rig: CameraRig::new(&entry.positions, spawn)?,
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
        // wgpu requires write_buffer sizes to be a multiple of
        // COPY_BUFFER_ALIGNMENT (4). Pad with zeros; the GPU never reads
        // past the real data, so trailing padding is inert.
        let mut vbytes_padded = vbytes.to_vec();
        while vbytes_padded.len() % 4 != 0 {
            vbytes_padded.push(0);
        }
        let vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("srm textured vertices"),
            size: vbytes_padded.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&vertex_buffer, 0, &vbytes_padded);

        let mut ranges: Vec<(usize, Vec<u32>)> =
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
            let mut ibytes: Vec<u8> = tri_indices.iter().flat_map(|i| i.to_le_bytes()).collect();
            // Pad to COPY_BUFFER_ALIGNMENT (4); the GPU never reads past
            // index_count, so the trailing padding is inert.
            while ibytes.len() % 4 != 0 {
                ibytes.push(0);
            }
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
        let mvp = self.rig.mvp(aspect);
        self.queue.write_buffer(&self.uniform_buffer, 0, &mvp);

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
                    pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
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
    spawn: Option<[f32; 3]>,
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
        match pollster::block_on(TexturedMeshState::new(window, &self.entry, self.spawn)) {
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
        if state.rig.handle_input(&event) { return; }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => state.render(),
            WindowEvent::KeyboardInput { event, .. } => {
                if state.rig.handle_key(&state.window, &event) {
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
    indices: &[u32],
    material_indices: &[u8],
    materials: &[crate::srm::Material],
    textures: &[crate::textures_object::MaterialTextures],
    title: &str,
    spawn: Option<[f32; 3]>,
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
    let mut app =
        TexturedMeshApp { entry, spawn, state: None, init_error: None };
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
                    outln!("{label}");
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
        match pollster::block_on(TexturedMeshState::new(window, &self.current, None)) {
            Ok(state) => {
                outln!("{label}");
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
        if state.rig.handle_input(&event) { return; }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => state.render(),
            WindowEvent::KeyboardInput { event, .. } => {
                let step = if event.state == ElementState::Pressed && !event.repeat {
                    match event.logical_key {
                        Key::Named(NamedKey::ArrowRight) => Some(Direction::Next),
                        Key::Named(NamedKey::ArrowLeft) => Some(Direction::Prev),
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some(dir) = step {
                    mesh_gallery_step(
                        self.entries,
                        self.tex_dir,
                        &mut self.index,
                        &mut self.current,
                        state,
                        dir,
                    );
                    return;
                }
                if state.rig.handle_key(&state.window, &event) {
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
    use super::{
        fit_point, fit_positions, initial_fly_camera, validate_entry_shape, CameraMode, CameraRig,
        FlyCamera, OrbitCamera, FIT_DISTANCE, FLY_FAST_MULTIPLIER, FLY_MAX_STEP, FLY_SPEED,
        MAX_DISTANCE, MIN_DISTANCE, PITCH_LIMIT,
    };

    fn origin_clip(camera: &OrbitCamera, aspect: f32) -> [f32; 4] {
        let m = camera.mvp(aspect);
        [m[3][0], m[3][1], m[3][2], m[3][3]]
    }

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
    fn camera_starts_at_the_historical_auto_fit() {
        let camera = OrbitCamera::new();
        assert_eq!((camera.yaw, camera.pitch), (0.0, 0.0));
        assert_eq!(camera.distance, FIT_DISTANCE);
        assert!(camera.auto_rotate);
        let clip = origin_clip(&camera, 1.0);
        assert!(clip[0].abs() < 1e-6 && clip[1].abs() < 1e-6, "origin stays centred");
        assert!((clip[3] - FIT_DISTANCE).abs() < 1e-5, "eye distance is the fit distance");
        assert!((0.0..=1.0).contains(&(clip[2] / clip[3])), "NDC z lies in [0, 1]");
    }

    #[test]
    fn orbit_clamps_pitch_and_freezes_auto_rotation() {
        let mut camera = OrbitCamera::new();
        camera.apply_orbit(10.0, 0.0);
        assert!(camera.yaw < 0.0);
        assert!(!camera.auto_rotate);
        let yaw = camera.yaw;
        camera.tick(1.0);
        assert_eq!(camera.yaw, yaw, "auto-rotation stays stopped after input");
        camera.apply_orbit(0.0, 1e6);
        assert!((camera.pitch - PITCH_LIMIT).abs() < 1e-5);
        camera.apply_orbit(0.0, -2e6);
        assert!((camera.pitch + PITCH_LIMIT).abs() < 1e-5);
        camera.reset();
        camera.tick(0.5);
        assert!(camera.yaw > 0.0, "reset re-arms auto-rotation");
    }

    #[test]
    fn zoom_reaches_the_interior_without_inverting() {
        let mut camera = OrbitCamera::new();
        for _ in 0..200 {
            camera.apply_zoom(1.0);
        }
        assert_eq!(camera.distance, MIN_DISTANCE);
        assert!(camera.distance < 0.01, "eye enters well inside the unit scene");
        assert!(origin_clip(&camera, 1.0)[3] > 0.0, "target stays ahead of the eye");
        for _ in 0..200 {
            camera.apply_zoom(-1.0);
        }
        assert_eq!(camera.distance, MAX_DISTANCE);
        assert!(camera.distance.is_finite(), "zoom-out never reaches infinity");
    }

    #[test]
    fn fly_moves_forward_and_strafes_in_view_space() {
        let mut fly = FlyCamera::new([0.0; 3]);
        fly.fly(1.0, 0.0, 0.0, 0.1, false);
        assert!((fly.position[2] + FLY_SPEED * 0.1).abs() < 1e-5, "forward is -Z at yaw 0");
        fly.fly(0.0, 1.0, 0.0, 0.1, false);
        assert!((fly.position[0] - FLY_SPEED * 0.1).abs() < 1e-5, "strafe follows +X");
    }

    #[test]
    fn fly_rises_along_world_up_and_shift_scales_the_step() {
        let mut fly = FlyCamera::new([0.0; 3]);
        fly.fly(0.0, 0.0, 1.0, 0.1, false);
        assert!((fly.position[1] - FLY_SPEED * 0.1).abs() < 1e-5);
        let mut fast = FlyCamera::new([0.0; 3]);
        fast.fly(1.0, 0.0, 0.0, 0.1, true);
        assert!((fast.position[2] + FLY_SPEED * 0.1 * FLY_FAST_MULTIPLIER).abs() < 1e-5);
    }

    #[test]
    fn fly_step_is_clamped_against_long_frames() {
        let mut fly = FlyCamera::new([0.0; 3]);
        fly.fly(1.0, 0.0, 0.0, 10.0, false);
        assert!((fly.position[2] + FLY_SPEED * FLY_MAX_STEP).abs() < 1e-5);
    }

    #[test]
    fn fly_look_clamps_pitch_and_starts_at_the_given_point() {
        let mut fly = FlyCamera::new([1.0, 2.0, 3.0]);
        fly.look(0.0, 1e6);
        assert!((fly.pitch + PITCH_LIMIT).abs() < 1e-5);
        fly.look(0.0, -2e6);
        assert!((fly.pitch - PITCH_LIMIT).abs() < 1e-5);
        assert_eq!(fly.position, [1.0, 2.0, 3.0], "look never moves the eye");
    }

    #[test]
    fn fly_yaw_turns_the_forward_axis() {
        let mut fly = FlyCamera::new([0.0; 3]);
        fly.look(523.5988, 0.0);
        fly.fly(1.0, 0.0, 0.0, 0.1, false);
        assert!((fly.position[0] - FLY_SPEED * 0.1).abs() < 1e-3, "a quarter turn faces +X");
        assert!(fly.position[2].abs() < 1e-3);
    }

    #[test]
    fn z_up_cameras_use_world_z_as_up_and_rise_along_it() {
        let mut fly = FlyCamera::new([0.0; 3]).z_up();
        fly.fly(0.0, 0.0, 1.0, 0.1, false);
        assert!((fly.position[2] - FLY_SPEED * 0.1).abs() < 1e-5, "rise is +Z");
        assert!(fly.position[1].abs() < 1e-6, "rise never moves Y");
        let (_, _, up) = fly.basis();
        assert!((up[2] - 1.0).abs() < 1e-6, "fly camera up is world +Z");

        let orbit = OrbitCamera::new().z_up();
        let (_, up) = orbit.basis();
        assert!((up[2] - 1.0).abs() < 1e-6, "orbit camera up is world +Z");
    }

    #[test]
    fn z_up_looking_at_faces_the_target_with_world_z_up() {
        let fly = FlyCamera::looking_at_with_up([0.0, -5.0, 0.0], [0.0, 0.0, 0.0], true);
        let m = fly.mvp(1.0);
        let point = [0.0, 0.0, 0.0, 1.0];
        let clip: Vec<f32> =
            (0..4).map(|row| (0..4).map(|col| m[col][row] * point[col]).sum()).collect();
        assert!(clip[3] > 0.0, "the target stays in front of the camera");
        let (_, _, up) = fly.basis();
        assert!((up[2] - 1.0).abs() < 1e-6, "up remains world +Z");
    }

    #[test]
    fn fit_point_maps_a_world_position_into_the_fitted_scene() {
        let mesh = [[0.0, 0.0, 0.0], [4.0, 2.0, 0.0]];
        assert_eq!(fit_point(&mesh, [2.0, 1.0, 0.0]).unwrap(), [0.0, 0.0, 0.0]);
        assert_eq!(fit_point(&mesh, [4.0, 2.0, 0.0]).unwrap(), [0.5, 0.25, 0.0]);
    }

    #[test]
    fn fly_projection_keeps_a_point_ahead_in_front() {
        let m = FlyCamera::new([0.0; 3]).mvp(1.0);
        let point = [0.0, 0.0, -1.0, 1.0];
        let clip: Vec<f32> =
            (0..4).map(|row| (0..4).map(|col| m[col][row] * point[col]).sum()).collect();
        assert!(clip.iter().all(|v| v.is_finite()));
        assert!(clip[3] > 0.0, "a point down the view axis stays in front");
    }

    #[test]
    fn pan_moves_the_target_and_keeps_the_zoom() {
        let mut camera = OrbitCamera::new();
        camera.apply_pan(10.0, -10.0);
        assert_ne!(camera.target, [0.0; 3]);
        assert_eq!(camera.distance, FIT_DISTANCE);
        assert!(!camera.auto_rotate);
    }

    #[test]
    fn fly_starts_facing_the_scene_from_an_edge_spawn() {
        // Reproduces the `under`-group shape without game bytes: the spawn sits
        // on the x-min edge while the scene centre is 5 world units along +X and
        // the scene is thin in Z. A default -Z eye looks away and renders black.
        let positions = [[0.0, -1.0, -1.0], [10.0, 1.0, 1.0]];
        let fly = initial_fly_camera(&positions, Some([0.0, 0.0, 0.0])).unwrap();
        let m = fly.mvp(1.0);
        let centre = [0.0f32, 0.0, 0.0, 1.0];
        let clip: Vec<f32> =
            (0..4).map(|row| (0..4).map(|col| m[col][row] * centre[col]).sum()).collect();
        assert!(clip[3] > 0.0, "scene centre is in front of the initial eye");
        let ndc = [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]];
        assert!(
            ndc[0].abs() <= 1.0 && ndc[1].abs() <= 1.0 && (0.0..=1.0).contains(&ndc[2]),
            "scene centre lands inside the frustum, got {ndc:?}"
        );
    }

    #[test]
    fn validate_entry_shape_rejects_mismatched_arrays() {
        let entry = crate::mesh_entry::MeshEntry {
            srm_path: std::path::PathBuf::from("x.srm"),
            title: "x.srm".to_string(),
            positions: vec![[0.0; 3]; 3],
            uvs: vec![[0, 0]; 2],
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
            material_indices: vec![2, 2, 2],
            materials: vec![crate::srm::Material { name: "M".into(), flags: 0x07 }],
            textures: vec![],
        };
        let e = validate_entry_shape(&entry).unwrap_err();
        assert!(e.contains("materials/textures disagree") || e.contains("references material index"));
    }

    #[test]
    fn validate_entry_shape_rejects_bad_indices() {
        let entry = crate::mesh_entry::MeshEntry {
            srm_path: std::path::PathBuf::from("x.srm"),
            title: "x.srm".to_string(),
            positions: vec![[0.0; 3]; 3],
            uvs: vec![[0, 0]; 3],
            indices: vec![0, 1],
            material_indices: vec![1, 1, 1],
            materials: Vec::new(),
            textures: Vec::new(),
        };
        let e = validate_entry_shape(&entry).unwrap_err();
        assert!(e.contains("indices") || e.contains("materials/textures"));
    }

    #[test]
    fn rig_starts_in_orbit_and_resets_to_the_spawn_view() {
        let positions = [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0]];
        let spawn = Some([4.0, 0.0, 0.0]);
        let start = initial_fly_camera(&positions, spawn).unwrap();
        let mut rig = CameraRig::new(&positions, spawn).unwrap();
        assert_eq!(rig.input.mode, CameraMode::Orbit);
        assert_eq!(rig.fly, start);
        assert_eq!(rig.start, start);
        rig.input.toggle_mode();
        rig.keys.forward = true;
        rig.reset();
        assert_eq!(rig.input.mode, CameraMode::Orbit);
        assert_eq!(rig.fly, start, "reset returns the fly eye to its spawn");
        assert!(!rig.keys.forward, "reset drops held movement keys");
    }
}