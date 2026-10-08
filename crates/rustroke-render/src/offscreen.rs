use rustroke_core::{PhysicalSize, TextureAtlas};

use crate::{PaintJob, Painter, RendererError, request_adapter, request_device};

/// Renders into an in-memory image instead of a window. Used for snapshot
/// tests and screenshots.
#[derive(Debug)]
pub struct OffscreenRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    painter: Painter,
}

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

impl OffscreenRenderer {
    /// Creates a renderer on the default GPU, without a window.
    pub async fn new(atlas: &TextureAtlas) -> Result<Self, RendererError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = request_adapter(&instance, None).await?;
        let (device, queue) = request_device(&adapter).await?;
        let painter = Painter::new(&device, FORMAT, atlas.size());
        Ok(Self {
            device,
            queue,
            painter,
        })
    }

    /// Draws `job` into a `size` image and returns its pixels as tightly
    /// packed sRGB RGBA8 rows (premultiplied alpha).
    pub fn render(
        &mut self,
        size: PhysicalSize,
        job: &PaintJob<'_>,
        atlas: &mut TextureAtlas,
    ) -> Vec<u8> {
        let extent = wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rustroke offscreen target"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        // Buffer rows must be aligned to COPY_BYTES_PER_ROW_ALIGNMENT.
        let row_bytes = size.width * 4;
        let padded_row_bytes = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rustroke readback"),
            size: u64::from(padded_row_bytes) * u64::from(size.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        self.painter.update_atlas(&self.device, &self.queue, atlas);
        self.painter
            .set_textures(&self.device, &self.queue, job.textures);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rustroke offscreen"),
            });
        self.painter
            .paint(&self.device, &self.queue, &mut encoder, &view, size, job);
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes),
                    rows_per_image: Some(size.height),
                },
            },
            extent,
        );
        self.queue.submit([encoder.finish()]);
        self.painter.free_textures(job.textures);

        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("failed to map readback buffer");
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU device lost while reading back");

        let mapped = slice
            .get_mapped_range()
            .expect("readback buffer was mapped above");
        let mut pixels = Vec::with_capacity((row_bytes * size.height) as usize);
        for row in mapped.chunks_exact(padded_row_bytes as usize) {
            pixels.extend_from_slice(&row[..row_bytes as usize]);
        }
        pixels
    }
}
