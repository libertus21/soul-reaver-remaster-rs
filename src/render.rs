//! wgpu fullscreen-quad renderer (`dds-texture-viewer` + `bc7-texture-support`).
//!
//! One job: take image bytes, upload them to a GPU texture, and draw
//! them as a stretched fullscreen quad in a winit window until the user
//! closes it. Two upload flavors share one pipeline: [`show_rgba8`] for
//! decoded RGBA8 pixels and [`show_bc7`] for raw BC7 blocks uploaded to
//! a natively-compressed texture. Deliberately deferred (see design):
//! sRGB correctness, resize sophistication beyond re-configuring the
//! surface, mip selection.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

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
    bind_group: wgpu::BindGroup,
}

impl<'window> State<'window> {
    async fn new(
        window: Arc<Window>,
        width: u32,
        height: u32,
        pixels: &[u8],
        format: wgpu::TextureFormat,
        required_features: wgpu::Features,
    ) -> Result<Self, String> {
        // Expected upload size depends on the format: RGBA8 is 4 bytes
        // per texel; block-compressed formats store one fixed-size block
        // per block-dimensions tile (BC7: 16 bytes per 4x4 tile).
        let expected_len = match format {
            wgpu::TextureFormat::Bc7RgbaUnorm => {
                let (block_w, block_h) = format.block_dimensions();
                let block_size = format.block_copy_size(None).unwrap_or(16) as usize;
                width.div_ceil(block_w) as usize
                    * height.div_ceil(block_h) as usize
                    * block_size
            }
            _ => (width as usize) * (height as usize) * 4,
        };
        assert_eq!(
            pixels.len(),
            expected_len,
            "upload buffer must hold exactly one {format:?} image ({width}x{height})"
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
        // Compressed formats need an explicit device feature (e.g. BC7
        // needs TEXTURE_COMPRESSION_BC). Fail loud before opening the
        // device so headless/software GPUs report the missing feature
        // instead of a cryptic validation error.
        if !adapter.features().contains(required_features) {
            // Only `show_bc7` requests non-empty features (exactly
            // TEXTURE_COMPRESSION_BC), so name it literally per spec R7.
            return Err(format!(
                "GPU adapter lacks TEXTURE_COMPRESSION_BC (required for {format:?} textures)"
            ));
        }
        // Request the adapter's own limits (not `Limits::default()`): this
        // viewer needs nothing special, so take whatever the GPU offers.
        // `Limits::default()` can exceed what low-capability / software
        // adapters allow and fail device creation there.
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("dds viewer"),
                required_features,
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
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Row pitch follows the format: RGBA8 packs 4 bytes per texel;
        // BC7 packs one 16-byte block per 4-texel-wide tile (2048 for a
        // 512-wide texture — a multiple of wgpu's 256-byte row alignment,
        // so no padding path is needed).
        let bytes_per_row = match format {
            wgpu::TextureFormat::Bc7RgbaUnorm => {
                let (block_w, _) = format.block_dimensions();
                Some(width.div_ceil(block_w) * format.block_copy_size(None).unwrap_or(16) as u32)
            }
            _ => Some(4 * width),
        };
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

        let bind_group_layout =
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
            });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dds bind group"),
            layout: &bind_group_layout,
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
        });

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
    format: wgpu::TextureFormat,
    required_features: wgpu::Features,
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
        match pollster::block_on(State::new(
            window,
            self.width,
            self.height,
            self.pixels,
            self.format,
            self.required_features,
        )) {
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
    show(width, height, pixels, wgpu::TextureFormat::Rgba8Unorm, wgpu::Features::empty())
}

/// Open a window showing raw BC7 base-mip `blocks` (`width` x `height`)
/// via a natively-uploaded `Bc7RgbaUnorm` texture and block until the
/// user closes it. Needs the `TEXTURE_COMPRESSION_BC` device feature;
/// returns a loud `Err` naming it when the adapter lacks it.
pub fn show_bc7(width: u32, height: u32, blocks: &[u8]) -> Result<(), String> {
    show(
        width,
        height,
        blocks,
        wgpu::TextureFormat::Bc7RgbaUnorm,
        wgpu::Features::TEXTURE_COMPRESSION_BC,
    )
}

fn show(
    width: u32,
    height: u32,
    pixels: &[u8],
    format: wgpu::TextureFormat,
    required_features: wgpu::Features,
) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("cannot show an empty (0x0) image".to_string());
    }
    let event_loop =
        EventLoop::new().map_err(|e| format!("cannot create window event loop: {e}"))?;
    let mut app = App {
        width,
        height,
        pixels,
        format,
        required_features,
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
