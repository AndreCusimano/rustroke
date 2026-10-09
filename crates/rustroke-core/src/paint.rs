//! Paints beyond solid colors (gradients, shadows), curves, dashes and 2D
//! transforms.

use crate::{Color, Point, Vec2};

/// A 2D affine transform: `p' = [a c; b d] · p + (tx, ty)`. Combine
/// transforms with [`Transform::then`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// First column of the linear part (where the x axis goes).
    pub a: f32,
    /// See `a`.
    pub b: f32,
    /// Second column of the linear part (where the y axis goes).
    pub c: f32,
    /// See `c`.
    pub d: f32,
    /// Translation along x.
    pub tx: f32,
    /// Translation along y.
    pub ty: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    /// Leaves everything in place.
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    /// Moves by `offset`.
    pub fn translate(offset: Vec2) -> Self {
        Self {
            tx: offset.x,
            ty: offset.y,
            ..Self::IDENTITY
        }
    }

    /// Scales by `sx`, `sy` around `origin`.
    pub fn scale_around(origin: Point, sx: f32, sy: f32) -> Self {
        Self::translate(-origin.to_vec2())
            .then(Self {
                a: sx,
                d: sy,
                ..Self::IDENTITY
            })
            .then(Self::translate(origin.to_vec2()))
    }

    /// Rotates by `angle` radians (clockwise on screen, where y points
    /// down) around `origin`.
    pub fn rotate_around(origin: Point, angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self::translate(-origin.to_vec2())
            .then(Self {
                a: c,
                b: s,
                c: -s,
                d: c,
                ..Self::IDENTITY
            })
            .then(Self::translate(origin.to_vec2()))
    }

    /// A transform from its linear part `[[a, b], [c, d]]` (rows: where
    /// x and y map) and the image of the origin (e.g. to draw text on a
    /// parallelogram: `x_axis` and `y_axis` are where a unit step along
    /// x and y lands).
    pub fn from_axes(origin: Point, x_axis: Vec2, y_axis: Vec2) -> Self {
        Self {
            a: x_axis.x,
            b: x_axis.y,
            c: y_axis.x,
            d: y_axis.y,
            tx: origin.x,
            ty: origin.y,
        }
    }

    /// This transform followed by `next`.
    pub fn then(self, next: Self) -> Self {
        Self {
            a: next.a * self.a + next.c * self.b,
            b: next.b * self.a + next.d * self.b,
            c: next.a * self.c + next.c * self.d,
            d: next.b * self.c + next.d * self.d,
            tx: next.a * self.tx + next.c * self.ty + next.tx,
            ty: next.b * self.tx + next.d * self.ty + next.ty,
        }
    }

    /// Where `p` goes.
    pub fn apply(&self, p: Point) -> Point {
        Point::new(
            self.a * p.x + self.c * p.y + self.tx,
            self.b * p.x + self.d * p.y + self.ty,
        )
    }

    /// Where the vector `v` goes (without the translation).
    pub fn apply_vec(&self, v: Vec2) -> Vec2 {
        Vec2::new(self.a * v.x + self.c * v.y, self.b * v.x + self.d * v.y)
    }

    /// How much lengths grow on average (to scale stroke widths).
    pub fn scale_factor(&self) -> f32 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }

    /// Whether it only moves things (no rotation, scale or shear).
    pub fn is_translation(&self) -> bool {
        self.a == 1.0 && self.b == 0.0 && self.c == 0.0 && self.d == 1.0
    }
}

/// Where the colors of a [`Gradient`] change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientKind {
    /// Along the line from `from` (position 0) to `to` (position 1).
    Linear {
        /// Where position 0 is.
        from: Point,
        /// Where position 1 is.
        to: Point,
    },
    /// Around `center`, from the center (0) to `radius` (1).
    Radial {
        /// Where position 0 is.
        center: Point,
        /// Distance of position 1.
        radius: f32,
    },
}

/// Colors changing smoothly across a shape: linear (along a line) or
/// radial (around a point), through any number of color stops.
///
/// ```
/// use rustroke_core::{Color, Gradient, point};
/// let sky = Gradient::linear(point(0.0, 0.0), point(0.0, 100.0), Color::WHITE, Color::BLACK)
///     .with_stop(0.5, Color::from_srgb8(120, 160, 255));
/// assert_eq!(sky.stops.len(), 3);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    /// Linear or radial, and where.
    pub kind: GradientKind,
    /// Positions (0..1, sorted) and their colors. Before the first and
    /// after the last stop the end colors continue.
    pub stops: Vec<(f32, Color)>,
}

