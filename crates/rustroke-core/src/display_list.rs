use std::sync::Arc;

use crate::{
    Color, Galley, Gradient, Mesh, Point, Rect, Shadow, Shape, Stroke, Transform, Vertex,
    cubic_bezier_points, dashes, quadratic_bezier_points,
};

/// How far curves drawn with [`DisplayList::cubic_bezier`] and friends may
/// stray from the true curve, in points.
const CURVE_TOLERANCE: f32 = 0.05;

/// A shape together with the clip rectangle it is drawn within.
#[derive(Clone, Debug, PartialEq)]
pub struct ClippedShape {
    /// Area outside of which the shape is not drawn, in logical points.
    pub clip_rect: Rect,
    /// What to draw.
    pub shape: Shape,
}

/// An ordered list of shapes to draw in one frame, back to front.
///
/// Shapes are clipped to the current clip rectangle, which can be narrowed
/// temporarily with [`DisplayList::with_clip`].
#[derive(Clone, Debug)]
pub struct DisplayList {
    shapes: Vec<ClippedShape>,
    clip_rect: Rect,
}

impl Default for DisplayList {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayList {
    /// Creates an empty list with no clipping.
    pub fn new() -> Self {
        Self {
            shapes: Vec::new(),
            clip_rect: Rect::EVERYTHING,
        }
    }

    /// The shapes added so far, in drawing order.
    pub fn shapes(&self) -> &[ClippedShape] {
        &self.shapes
    }

    /// True if no shape was added.
    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty()
    }

    /// Number of shapes added so far.
    pub fn len(&self) -> usize {
        self.shapes.len()
    }

    /// Fades the shapes added since index `start` (see [`DisplayList::len`]).
    pub fn multiply_alpha_from(&mut self, start: usize, factor: f32) {
        for clipped in self.shapes.iter_mut().skip(start) {
            clipped.shape.multiply_alpha(factor);
        }
    }

    /// Removes all shapes and resets clipping, keeping the allocation.
    pub fn clear(&mut self) {
        self.shapes.clear();
        self.clip_rect = Rect::EVERYTHING;
    }

    /// The clip rectangle applied to shapes added now.
    pub fn clip_rect(&self) -> Rect {
        self.clip_rect
    }

    /// Sets the clip rectangle for the shapes added from now on.
    pub fn set_clip_rect(&mut self, clip_rect: Rect) {
        self.clip_rect = clip_rect;
    }

    /// Moves all shapes of `other` to the end of this list (drawn on top),
    /// keeping their clip rectangles. `other` is left empty.
    pub fn append(&mut self, other: &mut Self) {
        self.shapes.append(&mut other.shapes);
        other.clip_rect = Rect::EVERYTHING;
    }

    /// Runs `add_shapes` with the clip rectangle narrowed to `clip_rect`
    /// (intersected with the current one), then restores it.
    pub fn with_clip<R>(&mut self, clip_rect: Rect, add_shapes: impl FnOnce(&mut Self) -> R) -> R {
        let previous = self.clip_rect;
        self.clip_rect = previous.intersect(clip_rect);
        let result = add_shapes(self);
        self.clip_rect = previous;
        result
    }

    /// Adds a shape, unless it is entirely outside the current clip rectangle.
    pub fn add(&mut self, shape: Shape) {
        if self.clip_rect.is_empty() {
            return;
        }
        self.shapes.push(ClippedShape {
            clip_rect: self.clip_rect,
            shape,
        });
    }

    /// A rectangle with rounded corners, a fill and an outline.
    pub fn rect(&mut self, rect: Rect, corner_radius: f32, fill: Color, stroke: Stroke) {
        self.add(Shape::Rect {
            rect,
            corner_radius,
            fill,
            stroke,
        });
    }

    /// A filled rectangle without outline.
    pub fn rect_filled(&mut self, rect: Rect, corner_radius: f32, fill: Color) {
        self.rect(rect, corner_radius, fill, Stroke::NONE);
    }

    /// An outlined rectangle without fill.
    pub fn rect_stroke(&mut self, rect: Rect, corner_radius: f32, stroke: Stroke) {
        self.rect(rect, corner_radius, Color::TRANSPARENT, stroke);
    }

    /// A circle with a fill and an outline.
    pub fn circle(&mut self, center: Point, radius: f32, fill: Color, stroke: Stroke) {
        self.add(Shape::Circle {
            center,
            radius,
            fill,
            stroke,
        });
    }

    /// A filled circle without outline.
    pub fn circle_filled(&mut self, center: Point, radius: f32, fill: Color) {
        self.circle(center, radius, fill, Stroke::NONE);
    }

