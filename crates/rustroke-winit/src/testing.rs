//! Run an [`App`] without a window, for tests: feed it mouse and keyboard
//! input, then inspect its widgets and output.
//!
//! ```
//! use rustroke_winit::{Frame, testing::Harness};
//!
//! let mut count = 0;
//! let mut app = |frame: &mut Frame| {
//!     frame.ui(|ui| {
//!         if ui.button("Add").clicked() {
//!             count += 1;
//!         }
//!     });
//! };
//! let mut harness = Harness::new();
//! assert!(harness.click(&mut app, "Add"));
//! drop(app);
//! assert_eq!(count, 1);
//! ```
//!
//! Widgets are found by the label they report to screen readers (the
//! visible text, or `accessible_label`). Text uses only the bundled font,
//! so results are the same on every machine.
//!
//! [`Harness::render`] draws the last frame offscreen on the GPU, e.g. to
//! save it as a PNG or compare it with a reference image.

use std::path::Path;
use std::time::Duration;

use rustroke_core::{
    Color, ColorImage, DisplayList, Event, Key, Modifiers, PhysicalSize, Point, PointerButton,
    RawInput, Rect, Tessellator, TextureId, TexturesDelta, vec2,
};
use rustroke_render::{OffscreenRenderer, PaintJob, RendererError};
use rustroke_text::Fonts;
use rustroke_widgets::{Context, FrameOutput, Id, WidgetDescription};

use crate::{App, Frame, WindowOptions};

/// A window-less host for an [`App`]. See the [module docs](self).
pub struct Harness {
    ctx: Context,
    fonts: Fonts,
    time: f64,
    screen_rect: Rect,
    pixels_per_point: f32,
    clear_color: Color,
    title: Option<String>,
    shapes: DisplayList,
    output: FrameOutput,
    frames: u64,
    /// Every texture still alive, for rendering (each frame's
    /// `TexturesDelta` only has the changes).
    textures: Vec<(TextureId, ColorImage)>,
    renderer: Option<OffscreenRenderer>,
    /// Extra windows requested by the last frame.
    windows: Vec<(Id, WindowOptions)>,
    /// The UI state of each extra window.
    window_contexts: Vec<(Id, Context)>,
    /// The last frame called [`Frame::close`].
    close_requested: bool,
}

impl std::fmt::Debug for Harness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Harness")
            .field("time", &self.time)
            .field("screen_rect", &self.screen_rect)
            .field("frames", &self.frames)
            .finish_non_exhaustive()
    }
}

/// An image of a frame: sRGB RGBA8 pixels, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screenshot {
    /// Width and height in physical pixels.
    pub size: [u32; 2],
    /// `size[0] * size[1] * 4` bytes, top row first.
    pub pixels: Vec<u8>,
}

impl Screenshot {
    /// The color of pixel `(x, y)` as sRGB RGBA bytes.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.size[0] + x) * 4) as usize;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }

    /// Saves the image as a PNG file.
    pub fn save_png(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let file = std::io::BufWriter::new(std::fs::File::create(path)?);
        let mut encoder = png::Encoder::new(file, self.size[0], self.size[1]);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        encoder
            .write_header()
            .and_then(|mut w| w.write_image_data(&self.pixels))
            .map_err(std::io::Error::other)
    }
}

impl Default for Harness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness {
    /// A host with an 800 × 600 point "window" at one pixel per point.
    pub fn new() -> Self {
        Self::with_size(800.0, 600.0)
    }

    /// A host with a window of `width × height` points.
    pub fn with_size(width: f32, height: f32) -> Self {
        Self {
            ctx: Context::new(),
            fonts: Fonts::bundled_only(),
            time: 0.0,
            screen_rect: Rect::from_min_size(Point::ZERO, vec2(width, height)),
            pixels_per_point: 1.0,
            clear_color: Color::BLACK,
            title: None,
            shapes: DisplayList::new(),
            output: FrameOutput::default(),
            frames: 0,
            textures: Vec::new(),
            renderer: None,
            windows: Vec::new(),
            window_contexts: Vec::new(),
            close_requested: false,
        }
    }

