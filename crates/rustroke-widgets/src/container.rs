//! The box around a [`crate::Stack`] or [`crate::Grid`]: background
//! (color, gradient or image), border, corners, shadow, margins and size.

use rustroke_core::{
    Color, DisplayList, Gradient, GradientKind, Point, Rect, Shadow, Shape, Stroke, TextureId,
    Vec2, point, vec2,
};

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

/// How a background image covers its box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageFit {
    /// Stretched to the box, ignoring its proportions.
    #[default]
    Stretch,
    /// As big as possible while fully visible, centered (may leave
    /// empty bands).
    Contain,
    /// Covers the whole box, centered, cut where it overflows.
    Cover,
    /// At its own size (one point per pixel), centered, cut if bigger.
    Center,
    /// Repeated at its own size from the top-left corner.
    Tile,
}

/// What a box is filled with.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Background {
    /// Nothing: whatever is below shows through.
    #[default]
    None,
    /// A plain color.
    Color(Color),
    /// A gradient whose points are relative to the box: (0, 0) is its
    /// top-left corner and (1, 1) its bottom-right; a radial gradient's
    /// radius is relative to the shorter side. E.g. top to bottom:
    /// `Gradient::linear(point(0.0, 0.0), point(0.0, 1.0), top, bottom)`.
    Gradient(Gradient),
    /// An image (see [`Background::image`]).
    Image {
        /// The texture, from `Context::load_texture`.
        texture: TextureId,
        /// Its size in points (one point per pixel).
        size: Vec2,
        /// How it covers the box.
        fit: ImageFit,
        /// Multiplies its colors (white: unchanged; alpha for opacity).
        tint: Color,
    },
}

impl Background {
    /// An image background from a loaded texture.
    pub fn image(texture: &crate::TextureHandle, fit: ImageFit) -> Self {
        Self::Image {
            texture: texture.id(),
            size: texture.size_vec2(),
            fit,
            tint: Color::WHITE,
        }
    }

    /// The same image with its colors multiplied by `tint` (e.g.
    /// `Color::WHITE.with_alpha(0.5)` for half opacity). Other
    /// backgrounds are returned unchanged.
    pub fn with_tint(mut self, tint: Color) -> Self {
        if let Self::Image { tint: t, .. } = &mut self {
            *t = tint;
        }
        self
    }

    /// Paints the background over `rect`.
    fn paint(&self, painter: &mut DisplayList, rect: Rect, corner_radius: f32) {
        match self {
            Self::None => {}
            Self::Color(color) => {
                if color.a > 0.0 {
                    painter.rect_filled(rect, corner_radius, *color);
                }
            }
            Self::Gradient(gradient) => {
                let at = |p: Point| rect.min + vec2(p.x * rect.width(), p.y * rect.height());
                let kind = match gradient.kind {
                    GradientKind::Linear { from, to } => GradientKind::Linear {
                        from: at(from),
                        to: at(to),
                    },
                    GradientKind::Radial { center, radius } => GradientKind::Radial {
                        center: at(center),
                        radius: radius * rect.width().min(rect.height()),
                    },
                };
                let gradient = Gradient {
                    kind,
                    stops: gradient.stops.clone(),
                };
                painter.rect_gradient(rect, corner_radius, gradient, Stroke::NONE);
            }
            Self::Image {
                texture,
                size,
                fit,
                tint,
            } => paint_image(painter, rect, corner_radius, *texture, *size, *fit, *tint),
        }
    }
}

/// Draws the part `uv` of `texture` at `rect`.
fn image_part(
    painter: &mut DisplayList,
    rect: Rect,
    uv: Rect,
    radius: f32,
    texture: TextureId,
    tint: Color,
) {
    painter.add(Shape::Image {
        rect,
        texture,
        uv,
        tint,
        corner_radius: radius,
    });
}

