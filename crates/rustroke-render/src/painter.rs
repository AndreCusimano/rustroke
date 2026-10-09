use std::collections::HashMap;

use rustroke_core::{ClippedMesh, Color, PhysicalSize, TextureAtlas, TextureId, TexturesDelta};

use crate::{CallbackFn, CallbackInfo};

/// Floats per vertex: position (2), uv (2), color (4).
const VERTEX_FLOATS: usize = 8;
const VERTEX_STRIDE: u64 = (VERTEX_FLOATS * size_of::<f32>()) as u64;

/// Draws [`ClippedMesh`]es into any texture view. Independent of windows
/// and surfaces, so it serves both on-screen and offscreen rendering.
#[derive(Debug)]
pub struct Painter {
    pipeline: wgpu::RenderPipeline,
    target_format: wgpu::TextureFormat,
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
    /// Atlas revision on the GPU (`None`: nothing uploaded yet).
    atlas_revision: Option<u64>,
}

/// A texture on the GPU and the bind group that samples it.
#[derive(Debug)]
struct GpuTexture {
    /// `None` for native textures, which the application owns.
    texture: Option<wgpu::Texture>,
    bind_group: wgpu::BindGroup,
}

impl GpuTexture {
    fn owned(&self) -> &wgpu::Texture {
        self.texture
            .as_ref()
            .expect("the atlas is always owned by the painter")
    }
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
            &atlas_texture.create_view(&wgpu::TextureViewDescriptor::default()),
            &sampler,
        );
        let mut textures = HashMap::new();
        textures.insert(
            TextureId::Atlas,
            GpuTexture {
                texture: Some(atlas_texture),
                bind_group,
            },
        );

        Self {
            pipeline,
            target_format,
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
            atlas_revision: None,
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
        let resized = self.textures[&TextureId::Atlas].owned().width() != size;
        if resized {
            let texture = create_texture(device, "rustroke atlas", size, size);
            self.insert_texture(device, TextureId::Atlas, texture);
        }
        let base = atlas.dirty_base();
        let dirty = atlas.take_dirty();
        let revision = atlas.revision();
        if !resized && self.atlas_revision == Some(revision) {
            return;
        }
        let texture = self.textures[&TextureId::Atlas].owned();
        match dirty {
            // Only what changed since this painter's last upload.
            Some(r) if !resized && self.atlas_revision == Some(base) => {
                upload_atlas_region(queue, texture, atlas, r.x, r.y, r.width, r.height);
            }
            // A new texture, or changes another painter (window) already
            // took: upload everything.
            _ => upload_atlas_region(queue, texture, atlas, 0, 0, size, size),
        }
        self.atlas_revision = Some(revision);
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
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = create_bind_group(
            device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            &view,
            &self.sampler,
        );
        self.textures.insert(
            id,
            GpuTexture {
                texture: Some(texture),
                bind_group,
            },
        );
    }

    /// Draws meshes using texture `id` from `view`, a texture owned by the
    /// application (no copy is made). Call again with a new view when the
    /// application recreates its texture. The view must be a filterable
    /// float 2D texture (e.g. `Rgba8UnormSrgb`); colors are read as linear
    /// with premultiplied alpha, like the rest of the UI.
    pub fn set_native_texture(
        &mut self,
        device: &wgpu::Device,
        id: TextureId,
        view: &wgpu::TextureView,
    ) {
        let bind_group = create_bind_group(
            device,
            &self.bind_group_layout,
            &self.uniform_buffer,
            view,
            &self.sampler,
        );
        self.textures.insert(
            id,
            GpuTexture {
                texture: None,
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
        let clear = wgpu::LoadOp::Clear(to_wgpu_color(job.clear_color));
        self.paint_with(device, queue, encoder, target, target_size, job, clear);
    }

    /// Like [`Painter::paint`], but draws over what `target` already holds
    /// (e.g. the application's 3D scene) instead of clearing it.
    pub fn paint_over(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        target_size: PhysicalSize,
        job: &PaintJob<'_>,
    ) {
        let load = wgpu::LoadOp::Load;
        self.paint_with(device, queue, encoder, target, target_size, job, load);
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_with(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        target_size: PhysicalSize,
        job: &PaintJob<'_>,
        load: wgpu::LoadOp<wgpu::Color>,
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

        // Custom drawing first prepares, before the render pass begins.
        let callbacks: Vec<(usize, &CallbackFn, CallbackInfo)> = job
            .meshes
            .iter()
            .enumerate()
            .filter_map(|(i, clipped)| {
                let paint = clipped.callback.as_ref()?;
                let Some(callback) = paint.callback.downcast_ref::<CallbackFn>() else {
                    log::warn!(
                        "paint callback of an unknown type; use rustroke_render::CallbackFn"
                    );
                    return None;
                };
                let info = self.callback_info(paint.rect, clipped.clip_rect, ppp, target_size)?;
                Some((i, callback, info))
            })
            .collect();
        for (_, callback, info) in &callbacks {
            callback.run_prepare(device, queue, encoder, info);
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rustroke pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        if self.index_data.is_empty() && callbacks.is_empty() {
            return;
        }
        let set_ui_state = |pass: &mut wgpu::RenderPass<'_>| {
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        };
        set_ui_state(&mut pass);

        let mut index_offset = 0u32;
        for (i, clipped) in job.meshes.iter().enumerate() {
            if clipped.callback.is_some() {
                if let Some((_, callback, info)) = callbacks.iter().find(|(j, ..)| *j == i) {
                    let [x, y, w, h] = info.viewport;
                    pass.set_viewport(x, y, w, h, 0.0, 1.0);
                    let [cx, cy, cw, ch] = info.clip;
                    pass.set_scissor_rect(cx, cy, cw, ch);
                    callback.run_paint(info, &mut pass);
                    // Back to the UI's state for the meshes that follow.
                    pass.set_viewport(
                        0.0,
                        0.0,
                        target_size.width as f32,
                        target_size.height as f32,
                        0.0,
                        1.0,
                    );
                    set_ui_state(&mut pass);
                }
                continue;
            }
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
                pass.draw_indexed(index_offset..index_offset + index_count, 0, 0..1);
            }
            index_offset += index_count;
        }
    }

    /// Viewport and scissor of a callback drawing in `rect` (points),
    /// clipped to `clip`. `None` if nothing would be visible.
    fn callback_info(
        &self,
        rect: rustroke_core::Rect,
        clip: rustroke_core::Rect,
        ppp: f32,
        target_size: PhysicalSize,
    ) -> Option<CallbackInfo> {
        let (tw, th) = (target_size.width as f32, target_size.height as f32);
        let x0 = (rect.min.x * ppp).round().clamp(0.0, tw);
        let y0 = (rect.min.y * ppp).round().clamp(0.0, th);
        let x1 = (rect.max.x * ppp).round().clamp(0.0, tw);
        let y1 = (rect.max.y * ppp).round().clamp(0.0, th);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let scissor = scissor_rect(clip.intersect(rect), ppp, target_size)?;
        Some(CallbackInfo {
            rect,
            viewport: [x0, y0, x1 - x0, y1 - y0],
            clip: scissor,
            pixels_per_point: ppp,
            target_size,
            target_format: self.target_format,
        })
    }

    /// Format of the textures this painter draws into.
    pub fn target_format(&self) -> wgpu::TextureFormat {
        self.target_format
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
        let mut base = 0u32;
        for m in meshes {
            for v in &m.mesh.vertices {
                self.vertex_data
                    .extend_from_slice(&[v.pos.x, v.pos.y, v.uv[0], v.uv[1]]);
                self.vertex_data.extend_from_slice(&v.color);
            }
            // Indices point into the shared vertex buffer: WebGL can't
            // draw with a base vertex.
            self.index_data
                .extend(m.mesh.indices.iter().map(|i| i + base));
            base += m.mesh.vertices.len() as u32;
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
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
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
                resource: wgpu::BindingResource::TextureView(view),
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
