//! rustroke inside an application that already owns its window, event
//! loop and wgpu device (e.g. a game or a CAD program with its own
//! renderer), instead of [`crate::run`].

use std::time::{Duration, Instant};

use rustroke_core::{
    ClippedMesh, Color, DisplayList, PhysicalSize, RawInput, Rect, Tessellator, TexturesDelta,
    point,
};
use rustroke_render::{PaintJob, Painter, wgpu};
use rustroke_text::Fonts;
use rustroke_widgets::{Context, CursorIcon};
use winit::event::WindowEvent;
use winit::window::Window;

use crate::Frame;
use crate::input::{self, InputCollector};

/// What [`Integration::on_window_event`] did with an event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventResponse {
    /// The UI needs a new frame: call `window.request_redraw()`.
    pub repaint: bool,
    /// The event was for the UI (the pointer is over a panel or widget, a
    /// UI drag is in progress, a text field has focus): don't also use it
    /// for the application's own content.
    pub consumed: bool,
}

/// What [`Integration::run`] asks of the host after a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunOutput {
    /// Draw another frame right away (animations).
    pub repaint: bool,
    /// Draw another frame after this delay (e.g. to show a tooltip).
    pub repaint_after: Option<Duration>,
}

/// The UI of an application that drives winit and wgpu itself. Per
/// window:
///
/// 1. pass every `WindowEvent` to [`Integration::on_window_event`]
///    (request a redraw when it says so, and skip your own handling when
///    the UI consumed the event);
/// 2. on `RedrawRequested`, render your content, then call
///    [`Integration::run`] with your UI code and [`Integration::paint`]
///    to draw the UI on top, in the same encoder;
/// 3. honor [`RunOutput`] (request redraws).
///
/// ```ignore
/// let mut ui = Integration::new(&device, surface_format);
/// // in window_event:
/// let response = ui.on_window_event(&window, &event);
/// if response.repaint { window.request_redraw(); }
/// if !response.consumed { /* camera controls... */ }
/// // in RedrawRequested, after drawing the scene into `view`:
/// let out = ui.run(&window, |frame| {
///     Panel::left("tools").show(frame, |ui| { ui.button("Extrude"); });
/// });
/// ui.paint(&device, &queue, &mut encoder, &view, size);
/// queue.submit([encoder.finish()]);
/// if out.repaint { window.request_redraw(); }
/// ```
///
/// [`Frame::wgpu`] and [`Frame::register_native_texture`] are not
/// available here (the application has the device); register textures
/// with [`Integration::painter`] instead.
pub struct Integration {
    ctx: Context,
    fonts: Fonts,
    input: InputCollector,
    painter: Painter,
    start: Instant,
    cursor: CursorIcon,
    ime_allowed: bool,
    shapes: DisplayList,
    meshes: Vec<ClippedMesh>,
    /// Texture changes not yet uploaded (frames may run without paint).
    textures: TexturesDelta,
    pixels_per_point: f32,
    title: Option<String>,
    /// Window changes from the last frame, applied by `run`.
    pending_cursor: Option<CursorIcon>,
    pending_ime: Option<Rect>,
}

impl std::fmt::Debug for Integration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Integration")
            .field("pixels_per_point", &self.pixels_per_point)
            .finish_non_exhaustive()
    }
}