impl Gradient {
    /// From color `a` at `from` to color `b` at `to`.
    pub fn linear(from: Point, to: Point, a: Color, b: Color) -> Self {
        Self {
            kind: GradientKind::Linear { from, to },
            stops: vec![(0.0, a), (1.0, b)],
        }
    }

    /// From color `inner` at `center` to `outer` at distance `radius`.
    pub fn radial(center: Point, radius: f32, inner: Color, outer: Color) -> Self {
        Self {
            kind: GradientKind::Radial { center, radius },
            stops: vec![(0.0, inner), (1.0, outer)],
        }
    }

    /// Adds a color stop at `position` (0..1).
    pub fn with_stop(mut self, position: f32, color: Color) -> Self {
        self.stops.push((position, color));
        self.stops.sort_by(|a, b| a.0.total_cmp(&b.0));
        self
    }

    /// Position (0..1, not clamped) of point `p`.
    pub fn position(&self, p: Point) -> f32 {
        match self.kind {
            GradientKind::Linear { from, to } => {
                let axis = to - from;
                let len_sq = axis.length_sq();
                if len_sq <= f32::EPSILON {
                    0.0
                } else {
                    (p - from).dot(axis) / len_sq
                }
            }
            GradientKind::Radial { center, radius } => {
                (p - center).length() / radius.max(f32::EPSILON)
            }
        }
    }

    /// The color at position `t`.
    pub fn color_at(&self, t: f32) -> Color {
        let Some(first) = self.stops.first() else {
            return Color::TRANSPARENT;
        };
        if t <= first.0 {
            return first.1;
        }
        for pair in self.stops.windows(2) {
            let ((t0, c0), (t1, c1)) = (pair[0], pair[1]);
            if t <= t1 {
                let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
                return c0.lerp(c1, f);
            }
        }
        self.stops.last().map_or(first.1, |s| s.1)
    }

    /// The gradient moved by `transform`.
    pub fn transformed(&self, transform: &Transform) -> Self {
        let kind = match self.kind {
            GradientKind::Linear { from, to } => GradientKind::Linear {
                from: transform.apply(from),
                to: transform.apply(to),
            },
            GradientKind::Radial { center, radius } => GradientKind::Radial {
                center: transform.apply(center),
                radius: radius * transform.scale_factor(),
            },
        };
        Self {
            kind,
            stops: self.stops.clone(),
        }
    }
}

/// A soft shadow (or glow) under a rounded rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    /// Moves the shadow (e.g. down a little, as if lit from above).
    pub offset: Vec2,
    /// How far it fades out, in points (0 = hard edge).
    pub blur: f32,
    /// Grows (or, negative, shrinks) the shadow before blurring.
    pub spread: f32,
    /// Its color at full strength.
    pub color: Color,
}

impl Shadow {
    /// No shadow.
    pub const NONE: Self = Self {
        offset: Vec2::ZERO,
        blur: 0.0,
        spread: 0.0,
        color: Color::TRANSPARENT,
    };
}

/// Points along the quadratic Bézier curve `p0 p1 p2` (from `p0` to `p2`),
/// close enough that the polyline strays at most `tolerance` from it.
pub fn quadratic_bezier_points(p: [Point; 3], tolerance: f32) -> Vec<Point> {
    let [p0, p1, p2] = p;
    let c1 = p0 + (p1 - p0) * (2.0 / 3.0);
    let c2 = p2 + (p1 - p2) * (2.0 / 3.0);
    cubic_bezier_points([p0, c1, c2, p2], tolerance)
}

/// Points along the cubic Bézier curve `p0 p1 p2 p3` (from `p0` to `p3`),
/// close enough that the polyline strays at most `tolerance` from it.
pub fn cubic_bezier_points(p: [Point; 4], tolerance: f32) -> Vec<Point> {
    let [p0, p1, p2, p3] = p;
    // The control polygon's bend bounds the error of a uniform split.
    let dd =
        |a: Point, b: Point, c: Point| (a.to_vec2() - b.to_vec2() * 2.0 + c.to_vec2()).length();
    let bend = dd(p0, p1, p2).max(dd(p1, p2, p3));
    let n = ((bend * 0.75 / tolerance.max(1e-4)).sqrt().ceil() as usize).clamp(1, 1000);
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            Point::new(
                w[0] * p0.x + w[1] * p1.x + w[2] * p2.x + w[3] * p3.x,
                w[0] * p0.y + w[1] * p1.y + w[2] * p2.y + w[3] * p3.y,
            )
        })
        .collect()
}

