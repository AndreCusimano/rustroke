//! A group of widgets with a background, border, margins and size limits.

use rustroke_core::{Color, DisplayList, Rect, Shadow, Stroke, Vec2, vec2};

use crate::{Align, InnerResponse, Layout, Style, Ui};

/// Space on each side of a rectangle, in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Margin {
    /// Space on the left.
    pub left: f32,
    /// Space on the right.
    pub right: f32,
    /// Space at the top.
    pub top: f32,
    /// Space at the bottom.
    pub bottom: f32,
}

impl Margin {
    /// No space.
    pub const ZERO: Self = Self::same(0.0);

    /// The same space on every side.
    pub const fn same(margin: f32) -> Self {
        Self {
            left: margin,
            right: margin,
            top: margin,
            bottom: margin,
        }
    }

    /// `x` on the left and right, `y` at the top and bottom.
    pub const fn symmetric(x: f32, y: f32) -> Self {
        Self {
            left: x,
            right: x,
            top: y,
            bottom: y,
        }
    }

    /// Space on the left and at the top.
    pub fn left_top(&self) -> Vec2 {
        vec2(self.left, self.top)
    }

    /// Space on the right and at the bottom.
    pub fn right_bottom(&self) -> Vec2 {
        vec2(self.right, self.bottom)
    }

    /// Total horizontal and vertical space.
    pub fn sum(&self) -> Vec2 {
        self.left_top() + self.right_bottom()
    }
}

impl From<f32> for Margin {
    fn from(margin: f32) -> Self {
        Self::same(margin)
    }
}

impl From<Vec2> for Margin {
    fn from(margin: Vec2) -> Self {
        Self::symmetric(margin.x, margin.y)
    }
}

/// A group of widgets drawn over a background, with a border, rounded
/// corners, a shadow and margins inside and outside (a card, a toolbar,
/// a whole view). It can also limit the size of its content, or fill the
/// available width or height.
///
/// ```ignore
/// Container::new()
///     .fill(Color::from_srgb8(240, 244, 250))
///     .stroke(Stroke::new(1.0, Color::from_srgb8(200, 208, 220)))
///     .corner_radius(8.0)
///     .inner_margin(12.0)
///     .fill_width(true)
///     .show(ui, |ui| {
///         ui.heading("Card");
///         ui.label("Content drawn on the card's background.");
///     });
/// ```
///
/// The background is painted after the content is laid out (when its
/// size is known), behind it. The content uses the Ui's layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Container {
    fill: Color,
    stroke: Stroke,
    corner_radius: f32,
    shadow: Shadow,
    inner_margin: Margin,
    outer_margin: Margin,
    min_size: Vec2,
    max_size: Option<Vec2>,
    fill_width: bool,
    fill_height: bool,
}

impl Default for Container {
    fn default() -> Self {
        Self {
            fill: Color::TRANSPARENT,
            stroke: Stroke::NONE,
            corner_radius: 0.0,
            shadow: Shadow::NONE,
            inner_margin: Margin::ZERO,
            outer_margin: Margin::ZERO,
            min_size: Vec2::ZERO,
            max_size: None,
            fill_width: false,
            fill_height: false,
        }
    }
}

impl Container {
    /// No background, border or margins: only groups the content (and
    /// applies size limits).
    pub fn new() -> Self {
        Self::default()
    }

    /// The look of a floating window in `style`: window fill, border,
    /// corner radius, shadow and padding.
    pub fn window(style: &Style) -> Self {
        let visuals = &style.visuals;
        Self::new()
            .fill(visuals.window_fill)
            .stroke(visuals.window_stroke)
            .corner_radius(visuals.window_corner_radius)
            .shadow(visuals.shadow)
            .inner_margin(style.spacing.window_padding)
    }

    /// A light frame around related widgets: the widgets' border color,
    /// no fill.
    pub fn group(style: &Style) -> Self {
        Self::new()
            .stroke(style.visuals.inactive.stroke)
            .corner_radius(style.visuals.corner_radius)
            .inner_margin(6.0)
    }

    /// The panel background of `style`, without border or margins.
    pub fn panel(style: &Style) -> Self {
        Self::new().fill(style.visuals.panel_fill)
    }

    /// Background color (default: none).
    pub fn fill(mut self, fill: Color) -> Self {
        self.fill = fill;
        self
    }