fn paint_image(
    painter: &mut DisplayList,
    rect: Rect,
    radius: f32,
    texture: TextureId,
    size: Vec2,
    fit: ImageFit,
    tint: Color,
) {
    let full = Rect::from_min_max(point(0.0, 0.0), point(1.0, 1.0));
    if size.x <= 0.0 || size.y <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    // The image's rect in box coordinates, possibly bigger than the box;
    // what falls outside is cut by adjusting the texture coordinates.
    let placed = match fit {
        ImageFit::Stretch => rect,
        ImageFit::Contain | ImageFit::Cover => {
            let sx = rect.width() / size.x;
            let sy = rect.height() / size.y;
            let scale = if fit == ImageFit::Contain {
                sx.min(sy)
            } else {
                sx.max(sy)
            };
            Rect::from_center_size(rect.center(), size * scale)
        }
        ImageFit::Center => Rect::from_center_size(rect.center(), size),
        ImageFit::Tile => {
            let mut y = rect.min.y;
            while y < rect.max.y {
                let mut x = rect.min.x;
                while x < rect.max.x {
                    let tile = Rect::from_min_size(point(x, y), size);
                    let shown = tile.intersect(rect);
                    let uv = Rect::from_min_max(
                        point(0.0, 0.0),
                        point(shown.width() / size.x, shown.height() / size.y),
                    );
                    image_part(painter, shown, uv, 0.0, texture, tint);
                    x += size.x;
                }
                y += size.y;
            }
            return;
        }
    };
    let shown = placed.intersect(rect);
    if shown.is_empty() {
        return;
    }
    let uv = if shown == placed {
        full
    } else {
        let u = |x: f32| (x - placed.min.x) / placed.width();
        let v = |y: f32| (y - placed.min.y) / placed.height();
        Rect::from_min_max(
            point(u(shown.min.x), v(shown.min.y)),
            point(u(shown.max.x), v(shown.max.y)),
        )
    };
    // Round only images that fill the box (otherwise the corners of the
    // box and of the image don't match).
    let radius = if shown == rect { radius } else { 0.0 };
    image_part(painter, shown, uv, radius, texture, tint);
}

/// How big a box is along one axis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Sizing {
    /// As big as its content (the default).
    #[default]
    Content,
    /// All the available space (as the content inside a vertically
    /// scrolling area, the height is unlimited: then like `Content`).
    Fill,
    /// Exactly this many points (padding included).
    Points(f32),
}

/// The look and size of the box around a [`crate::Stack`] or
/// [`crate::Grid`] (also usable for a [`crate::Window`] with
/// `Window::frame`): background, border, corner radius, shadow, padding
/// inside the border, margin outside it, width and height with limits.
///
/// Usually set with the same-named methods of `Stack` and `Grid`; a
/// `BoxStyle` value lets several boxes share a look:
///
/// ```ignore
/// let card = BoxStyle::new().fill(card_color).corner_radius(8.0).padding(12.0);
/// Stack::column().style(card.clone()).show(ui, |ui| { /* ... */ });
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct BoxStyle {
    pub(crate) background: Background,
    pub(crate) stroke: Stroke,
    pub(crate) corner_radius: f32,
    pub(crate) shadow: Shadow,
    pub(crate) padding: Margin,
    pub(crate) margin: Margin,
    pub(crate) width: Sizing,
    pub(crate) height: Sizing,
    pub(crate) min_size: Vec2,
    pub(crate) max_size: Vec2,
    pub(crate) clip: Option<bool>,
}

impl Default for BoxStyle {
    fn default() -> Self {
        Self {
            background: Background::None,
            stroke: Stroke::NONE,
            corner_radius: 0.0,
            shadow: Shadow::NONE,
            padding: Margin::ZERO,
            margin: Margin::ZERO,
            width: Sizing::Content,
            height: Sizing::Content,
            min_size: Vec2::ZERO,
            max_size: Vec2::splat(f32::INFINITY),
            clip: None,
        }
    }
}

