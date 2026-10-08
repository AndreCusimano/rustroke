//! Built-in widgets. Each is a small builder struct implementing
//! [`Widget`]; `Ui` has shorthand methods (`ui.button(..)`, ...) for them.

use std::ops::RangeInclusive;
use std::sync::Arc;

use rustroke_core::{Color, Galley, Key, Modifiers, Point, Rect, Stroke, Vec2, point, vec2};
use rustroke_text::TextStyle;

use crate::{CursorIcon, NumericInfo, Response, Sense, Ui, WidgetInfo, WidgetRole};

/// Anything that can be added to a [`Ui`].
pub trait Widget {
    /// Places, interacts with and draws the widget; returns its response.
    fn ui(self, ui: &mut Ui<'_>) -> Response;
}

/// Draws a focus ring around `rect` if `response` has visible focus.
fn paint_focus_ring(ui: &mut Ui<'_>, response: &Response, rect: Rect, corner_radius: f32) {
    if response.focus_visible() {
        let color = ui.style().visuals.focus;
        ui.painter().rect_stroke(
            rect.expand(3.0),
            corner_radius + 3.0,
            Stroke::new(2.0, color),
        );
    }
}

/// Top-left position that vertically centers `galley` in `rect`, starting at `x`.
fn text_pos(rect: Rect, x: f32, galley: &Galley) -> Point {
    point(x, rect.center().y - galley.size.y / 2.0)
}

/// Text that wraps to the available width.
#[derive(Clone, Debug)]
pub struct Label {
    text: String,
    style: Option<TextStyle>,
    color: Option<Color>,
    wrap: bool,
}

impl Label {
    /// A label showing `text`.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: None,
            color: None,
            wrap: true,
        }
    }

    /// Uses `style` instead of the body text style.
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Uses `color` instead of the theme's text color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Keep the text on one line (only explicit newlines break it).
    pub fn no_wrap(mut self) -> Self {
        self.wrap = false;
        self
    }
}

impl Widget for Label {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let text_style = self.style.unwrap_or_else(|| style.body.clone());
        // Rows grow sideways, so text there stays on one line.
        let wrap = (self.wrap && !ui.layout().is_horizontal()).then(|| ui.available_width());
        let galley = ui.layout_text(&self.text, &text_style, wrap);
        let response = ui.allocate_response(galley.size, Sense::HOVER);
        ui.describe(
            &response,
            WidgetInfo::new(WidgetRole::Label, self.text.clone()),
        );
        let color = self.color.unwrap_or(style.visuals.text);
        ui.painter().galley(response.rect.min, galley, color);
        response
    }
}

/// A clickable button with a text label.
#[derive(Clone, Debug)]
pub struct Button {
    text: String,
    frame: bool,
}

impl Button {
    /// A button showing `text`.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            frame: true,
        }
    }

    /// Without a frame the button only shows a background while hovered
    /// or pressed (toolbars, menu bars).
    pub fn frame(mut self, frame: bool) -> Self {
        self.frame = frame;
        self
    }

    /// Frameless when `in_bar` (menu bars), framed otherwise.
    pub(crate) fn menu_style(self, in_bar: bool) -> Self {
        self.frame(!in_bar)
    }
}

impl Widget for Button {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let padding = style.spacing.button_padding;
        let galley = ui.layout_text(&self.text, &style.body, None);
        let size = vec2(
            galley.size.x + 2.0 * padding.x,
            (galley.size.y + 2.0 * padding.y).max(style.spacing.interact_height),
        );
        let id = ui.next_auto_id();
        let mut rect = ui.allocate_rect(size);
        let menu_width = ui.in_menu();
        if let Some(width) = menu_width {
            // Menu items: left-aligned text, highlight across the menu.
            rect = Rect::from_min_size(rect.min, vec2(rect.width().max(width), rect.height()));
        }
        let response = ui.interact(id, rect, Sense::CLICK);
        ui.describe(
            &response,
            WidgetInfo::new(WidgetRole::Button, self.text.clone()),
        );
        if response.clicked() && menu_width.is_some() {
            ui.close_menu();
        }