/// Splits the polyline `points` into dashes `dash` long with `gap`
/// between them, starting with a dash. Returns each dash's points.
pub fn dashes(points: &[Point], dash: f32, gap: f32) -> Vec<Vec<Point>> {
    let mut out = Vec::new();
    if points.len() < 2 || dash <= 0.0 {
        return out;
    }
    let gap = gap.max(0.0);
    let mut drawing = true;
    let mut left = dash;
    let mut current = vec![points[0]];
    for w in points.windows(2) {
        let (mut a, b) = (w[0], w[1]);
        let mut seg = (b - a).length();
        while seg > 0.0 {
            let step = left.min(seg);
            let dir = (b - a) * (step / seg);
            a += dir;
            seg -= step;
            left -= step;
            if drawing {
                current.push(a);
            }
            if left <= 1e-6 {
                if drawing {
                    out.push(std::mem::take(&mut current));
                } else {
                    current = vec![a];
                }
                drawing = !drawing;
                left = if drawing { dash } else { gap };
                if left <= 0.0 {
                    drawing = true;
                    left = dash;
                    current = vec![a];
                }
            }
        }
    }
    if drawing && current.len() >= 2 {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point, vec2};

    #[test]
    fn transforms_combine_in_order() {
        let t = Transform::rotate_around(point(0.0, 0.0), std::f32::consts::FRAC_PI_2)
            .then(Transform::translate(vec2(10.0, 0.0)));
        let p = t.apply(point(1.0, 0.0));
        assert!(
            (p.x - 10.0).abs() < 1e-5 && (p.y - 1.0).abs() < 1e-5,
            "{p:?}"
        );
        let s = Transform::scale_around(point(5.0, 5.0), 2.0, 2.0);
        assert_eq!(s.apply(point(6.0, 5.0)), point(7.0, 5.0));
        assert!((s.scale_factor() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn gradient_colors_between_stops() {
        let g = Gradient::linear(
            point(0.0, 0.0),
            point(10.0, 0.0),
            Color::BLACK,
            Color::WHITE,
        )
        .with_stop(0.5, Color::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(g.position(point(5.0, 3.0)), 0.5);
        assert_eq!(g.color_at(-1.0), Color::BLACK);
        assert_eq!(g.color_at(0.5), Color::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(g.color_at(0.75), Color::new(1.0, 0.5, 0.5, 1.0));
        let r = Gradient::radial(point(0.0, 0.0), 4.0, Color::BLACK, Color::WHITE);
        assert_eq!(r.position(point(0.0, 2.0)), 0.5);
    }

    #[test]
    fn bezier_ends_on_its_end_points_and_is_smooth() {
        let pts = cubic_bezier_points(
            [
                point(0.0, 0.0),
                point(0.0, 100.0),
                point(100.0, 100.0),
                point(100.0, 0.0),
            ],
            0.1,
        );
        assert_eq!(pts[0], point(0.0, 0.0));
        assert_eq!(*pts.last().unwrap(), point(100.0, 0.0));
        assert!(pts.len() > 20);
        let line =
            quadratic_bezier_points([point(0.0, 0.0), point(5.0, 0.0), point(10.0, 0.0)], 0.1);
        assert_eq!(line.len(), 2, "a straight curve needs no subdivision");
    }

    #[test]
    fn dashes_alternate_along_the_line() {
        let d = dashes(
            &[point(0.0, 0.0), point(10.0, 0.0), point(10.0, 10.0)],
            4.0,
            2.0,
        );
        // 20 points of length: dashes at 0-4, 6-10, 12-16, 18-20.
        assert_eq!(d.len(), 4);
        assert_eq!(d[0], [point(0.0, 0.0), point(4.0, 0.0)]);
        assert_eq!(
            d[1],
            [point(6.0, 0.0), point(10.0, 0.0)],
            "the corner ends a dash"
        );
        assert_eq!(d[2], [point(10.0, 2.0), point(10.0, 6.0)]);
        assert_eq!(d[3], [point(10.0, 8.0), point(10.0, 10.0)]);
    }
}
