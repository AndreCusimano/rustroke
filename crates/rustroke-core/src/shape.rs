use std::any::Any;
use std::sync::Arc;

use crate::{Color, Galley, Point, Rect, TextureId};

/// The outline of a shape: a width in logical points and a color.
///
/// Strokes are centered on the outline: half of the width lies outside
/// the shape and half inside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    /// Width in logical points.
    pub width: f32,
    /// Line color.
    pub color: Color,
}

impl Stroke {
    /// No outline.
    pub const NONE: Self = Self::new(0.0, Color::TRANSPARENT);

    /// A stroke of `width` points in `color`.
    pub const fn new(width: f32, color: Color) -> Self {
        Self { width, color }
    }

    /// True if drawing this stroke would have no visible effect.
    pub fn is_none(&self) -> bool {
        self.width <= 0.0 || self.color.a <= 0.0
    }
}

/// Custom drawing inside `rect`, done by the renderer instead of being
/// tessellated: e.g. a wgpu render pass of the application. The renderer
/// recognizes the callback type it supports (for the wgpu renderer:
/// `rustroke_render::CallbackFn`) and ignores others.
#[derive(Clone)]
pub struct PaintCallback {
    /// Where to draw, in logical points; the renderer sets its viewport
    /// to this area.
    pub rect: Rect,
    /// The renderer-specific drawing code.
    pub callback: Arc<dyn Any + Send + Sync>,
}

impl PaintCallback {
    /// A callback drawing into `rect`.
    pub fn new(rect: Rect, callback: impl Any + Send + Sync) -> Self {
        Self {
            rect,
            callback: Arc::new(callback),
        }
    }
}

impl std::fmt::Debug for PaintCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaintCallback")
            .field("rect", &self.rect)
            .finish_non_exhaustive()
    }
}

impl PartialEq for PaintCallback {
    fn eq(&self, other: &Self) -> bool {
        self.rect == other.rect && Arc::ptr_eq(&self.callback, &other.callback)
    }
}

/// A primitive that can be drawn. Coordinates are in logical points.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A rectangle with optionally rounded corners.
    Rect {
        /// Outer bounds (the stroke is centered on them).
        rect: Rect,
        /// Corner radius; clamped to half of the shortest side.
        corner_radius: f32,
        /// Interior color ([`Color::TRANSPARENT`] for none).
        fill: Color,
        /// Outline ([`Stroke::NONE`] for none).
        stroke: Stroke,
    },
    /// A circle.
    Circle {
        /// Center point.
        center: Point,
        /// Radius in points.
        radius: f32,
        /// Interior color ([`Color::TRANSPARENT`] for none).
        fill: Color,
        /// Outline ([`Stroke::NONE`] for none).
        stroke: Stroke,
    },
    /// A straight line with flat ends.
    LineSegment {
        /// Start and end.
        points: [Point; 2],
        /// Width and color of the line.
        stroke: Stroke,
    },
    /// Laid out text, drawn with its top-left corner at `pos`.
    ///
    /// `color` tints normal glyphs; color glyphs (emoji) only take its alpha.
    Text {
        /// Top-left corner of the text.
        pos: Point,
        /// The laid out, rasterized text.
        galley: Arc<Galley>,
        /// Text color.
        color: Color,
    },
    /// A textured rectangle (an image).
    Image {
        /// Where the image is drawn (it is stretched to fill it).
        rect: Rect,
        /// The texture to show.
        texture: TextureId,
        /// Part of the texture to show, in texture coordinates (0..1).
        uv: Rect,
        /// Multiplies the image colors; white shows it unchanged.
        tint: Color,
        /// Rounds the image's corners (anti-aliased); 0 for square ones.
        corner_radius: f32,
    },
    /// A sequence of connected points.
    ///
    /// `fill` is only drawn correctly for **convex** closed paths.
    Path {
        /// The points, in order.
        points: Vec<Point>,
        /// Whether the last point connects back to the first.
        closed: bool,
        /// Interior color ([`Color::TRANSPARENT`] for none).
        fill: Color,
        /// Outline ([`Stroke::NONE`] for none).
        stroke: Stroke,
    },
    /// Drawing done by the renderer itself (see [`PaintCallback`]).
    Callback(PaintCallback),
}

impl Shape {
    /// Multiplies the opacity of every color in the shape by `factor`
    /// (used to draw disabled widgets faded).
    pub fn multiply_alpha(&mut self, factor: f32) {
        let fade = |c: &mut Color| c.a *= factor;
        match self {
            Self::Rect { fill, stroke, .. }
            | Self::Circle { fill, stroke, .. }
            | Self::Path { fill, stroke, .. } => {
                fade(fill);
                fade(&mut stroke.color);
            }
            Self::LineSegment { stroke, .. } => fade(&mut stroke.color),
            Self::Text { color, .. } => fade(color),
            Self::Image { tint, .. } => fade(tint),
            // Custom drawing can't be faded from here.
            Self::Callback(_) => {}
        }
    }

    /// The area this shape may touch, including stroke but not anti-aliasing.
    pub fn bounding_rect(&self) -> Rect {
        match self {
            Self::Rect { rect, stroke, .. } => rect.expand(half_width(stroke)),
            Self::Circle {
                center,
                radius,
                stroke,
                ..
            } => Rect::from_min_max(*center, *center).expand(radius + half_width(stroke)),
            Self::LineSegment { points, stroke } => {
                Rect::from_min_max(points[0].min(points[1]), points[0].max(points[1]))
                    .expand(half_width(stroke))
            }
            Self::Image { rect, .. } | Self::Callback(PaintCallback { rect, .. }) => *rect,
            Self::Text { pos, galley, .. } => {
                let r = galley.bounding_rect();
                Rect::from_min_max(*pos + r.min.to_vec2(), *pos + r.max.to_vec2())
            }
            Self::Path { points, stroke, .. } => points
                .iter()
                .fold(Rect::NOTHING, |r, p| r.union(Rect::from_min_max(*p, *p)))
                .expand(half_width(stroke)),
        }
    }
}

fn half_width(stroke: &Stroke) -> f32 {
    if stroke.is_none() {
        0.0
    } else {
        stroke.width / 2.0
    }
}
