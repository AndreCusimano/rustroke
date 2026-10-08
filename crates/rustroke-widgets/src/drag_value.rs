//! A number edited by dragging, arrow keys or typing.

use std::ops::RangeInclusive;

use rustroke_core::{Key, Modifiers, Stroke, vec2};

use crate::{
    CursorIcon, FocusLost, Numeric, NumericInfo, Response, Sense, TextEdit, Ui, Widget, WidgetInfo,
    WidgetRole,
};

/// Pointer movement (points) before a press counts as a drag, not a click.
const DRAG_THRESHOLD: f32 = 3.0;

/// Drag in progress, kept between frames.
#[derive(Clone, Copy, Debug)]
struct DragState {
    start_value: f64,
    moved: bool,
}

/// A number shown as text: drag it sideways to change it, use the arrow
/// keys while it is focused, or click it (or press Enter) to type a value.
/// While typing, Enter or a click elsewhere applies the value and Escape
/// cancels.
///
/// ```ignore
/// ui.add(DragValue::new(&mut length).speed(0.1).suffix(" mm").range(0.0..=500.0));
/// ```
#[derive(Debug)]
pub struct DragValue<'a, T: Numeric> {
    value: &'a mut T,
    speed: f64,
    range: Option<RangeInclusive<f64>>,
    prefix: String,
    suffix: String,
    decimals: Option<usize>,
    accessible_label: Option<String>,
}

impl<'a, T: Numeric> DragValue<'a, T> {
    /// Edits `value`, changing by 1 per point dragged.
    pub fn new(value: &'a mut T) -> Self {
        Self {
            value,
            speed: 1.0,
            range: None,
            prefix: String::new(),
            suffix: String::new(),
            decimals: None,
            accessible_label: None,
        }
    }

    /// How much the value changes per point dragged, and per arrow key
    /// press.
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = speed.abs();
        self
    }

    /// Keeps the value within `range`.
    pub fn range(mut self, range: RangeInclusive<T>) -> Self {
        self.range = Some(range.start().to_f64()..=range.end().to_f64());
        self
    }

    /// Text before the number (e.g. "x: ").
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Text after the number (e.g. " mm").
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }

    /// Digits after the decimal point (default: 0 for integers, otherwise
    /// enough to show a change of `speed`).
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = Some(decimals);
        self
    }

    /// The name screen readers announce (and tests find the widget by).
    pub fn accessible_label(mut self, label: impl Into<String>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    fn effective_decimals(&self) -> usize {
        match self.decimals {
            Some(d) => d,
            None if T::INTEGRAL => 0,
            None if self.speed > 0.0 => (-self.speed.log10().floor()).clamp(0.0, 6.0) as usize,
            None => 2,
        }
    }

    /// Rounds to the shown decimals and clamps to the range.
    fn sanitize(&self, value: f64) -> f64 {
        let scale = 10f64.powi(self.effective_decimals() as i32);
        let mut v = (value * scale).round() / scale;
        if let Some(range) = &self.range {
            let (a, b) = (*range.start(), *range.end());
            v = v.clamp(a.min(b), a.max(b));
        }
        v
    }

    /// Parses typed text, ignoring the prefix and suffix.
    fn parse(&self, text: &str) -> Option<f64> {
        let mut t = text.trim();
        t = t.strip_prefix(self.prefix.trim()).unwrap_or(t).trim();
        t = t.strip_suffix(self.suffix.trim()).unwrap_or(t).trim();
        t.replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
    }
}