    /// Physical pixels per point (e.g. 2.0 for a HiDPI screen).
    pub fn with_pixels_per_point(mut self, pixels_per_point: f32) -> Self {
        self.pixels_per_point = pixels_per_point;
        self
    }

    /// The context (style, focus, data, ...).
    pub fn ctx(&mut self) -> &mut Context {
        &mut self.ctx
    }

    /// Lets `seconds` pass before the next frame (animations, tooltips).
    pub fn advance_time(&mut self, seconds: f64) {
        self.time += seconds;
    }

    /// Runs one frame without input.
    pub fn run(&mut self, app: &mut impl App) -> &FrameOutput {
        self.run_with_events(app, Vec::new())
    }

    /// Runs one frame with the given input events.
    pub fn run_with_events(&mut self, app: &mut impl App, events: Vec<Event>) -> &FrameOutput {
        self.time += 1.0 / 60.0;
        self.frames += 1;
        self.ctx.begin_frame(RawInput {
            time: self.time,
            screen_rect: self.screen_rect,
            pixels_per_point: self.pixels_per_point,
            events,
        });
        let mut frame = Frame {
            time: Duration::from_secs_f64(self.time),
            screen_rect: self.screen_rect,
            pixels_per_point: self.pixels_per_point,
            shapes: DisplayList::new(),
            clear_color: self.clear_color,
            request_repaint: false,
            fonts: &mut self.fonts,
            ctx: &mut self.ctx,
            renderer: None,
            title: None,
            windows: Vec::new(),
            window_id: None,
            close: false,
            titlebar_height: 0.0,
        };
        app.update(&mut frame);
        let Frame {
            mut shapes,
            clear_color,
            title,
            windows,
            close,
            ..
        } = frame;
        self.windows = windows;
        self.close_requested = close;
        self.clear_color = clear_color;
        if title.is_some() {
            self.title = title;
        }
        self.fonts.end_frame();
        let mut output = self.ctx.end_frame();
        shapes.append(&mut output.shapes);
        self.shapes = shapes;
        for (id, image) in &output.textures.set {
            self.textures.retain(|(t, _)| t != id);
            self.textures.push((*id, image.clone()));
        }
        self.textures
            .retain(|(t, _)| !output.textures.free.contains(t));
        self.output = output;
        &self.output
    }

    /// Clicks the center of the widget labelled `label` (as found in the
    /// last frame), then runs another frame so the app reacts to the
    /// click. Returns `false` if there is no such widget.
    pub fn click(&mut self, app: &mut impl App, label: &str) -> bool {
        self.click_with(app, label, PointerButton::Primary)
    }

    /// Like [`Harness::click`] with the secondary (right) button, e.g. to
    /// open a context menu.
    pub fn right_click(&mut self, app: &mut impl App, label: &str) -> bool {
        self.click_with(app, label, PointerButton::Secondary)
    }