        let visuals = ui.widget_visuals(&response);
        let radius = style.visuals.corner_radius;
        if self.frame && menu_width.is_none() {
            ui.painter()
                .rect(rect, radius, visuals.bg_fill, visuals.stroke);
        } else if response.hovered() || response.is_pressed() {
            ui.painter().rect_filled(rect, radius, visuals.bg_fill);
        }
        let pos = if menu_width.is_some() {
            point(
                rect.min.x + padding.x,
                rect.center().y - galley.size.y / 2.0,
            )
        } else {
            rect.center() - galley.size / 2.0
        };
        ui.painter().galley(pos, galley, visuals.fg);
        paint_focus_ring(ui, &response, rect, radius);
        response
    }
}

/// A box that toggles a `bool`, followed by a label. The whole row is
/// clickable.
#[derive(Debug)]
pub struct Checkbox<'a> {
    checked: &'a mut bool,
    text: String,
}

impl<'a> Checkbox<'a> {
    /// A checkbox toggling `checked`, followed by `text`.
    pub fn new(checked: &'a mut bool, text: impl Into<String>) -> Self {
        Self {
            checked,
            text: text.into(),
        }
    }
}

impl Widget for Checkbox<'_> {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let icon = style.spacing.icon_size;
        let galley = ui.layout_text(&self.text, &style.body, None);
        let size = vec2(
            icon + style.spacing.icon_spacing + galley.size.x,
            galley.size.y.max(style.spacing.interact_height),
        );
        let mut response = ui.allocate_response(size, Sense::CLICK);
        if response.clicked() {
            *self.checked = !*self.checked;
            response.mark_changed();
        }
        ui.describe(
            &response,
            WidgetInfo::new(WidgetRole::Checkbox, self.text.clone()).toggled(*self.checked),
        );

        let rect = response.rect;
        let visuals = ui.widget_visuals(&response);
        let radius = style.visuals.small_corner_radius;
        let box_rect = Rect::from_min_size(
            point(rect.min.x, rect.center().y - icon / 2.0),
            Vec2::splat(icon),
        );
        // Fade between the empty box and the filled, checked one.
        let checked = ui
            .ctx()
            .animate_bool(response.id.with("checked"), *self.checked);
        let accent = style.visuals.accent;
        let fill = visuals.bg_fill.lerp(accent, checked);
        let stroke = Stroke::new(
            visuals.stroke.width,
            visuals.stroke.color.lerp(accent, checked),
        );
        ui.painter().rect(box_rect, radius, fill, stroke);
        if checked > 0.0 {
            let at = |x: f32, y: f32| box_rect.min + vec2(x * icon, y * icon);
            let mark = style
                .visuals
                .on_accent
                .with_alpha(style.visuals.on_accent.a * checked);
            ui.painter().polyline(
                vec![at(0.24, 0.52), at(0.42, 0.70), at(0.76, 0.32)],
                Stroke::new(2.0, mark),
            );
        }
        let text_x = box_rect.max.x + style.spacing.icon_spacing;
        ui.painter()
            .galley(text_pos(rect, text_x, &galley), galley, visuals.fg);
        paint_focus_ring(ui, &response, box_rect, radius);
        response
    }
}

/// A round indicator with a label, for choosing one of several options.
/// Usually created with [`Ui::radio_value`].
#[derive(Clone, Debug)]
pub struct RadioButton {
    selected: bool,
    text: String,
}

impl RadioButton {
    /// A radio button, drawn as chosen if `selected`.
    pub fn new(selected: bool, text: impl Into<String>) -> Self {
        Self {
            selected,
            text: text.into(),
        }
    }
}