/// The builder methods of [`BoxStyle`], also on `Stack` and `Grid`
/// (whose `box_style` field they change).
macro_rules! box_setters {
    ($($field:ident)?) => {
        /// Background color.
        pub fn fill(mut self, color: rustroke_core::Color) -> Self {
            self $(.$field)? .background = $crate::Background::Color(color);
            self
        }

        /// Background gradient, with points relative to the box (see
        /// [`Background::Gradient`](crate::Background::Gradient)).
        pub fn gradient(mut self, gradient: rustroke_core::Gradient) -> Self {
            self $(.$field)? .background = $crate::Background::Gradient(gradient);
            self
        }

        /// Background image (see [`ImageFit`](crate::ImageFit) for how it
        /// covers the box).
        pub fn image(mut self, texture: &$crate::TextureHandle, fit: $crate::ImageFit) -> Self {
            self $(.$field)? .background = $crate::Background::image(texture, fit);
            self
        }

        /// Any [`Background`](crate::Background).
        pub fn background(mut self, background: $crate::Background) -> Self {
            self $(.$field)? .background = background;
            self
        }

        /// Border, centered on the edge of the box.
        pub fn stroke(mut self, stroke: impl Into<rustroke_core::Stroke>) -> Self {
            self $(.$field)? .stroke = stroke.into();
            self
        }

        /// Radius of the box's corners.
        pub fn corner_radius(mut self, radius: f32) -> Self {
            self $(.$field)? .corner_radius = radius;
            self
        }

        /// Shadow under the box.
        pub fn shadow(mut self, shadow: rustroke_core::Shadow) -> Self {
            self $(.$field)? .shadow = shadow;
            self
        }

        /// Space between the border and the content: one value, a `Vec2`
        /// (horizontal, vertical) or a [`Margin`](crate::Margin).
        pub fn padding(mut self, padding: impl Into<$crate::Margin>) -> Self {
            self $(.$field)? .padding = padding.into();
            self
        }

        /// Space around the box, outside the border.
        pub fn margin(mut self, margin: impl Into<$crate::Margin>) -> Self {
            self $(.$field)? .margin = margin.into();
            self
        }

        /// How wide the box is (default: as wide as its content).
        pub fn width(mut self, width: $crate::Sizing) -> Self {
            self $(.$field)? .width = width;
            self
        }

        /// How tall the box is (default: as tall as its content).
        pub fn height(mut self, height: $crate::Sizing) -> Self {
            self $(.$field)? .height = height;
            self
        }

        /// Takes all the available width (`width(Sizing::Fill)`).
        pub fn fill_width(self) -> Self {
            self.width($crate::Sizing::Fill)
        }

        /// Takes all the available height (`height(Sizing::Fill)`).
        pub fn fill_height(self) -> Self {
            self.height($crate::Sizing::Fill)
        }

        /// Exactly this big, in points (padding included).
        pub fn size(self, size: rustroke_core::Vec2) -> Self {
            self.width($crate::Sizing::Points(size.x))
                .height($crate::Sizing::Points(size.y))
        }

        /// At least this big (padding included).
        pub fn min_size(mut self, size: rustroke_core::Vec2) -> Self {
            self $(.$field)? .min_size = size;
            self
        }

        /// At most this big (padding included); `f32::INFINITY` leaves a
        /// side free. Content that doesn't fit is cut (see `clip`).
        pub fn max_size(mut self, size: rustroke_core::Vec2) -> Self {
            self $(.$field)? .max_size = size;
            self
        }

        /// At least this wide.
        pub fn min_width(mut self, width: f32) -> Self {
            self $(.$field)? .min_size.x = width;
            self
        }

        /// At most this wide.
        pub fn max_width(mut self, width: f32) -> Self {
            self $(.$field)? .max_size.x = width;
            self
        }

        /// At least this tall.
        pub fn min_height(mut self, height: f32) -> Self {
            self $(.$field)? .min_size.y = height;
            self
        }

        /// At most this tall.
        pub fn max_height(mut self, height: f32) -> Self {
            self $(.$field)? .max_size.y = height;
            self
        }

        /// Whether content overflowing the box is cut (default: only when
        /// a maximum size is set).
        pub fn clip(mut self, clip: bool) -> Self {
            self $(.$field)? .clip = Some(clip);
            self
        }
    };
}
pub(crate) use box_setters;

impl BoxStyle {
    /// No background, border, margins or size limits.
    pub fn new() -> Self {
        Self::default()
    }

    box_setters!();

    /// The look of a floating window in `style`: window fill, border,
    /// corner radius, shadow and padding.
    pub fn window(style: &Style) -> Self {
        let visuals = &style.visuals;
        Self::new()
            .fill(visuals.window_fill)
            .stroke(visuals.window_stroke)
            .corner_radius(visuals.window_corner_radius)
            .shadow(visuals.shadow)
            .padding(style.spacing.window_padding)
    }

    /// A light frame around related widgets: the widgets' border color,
    /// no fill.
    pub fn group(style: &Style) -> Self {
        Self::new()
            .stroke(style.visuals.inactive.stroke)
            .corner_radius(style.visuals.corner_radius)
            .padding(6.0)
    }

    /// The panel background of `style`, without border or margins.
    pub fn panel(style: &Style) -> Self {
        Self::new().fill(style.visuals.panel_fill)
    }

    /// The space between the border and the content.
    pub fn get_padding(&self) -> Margin {
        self.padding
    }

    /// Paints the shadow, background and border at `rect`.
    pub fn paint(&self, painter: &mut DisplayList, rect: Rect) {
        let radius = self.corner_radius;
        if self.shadow.color.a > 0.0 {
            painter.shadow(rect, radius, self.shadow);
        }
        self.background.paint(painter, rect, radius);
        if self.stroke.width > 0.0 && self.stroke.color.a > 0.0 {
            painter.rect_stroke(rect, radius, self.stroke);
        }
    }