    /// Like [`Harness::click`] with any mouse button.
    pub fn click_with(&mut self, app: &mut impl App, label: &str, button: PointerButton) -> bool {
        if self.frames == 0 {
            self.run(app);
        }
        let Some(pos) = self.find(label).map(|w| w.rect.center()) else {
            return false;
        };
        let which = button;
        let button = |pressed| Event::PointerButton {
            pos,
            button: which,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.run_with_events(
            app,
            vec![Event::PointerMoved(pos), button(true), button(false)],
        );
        self.run(app);
        true
    }

    /// Types `text` into the focused widget (e.g. after clicking a field).
    pub fn type_text(&mut self, app: &mut impl App, text: &str) {
        self.run_with_events(app, vec![Event::Text(text.to_owned())]);
    }

    /// Presses and releases `key` with `modifiers`.
    pub fn key(&mut self, app: &mut impl App, key: Key, modifiers: Modifiers) {
        let event = |pressed| Event::Key {
            key,
            pressed,
            repeat: false,
            modifiers,
        };
        self.run_with_events(app, vec![event(true), event(false)]);
    }

    /// The extra windows the last frame asked for (see
    /// [`Frame::show_window`]).
    pub fn requested_windows(&self) -> &[(Id, WindowOptions)] {
        &self.windows
    }

    /// Runs one frame of extra window `id` ([`App::update_window`]), with
    /// that window's own context, and returns its widgets.
    pub fn run_window(&mut self, app: &mut impl App, id: Id) -> Vec<WidgetDescription> {
        self.time += 1.0 / 60.0;
        let index = match self.window_contexts.iter().position(|(w, _)| *w == id) {
            Some(i) => i,
            None => {
                self.window_contexts.push((id, Context::new()));
                self.window_contexts.len() - 1
            }
        };
        let ctx = &mut self.window_contexts[index].1;
        ctx.begin_frame(RawInput {
            time: self.time,
            screen_rect: self.screen_rect,
            pixels_per_point: self.pixels_per_point,
            events: Vec::new(),
        });
        let mut frame = Frame {
            time: Duration::from_secs_f64(self.time),
            screen_rect: self.screen_rect,
            pixels_per_point: self.pixels_per_point,
            shapes: DisplayList::new(),
            clear_color: self.clear_color,
            request_repaint: false,
            fonts: &mut self.fonts,
            ctx: &mut *ctx,
            renderer: None,
            title: None,
            windows: Vec::new(),
            window_id: Some(id),
            close: false,
            titlebar_height: 0.0,
        };
        app.update_window(id, &mut frame);
        self.fonts.end_frame();
        ctx.end_frame();
        ctx.widgets().to_vec()
    }

    /// The widgets of the last frame.
    pub fn widgets(&self) -> &[WidgetDescription] {
        self.ctx.widgets()
    }

    /// The first widget of the last frame labelled `label`.
    pub fn find(&self, label: &str) -> Option<&WidgetDescription> {
        self.ctx.find_widget(label)
    }

    /// Everything drawn in the last frame: `Frame::shapes` followed by the
    /// widgets, back to front.
    pub fn shapes(&self) -> &DisplayList {
        &self.shapes
    }

    /// The output of the last frame (cursor, repaint requests, textures...).
    /// Its `shapes` are in [`Harness::shapes`].
    pub fn output(&self) -> &FrameOutput {
        &self.output
    }

    /// Whether the last frame of the main window called [`Frame::close`].
    pub fn close_requested(&self) -> bool {
        self.close_requested
    }

    /// The window title set by the app, if any.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Draws the last frame on the GPU, without a window. The first call
    /// creates the renderer (slow, ~0.1 s); later calls reuse it. Fails
    /// when the machine has no usable GPU adapter (e.g. some CI runners):
    /// skip the check then.
    ///
    /// ```no_run
    /// # use rustroke_winit::{Frame, testing::Harness};
    /// let mut app = |frame: &mut Frame| {
    ///     frame.ui(|ui| ui.label("Hello"));
    /// };
    /// let mut harness = Harness::new().with_pixels_per_point(2.0);
    /// harness.run(&mut app);
    /// if let Ok(image) = harness.render() {
    ///     image.save_png("hello.png").unwrap();
    /// }
    /// ```
    pub fn render(&mut self) -> Result<Screenshot, RendererError> {
        let ppp = self.pixels_per_point;
        if self.renderer.is_none() {
            let renderer = pollster::block_on(OffscreenRenderer::new(self.fonts.atlas()))?;
            self.renderer = Some(renderer);
        }
        let renderer = self.renderer.as_mut().expect("created above");
        let size = PhysicalSize::new(
            (self.screen_rect.width() * ppp).round() as u32,
            (self.screen_rect.height() * ppp).round() as u32,
        );
        let meshes = Tessellator::new(ppp, self.fonts.atlas()).tessellate(&self.shapes);
        let textures = TexturesDelta {
            set: self.textures.clone(),
            free: Vec::new(),
        };
        let job = PaintJob {
            meshes: &meshes,
            textures: &textures,
            pixels_per_point: ppp,
            clear_color: self.clear_color,
        };
        let pixels = renderer.render(size, &job, self.fonts.atlas_mut());
        Ok(Screenshot {
            size: [size.width, size.height],
            pixels,
        })
    }

    /// The fonts (and glyph atlas), e.g. to render [`Harness::shapes`]
    /// offscreen.
    pub fn fonts(&mut self) -> &mut Fonts {
        &mut self.fonts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::App;
    use rustroke_widgets::TextEdit;

    struct Form {
        name: String,
        saved: Vec<String>,
        agree: bool,
        dirty: bool,
        close_attempts: u32,
    }

    impl App for Form {
        fn update(&mut self, frame: &mut Frame<'_>) {
            let title = if self.dirty { "Form*" } else { "Form" };
            frame.set_title(title);
            frame.ui(|ui| {
                if ui
                    .add(TextEdit::singleline(&mut self.name).accessible_label("Name"))
                    .changed()
                {
                    self.dirty = true;
                }
                ui.add(
                    rustroke_widgets::Checkbox::new(&mut self.agree, "").accessible_label("Agree"),
                );
                if ui.button("Save").clicked() {
                    self.saved.push(self.name.clone());
                    self.dirty = false;
                }
            });
        }

        fn on_close_requested(&mut self) -> bool {
            self.close_attempts += 1;
            !self.dirty
        }
    }

    #[test]
    fn drive_an_app_by_widget_labels() {
        let mut app = Form {
            name: String::new(),
            saved: Vec::new(),
            agree: false,
            dirty: false,
            close_attempts: 0,
        };
        let mut h = Harness::new();
        assert!(h.click(&mut app, "Name"));
        h.type_text(&mut app, "Ada");
        assert_eq!(app.name, "Ada");
        h.run(&mut app);
        assert_eq!(h.title(), Some("Form*"));
        assert!(
            !app.on_close_requested(),
            "unsaved changes keep the window open"
        );

        assert!(h.click(&mut app, "Agree"), "found by accessible label");
        assert!(app.agree);
        assert!(h.click(&mut app, "Save"));
        assert_eq!(app.saved, ["Ada"]);
        assert_eq!(h.title(), Some("Form"));
        assert!(app.on_close_requested());
        assert!(!h.click(&mut app, "Missing"));
        assert!(h.widgets().len() >= 3);
    }

    /// WIN-06: after a "save changes?" dialog the app closes the window.
    #[test]
    fn frame_close_is_reported_for_the_frame_that_called_it() {
        let mut asking = true;
        let mut app = |frame: &mut Frame| {
            frame.ui(|ui| {
                if asking && ui.button("Discard").clicked() {
                    asking = false;
                }
            });
            if !asking {
                frame.close();
            }
        };
        let mut h = Harness::new();
        h.run(&mut app);
        assert!(!h.close_requested());
        assert!(h.click(&mut app, "Discard"));
        assert!(h.close_requested());
    }

    /// TST-04: textures loaded in earlier frames are still drawn.
    #[test]
    fn render_draws_the_last_frame_with_all_live_textures() {
        let mut texture = None;
        let mut app = |frame: &mut Frame| {
            frame.clear_color = Color::from_srgb8(0, 0, 255);
            let handle = texture.get_or_insert_with(|| {
                let red = ColorImage::from_fn([4, 4], |_, _| Color::from_srgb8(255, 0, 0));
                frame.ctx().load_texture(red)
            });
            let handle = handle.clone();
            frame.ui(|ui| {
                ui.add(rustroke_widgets::Image::new(&handle).size(vec2(40.0, 40.0)));
            });
        };
        let mut h = Harness::with_size(100.0, 80.0).with_pixels_per_point(2.0);
        h.run(&mut app);
        h.run(&mut app);
        let image = match h.render() {
            Ok(image) => image,
            Err(e) => {
                eprintln!("skipping render test: {e:?}");
                return;
            }
        };
        assert_eq!(image.size, [200, 160]);
        let image_rect = h.find("").map(|w| w.rect);
        let center = image_rect.map_or(rustroke_core::point(36.0, 36.0), |r| r.center());
        let [r, g, b, _] = image.pixel((center.x * 2.0) as u32, (center.y * 2.0) as u32);
        assert!(
            r > 200 && g < 30 && b < 30,
            "the image, uploaded two frames ago"
        );
        assert_eq!(image.pixel(198, 158), [0, 0, 255, 255], "the clear color");
        let path = std::env::temp_dir().join("rustroke_harness_render.png");
        image.save_png(&path).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 100);
    }

    struct TwoWindows {
        show_assembly: bool,
        clicks: u32,
    }

    impl App for TwoWindows {
        fn update(&mut self, frame: &mut Frame<'_>) {
            if self.show_assembly {
                frame.show_window(
                    Id::new("assembly"),
                    WindowOptions {
                        title: "Assembly".into(),
                        ..WindowOptions::default()
                    },
                );
            }
            frame.ui(|ui| ui.checkbox(&mut self.show_assembly, "Show assembly"));
        }

        fn update_window(&mut self, id: Id, frame: &mut Frame<'_>) {
            assert_eq!(frame.window_id(), Some(id));
            frame.ui(|ui| {
                if ui.button("Explode").clicked() {
                    self.clicks += 1;
                }
            });
        }

        fn on_window_close_requested(&mut self, _id: Id) -> bool {
            self.show_assembly = false;
            true
        }
    }

    /// LAY-05: an app keeps extra windows open by asking for them.
    #[test]
    fn extra_windows_are_requested_and_drawn() {
        let mut app = TwoWindows {
            show_assembly: false,
            clicks: 0,
        };
        let mut h = Harness::new();
        h.run(&mut app);
        assert!(h.requested_windows().is_empty());
        assert!(h.click(&mut app, "Show assembly"));
        let requested = h.requested_windows();
        assert_eq!(requested.len(), 1);
        assert_eq!(requested[0].1.title, "Assembly");
        let id = requested[0].0;
        let widgets = h.run_window(&mut app, id);
        assert!(widgets.iter().any(|w| w.info.label == "Explode"));
        assert!(h.find("Explode").is_none(), "not in the main window");

        assert!(app.on_window_close_requested(id));
        h.run(&mut app);
        assert!(h.requested_windows().is_empty(), "closed for good");
    }

    #[test]
    fn context_menus_and_combo_boxes_by_label() {
        let mut deleted = false;
        let mut axis = "X";
        let mut app = |frame: &mut Frame| {
            frame.ui(|ui| {
                ui.button("Shaft").context_menu(ui, |ui| {
                    if ui.button("Delete").clicked() {
                        deleted = true;
                    }
                });
                rustroke_widgets::ComboBox::from_label("Axis")
                    .selected_text(axis)
                    .show_ui(ui, |ui| {
                        for a in ["X", "Y", "Z"] {
                            ui.selectable_value(&mut axis, a, a);
                        }
                    });
            });
        };
        let mut h = Harness::new();
        assert!(h.right_click(&mut app, "Shaft"));
        h.run(&mut app);
        assert!(h.click(&mut app, "Delete"));
        assert!(h.click(&mut app, "Axis"));
        h.run(&mut app);
        assert!(h.click(&mut app, "Z"));
        drop(app);
        assert!(deleted);
        assert_eq!(axis, "Z");
    }
}