impl Widget for RadioButton {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let icon = style.spacing.icon_size;
        let galley = ui.layout_text(&self.text, &style.body, None);
        let size = vec2(
            icon + style.spacing.icon_spacing + galley.size.x,
            galley.size.y.max(style.spacing.interact_height),
        );
        let response = ui.allocate_response(size, Sense::CLICK);
        ui.describe(
            &response,
            WidgetInfo::new(WidgetRole::RadioButton, self.text.clone()).toggled(self.selected),
        );

        let rect = response.rect;
        let visuals = ui.widget_visuals(&response);
        let center = point(rect.min.x + icon / 2.0, rect.center().y);
        let selected = ui
            .ctx()
            .animate_bool(response.id.with("selected"), self.selected);
        let accent = style.visuals.accent;
        let stroke = Stroke::new(
            visuals.stroke.width,
            visuals.stroke.color.lerp(accent, selected),
        );
        ui.painter().circle(
            center,
            icon / 2.0,
            visuals.bg_fill.lerp(accent, selected),
            stroke,
        );
        if selected > 0.0 {
            // The dot grows in.
            ui.painter()
                .circle_filled(center, icon / 5.0 * selected, style.visuals.on_accent);
        }
        let text_x = rect.min.x + icon + style.spacing.icon_spacing;
        ui.painter()
            .galley(text_pos(rect, text_x, &galley), galley, visuals.fg);
        let indicator = Rect::from_center_size(center, Vec2::splat(icon));
        paint_focus_ring(ui, &response, indicator, icon / 2.0);
        response
    }
}

/// Numbers a [`Slider`] can edit.
pub trait Numeric: Copy + PartialOrd {
    /// Whole numbers only (values are rounded).
    const INTEGRAL: bool;
    /// The value as `f64`.
    fn to_f64(self) -> f64;
    /// A value from an `f64` (rounded/truncated for integers).
    fn from_f64(value: f64) -> Self;
}

macro_rules! impl_numeric {
    ($integral:expr => $($t:ty),*) => {$(
        impl Numeric for $t {
            const INTEGRAL: bool = $integral;
            fn to_f64(self) -> f64 {
                self as f64
            }
            fn from_f64(value: f64) -> Self {
                value as Self
            }
        }
    )*};
}
impl_numeric!(false => f32, f64);
impl_numeric!(true => i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

/// Drag a handle along a track to pick a number in a range. When focused,
/// arrow keys change the value by one step and Home/End jump to the ends.
#[derive(Debug)]
pub struct Slider<'a, T: Numeric> {
    value: &'a mut T,
    range: RangeInclusive<T>,
    text: Option<String>,
    step: Option<f64>,
    show_value: bool,
}

impl<'a, T: Numeric> Slider<'a, T> {
    /// A slider editing `value` within `range`.
    pub fn new(value: &'a mut T, range: RangeInclusive<T>) -> Self {
        Self {
            value,
            range,
            text: None,
            step: None,
            show_value: true,
        }
    }

    /// A label shown after the value.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Values snap to multiples of `step` (from the start of the range).
    pub fn step(mut self, step: f64) -> Self {
        self.step = Some(step);
        self
    }

    /// Whether to show the value next to the slider (default `true`).
    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    fn effective_step(&self) -> Option<f64> {
        match self.step {
            Some(step) if step > 0.0 => Some(step),
            _ if T::INTEGRAL => Some(1.0),
            _ => None,
        }
    }

    /// Clamps to the range and snaps to the step.
    fn sanitize(&self, value: f64) -> f64 {
        let (min, max) = (self.range.start().to_f64(), self.range.end().to_f64());
        let mut v = value.clamp(min.min(max), max.max(min));
        if let Some(step) = self.effective_step() {
            v = min + ((v - min) / step).round() * step;
            v = v.clamp(min.min(max), max.max(min));
        }
        v
    }

