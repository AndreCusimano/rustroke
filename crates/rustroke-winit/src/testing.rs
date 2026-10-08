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

use std::time::Duration;

use rustroke_core::{
    Color, DisplayList, Event, Key, Modifiers, Point, PointerButton, RawInput, Rect, vec2,
};
use rustroke_text::Fonts;
use rustroke_widgets::{Context, FrameOutput, WidgetDescription};

use crate::{App, Frame};

/// A window-less host for an [`App`]. See the [module docs](self).
#[derive(Debug)]
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
        };
        app.update(&mut frame);
        let Frame {
            mut shapes,
            clear_color,
            title,
            ..
        } = frame;
        self.clear_color = clear_color;
        if title.is_some() {
            self.title = title;
        }
        self.fonts.end_frame();
        let mut output = self.ctx.end_frame();
        shapes.append(&mut output.shapes);
        self.shapes = shapes;
        self.output = output;
        &self.output
    }

    /// Clicks the center of the widget labelled `label` (as found in the
    /// last frame), then runs another frame so the app reacts to the
    /// click. Returns `false` if there is no such widget.
    pub fn click(&mut self, app: &mut impl App, label: &str) -> bool {
        if self.frames == 0 {
            self.run(app);
        }
        let Some(pos) = self.find(label).map(|w| w.rect.center()) else {
            return false;
        };
        let button = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
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

    /// The window title set by the app, if any.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
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
}
