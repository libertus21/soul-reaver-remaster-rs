//! wgpu implementation of [`RenderBackend`].
//!
//! The backend merges the CPU scene into one vertex stream and one index
//! stream per frame, then draws it with the **existing** textured mesh shader
//! and pipeline builder from `src/render.rs`. The viewer path is untouched:
//! both callers now share [`render::build_textured_pipeline`], so there is one
//! pipeline definition, not two.
//!
//! The merge is deterministic: visible items are walked in scene order, each
//! item's vertices are transformed by the scene transform, and triangles are
//! grouped by material in ascending material order. No hash iteration and no
//! wall clock participate.

use std::collections::BTreeMap;
use std::sync::Arc;

use winit::window::Window;

use crate::engine::render_backend::{RenderBackend, RenderError};
use crate::engine::scene::RenderScene;
use crate::object_texture::{self, PageBank};
use crate::render;
use crate::textures_object::MaterialTextures;

/// One merged CPU mesh, ready for a single vertex/index upload.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct MergedMesh {
    pub(crate) vertices: Vec<render::TVertex>,
    pub(crate) indices: Vec<u32>,
    /// `(material, first_index, index_count)` in ascending material order.
    pub(crate) ranges: Vec<(u16, u32, u32)>,
}

/// Merge every visible scene item into one CPU mesh.
///
/// Vertex order follows scene order; index order groups triangles by material
/// while preserving their relative order inside each material.
pub(crate) fn merge_scene(scene: &RenderScene) -> MergedMesh {
    let mut vertices = Vec::new();
    let mut by_material: Vec<Vec<u32>> = vec![Vec::new(); scene.material_count().max(1) + 1];

    for item in scene.visible_items() {
        let Some(mesh) = scene.mesh(item.mesh) else {
            continue;
        };
        if mesh.indices.is_empty() || mesh.positions.is_empty() {
            continue;
        }
        let base = vertices.len() as u32;
        let world = [
            item.world[0] as f32,
            item.world[1] as f32,
            item.world[2] as f32,
        ];
        for (index, position) in mesh.positions.iter().enumerate() {
            let point = scene.transform.apply([
                position[0] + world[0],
                position[1] + world[1],
                position[2] + world[2],
            ]);
            let uv = mesh.uvs.get(index).copied().unwrap_or([0, 0]);
            vertices.push(render::TVertex {
                pos: point,
                uv: [f32::from(uv[0]) / 255.0, f32::from(uv[1]) / 255.0],
                _pad: 0.0,
            });
        }
        for triangle in mesh.indices.chunks_exact(3) {
            let material = mesh
                .material_indices
                .get(triangle[0] as usize)
                .copied()
                .unwrap_or(1);
            let slot = usize::from(material)
                .min(by_material.len() - 1)
                .max(1);
            by_material[slot].extend([
                base + triangle[0],
                base + triangle[1],
                base + triangle[2],
            ]);
        }
    }

    let mut indices = Vec::new();
    let mut ranges = Vec::new();
    for (material, bucket) in by_material.iter().enumerate().skip(1) {
        if bucket.is_empty() {
            continue;
        }
        let start = indices.len() as u32;
        indices.extend_from_slice(bucket);
        ranges.push((material as u16, start, bucket.len() as u32));
    }

    MergedMesh {
        vertices,
        indices,
        ranges,
    }
}

fn material_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    textures: &MaterialTextures,
    label: &str,
) -> wgpu::BindGroup {
    let diffuse = render::upload_rgba_texture(device, queue, &textures.diffuse, label);
    let normal = render::upload_rgba_texture(device, queue, &textures.normal, label);
    let specular = render::upload_rgba_texture(device, queue, &textures.specular, label);
    let diffuse = diffuse.create_view(&wgpu::TextureViewDescriptor::default());
    let normal = normal.create_view(&wgpu::TextureViewDescriptor::default());
    let specular = specular.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&diffuse),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&normal),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&specular),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn upload_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &mut wgpu::Buffer,
    capacity: &mut u64,
    bytes: &[u8],
    usage: wgpu::BufferUsages,
    label: &str,
) {
    let mut padded = bytes.to_vec();
    while padded.len() % 4 != 0 {
        padded.push(0);
    }
    let size = padded.len().max(4) as u64;
    if size > *capacity {
        *buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        *capacity = size;
    }
    if !padded.is_empty() {
        queue.write_buffer(buffer, 0, &padded);
    }
}

/// wgpu backend over one window.
pub struct WgpuBackend {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    depth_view: wgpu::TextureView,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    index_capacity: u64,
    flat: wgpu::BindGroup,
    textures: BTreeMap<u16, wgpu::BindGroup>,
    bank: PageBank,
}

impl WgpuBackend {
    /// Open the GPU for `window` and prepare the shared textured pipeline.
    pub fn new(window: Arc<Window>, bank: PageBank) -> Result<Self, RenderError> {
        let core = pollster::block_on(render::open_core(window, "engine play"))
            .map_err(RenderError::Surface)?;
        let render::GpuCore {
            surface,
            device,
            queue,
            config,
        } = core;

        let uniform_layout =
            render::textured_uniform_layout(&device, "engine uniform layout");
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("engine mvp"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("engine uniform"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let texture_layout = render::textured_texture_layout(&device, "engine texture layout");
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("engine sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let pipeline = render::build_textured_pipeline(
            &device,
            config.format,
            &uniform_layout,
            &texture_layout,
            "engine play",
        );
        let depth_view =
            render::make_depth_view(&device, config.width, config.height, "engine depth");
        let flat = material_bind_group(
            &device,
            &queue,
            &texture_layout,
            &sampler,
            &object_texture::flat_textures(),
            "engine flat material",
        );
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("engine vertices"),
            size: 4,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("engine indices"),
            size: 4,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
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
            vertex_buffer,
            index_buffer,
            vertex_capacity: 4,
            index_capacity: 4,
            flat,
            textures: BTreeMap::new(),
            bank,
        })
    }