    /// Decimals shown for the value: enough to display one step.
    fn decimals(&self) -> usize {
        match self.effective_step() {
            Some(step) => (-step.log10().floor()).max(0.0) as usize,
            None => 2,
        }
    }
}

impl<T: Numeric> Widget for Slider<'_, T> {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let height = style.spacing.interact_height;
        let handle_radius = height / 3.0;
        let (min, max) = (self.range.start().to_f64(), self.range.end().to_f64());
        let old = self.value.to_f64();
        let mut value = old;

        // The text after the slider is laid out with the *new* value, but
        // the width must be known first: use the widest of the two ends.
        let (show_value, decimals, text) = (self.show_value, self.decimals(), self.text.clone());
        let label = move |v: f64| {
            let mut s = if show_value {
                format!("{v:.decimals$}")
            } else {
                String::new()
            };
            if let Some(text) = &text {
                if !s.is_empty() {
                    s.push_str("  ");
                }
                s.push_str(text);
            }
            s
        };
        let widest = [min, max]
            .map(|v| ui.layout_text(&label(v), &style.body, None).size.x)
            .into_iter()
            .fold(0.0, f32::max);
        let text_width = if widest > 0.0 {
            style.spacing.icon_spacing + widest
        } else {
            0.0
        };

        // Shrink the track (not the text) when space is short.
        let width = style
            .spacing
            .slider_width
            .min(ui.available_width() - text_width)
            .max(3.0 * height);

        let id = ui.next_auto_id();
        let rect = ui.allocate_rect(vec2(width + text_width, height));
        let slider_rect = Rect::from_min_size(rect.min, vec2(width, height));
        let mut response = ui.interact(id, slider_rect, Sense::DRAG);
        response.rect = rect;

        // The handle's center travels between these x positions.
        let track_min = slider_rect.min.x + handle_radius;
        let track_max = slider_rect.max.x - handle_radius;
        let x_of = |v: f64| {
            let t = if max == min {
                0.0
            } else {
                ((v - min) / (max - min)) as f32
            };
            track_min + t.clamp(0.0, 1.0) * (track_max - track_min)
        };

        if (response.is_pressed() || response.drag_started())
            && let Some(pos) = ui.input().pointer.pos()
        {
            let t = f64::from((pos.x - track_min) / (track_max - track_min));
            value = min + t.clamp(0.0, 1.0) * (max - min);
        }
        if response.has_focus() {
            let step = self.effective_step().unwrap_or((max - min) / 100.0);
            let input = ui.ctx().input_mut();
            for (key, delta) in [
                (Key::ArrowRight, step),
                (Key::ArrowUp, step),
                (Key::ArrowLeft, -step),
                (Key::ArrowDown, -step),
            ] {
                while input.consume_key(key, Modifiers::NONE) {
                    value += delta;
                }
            }
            if input.consume_key(Key::Home, Modifiers::NONE) {
                value = min;
            }
            if input.consume_key(Key::End, Modifiers::NONE) {
                value = max;
            }
        }
        let value = self.sanitize(value);
        if value != old {
            *self.value = T::from_f64(value);
            response.mark_changed();
        }
        // Show what was actually stored (e.g. rounded for integers).
        let value = self.value.to_f64();
        ui.describe(
            &response,
            WidgetInfo::new(WidgetRole::Slider, self.text.clone().unwrap_or_default()).numeric(
                NumericInfo {
                    value,
                    min,
                    max,
                    step: self.effective_step(),
                },
            ),
        );

        if response.dragged() {
            ui.ctx().set_cursor(CursorIcon::Grabbing);
        } else if response.hovered() {
            ui.ctx().set_cursor(CursorIcon::Grab);
        }

        // Track, filled part and handle.
        let cy = slider_rect.center().y;
        let track = Rect::from_min_max(point(track_min, cy - 2.0), point(track_max, cy + 2.0));
        ui.painter()
            .rect_filled(track, 2.0, style.visuals.inactive.bg_fill);
        let handle_x = x_of(value);
        let filled = Rect::from_min_max(track.min, point(handle_x, track.max.y));
        ui.painter().rect_filled(filled, 2.0, style.visuals.accent);
        let radius = if response.hovered() || response.is_pressed() {
            handle_radius + 1.0
        } else {
            handle_radius
        };
        let handle_center = point(handle_x, cy);
        ui.painter().circle(
            handle_center,
            radius,
            style.visuals.handle_fill,
            style.visuals.handle_stroke,
        );
        paint_focus_ring(
            ui,
            &response,
            Rect::from_center_size(handle_center, Vec2::splat(2.0 * radius)),
            radius,
        );

        let text = label(value);
        if !text.is_empty() {
            let galley: Arc<Galley> = ui.layout_text(&text, &style.body, None);
            let x = slider_rect.max.x + style.spacing.icon_spacing;
            ui.painter()
                .galley(text_pos(rect, x, &galley), galley, style.visuals.text);
        }
        response
    }
}

