use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// A size in physical pixels (device pixels, not scaled by DPI).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PhysicalSize {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl PhysicalSize {
    /// A size of `width × height` pixels.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// True if either dimension is zero (e.g. a minimized window).
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// A 2D vector (a direction or an offset), in logical points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    /// Horizontal component.
    pub x: f32,
    /// Vertical component (positive is down).
    pub y: f32,
}

/// Shorthand for [`Vec2::new`].
pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

impl Vec2 {
    /// The zero vector.
    pub const ZERO: Self = Self::new(0.0, 0.0);

    /// A vector with the given components.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// A vector with both components equal to `v`.
    pub const fn splat(v: f32) -> Self {
        Self::new(v, v)
    }

    /// Euclidean length.
    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    /// Squared length (cheaper than [`Vec2::length`]).
    pub fn length_sq(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    /// Dot product.
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// Returns a unit vector in the same direction, or zero for a zero vector.
    pub fn normalized(self) -> Self {
        let len = self.length();
        if len > 0.0 { self / len } else { Self::ZERO }
    }

    /// Rotated 90° clockwise on screen (y points down): `(x, y) -> (-y, x)`.
    pub fn rot90(self) -> Self {
        Self::new(-self.y, self.x)
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Neg for Vec2 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl Div<f32> for Vec2 {
    type Output = Self;
    fn div(self, rhs: f32) -> Self {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

/// A position on screen, in logical points. Origin is top-left, y points down.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal position, from the left edge.
    pub x: f32,
    /// Vertical position, from the top edge.
    pub y: f32,
}

/// Shorthand for [`Point::new`].
pub const fn point(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

impl Point {
    /// The origin (top-left).
    pub const ZERO: Self = Self::new(0.0, 0.0);

    /// A point at the given coordinates.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// The vector from the origin to this point.
    pub const fn to_vec2(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    /// Distance to `other`.
    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }

    /// Component-wise minimum.
    pub fn min(self, other: Self) -> Self {
        Self::new(self.x.min(other.x), self.y.min(other.y))
    }

    /// Component-wise maximum.
    pub fn max(self, other: Self) -> Self {
        Self::new(self.x.max(other.x), self.y.max(other.y))
    }
}

impl Add<Vec2> for Point {
    type Output = Self;
    fn add(self, rhs: Vec2) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign<Vec2> for Point {
    fn add_assign(&mut self, rhs: Vec2) {
        *self = *self + rhs;
    }
}

impl Sub<Vec2> for Point {
    type Output = Self;
    fn sub(self, rhs: Vec2) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Sub for Point {
    type Output = Vec2;
    fn sub(self, rhs: Self) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

/// An axis-aligned rectangle in logical points, from `min` (top-left,
/// inclusive) to `max` (bottom-right, exclusive).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// Top-left corner.
    pub min: Point,
    /// Bottom-right corner.
    pub max: Point,
}

impl Rect {
    /// A rectangle that contains nothing; the identity for [`Rect::union`].
    pub const NOTHING: Self = Self {
        min: Point::new(f32::INFINITY, f32::INFINITY),
        max: Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
    };

    /// A rectangle that contains everything; the identity for [`Rect::intersect`].
    pub const EVERYTHING: Self = Self {
        min: Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
        max: Point::new(f32::INFINITY, f32::INFINITY),
    };

    /// A rectangle from its top-left and bottom-right corners.
    pub const fn from_min_max(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    /// A rectangle from its top-left corner and size.
    pub fn from_min_size(min: Point, size: Vec2) -> Self {
        Self::from_min_max(min, min + size)
    }

    /// A rectangle of `size` centered on `center`.
    pub fn from_center_size(center: Point, size: Vec2) -> Self {
        Self::from_min_max(center - size / 2.0, center + size / 2.0)
    }

    /// Horizontal extent.
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    /// Vertical extent.
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Width and height.
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    /// The middle point.
    pub fn center(&self) -> Point {
        self.min + self.size() / 2.0
    }

    /// True if the rectangle has no area.
    pub fn is_empty(&self) -> bool {
        // Written so that NaN coordinates also count as empty.
        !(self.min.x < self.max.x && self.min.y < self.max.y)
    }

    /// True if `p` is inside (the right and bottom edges are excluded).
    pub fn contains(&self, p: Point) -> bool {
        self.min.x <= p.x && p.x < self.max.x && self.min.y <= p.y && p.y < self.max.y
    }

    /// True if the two rectangles overlap.
    pub fn intersects(&self, other: Self) -> bool {
        !self.intersect(other).is_empty()
    }

    /// The overlapping area; may be empty.
    pub fn intersect(&self, other: Self) -> Self {
        Self::from_min_max(self.min.max(other.min), self.max.min(other.max))
    }

    /// The smallest rectangle containing both.
    pub fn union(&self, other: Self) -> Self {
        Self::from_min_max(self.min.min(other.min), self.max.max(other.max))
    }

    /// Grows the rectangle by `amount` on every side (shrinks if negative).
    pub fn expand(&self, amount: f32) -> Self {
        Self::from_min_max(
            self.min - Vec2::splat(amount),
            self.max + Vec2::splat(amount),
        )
    }

    /// Top-left corner.
    pub fn left_top(&self) -> Point {
        self.min
    }

    /// Top-right corner.
    pub fn right_top(&self) -> Point {
        Point::new(self.max.x, self.min.y)
    }

    /// Bottom-right corner.
    pub fn right_bottom(&self) -> Point {
        self.max
    }

    /// Bottom-left corner.
    pub fn left_bottom(&self) -> Point {
        Point::new(self.min.x, self.max.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_intersection_and_union() {
        let a = Rect::from_min_size(point(0.0, 0.0), vec2(10.0, 10.0));
        let b = Rect::from_min_size(point(5.0, 5.0), vec2(10.0, 10.0));
        assert_eq!(
            a.intersect(b),
            Rect::from_min_max(point(5.0, 5.0), point(10.0, 10.0))
        );
        assert_eq!(
            a.union(b),
            Rect::from_min_max(point(0.0, 0.0), point(15.0, 15.0))
        );

        let far = Rect::from_min_size(point(20.0, 20.0), vec2(1.0, 1.0));
        assert!(!a.intersects(far));
        assert_eq!(Rect::NOTHING.union(a), a);
        assert_eq!(Rect::EVERYTHING.intersect(a), a);
    }

    #[test]
    fn rect_contains_is_half_open() {
        let r = Rect::from_min_size(point(0.0, 0.0), vec2(10.0, 10.0));
        assert!(r.contains(point(0.0, 0.0)));
        assert!(r.contains(point(9.99, 9.99)));
        assert!(!r.contains(point(10.0, 5.0)));
    }

    #[test]
    fn empty_rects() {
        assert!(Rect::NOTHING.is_empty());
        assert!(Rect::from_min_size(point(1.0, 1.0), vec2(0.0, 5.0)).is_empty());
        assert!(Rect::from_min_max(point(f32::NAN, 0.0), point(1.0, 1.0)).is_empty());
    }

    #[test]
    fn vec_helpers() {
        assert_eq!(vec2(3.0, 4.0).length(), 5.0);
        assert_eq!(vec2(0.0, 0.0).normalized(), Vec2::ZERO);
        assert_eq!(vec2(1.0, 0.0).rot90(), vec2(0.0, 1.0));
    }
}
