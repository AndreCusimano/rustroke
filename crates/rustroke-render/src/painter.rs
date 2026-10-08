use std::collections::HashMap;

use rustroke_core::{ClippedMesh, Color, PhysicalSize, TextureAtlas, TextureId, TexturesDelta};

/// Floats per vertex: position (2), uv (2), color (4).
const VERTEX_FLOATS: usize = 8;
const VERTEX_STRIDE: u64 = (VERTEX_FLOATS * size_of::<f32>()) as u64;

/// Draws [`ClippedMesh`]es into any texture view. Independent of windows
/// and surfaces, so it serves both on-screen and offscreen rendering.
#[derive(Debug)]
pub struct Painter {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    /// The atlas and every user texture, each with its bind group.
    textures: HashMap<TextureId, GpuTexture>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    /// Reused CPU staging memory.
    vertex_data: Vec<f32>,
    index_data: Vec<u32>,
}

/// A texture on the GPU and the bind group that samples it.
#[derive(Debug)]
struct GpuTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

/// Everything needed to draw one frame.
#[derive(Debug)]
pub struct PaintJob<'a> {
    /// Meshes to draw, back to front.
    pub meshes: &'a [ClippedMesh],
    /// User textures to create before drawing and free afterwards.
    pub textures: &'a TexturesDelta,
    /// Physical pixels per logical point.
    pub pixels_per_point: f32,
    /// Color the target is cleared to first.
    pub clear_color: Color,
}

impl Painter {
    /// `target_format` should be an sRGB format so blending happens in
    /// linear space.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat, atlas_size: u32) -> Self {
        if !target_format.is_srgb() {
            log::warn!("render target {target_format:?} is not sRGB; colors will look too dark");
        }
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rustroke bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
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
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rustroke pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let premultiplied_alpha = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rustroke pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: VERTEX_STRIDE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x4,
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState {
                // Feathering produces triangles of both windings.
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState {
                        color: premultiplied_alpha,
                        alpha: premultiplied_alpha,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rustroke uniforms"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rustroke atlas sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let atlas_texture = create_texture(device, "rustroke atlas", atlas_size, atlas_size);
        let bind_group = create_bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            &atlas_texture,
            &sampler,
        );
        let mut textures = HashMap::new();
        textures.insert(
            TextureId::Atlas,
            GpuTexture {
                texture: atlas_texture,
                bind_group,
            },
        );

        Self {
            pipeline,
            bind_group_layout,
            uniform_buffer,
            sampler,
            textures,
            vertex_buffer: create_buffer(
                device,
                "rustroke vertices",
                wgpu::BufferUsages::VERTEX,
                1024 * VERTEX_STRIDE,
            ),
            index_buffer: create_buffer(
                device,
                "rustroke indices",
                wgpu::BufferUsages::INDEX,
                3 * 1024 * 4,
            ),
            vertex_data: Vec::new(),
            index_data: Vec::new(),
        }
    }

    /// Uploads the parts of `atlas` that changed since the last call.
    pub fn update_atlas(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &mut TextureAtlas,
    ) {
        let size = atlas.size();
        if self.textures[&TextureId::Atlas].texture.width() != size {
            let texture = create_texture(device, "rustroke atlas", size, size);
            self.insert_texture(device, TextureId::Atlas, texture);
            // New texture: everything must be uploaded.
            atlas.take_dirty();
            let texture = &self.textures[&TextureId::Atlas].texture;
            upload_atlas_region(queue, texture, atlas, 0, 0, size, size);
            return;
        }
        if let Some(r) = atlas.take_dirty() {
            let texture = &self.textures[&TextureId::Atlas].texture;
            upload_atlas_region(queue, texture, atlas, r.x, r.y, r.width, r.height);
        }
    }