impl<T: Numeric> Widget for DragValue<'_, T> {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let id = ui.next_auto_id();
        let edit_id = id.with("edit");
        let decimals = self.effective_decimals();
        let old = self.value.to_f64();
        let label = self
            .accessible_label
            .clone()
            .unwrap_or_else(|| format!("{}{}", self.prefix, self.suffix).trim().to_owned());

        // Typing mode: a text field with the number.
        let editing: Option<String> = ui.ctx().data(edit_id);
        if let Some(mut text) = editing {
            let width = ui.ctx().data::<f32>(id).unwrap_or(80.0);
            let mut response = ui.add(
                TextEdit::singleline(&mut text)
                    .id(edit_id)
                    .select_all_on_focus(true)
                    .desired_width(width)
                    .accessible_label(label),
            );
            // Only applying the typed value counts as a change.
            response.changed = false;
            match response.lost_focus_reason() {
                Some(FocusLost::Cancel) => ui.ctx().remove_data(edit_id),
                Some(_) => {
                    ui.ctx().remove_data(edit_id);
                    if let Some(v) = self.parse(&text) {
                        let v = self.sanitize(v);
                        if v != old {
                            *self.value = T::from_f64(v);
                            response.mark_changed();
                        }
                    }
                }
                // Focus went away without the field noticing (e.g. it was
                // hidden for a frame): give up typing.
                None if !response.has_focus() => ui.ctx().remove_data(edit_id),
                None => ui.ctx().insert_data(edit_id, text),
            }
            if response.lost_focus() {
                ui.ctx().request_repaint();
            }
            return response;
        }

        let text = format!("{}{old:.decimals$}{}", self.prefix, self.suffix);
        let galley = ui.layout_text(&text, &style.body, None);
        let padding = style.spacing.button_padding;
        let size = vec2(
            (galley.size.x + 2.0 * padding.x).max(2.0 * style.spacing.interact_height),
            style.spacing.interact_height,
        );
        let rect = ui.allocate_rect(size);
        let mut response = ui.interact(id, rect, Sense::DRAG);

        let mut value = old;
        let drag_key = id.with("drag");
        if response.drag_started() {
            ui.ctx().insert_data(
                drag_key,
                DragState {
                    start_value: old,
                    moved: false,
                },
            );
        }
        if response.dragged()
            && let Some(mut drag) = ui.ctx().data::<DragState>(drag_key)
        {
            let delta = response.drag_delta().x;
            if delta.abs() >= DRAG_THRESHOLD {
                drag.moved = true;
            }
            if drag.moved {
                value = drag.start_value + f64::from(delta) * self.speed;
            }
            ui.ctx().insert_data(drag_key, drag);
        }
        let dragged_away = ui
            .ctx()
            .data::<DragState>(drag_key)
            .is_some_and(|d| d.moved);
        if response.drag_stopped() {
            ui.ctx().remove_data(drag_key);
        }
        if response.clicked() && !dragged_away {
            // Switch to typing: the field gets focus and selects the number.
            ui.ctx().insert_data(edit_id, format!("{old:.decimals$}"));
            ui.ctx().insert_data(id, rect.width());
            ui.ctx().request_focus(edit_id);
            // The field appears next frame; keep the focus until then.
            ui.ctx().keep_alive(edit_id);
            ui.ctx().request_repaint();
        }
        if response.has_focus() {
            let input = ui.ctx().input_mut();
            for (key, delta) in [
                (Key::ArrowRight, self.speed),
                (Key::ArrowUp, self.speed),
                (Key::ArrowLeft, -self.speed),
                (Key::ArrowDown, -self.speed),
            ] {
                while input.consume_key(key, Modifiers::NONE) {
                    value += delta;
                }
            }
        }
        let value = self.sanitize(value);
        if value != old {
            *self.value = T::from_f64(value);
            response.mark_changed();
        }

        let mut info = WidgetInfo::new(WidgetRole::DragValue, label)
            .value(format!("{}{value:.decimals$}{}", self.prefix, self.suffix));
        if let Some(range) = &self.range {
            info = info.numeric(NumericInfo {
                value,
                min: *range.start(),
                max: *range.end(),
                step: Some(self.speed),
            });
        }
        ui.describe(&response, info);
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor(CursorIcon::ResizeHorizontal);
        }

        let visuals = ui.widget_visuals(&response);
        let radius = style.visuals.corner_radius;
        ui.painter()
            .rect(rect, radius, visuals.bg_fill, visuals.stroke);
        let text = format!("{}{value:.decimals$}{}", self.prefix, self.suffix);
        let galley = ui.layout_text(&text, &style.body, None);
        ui.painter()
            .galley(rect.center() - galley.size / 2.0, galley, visuals.fg);
        if response.focus_visible() {
            ui.painter().rect_stroke(
                rect.expand(3.0),
                radius + 3.0,
                Stroke::new(2.0, style.visuals.focus),
            );
        }
        response
    }
}