    /// Border, centered on the edge of the background (default: none).
    pub fn stroke(mut self, stroke: impl Into<Stroke>) -> Self {
        self.stroke = stroke.into();
        self
    }

    /// Radius of the background's corners.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    /// Shadow under the background.
    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.shadow = shadow;
        self
    }

    /// Space between the edge of the background and the content: one
    /// value, a `Vec2` (horizontal, vertical) or a [`Margin`].
    pub fn inner_margin(mut self, margin: impl Into<Margin>) -> Self {
        self.inner_margin = margin.into();
        self
    }

    /// Space around the background, outside it.
    pub fn outer_margin(mut self, margin: impl Into<Margin>) -> Self {
        self.outer_margin = margin.into();
        self
    }

    /// The background is at least this big (inner margin included).
    pub fn min_size(mut self, size: Vec2) -> Self {
        self.min_size = size;
        self
    }

    /// The background is at most this big (inner margin included); the
    /// content gets less room and what doesn't fit is clipped. Use
    /// `f32::INFINITY` to limit only one side.
    pub fn max_size(mut self, size: Vec2) -> Self {
        self.max_size = Some(size);
        self
    }

    /// Takes all the available width (not inside a horizontally scrolling
    /// area, where the width is unlimited).
    pub fn fill_width(mut self, fill: bool) -> Self {
        self.fill_width = fill;
        self
    }

    /// Takes all the available height (not inside a vertically scrolling
    /// area, where the height is unlimited).
    pub fn fill_height(mut self, fill: bool) -> Self {
        self.fill_height = fill;
        self
    }

    /// The space between the background and the content.
    pub fn get_inner_margin(&self) -> Margin {
        self.inner_margin
    }

    /// Paints the shadow, background and border at `rect`.
    pub fn paint(&self, painter: &mut DisplayList, rect: Rect) {
        let radius = self.corner_radius;
        if self.shadow.color.a > 0.0 {
            painter.shadow(rect, radius, self.shadow);
        }
        if self.fill.a > 0.0 || (self.stroke.width > 0.0 && self.stroke.color.a > 0.0) {
            painter.rect(rect, radius, self.fill, self.stroke);
        }
    }

    /// Shows `add_contents` on the background. The response covers the
    /// background (outer margin excluded).
    pub fn show<R>(
        self,
        ui: &mut Ui<'_>,
        add_contents: impl FnOnce(&mut Ui<'_>) -> R,
    ) -> InnerResponse<R> {
        let layout = *ui.layout();
        let available = ui.available_rect();
        let mut frame_rect = Rect::NOTHING;
        let mut result = ui.scope_with(available, Layout::top_down(Align::Min), |outer| {
            let (om, im) = (self.outer_margin, self.inner_margin);
            let frame_min = outer.max_rect().min + om.left_top();
            let mut frame_max = outer.max_rect().max - om.right_bottom();
            if let Some(max) = self.max_size {
                frame_max.x = frame_max.x.min(frame_min.x + max.x);
                frame_max.y = frame_max.y.min(frame_min.y + max.y);
            }
            let frame_max = frame_max.max(frame_min);
            let content_min = frame_min + im.left_top();
            let content_max = (frame_max - im.right_bottom()).max(content_min);
            let content_rect = Rect::from_min_max(content_min, content_max);

            let start = outer.painter().len();
            let saved_clip = outer.clip_rect();
            if self.max_size.is_some() {
                outer.set_clip_rect(Rect::from_min_max(frame_min, frame_max));
            }
            let content = outer.scope_with_no_advance(content_rect, layout, add_contents);
            outer.clip_rect_restore(saved_clip);

            let used = content.response.rect;
            let used_max = used.max.max(content_min);
            let mut size = used_max - frame_min + im.right_bottom();
            let room = frame_max - frame_min;
            if self.fill_width && room.x.is_finite() {
                size.x = room.x;
            }
            if self.fill_height && room.y.is_finite() {
                size.y = room.y;
            }
            size = vec2(size.x.max(self.min_size.x), size.y.max(self.min_size.y));
            if let Some(max) = self.max_size {
                size = vec2(size.x.min(max.x), size.y.min(max.y));
            }
            frame_rect = Rect::from_min_size(frame_min, size);
            outer
                .painter()
                .insert_at(start, |painter| self.paint(painter, frame_rect));
            outer.allocate_rect(size + om.sum());
            content.inner
        });
        result.response.rect = frame_rect;
        result
    }
}