    /// Current surface aspect ratio.
    pub(crate) fn aspect(&self) -> f32 {
        self.config.width.max(1) as f32 / self.config.height.max(1) as f32
    }

    fn ensure_texture(&mut self, page: u16) -> Result<(), RenderError> {
        if self.textures.contains_key(&page) {
            return Ok(());
        }
        let image = self
            .bank
            .decode_page(page)
            .map_err(|error| RenderError::Texture(error.to_string()))?;
        let flat = object_texture::flat_textures();
        let material = MaterialTextures {
            diffuse: image,
            normal: flat.normal,
            specular: flat.specular,
        };
        let bind_group = material_bind_group(
            &self.device,
            &self.queue,
            &self.texture_layout,
            &self.sampler,
            &material,
            "engine page material",
        );
        self.textures.insert(page, bind_group);
        Ok(())
    }

    fn clear_frame(&mut self) -> Result<(), RenderError> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(()),
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("engine clear"),
            });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("engine clear"),
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
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }
}

impl RenderBackend for WgpuBackend {
    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view =
                render::make_depth_view(&self.device, width, height, "engine depth");
        }
    }

    fn draw(&mut self, scene: &RenderScene) -> Result<(), RenderError> {
        let merged = merge_scene(scene);
        if merged.vertices.is_empty() || merged.indices.is_empty() {
            return self.clear_frame();
        }

        for page in scene.material_pages.iter().flatten() {
            self.ensure_texture(*page)?;
        }

        let mvp = render::mat_bytes(&scene.camera.mvp);
        self.queue.write_buffer(&self.uniform_buffer, 0, &mvp);
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&merged.vertices);
        upload_buffer(
            &self.device,
            &self.queue,
            &mut self.vertex_buffer,
            &mut self.vertex_capacity,
            vertex_bytes,
            wgpu::BufferUsages::VERTEX,
            "engine vertices",
        );
        let mut index_bytes = Vec::with_capacity(merged.indices.len() * 4);
        for index in &merged.indices {
            index_bytes.extend_from_slice(&index.to_le_bytes());
        }
        upload_buffer(
            &self.device,
            &self.queue,
            &mut self.index_buffer,
            &mut self.index_capacity,
            &index_bytes,
            wgpu::BufferUsages::INDEX,
            "engine indices",
        );

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(()),
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("engine play"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("engine play"),
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
            for (material, start, count) in &merged.ranges {
                let bind_group = match scene
                    .material_pages
                    .get(usize::from(*material) - 1)
                    .copied()
                    .flatten()
                {
                    None => &self.flat,
                    Some(page) => self
                        .textures
                        .get(&page)
                        .expect("the page was ensured before the pass"),
                };
                pass.set_bind_group(1, bind_group, &[]);
                pass.draw_indexed(*start..*start + *count, 0, 0..1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::merge_scene;
    use crate::engine::scene::{
        CameraView, DrawItem, MeshData, MeshHandle, RenderScene, SceneTransform,
    };

    fn mesh(material: u16) -> MeshData {
        MeshData {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            uvs: vec![[0, 0], [255, 0], [0, 255]],
            indices: vec![0, 1, 2],
            material_indices: vec![material, material, material],
        }
    }

    fn scene() -> RenderScene {
        RenderScene {
            camera: CameraView::identity(),
            transform: SceneTransform {
                center: [0.0, 0.0, 0.0],
                scale: 1.0,
            },
            items: vec![
                DrawItem {
                    mesh: MeshHandle(0),
                    world: [0, 0, 0],
                    visible: true,
                    class: 0xFF,
                },
                DrawItem {
                    mesh: MeshHandle(1),
                    world: [10, 0, 0],
                    visible: true,
                    class: 4,
                },
                DrawItem {
                    mesh: MeshHandle(1),
                    world: [20, 0, 0],
                    visible: false,
                    class: 4,
                },
            ],
            meshes: vec![mesh(1), mesh(2)],
            material_pages: vec![None, Some(7)],
        }
    }

    #[test]
    fn the_merge_skips_invisible_items_and_applies_the_world_offset() {
        let merged = merge_scene(&scene());
        assert_eq!(merged.vertices.len(), 6, "two visible triangles");
        assert_eq!(merged.vertices[0].pos, [0.0, 0.0, 0.0]);
        assert_eq!(
            merged.vertices[3].pos,
            [10.0, 0.0, 0.0],
            "the second item is translated by its integer world position"
        );
        assert_eq!(merged.indices, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn the_merge_groups_indices_by_material_in_ascending_order() {
        let merged = merge_scene(&scene());
        assert_eq!(merged.ranges, vec![(1, 0, 3), (2, 3, 3)]);
    }

    #[test]
    fn the_merge_is_deterministic() {
        let first = merge_scene(&scene());
        let second = merge_scene(&scene());
        assert_eq!(first, second);
    }
}