    /// A straight line from `a` to `b`.
    pub fn line(&mut self, a: Point, b: Point, stroke: Stroke) {
        self.add(Shape::LineSegment {
            points: [a, b],
            stroke,
        });
    }

    /// Draws a whole texture stretched over `rect`.
    pub fn image(&mut self, rect: Rect, texture: crate::TextureId, tint: Color) {
        let uv = Rect::from_min_max(Point::new(0.0, 0.0), Point::new(1.0, 1.0));
        self.add(Shape::Image {
            rect,
            texture,
            uv,
            tint,
            corner_radius: 0.0,
        });
    }

    /// Draws an image with rounded corners (e.g. a 3D viewport in a card).
    pub fn image_rounded(
        &mut self,
        rect: Rect,
        corner_radius: f32,
        texture: crate::TextureId,
        tint: Color,
    ) {
        let uv = Rect::from_min_max(Point::new(0.0, 0.0), Point::new(1.0, 1.0));
        self.add(Shape::Image {
            rect,
            texture,
            uv,
            tint,
            corner_radius,
        });
    }

    /// Draws laid out text with its top-left corner at `pos`.
    pub fn galley(&mut self, pos: Point, galley: Arc<Galley>, color: Color) {
        self.add(Shape::Text { pos, galley, color });
    }

    /// An open polyline through `points`.
    pub fn polyline(&mut self, points: Vec<Point>, stroke: Stroke) {
        self.add(Shape::Path {
            points,
            closed: false,
            fill: Color::TRANSPARENT,
            stroke,
        });
    }

    /// A closed polygon. `fill` is only correct for convex polygons.
    pub fn polygon(&mut self, points: Vec<Point>, fill: Color, stroke: Stroke) {
        self.add(Shape::Path {
            points,
            closed: true,
            fill,
            stroke,
        });
    }

    /// A rectangle filled with a gradient, with an outline.
    pub fn rect_gradient(
        &mut self,
        rect: Rect,
        corner_radius: f32,
        gradient: Gradient,
        stroke: Stroke,
    ) {
        self.gradient_fill(
            Shape::Rect {
                rect,
                corner_radius,
                fill: Color::WHITE,
                stroke,
            },
            gradient,
        );
    }

    /// A circle filled with a gradient, with an outline.
    pub fn circle_gradient(
        &mut self,
        center: Point,
        radius: f32,
        gradient: Gradient,
        stroke: Stroke,
    ) {
        self.gradient_fill(
            Shape::Circle {
                center,
                radius,
                fill: Color::WHITE,
                stroke,
            },
            gradient,
        );
    }

    /// A convex polygon filled with a gradient, with an outline.
    pub fn polygon_gradient(&mut self, points: Vec<Point>, gradient: Gradient, stroke: Stroke) {
        self.gradient_fill(
            Shape::Path {
                points,
                closed: true,
                fill: Color::WHITE,
                stroke,
            },
            gradient,
        );
    }

    /// `shape` (rectangle, circle or convex path) with its fill replaced by
    /// `gradient` (the fill's alpha still applies).
    pub fn gradient_fill(&mut self, shape: Shape, gradient: Gradient) {
        self.add(Shape::Gradient {
            shape: Box::new(shape),
            gradient,
        });
    }

    /// A soft shadow under the rounded rectangle `rect` (draw it before
    /// the rectangle).
    pub fn shadow(&mut self, rect: Rect, corner_radius: f32, shadow: Shadow) {
        self.add(Shape::Shadow {
            rect,
            corner_radius,
            shadow,
        });
    }

    /// A dashed polyline: dashes `dash` points long with `gap` points
    /// between them.
    pub fn dashed_line(&mut self, points: &[Point], stroke: Stroke, dash: f32, gap: f32) {
        for d in dashes(points, dash, gap) {
            self.polyline(d, stroke);
        }
    }

    /// A dotted polyline: round dots of `radius` every `spacing` points.
    pub fn dotted_line(&mut self, points: &[Point], radius: f32, spacing: f32, color: Color) {
        let spacing = spacing.max(radius * 2.0).max(0.1);
        let mut left = 0.0;
        for w in points.windows(2) {
            let len = (w[1] - w[0]).length();
            let mut t = left;
            while t <= len {
                let p = w[0] + (w[1] - w[0]) * (t / len.max(f32::EPSILON));
                self.circle_filled(p, radius, color);
                t += spacing;
            }
            left = t - len;
        }
    }

    /// The quadratic Bézier curve from `p[0]` to `p[2]`, bent towards `p[1]`.
    pub fn quadratic_bezier(&mut self, p: [Point; 3], stroke: Stroke) {
        self.polyline(quadratic_bezier_points(p, CURVE_TOLERANCE), stroke);
    }

