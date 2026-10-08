//! wgpu rendering backend.
//!
//! This crate is windowing-agnostic: [`Renderer`] receives anything wgpu can
//! turn into a surface (e.g. an `Arc<winit::window::Window>`), and
//! [`OffscreenRenderer`] draws into an image without any window at all.

mod offscreen;
mod painter;

use std::fmt;

use rustroke_core::{PhysicalSize, TextureAtlas};
use wgpu::wgt::WgpuHasDisplayHandle;

pub use offscreen::OffscreenRenderer;
pub use painter::{PaintJob, Painter};

/// Errors that can occur while creating a renderer.
#[derive(Debug)]
pub enum RendererError {
    /// The window surface could not be created.
    CreateSurface(wgpu::CreateSurfaceError),
    /// No GPU adapter fits the surface.
    RequestAdapter(wgpu::RequestAdapterError),
    /// The GPU device could not be created.
    RequestDevice(wgpu::RequestDeviceError),
    /// The surface has no supported configuration on the chosen adapter.
    UnsupportedSurface,
}

impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateSurface(e) => write!(f, "failed to create surface: {e}"),
            Self::RequestAdapter(e) => write!(f, "no suitable GPU adapter: {e}"),
            Self::RequestDevice(e) => write!(f, "failed to create GPU device: {e}"),
            Self::UnsupportedSurface => f.write_str("surface is not supported by the GPU adapter"),
        }
    }
}

impl std::error::Error for RendererError {}

/// What happened to a frame passed to [`Renderer::render`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderOutcome {
    /// The frame was drawn and presented.
    Presented,
    /// The surface was reconfigured; render again as soon as possible.
    Retry,
    /// Nothing to draw into right now (e.g. occluded window); wait for the next event.
    Skipped,
}

/// Owns the GPU device and the window surface it renders to.
#[derive(Debug)]
pub struct Renderer {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    painter: Painter,
}

impl Renderer {
    /// Creates a renderer for a window.
    ///
    /// `window` is used both as the surface target and as the display handle,
    /// which some backends (OpenGL on Wayland/X11) require.
    pub async fn new<W>(
        window: W,
        size: PhysicalSize,
        atlas: &TextureAtlas,
    ) -> Result<Self, RendererError>
    where
        W: Into<wgpu::SurfaceTarget<'static>> + WgpuHasDisplayHandle + Clone,
    {
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(window.clone())),
        );
        let surface = instance
            .create_surface(window)
            .map_err(RendererError::CreateSurface)?;
        let adapter = request_adapter(&instance, Some(&surface)).await?;
        let (device, queue) = request_device(&adapter).await?;

        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or(RendererError::UnsupportedSurface)?;
        // Work in linear space: prefer an sRGB surface so the GPU encodes on write.
        let caps = surface.get_capabilities(&adapter);
        if let Some(&srgb) = caps.formats.iter().find(|f| f.is_srgb()) {
            config.format = srgb;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);

        let painter = Painter::new(&device, config.format, atlas.size());
        Ok(Self {
            surface,
            config,
            device,
            queue,
            painter,
        })
    }

    /// Current surface size in physical pixels.
    pub fn size(&self) -> PhysicalSize {
        PhysicalSize::new(self.config.width, self.config.height)
    }

    /// Resizes the surface. Empty sizes (minimized window) are ignored.
    pub fn resize(&mut self, size: PhysicalSize) {
        if size.is_empty() || size == self.size() {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Draws and presents one frame.
    pub fn render(&mut self, job: &PaintJob<'_>, atlas: &mut TextureAtlas) -> RenderOutcome {
        // Upload texture changes even if this frame can't be shown, so
        // they are not lost.
        self.painter.update_atlas(&self.device, &self.queue, atlas);
        self.painter
            .set_textures(&self.device, &self.queue, job.textures);
        let outcome = self.draw_frame(job);
        self.painter.free_textures(job.textures);
        outcome
    }

    fn draw_frame(&mut self, job: &PaintJob<'_>) -> RenderOutcome {
        let (frame, suboptimal) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return RenderOutcome::Retry;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return RenderOutcome::Skipped;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                log::error!("surface validation error");
                return RenderOutcome::Skipped;
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rustroke frame"),
            });
        self.painter.paint(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            self.size(),
            job,
        );
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);

        if suboptimal {
            // Presented anyway; reconfigure for the next frame.
            self.surface.configure(&self.device, &self.config);
        }
        RenderOutcome::Presented
    }
}

async fn request_adapter(
    instance: &wgpu::Instance,
    compatible_surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, RendererError> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::from_env().unwrap_or_default(),
            compatible_surface,
            ..Default::default()
        })
        .await
        .map_err(RendererError::RequestAdapter)?;
    log::info!("GPU adapter: {:?}", adapter.get_info());
    Ok(adapter)
}

async fn request_device(
    adapter: &wgpu::Adapter,
) -> Result<(wgpu::Device, wgpu::Queue), RendererError> {
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("rustroke device"),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await
        .map_err(RendererError::RequestDevice)
}