    /// Lays out a box in `ui`: `run` adds the content in the area it is
    /// given and returns the content's size; then the box is sized,
    /// painted behind the content and allocated.
    pub(crate) fn show<R>(
        &self,
        ui: &mut Ui<'_>,
        run: impl FnOnce(&mut Ui<'_>, BoxArea) -> (R, Vec2),
    ) -> InnerResponse<R> {
        let available = ui.available_rect();
        let mut frame_rect = Rect::NOTHING;
        let mut result = ui.scope_with(available, Layout::top_down(Align::Min), |outer| {
            let (margin, pad) = (self.margin, self.padding);
            let frame_min = outer.max_rect().min + margin.left_top();
            let room = (outer.max_rect().max - margin.right_bottom()) - frame_min;
            let room = vec2(
                room.x.min(self.max_size.x).max(0.0),
                room.y.min(self.max_size.y).max(0.0),
            );
            let fixed = |sizing: Sizing, room: f32, min: f32, max: f32| match sizing {
                Sizing::Points(p) => Some(p.min(max).max(min)),
                Sizing::Fill if room.is_finite() => Some(room.max(min)),
                _ => None,
            };
            let fixed_w = fixed(self.width, room.x, self.min_size.x, self.max_size.x);
            let fixed_h = fixed(self.height, room.y, self.min_size.y, self.max_size.y);
            let content_min = frame_min + pad.left_top();
            let area = BoxArea {
                min: content_min,
                fixed: [
                    fixed_w.map(|w| (w - pad.left - pad.right).max(0.0)),
                    fixed_h.map(|h| (h - pad.top - pad.bottom).max(0.0)),
                ],
                room: vec2(
                    (fixed_w.unwrap_or(room.x) - pad.left - pad.right).max(0.0),
                    (fixed_h.unwrap_or(room.y) - pad.top - pad.bottom).max(0.0),
                ),
            };

            let start = outer.painter().len();
            let saved_clip = outer.clip_rect();
            let limited = self.max_size.x.is_finite() || self.max_size.y.is_finite();
            if self.clip.unwrap_or(limited) {
                let max = vec2(fixed_w.unwrap_or(room.x), fixed_h.unwrap_or(room.y));
                outer.set_clip_rect(Rect::from_min_size(frame_min, max));
            }
            let (inner, content) = run(outer, area);
            outer.clip_rect_restore(saved_clip);

            let natural = content + pad.sum();
            let size = vec2(
                fixed_w.unwrap_or_else(|| natural.x.min(self.max_size.x).max(self.min_size.x)),
                fixed_h.unwrap_or_else(|| natural.y.min(self.max_size.y).max(self.min_size.y)),
            );
            frame_rect = Rect::from_min_size(frame_min, size);
            outer
                .painter()
                .insert_at(start, |painter| self.paint(painter, frame_rect));
            outer.allocate_rect(size + margin.sum());
            inner
        });
        result.response.rect = frame_rect;
        result
    }
}

/// Where a box's content goes (see [`BoxStyle::show`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoxArea {
    /// Top-left corner of the content.
    pub min: Point,
    /// Content width and height when the box has a set size.
    pub fixed: [Option<f32>; 2],
    /// The most the content may take (may be unlimited).
    pub room: Vec2,
}

impl BoxArea {
    /// The area the content may use.
    pub fn rect(&self) -> Rect {
        Rect::from_min_size(self.min, self.room)
    }
}

#[allow(deprecated)]
impl From<Container> for BoxStyle {
    fn from(c: Container) -> Self {
        let mut style = Self::new()
            .fill(c.fill)
            .stroke(c.stroke)
            .corner_radius(c.corner_radius)
            .shadow(c.shadow)
            .padding(c.inner_margin)
            .margin(c.outer_margin)
            .min_size(c.min_size);
        if let Some(max) = c.max_size {
            style = style.max_size(max);
        }
        if c.fill_width {
            style = style.fill_width();
        }
        if c.fill_height {
            style = style.fill_height();
        }
        style
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
///
/// Deprecated: [`crate::Stack`] has all these options and more.
#[derive(Clone, Copy, Debug, PartialEq)]
#[deprecated(
    since = "0.20.0",
    note = "use `Stack` (same options: `fill`, `stroke`, `padding`, `margin`, `fill_width`, ...)"
)]
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

#[allow(deprecated)]
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

#[allow(deprecated)]
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
