//! wgpu fullscreen-quad renderer (`dds-texture-viewer` + `bc7-cpu-decode`).
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
                label: Some("dds viewer"),
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
