use std::sync::Arc;

use crate::{Color, Galley, Point, Rect, Shape, Stroke};

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
