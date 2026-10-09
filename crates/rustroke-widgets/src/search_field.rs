//! A text field for filtering: magnifying glass, hint and a × to clear.

use rustroke_core::{Color, Rect, Stroke, Vec2, point, vec2};

use crate::widgets::{FrameOverride, frame_setters};
use crate::{Response, Sense, TextEdit, Ui, Widget, WidgetInfo, WidgetRole};

/// A one-line field for filtering lists and trees: a magnifying glass on
/// the left, a hint ("Search…") while empty, and a × on the right that
/// clears it. The response is the text field's: `changed()` is also true
/// when the × clears the text.
///
/// ```ignore
/// if ui.add(SearchField::new(&mut filter)).changed() {
///     // filter the tree
/// }
/// ```
#[derive(Debug)]
pub struct SearchField<'t> {
    text: &'t mut String,
    hint: String,
    width: Option<f32>,
    accessible_label: Option<String>,
    frame_style: FrameOverride,
}

impl<'t> SearchField<'t> {
    /// A search field editing `text`, with the hint "Search…".
    pub fn new(text: &'t mut String) -> Self {
        Self {
            text,
            hint: "Search…".to_owned(),
            width: None,
            accessible_label: None,
            frame_style: FrameOverride::default(),
        }
    }

    frame_setters!();

    /// Text shown while the field is empty (default "Search…").
    pub fn hint_text(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Width in points (default 200, limited by the available width).
    pub fn desired_width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// The name screen readers announce, instead of the hint.
    pub fn accessible_label(mut self, label: impl Into<String>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }
}

impl Widget for SearchField<'_> {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let height = self
            .frame_style
            .min_size
            .map_or(style.spacing.interact_height, |m| m.y);
        let glass = (height * 0.5).clamp(10.0, 16.0);
        let clear_side = (height - 8.0).max(12.0);
        let id = ui.next_auto_id();
        let label = self.accessible_label.unwrap_or_else(|| self.hint.clone());

        let mut edit = TextEdit::singleline(&mut *self.text)
            .id(id)
            .hint_text(self.hint)
            .accessible_label(label.clone())
            .desired_width(self.width.unwrap_or(200.0));
        edit.frame_style = self.frame_style;
        edit.inset = [glass + 4.0, clear_side];
        let mut response = ui.add(edit);
        let rect = response.rect;

        // Magnifying glass.
        let color = style.visuals.weak_text;
        let r = glass * 0.32;
        let c = point(rect.min.x + 8.0 + r + 1.0, rect.center().y - r * 0.3);
        ui.painter()
            .circle(c, r, Color::TRANSPARENT, Stroke::new(1.5, color));
        let d = r * std::f32::consts::FRAC_1_SQRT_2;
        ui.painter().line(
            c + vec2(d, d),
            c + vec2(d, d) * 2.1,
            Stroke::new(1.5, color),
        );

        // Clear button, only while there is text.
        if !self.text.is_empty() {
            let button = Rect::from_center_size(
                point(rect.max.x - 4.0 - clear_side / 2.0, rect.center().y),
                Vec2::splat(clear_side),
            );
            let sense = Sense {
                focusable: false,
                activate_with_keys: false,
                ..Sense::CLICK
            };
            let clear = ui.interact(id.with("clear"), button, sense);
            ui.describe(
                &clear,
                WidgetInfo::new(WidgetRole::Button, format!("Clear {label}")),
            );
            let w = ui.widget_visuals(&clear);
            if clear.hovered() || clear.is_pressed() {
                ui.painter()
                    .circle_filled(button.center(), clear_side / 2.0, w.bg_fill);
            }
            let k = clear_side * 0.18;
            let m = button.center();
            let stroke = Stroke::new(1.5, if clear.hovered() { w.fg } else { color });
            ui.painter().line(m + vec2(-k, -k), m + vec2(k, k), stroke);
            ui.painter().line(m + vec2(-k, k), m + vec2(k, -k), stroke);
            if clear.clicked() {
                self.text.clear();
                ui.ctx().request_focus(id);
                response.mark_changed();
            }
        }
        response
    }
}