    /// The cubic Bézier curve from `p[0]` to `p[3]` with control points
    /// `p[1]` and `p[2]`.
    pub fn cubic_bezier(&mut self, p: [Point; 4], stroke: Stroke) {
        self.polyline(cubic_bezier_points(p, CURVE_TOLERANCE), stroke);
    }

    /// Triangles built by the app (see [`Mesh`]).
    pub fn mesh(&mut self, mesh: Arc<Mesh>) {
        self.add(Shape::Mesh(mesh));
    }

    /// Laid out text drawn through `transform`: rotated, scaled or sheared
    /// (e.g. on the faces of a view cube, with
    /// [`Transform::from_axes`]). Galley coordinates start at its
    /// top-left corner.
    pub fn galley_transformed(&mut self, galley: Arc<Galley>, transform: Transform, color: Color) {
        self.add(Shape::TransformedText {
            galley,
            transform,
            color,
        });
    }

    /// Runs `add_shapes`, then moves, rotates, scales or shears what it
    /// added with `transform`. Rectangles and circles become polygons,
    /// images and text are drawn through the transform; clip rectangles
    /// stay as they were, and paint callbacks are not transformed.
    pub fn with_transform<R>(
        &mut self,
        transform: Transform,
        add_shapes: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let start = self.shapes.len();
        let result = add_shapes(self);
        for clipped in &mut self.shapes[start..] {
            let shape = std::mem::replace(
                &mut clipped.shape,
                Shape::Path {
                    points: Vec::new(),
                    closed: false,
                    fill: Color::TRANSPARENT,
                    stroke: Stroke::NONE,
                },
            );
            clipped.shape = transform_shape(shape, &transform);
        }
        result
    }
}

/// Points around a rounded rectangle, clockwise from the top-left corner.
fn rounded_rect_points(rect: Rect, radius: f32) -> Vec<Point> {
    let r = radius
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0)
        .max(0.0);
    if r <= 0.0 {
        return vec![
            rect.min,
            Point::new(rect.max.x, rect.min.y),
            rect.max,
            Point::new(rect.min.x, rect.max.y),
        ];
    }
    let mut points = Vec::new();
    let corners = [
        (
            Point::new(rect.min.x + r, rect.min.y + r),
            std::f32::consts::PI,
        ),
        (
            Point::new(rect.max.x - r, rect.min.y + r),
            std::f32::consts::PI * 1.5,
        ),
        (Point::new(rect.max.x - r, rect.max.y - r), 0.0),
        (
            Point::new(rect.min.x + r, rect.max.y - r),
            std::f32::consts::FRAC_PI_2,
        ),
    ];
    let steps = arc_steps(r) / 4;
    for (c, start) in corners {
        for k in 0..=steps {
            let a = start + std::f32::consts::FRAC_PI_2 * k as f32 / steps as f32;
            points.push(Point::new(c.x + r * a.cos(), c.y + r * a.sin()));
        }
    }
    points
}

/// Segments for a full circle of `radius` points, fine enough up to 2×
/// zoom.
fn arc_steps(radius: f32) -> usize {
    ((radius * 2.0).sqrt() * 8.0).ceil().clamp(16.0, 512.0) as usize / 4 * 4
}