/// Shows an image loaded with `Context::load_texture`.
#[derive(Clone, Debug)]
pub struct Image {
    texture: rustroke_core::TextureId,
    size: Vec2,
    tint: Color,
    sense: Sense,
    alt_text: String,
}

impl Image {
    /// The image at one point per pixel.
    pub fn new(texture: &crate::TextureHandle) -> Self {
        Self {
            texture: texture.id(),
            size: texture.size_vec2(),
            tint: Color::WHITE,
            sense: Sense::HOVER,
            alt_text: String::new(),
        }
    }

    /// What the image shows, read by screen readers.
    pub fn alt_text(mut self, text: impl Into<String>) -> Self {
        self.alt_text = text.into();
        self
    }

    /// Exact size in points (may distort the image).
    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    /// Scales the image down (keeping its proportions) to at most `width`.
    pub fn max_width(mut self, width: f32) -> Self {
        if self.size.x > width {
            self.size = self.size * (width / self.size.x);
        }
        self
    }

    /// Scales the image down (keeping its proportions) to at most `height`.
    pub fn max_height(mut self, height: f32) -> Self {
        if self.size.y > height {
            self.size = self.size * (height / self.size.y);
        }
        self
    }

    /// Multiplies the image's colors (white = unchanged; alpha fades it).
    pub fn tint(mut self, tint: Color) -> Self {
        self.tint = tint;
        self
    }

    /// Makes the image clickable (e.g. a thumbnail).
    pub fn sense(mut self, sense: Sense) -> Self {
        self.sense = sense;
        self
    }
}

impl Widget for Image {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let response = ui.allocate_response(self.size, self.sense);
        ui.describe(&response, WidgetInfo::new(WidgetRole::Image, self.alt_text));
        ui.painter().image(response.rect, self.texture, self.tint);
        response
    }
}

/// A thin line: horizontal across a top-down Ui, vertical inside a row.
#[derive(Clone, Copy, Debug, Default)]
pub struct Separator;

impl Widget for Separator {
    fn ui(self, ui: &mut Ui<'_>) -> Response {
        let style = ui.style();
        let in_row = ui.layout().is_horizontal();
        // In a menu, span the menu (its width is measured from the items,
        // so the separator must not widen it).
        let width = ui.in_menu().unwrap_or_else(|| ui.available_width());
        let size = if in_row {
            vec2(1.0, style.spacing.interact_height)
        } else {
            vec2(width, 1.0)
        };
        let response = ui.allocate_response(size, Sense::HOVER);
        let r = response.rect;
        let (a, b) = if in_row {
            (point(r.center().x, r.min.y), point(r.center().x, r.max.y))
        } else {
            (point(r.min.x, r.center().y), point(r.max.x, r.center().y))
        };
        let color = style.visuals.inactive.stroke.color;
        ui.painter().line(a, b, Stroke::new(1.0, color));
        response
    }
}
