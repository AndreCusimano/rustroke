//! Custom wgpu drawing inside the UI.

use rustroke_core::{PaintCallback, PhysicalSize, Rect, Shape};

/// Where and how a [`CallbackFn`] draws this frame.
#[derive(Clone, Copy, Debug)]
pub struct CallbackInfo {
    /// The callback's area in logical points.
    pub rect: Rect,
    /// The viewport set for the callback, in physical pixels:
    /// `[x, y, width, height]` (the area, clamped to the target).
    pub viewport: [f32; 4],
    /// The scissor rectangle in physical pixels: `[x, y, width, height]`
    /// (the area within the clip rectangle of the shape).
    pub clip: [u32; 4],
    /// Physical pixels per logical point.
    pub pixels_per_point: f32,
    /// Size of the whole render target in physical pixels.
    pub target_size: PhysicalSize,
    /// Format of the render target, for creating a compatible pipeline.
    pub target_format: wgpu::TextureFormat,
}

type PrepareFn =
    dyn Fn(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &CallbackInfo) + Send + Sync;
type PaintFn = dyn for<'a> Fn(&CallbackInfo, &mut wgpu::RenderPass<'a>) + Send + Sync;

/// Drawing code run by the renderer inside a rectangle of the UI, in the
/// same render pass as the UI and in order with it (shapes added after
/// the callback are drawn on top). For small custom visuals (a gizmo, a
/// preview, a chart); a 3D viewport with its own depth buffer is better
/// drawn into a texture shown with `Frame::register_native_texture`.
///
/// The viewport is set to the rectangle, so clip-space coordinates
/// (-1..1) cover it; the scissor rectangle keeps drawing inside it. Create
/// pipelines for [`CallbackInfo::target_format`], without depth or
/// multisampling. Resources (pipelines, buffers) can be created up front
/// with `Frame::wgpu` and moved into the closures, or lazily in
/// [`CallbackFn::prepare`].
///
/// ```ignore
/// let callback = CallbackFn::new(move |_info, pass| {
///     pass.set_pipeline(&pipeline);
///     pass.draw(0..3, 0..1);
/// });
/// ui.painter().add(callback.into_shape(rect));
/// ```
pub struct CallbackFn {
    prepare: Option<Box<PrepareFn>>,
    paint: Box<PaintFn>,
}

impl std::fmt::Debug for CallbackFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallbackFn")
            .field("has_prepare", &self.prepare.is_some())
            .finish_non_exhaustive()
    }
}

impl CallbackFn {
    /// A callback that records draw calls into the UI's render pass.
    pub fn new(
        paint: impl for<'a> Fn(&CallbackInfo, &mut wgpu::RenderPass<'a>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            prepare: None,
            paint: Box::new(paint),
        }
    }

    /// Runs `prepare` before the UI's render pass begins: upload buffers
    /// with the queue, record other passes (e.g. into a texture) with the
    /// encoder, create resources.
    pub fn prepare(
        mut self,
        prepare: impl Fn(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &CallbackInfo)
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.prepare = Some(Box::new(prepare));
        self
    }

    /// The shape that runs this callback inside `rect` (logical points).
    pub fn into_shape(self, rect: Rect) -> Shape {
        Shape::Callback(PaintCallback::new(rect, self))
    }

    pub(crate) fn run_prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        info: &CallbackInfo,
    ) {
        if let Some(prepare) = &self.prepare {
            prepare(device, queue, encoder, info);
        }
    }

    pub(crate) fn run_paint(&self, info: &CallbackInfo, pass: &mut wgpu::RenderPass<'_>) {
        (self.paint)(info, pass);
    }
}