/// `shape` moved by `t` (see [`DisplayList::with_transform`]).
fn transform_shape(shape: Shape, t: &Transform) -> Shape {
    let scale_stroke = |s: Stroke| Stroke::new(s.width * t.scale_factor(), s.color);
    let map = |points: Vec<Point>| points.into_iter().map(|p| t.apply(p)).collect::<Vec<_>>();
    match shape {
        Shape::Rect {
            rect,
            corner_radius,
            fill,
            stroke,
        } => Shape::Path {
            points: map(rounded_rect_points(rect, corner_radius)),
            closed: true,
            fill,
            stroke: scale_stroke(stroke),
        },
        Shape::Circle {
            center,
            radius,
            fill,
            stroke,
        } => {
            let n = arc_steps(radius);
            let points = (0..n)
                .map(|k| {
                    let a = std::f32::consts::TAU * k as f32 / n as f32;
                    Point::new(center.x + radius * a.cos(), center.y + radius * a.sin())
                })
                .collect();
            Shape::Path {
                points: map(points),
                closed: true,
                fill,
                stroke: scale_stroke(stroke),
            }
        }
        Shape::LineSegment { points, stroke } => Shape::LineSegment {
            points: points.map(|p| t.apply(p)),
            stroke: scale_stroke(stroke),
        },
        Shape::Path {
            points,
            closed,
            fill,
            stroke,
        } => Shape::Path {
            points: map(points),
            closed,
            fill,
            stroke: scale_stroke(stroke),
        },
        Shape::Text { pos, galley, color } => Shape::TransformedText {
            galley,
            transform: Transform::translate(pos.to_vec2()).then(*t),
            color,
        },
        Shape::TransformedText {
            galley,
            transform,
            color,
        } => Shape::TransformedText {
            galley,
            transform: transform.then(*t),
            color,
        },
        Shape::Image {
            rect,
            texture,
            uv,
            tint,
            ..
        } => {
            let c = tint.with_alpha(tint.a);
            let color = [c.r * c.a, c.g * c.a, c.b * c.a, c.a];
            let corners = [
                (rect.min, [uv.min.x, uv.min.y]),
                (Point::new(rect.max.x, rect.min.y), [uv.max.x, uv.min.y]),
                (rect.max, [uv.max.x, uv.max.y]),
                (Point::new(rect.min.x, rect.max.y), [uv.min.x, uv.max.y]),
            ];
            Shape::Mesh(Arc::new(Mesh {
                texture,
                vertices: corners
                    .into_iter()
                    .map(|(p, uv)| Vertex {
                        pos: t.apply(p),
                        uv,
                        color,
                    })
                    .collect(),
                indices: vec![0, 1, 2, 0, 2, 3],
            }))
        }
        Shape::Mesh(mut mesh) => {
            for v in &mut Arc::make_mut(&mut mesh).vertices {
                v.pos = t.apply(v.pos);
            }
            Shape::Mesh(mesh)
        }
        Shape::Gradient { shape, gradient } => Shape::Gradient {
            shape: Box::new(transform_shape(*shape, t)),
            gradient: gradient.transformed(t),
        },
        Shape::Shadow {
            rect,
            corner_radius,
            shadow,
        } => {
            // Shadows stay axis-aligned: the bounds of the moved rectangle.
            let corners = [
                rect.min,
                Point::new(rect.max.x, rect.min.y),
                rect.max,
                Point::new(rect.min.x, rect.max.y),
            ];
            let bounds = corners
                .into_iter()
                .map(|p| t.apply(p))
                .fold(Rect::NOTHING, |r, p| r.union(Rect::from_min_max(p, p)));
            Shape::Shadow {
                rect: bounds,
                corner_radius: corner_radius * t.scale_factor(),
                shadow: Shadow {
                    blur: shadow.blur * t.scale_factor(),
                    spread: shadow.spread * t.scale_factor(),
                    offset: t.apply_vec(shadow.offset),
                    ..shadow
                },
            }
        }
        callback @ Shape::Callback(_) => callback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point, vec2};

    #[test]
    fn clip_is_nested_and_restored() {
        let mut list = DisplayList::new();
        let outer = Rect::from_min_size(point(0.0, 0.0), vec2(100.0, 100.0));
        let inner = Rect::from_min_size(point(50.0, 50.0), vec2(100.0, 100.0));
        list.with_clip(outer, |list| {
            list.with_clip(inner, |list| {
                list.circle_filled(point(60.0, 60.0), 5.0, Color::WHITE);
            });
            list.circle_filled(point(10.0, 10.0), 5.0, Color::WHITE);
        });
        list.circle_filled(point(10.0, 10.0), 5.0, Color::WHITE);

        let clips: Vec<Rect> = list.shapes().iter().map(|s| s.clip_rect).collect();
        assert_eq!(
            clips,
            [
                Rect::from_min_max(point(50.0, 50.0), point(100.0, 100.0)),
                outer,
                Rect::EVERYTHING
            ]
        );
    }

    #[test]
    fn fading_only_affects_later_shapes() {
        let mut list = DisplayList::new();
        list.circle_filled(point(0.0, 0.0), 1.0, Color::WHITE);
        let start = list.len();
        list.circle_filled(point(0.0, 0.0), 1.0, Color::WHITE);
        list.multiply_alpha_from(start, 0.5);
        let alphas: Vec<f32> = list
            .shapes()
            .iter()
            .map(|s| match &s.shape {
                Shape::Circle { fill, .. } => fill.a,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(alphas, [1.0, 0.5]);
    }

    #[test]
    fn shapes_in_empty_clip_are_dropped() {
        let mut list = DisplayList::new();
        let a = Rect::from_min_size(point(0.0, 0.0), vec2(10.0, 10.0));
        let b = Rect::from_min_size(point(20.0, 20.0), vec2(10.0, 10.0));
        list.with_clip(a, |list| {
            list.with_clip(b, |list| list.rect_filled(a, 0.0, Color::WHITE));
        });
        assert!(list.is_empty());
    }
}