impl Integration {
    /// A UI drawn with `device` into targets of `target_format` (use an
    /// sRGB format). Loads the bundled and system fonts.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self::with_fonts(device, target_format, Fonts::new())
    }

    /// Like [`Integration::new`] with the given fonts (e.g.
    /// `Fonts::bundled_only()` for tests).
    pub fn with_fonts(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        fonts: Fonts,
    ) -> Self {
        let painter = Painter::new(device, target_format, fonts.atlas().size());
        Self {
            ctx: Context::new(),
            fonts,
            input: InputCollector::default(),
            painter,
            start: Instant::now(),
            cursor: CursorIcon::Default,
            ime_allowed: false,
            shapes: DisplayList::new(),
            meshes: Vec::new(),
            textures: TexturesDelta::default(),
            pixels_per_point: 1.0,
            title: None,
            pending_cursor: None,
            pending_ime: None,
        }
    }

    /// The UI context (style, focus, data...).
    pub fn ctx(&mut self) -> &mut Context {
        &mut self.ctx
    }

    /// The fonts and glyph atlas (also where SVG icons are loaded).
    pub fn fonts(&mut self) -> &mut Fonts {
        &mut self.fonts
    }

    /// The painter, e.g. to show the application's own textures with
    /// [`Painter::set_native_texture`] (allocate the id with
    /// `Context::allocate_texture`).
    pub fn painter(&mut self) -> &mut Painter {
        &mut self.painter
    }

    /// Feeds a window event to the UI.
    pub fn on_window_event(&mut self, window: &Window, event: &WindowEvent) -> EventResponse {
        self.on_window_event_scaled(event, window.scale_factor())
    }

    /// [`Integration::on_window_event`] with the window's scale factor.
    pub fn on_window_event_scaled(
        &mut self,
        event: &WindowEvent,
        scale_factor: f64,
    ) -> EventResponse {
        let repaint = match event {
            WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. }
            | WindowEvent::Occluded(false) => true,
            event => self.input.on_window_event(event, scale_factor),
        };
        let consumed = match event {
            WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::PinchGesture { .. } => {
                self.ctx.wants_pointer_input()
                    || self
                        .input
                        .pointer()
                        .is_some_and(|p| self.ctx.is_pointer_over_ui(p))
            }
            WindowEvent::KeyboardInput { .. } | WindowEvent::Ime(_) => {
                self.ctx.wants_keyboard_input()
            }
            _ => false,
        };
        EventResponse { repaint, consumed }
    }

    /// Runs one frame of the UI for a window of `size` physical pixels at
    /// `pixels_per_point`, without a `winit` window (tests, offscreen).
    /// [`Integration::run`] also updates the window (cursor, IME, title).
    pub fn run_frame(
        &mut self,
        size: PhysicalSize,
        pixels_per_point: f32,
        add: impl FnOnce(&mut Frame<'_>),
    ) -> RunOutput {
        self.pixels_per_point = pixels_per_point;
        let screen_rect = Rect::from_min_max(
            point(0.0, 0.0),
            point(
                size.width as f32 / pixels_per_point,
                size.height as f32 / pixels_per_point,
            ),
        );
        let time = self.start.elapsed();
        self.ctx.begin_frame(RawInput {
            time: time.as_secs_f64(),
            screen_rect,
            pixels_per_point,
            events: self.input.take_events(),
        });
        let mut shapes = std::mem::take(&mut self.shapes);
        shapes.clear();
        let mut frame = Frame {
            time,
            screen_rect,
            pixels_per_point,
            shapes,
            clear_color: Color::TRANSPARENT,
            request_repaint: false,
            fonts: &mut self.fonts,
            ctx: &mut self.ctx,
            renderer: None,
            title: None,
            windows: Vec::new(),
            window_id: None,
        };
        add(&mut frame);
        let Frame {
            mut shapes,
            request_repaint,
            title,
            ..
        } = frame;
        if title.is_some() {
            self.title = title;
        }
        self.fonts.end_frame();
        let mut output = self.ctx.end_frame();
        shapes.append(&mut output.shapes);
        if let Some(text) = output.copied_text.take() {
            self.input.set_clipboard_text(text);
        }
        self.textures.set.append(&mut output.textures.set);
        self.textures.free.append(&mut output.textures.free);
        self.meshes = Tessellator::new(pixels_per_point, self.fonts.atlas()).tessellate(&shapes);
        self.shapes = shapes;
        self.pending_cursor = Some(output.cursor);
        self.pending_ime = output.ime_cursor;
        RunOutput {
            repaint: request_repaint || output.repaint,
            repaint_after: output.repaint_after.map(Duration::from_secs_f64),
        }
    }

    /// Runs one frame of the UI with `add` (which shows panels, windows
    /// and widgets as in [`crate::App::update`]) and applies its cursor,
    /// input method area and title to `window`.
    pub fn run(&mut self, window: &Window, add: impl FnOnce(&mut Frame<'_>)) -> RunOutput {
        let size = window.inner_size();
        // Window scale factors are small values like 1.0, 1.5 or 2.0.
        #[allow(clippy::cast_possible_truncation)]
        let ppp = window.scale_factor() as f32;
        let out = self.run_frame(PhysicalSize::new(size.width, size.height), ppp, add);
        if let Some(cursor) = self.pending_cursor.take()
            && cursor != self.cursor
        {
            self.cursor = cursor;
            window.set_cursor(input::cursor_icon(cursor));
        }
        let ime = self.pending_ime.take();
        if ime.is_some() != self.ime_allowed {
            self.ime_allowed = ime.is_some();
            window.set_ime_allowed(self.ime_allowed);
        }
        if let Some(r) = ime {
            window.set_ime_cursor_area(
                winit::dpi::LogicalPosition::new(r.min.x, r.min.y),
                winit::dpi::LogicalSize::new(r.width().max(1.0), r.height()),
            );
        }
        if let Some(title) = self.title.take() {
            window.set_title(&title);
        }
        out
    }

    /// Records the UI of the last [`Integration::run`] into `encoder`,
    /// drawn over what `target` (of `size` physical pixels) already holds.
    pub fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: PhysicalSize,
    ) {
        self.painter
            .update_atlas(device, queue, self.fonts.atlas_mut());
        let textures = std::mem::take(&mut self.textures);
        self.painter.set_textures(device, queue, &textures);
        let job = PaintJob {
            meshes: &self.meshes,
            textures: &textures,
            pixels_per_point: self.pixels_per_point,
            clear_color: Color::TRANSPARENT,
        };
        self.painter
            .paint_over(device, queue, encoder, target, size, &job);
        self.painter.free_textures(&textures);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustroke_render::OffscreenRenderer;
    use rustroke_widgets::Panel;
    use winit::dpi::PhysicalPosition;
    use winit::event::DeviceId;

    fn moved(x: f64, y: f64) -> WindowEvent {
        WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position: PhysicalPosition::new(x, y),
        }
    }

    /// INT-04: events over the UI are consumed, and the UI is drawn over
    /// the application's content without clearing it.
    #[test]
    fn integration_consumes_ui_events_and_paints_over() {
        let offscreen = match pollster::block_on(OffscreenRenderer::new(
            &rustroke_core::TextureAtlas::new(64),
        )) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipping integration test: {e}");
                return;
            }
        };
        let (device, queue) = (offscreen.device(), offscreen.queue());
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let mut ui = Integration::with_fonts(device, format, Fonts::bundled_only());
        let size = PhysicalSize::new(200, 100);
        let show = |ui: &mut Integration| {
            ui.run_frame(size, 1.0, |frame| {
                Panel::left("tools")
                    .default_size(80.0)
                    .resizable(false)
                    .show(frame, |ui| {
                        ui.button("Tool");
                    });
            })
        };
        show(&mut ui);
        show(&mut ui);

        let over_panel = ui.on_window_event_scaled(&moved(20.0, 50.0), 1.0);
        assert!(over_panel.consumed && over_panel.repaint);
        let over_scene = ui.on_window_event_scaled(&moved(150.0, 50.0), 1.0);
        assert!(!over_scene.consumed, "the app's 3D view gets it");
        show(&mut ui);

        // The app draws its scene (red), then the UI on top.
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene"),
            size: wgpu::Extent3d {
                width: 200,
                height: 100,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        }));
        ui.paint(device, queue, &mut encoder, &view, size);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256 * 4 * 100,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256 * 4),
                    rows_per_image: Some(100),
                },
            },
            wgpu::Extent3d {
                width: 200,
                height: 100,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = buffer.slice(..).get_mapped_range().unwrap();
        let px = |x: usize, y: usize| {
            let i = y * 256 * 4 + x * 4;
            [data[i], data[i + 1], data[i + 2]]
        };
        assert_eq!(px(150, 50), [255, 0, 0], "the scene is kept");
        assert_ne!(px(40, 90), [255, 0, 0], "the panel covers it");
    }
}