    /// Creates the user textures in `delta.set`. Call before painting.
    pub fn set_textures(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        delta: &TexturesDelta,
    ) {
        for (id, image) in &delta.set {
            let [w, h] = image.size;
            if w == 0 || h == 0 {
                continue;
            }
            let texture = create_texture(device, "rustroke user texture", w, h);
            let pixels = image.to_premultiplied();
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&pixels),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
            self.insert_texture(device, *id, texture);
        }
    }

    /// Releases the user textures in `delta.free`. Call after painting.
    pub fn free_textures(&mut self, delta: &TexturesDelta) {
        for id in &delta.free {
            if *id != TextureId::Atlas {
                self.textures.remove(id);
            }
        }
    }

    fn insert_texture(&mut self, device: &wgpu::Device, id: TextureId, texture: wgpu::Texture) {
        let bind_group = create_bind_group(
            device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            &texture,
            &self.sampler,
        );
        self.textures.insert(
            id,
            GpuTexture {
                texture,
                bind_group,
            },
        );
    }

    /// Records a render pass that clears `target` and draws `job` into it.
    pub fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        target_size: PhysicalSize,
        job: &PaintJob<'_>,
    ) {
        let ppp = job.pixels_per_point;
        let screen_points = [
            target_size.width as f32 / ppp,
            target_size.height as f32 / ppp,
            0.0,
            0.0,
        ];
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&screen_points),
        );
        self.upload_meshes(device, queue, job.meshes);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rustroke pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(to_wgpu_color(job.clear_color)),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        if self.index_data.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        let (mut index_offset, mut vertex_offset) = (0u32, 0i32);
        for clipped in job.meshes {
            let index_count = clipped.mesh.indices.len() as u32;
            let texture = self.textures.get(&clipped.mesh.texture);
            if texture.is_none() {
                log::warn!("mesh uses unknown texture {:?}", clipped.mesh.texture);
            }
            if let (Some(texture), Some([x, y, w, h])) =
                (texture, scissor_rect(clipped.clip_rect, ppp, target_size))
            {
                pass.set_bind_group(0, &texture.bind_group, &[]);
                pass.set_scissor_rect(x, y, w, h);
                pass.draw_indexed(
                    index_offset..index_offset + index_count,
                    vertex_offset,
                    0..1,
                );
            }
            index_offset += index_count;
            vertex_offset += clipped.mesh.vertices.len() as i32;
        }
    }

    /// Packs all meshes into one vertex and one index buffer, growing them
    /// when needed.
    fn upload_meshes(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        meshes: &[ClippedMesh],
    ) {
        self.vertex_data.clear();
        self.index_data.clear();
        for m in meshes {
            for v in &m.mesh.vertices {
                self.vertex_data
                    .extend_from_slice(&[v.pos.x, v.pos.y, v.uv[0], v.uv[1]]);
                self.vertex_data.extend_from_slice(&v.color);
            }
            // Indices stay mesh-relative; draw_indexed applies a base vertex.
            self.index_data.extend_from_slice(&m.mesh.indices);
        }
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&self.vertex_data);
        let index_bytes: &[u8] = bytemuck::cast_slice(&self.index_data);
        if vertex_bytes.len() as u64 > self.vertex_buffer.size() {
            let size = (vertex_bytes.len() as u64).next_power_of_two();
            self.vertex_buffer = create_buffer(
                device,
                "rustroke vertices",
                wgpu::BufferUsages::VERTEX,
                size,
            );
        }
        if index_bytes.len() as u64 > self.index_buffer.size() {
            let size = (index_bytes.len() as u64).next_power_of_two();
            self.index_buffer =
                create_buffer(device, "rustroke indices", wgpu::BufferUsages::INDEX, size);
        }
        if !vertex_bytes.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, vertex_bytes);
            queue.write_buffer(&self.index_buffer, 0, index_bytes);
        }
    }
}

/// Converts a clip rectangle in points to a scissor rectangle in pixels,
/// clamped to the target. `None` if nothing would be visible.
fn scissor_rect(clip: rustroke_core::Rect, ppp: f32, target: PhysicalSize) -> Option<[u32; 4]> {
    let (tw, th) = (target.width as f32, target.height as f32);
    let x0 = (clip.min.x * ppp).floor().clamp(0.0, tw);
    let y0 = (clip.min.y * ppp).floor().clamp(0.0, th);
    let x1 = (clip.max.x * ppp).ceil().clamp(0.0, tw);
    let y1 = (clip.max.y * ppp).ceil().clamp(0.0, th);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    // All values are clamped to [0, target size], so they fit in u32.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some([x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32])
}

fn create_buffer(
    device: &wgpu::Device,
    label: &str,
    usage: wgpu::BufferUsages,
    size: u64,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_texture(device: &wgpu::Device, label: &str, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn create_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    atlas: &wgpu::Texture,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    let view = atlas.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rustroke bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn upload_atlas_region(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    atlas: &TextureAtlas,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) {
    let stride = atlas.size() * 4;
    let bytes: &[u8] = bytemuck::cast_slice(atlas.pixels());
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        // Point into the full atlas image; write_texture handles the stride.
        &bytes[(y * stride + x * 4) as usize..],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(stride),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

fn to_wgpu_color(c: Color) -> wgpu::Color {
    // The clear color is written as-is, so premultiply it like vertex colors.
    wgpu::Color {
        r: f64::from(c.r * c.a),
        g: f64::from(c.g * c.a),
        b: f64::from(c.b * c.a),
        a: f64::from(c.a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustroke_core::{Rect, point};

    #[test]
    fn scissor_is_scaled_and_clamped() {
        let target = PhysicalSize::new(200, 100);
        let clip = Rect::from_min_max(point(10.2, -5.0), point(30.4, 20.0));
        assert_eq!(scissor_rect(clip, 2.0, target), Some([20, 0, 41, 40]));
        assert_eq!(
            scissor_rect(Rect::EVERYTHING, 1.0, target),
            Some([0, 0, 200, 100])
        );
        let outside = Rect::from_min_max(point(300.0, 0.0), point(400.0, 10.0));
        assert_eq!(scissor_rect(outside, 1.0, target), None);
    }
}
