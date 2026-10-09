//! Turns [`Shape`]s into triangle meshes the GPU can draw.
//!
//! Anti-aliasing uses *feathering*: every edge gets an extra strip, one
//! physical pixel wide, that fades from the shape color to transparent.
//! This gives smooth edges without multisampling.

use std::f32::consts::TAU;

use crate::{
    ClippedShape, Color, DisplayList, Galley, PaintCallback, Point, Rect, Shape, Stroke,
    TextureAtlas, TextureId, Vec2,
};

/// One vertex of a [`Mesh`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    /// Position in logical points.
    pub pos: Point,
    /// Texture coordinate in the atlas (0..1).
    pub uv: [f32; 2],
    /// Linear color with **premultiplied** alpha.
    pub color: [f32; 4],
}

/// An indexed triangle list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    /// The texture all vertices' `uv` refer to.
    pub texture: TextureId,
    /// The vertices referenced by `indices`.
    pub vertices: Vec<Vertex>,
    /// Three indices per triangle.
    pub indices: Vec<u32>,
}

impl Mesh {
    /// True if the mesh has no triangles.
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    fn next_index(&self) -> u32 {
        u32::try_from(self.vertices.len()).expect("mesh exceeds u32::MAX vertices")
    }

    fn triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }

    /// Two triangles for the quad `a b c d` (in order around the quad).
    fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.triangle(a, b, c);
        self.triangle(a, c, d);
    }
}

/// A mesh and the clip rectangle (in logical points) it must be drawn within.
#[derive(Clone, Debug, PartialEq)]
pub struct ClippedMesh {
    /// Scissor area, in logical points.
    pub clip_rect: Rect,
    /// Triangles to draw.
    pub mesh: Mesh,
    /// Custom drawing instead of triangles (the mesh is then empty), in
    /// the order it appears among the meshes.
    pub callback: Option<PaintCallback>,
}

/// Converts display lists into meshes. Reuse one instance across frames to
/// avoid reallocating scratch buffers.
#[derive(Clone, Debug)]
pub struct Tessellator {
    /// Width of the anti-aliasing strip, in points (one physical pixel).
    feather: f32,
    pixels_per_point: f32,
    white_uv: [f32; 2],
    /// Atlas side in texels, to turn glyph regions into texture coordinates.
    atlas_size: f32,
    path: Vec<Point>,
    normals: Vec<Vec2>,
}

/// Largest distance, in physical pixels, between a true curve and the
/// polygon approximating it.
const CURVE_TOLERANCE_PX: f32 = 0.1;

/// A ring of vertices offset from the path along its normals, with a color.
#[derive(Clone, Copy)]
struct Ring {
    offset: f32,
    color: [f32; 4],
}

const TRANSPARENT: [f32; 4] = [0.0; 4];

impl Tessellator {
    /// Create the tessellator *after* all text for the frame has been laid
    /// out, since laying out text may grow `atlas`.
    pub fn new(pixels_per_point: f32, atlas: &TextureAtlas) -> Self {
        Self {
            feather: 1.0 / pixels_per_point,
            pixels_per_point,
            white_uv: atlas.white_uv(),
            atlas_size: atlas.size() as f32,
            path: Vec::new(),
            normals: Vec::new(),
        }
    }

    /// Tessellates every shape in `list`. Consecutive shapes with the same
    /// clip rectangle are merged into one mesh, so they become one draw call.
    pub fn tessellate(&mut self, list: &DisplayList) -> Vec<ClippedMesh> {
        let mut out: Vec<ClippedMesh> = Vec::new();
        for ClippedShape { clip_rect, shape } in list.shapes() {
            if !shape
                .bounding_rect()
                .expand(self.feather)
                .intersects(*clip_rect)
            {
                continue; // fully clipped away
            }
            if let Shape::Callback(callback) = shape {
                out.push(ClippedMesh {
                    clip_rect: *clip_rect,
                    mesh: Mesh::default(),
                    callback: Some(callback.clone()),
                });
                continue;
            }
            let texture = match shape {
                Shape::Image { texture, .. } => *texture,
                _ => TextureId::Atlas,
            };
            let mesh = match out.last_mut() {
                Some(last)
                    if last.callback.is_none()
                        && last.clip_rect == *clip_rect
                        && last.mesh.texture == texture =>
                {
                    &mut last.mesh
                }
                _ => {
                    out.push(ClippedMesh {
                        clip_rect: *clip_rect,
                        mesh: Mesh {
                            texture,
                            ..Mesh::default()
                        },
                        callback: None,
                    });
                    &mut out.last_mut().expect("just pushed").mesh
                }
            };
            self.tessellate_shape(shape, mesh);
        }
        out.retain(|m| !m.mesh.is_empty() || m.callback.is_some());
        out
    }

    /// Appends the triangles for one shape to `mesh`.
    pub fn tessellate_shape(&mut self, shape: &Shape, mesh: &mut Mesh) {
        match shape {
            Shape::Rect {
                rect,
                corner_radius,
                fill,
                stroke,
            } => {
                self.path.clear();
                self.add_rounded_rect(*rect, *corner_radius);
                self.fill_and_stroke(true, *fill, *stroke, mesh);
            }
            Shape::Circle {
                center,
                radius,
                fill,
                stroke,
            } => {
                self.path.clear();
                self.add_arc(*center, *radius, 0.0, TAU, false);
                self.fill_and_stroke(true, *fill, *stroke, mesh);
            }
            Shape::LineSegment { points, stroke } => {
                self.path.clear();
                self.path.extend_from_slice(points);
                self.fill_and_stroke(false, Color::TRANSPARENT, *stroke, mesh);
            }
            Shape::Path {
                points,
                closed,
                fill,
                stroke,
            } => {
                self.path.clear();
                self.path.extend_from_slice(points);
                self.fill_and_stroke(*closed, *fill, *stroke, mesh);
            }
            Shape::Text { pos, galley, color } => self.add_text(*pos, galley, *color, mesh),
            // Drawn by the renderer (see `tessellate`), not as triangles.
            Shape::Callback(_) => {}
            Shape::Image {
                rect,
                uv,
                tint,
                corner_radius,
                ..
            } if *corner_radius > 0.0 => {
                if tint.a <= 0.0 {
                    return;
                }
                // The rounded outline, anti-aliased like a rectangle, with
                // texture coordinates following each vertex's position.
                let first = mesh.vertices.len();
                self.path.clear();
                self.add_rounded_rect(*rect, *corner_radius);
                self.fill_and_stroke(true, *tint, Stroke::NONE, mesh);
                let size = rect.size();
                for v in &mut mesh.vertices[first..] {
                    let t = Vec2::new(
                        ((v.pos.x - rect.min.x) / size.x.max(f32::EPSILON)).clamp(0.0, 1.0),
                        ((v.pos.y - rect.min.y) / size.y.max(f32::EPSILON)).clamp(0.0, 1.0),
                    );
                    v.uv = [uv.min.x + t.x * uv.width(), uv.min.y + t.y * uv.height()];
                }
            }
            Shape::Image { rect, uv, tint, .. } => {
                if tint.a <= 0.0 {
                    return;
                }
                let color = premultiplied(*tint);
                let first = mesh.next_index();
                for (pos, uv) in [
                    (rect.left_top(), [uv.min.x, uv.min.y]),
                    (rect.right_top(), [uv.max.x, uv.min.y]),
                    (rect.right_bottom(), [uv.max.x, uv.max.y]),
                    (rect.left_bottom(), [uv.min.x, uv.max.y]),
                ] {
                    mesh.vertices.push(Vertex { pos, uv, color });
                }
                mesh.quad(first, first + 1, first + 2, first + 3);
            }
        }
    }

    /// One textured quad per glyph. The origin is snapped to the physical
    /// pixel grid so glyph bitmaps map 1:1 onto screen pixels.
    fn add_text(&self, pos: Point, galley: &Galley, color: Color, mesh: &mut Mesh) {
        if color.a <= 0.0 {
            return;
        }
        let ppp = self.pixels_per_point;
        let origin = Point::new((pos.x * ppp).round() / ppp, (pos.y * ppp).round() / ppp);
        // Glyph offsets are in the galley's pixels, which normally match ours.
        let galley_ppp = galley.pixels_per_point;
        let tint = premultiplied(color);
        let emoji_tint = premultiplied(Color::WHITE.with_alpha(color.a));
        for glyph in &galley.glyphs {
            let r = glyph.region;
            let min = origin
                + Vec2::new(glyph.offset_px[0] as f32, glyph.offset_px[1] as f32) / galley_ppp;
            let max = min + Vec2::new(r.width as f32, r.height as f32) / galley_ppp;
            let uv_min = [r.x as f32 / self.atlas_size, r.y as f32 / self.atlas_size];
            let uv_max = [
                (r.x + r.width) as f32 / self.atlas_size,
                (r.y + r.height) as f32 / self.atlas_size,
            ];
            let color = if glyph.colored { emoji_tint } else { tint };
            let first = mesh.next_index();
            for (pos, uv) in [
                (min, uv_min),
                (Point::new(max.x, min.y), [uv_max[0], uv_min[1]]),
                (max, uv_max),
                (Point::new(min.x, max.y), [uv_min[0], uv_max[1]]),
            ] {
                mesh.vertices.push(Vertex { pos, uv, color });
            }
            mesh.quad(first, first + 1, first + 2, first + 3);
        }
    }

    fn fill_and_stroke(&mut self, closed: bool, fill: Color, stroke: Stroke, mesh: &mut Mesh) {
        dedup_points(&mut self.path, self.feather * 0.01);
        if self.path.len() < 2 {
            return;
        }
        compute_normals(&self.path, closed, &mut self.normals);

        if closed && fill.a > 0.0 && self.path.len() >= 3 {
            let half = self.feather / 2.0;
            let rings = [
                Ring {
                    offset: -half,
                    color: premultiplied(fill),
                },
                Ring {
                    offset: half,
                    color: TRANSPARENT,
                },
            ];
            let first = self.add_rings(&rings, closed, mesh);
            // Triangle fan over the inner ring (valid for convex paths).
            let ring_count = rings.len() as u32;
            for i in 1..self.path.len() as u32 - 1 {
                mesh.triangle(first, first + i * ring_count, first + (i + 1) * ring_count);
            }
        }

        if !stroke.is_none() {
            let color = premultiplied(stroke.color);
            let (w, f) = (stroke.width / 2.0, self.feather / 2.0);
            if stroke.width >= self.feather {
                let rings = [
                    Ring {
                        offset: -w - f,
                        color: TRANSPARENT,
                    },
                    Ring {
                        offset: -w + f,
                        color,
                    },
                    Ring {
                        offset: w - f,
                        color,
                    },
                    Ring {
                        offset: w + f,
                        color: TRANSPARENT,
                    },
                ];
                self.add_rings(&rings, closed, mesh);
            } else {
                // Thinner than a pixel: keep a one-pixel-wide profile and fade
                // the color, so the covered area still equals the width.
                let faded = color.map(|c| c * stroke.width / self.feather);
                let rings = [
                    Ring {
                        offset: -self.feather,
                        color: TRANSPARENT,
                    },
                    Ring {
                        offset: 0.0,
                        color: faded,
                    },
                    Ring {
                        offset: self.feather,
                        color: TRANSPARENT,
                    },
                ];
                self.add_rings(&rings, closed, mesh);
            }
        }
    }

    /// Adds one vertex per (point, ring) and stitches neighbouring rings
    /// with quads. Open paths also get feathered end caps.
    /// Returns the index of the first vertex; vertex `(i, k)` is at
    /// `first + i * rings.len() + k`.
    fn add_rings(&self, rings: &[Ring], closed: bool, mesh: &mut Mesh) -> u32 {
        let first = mesh.next_index();
        let r = rings.len() as u32;
        let n = self.path.len() as u32;
        let last = self.path.len() - 1;
        // Direction pointing away from the path at each open end.
        let (start_out, end_out) = if closed {
            (Vec2::ZERO, Vec2::ZERO)
        } else {
            (
                (self.path[0] - self.path[1]).normalized(),
                (self.path[last] - self.path[last - 1]).normalized(),
            )
        };
        for (i, (p, normal)) in self.path.iter().zip(&self.normals).enumerate() {
            // Pull open ends inward by half a feather; the cap strip extends
            // them outward by the same amount, centering the fade on the end.
            let p = match i {
                0 => *p - start_out * (self.feather / 2.0),
                i if i == last => *p - end_out * (self.feather / 2.0),
                _ => *p,
            };
            for ring in rings {
                self.push_vertex(mesh, p + *normal * ring.offset, ring.color);
            }
        }
        let segments = if closed { n } else { n - 1 };
        for i in 0..segments {
            let j = (i + 1) % n;
            for k in 0..r - 1 {
                let (a, b) = (first + i * r + k, first + i * r + k + 1);
                let (c, d) = (first + j * r + k + 1, first + j * r + k);
                mesh.quad(a, b, c, d);
            }
        }
        if !closed {
            self.add_cap(rings, 0, start_out, first, mesh);
            self.add_cap(rings, last, end_out, first + (n - 1) * r, mesh);
        }
        first
    }

    /// Feathers the flat end of an open stroke: a strip fading outward
    /// along `outward` from the ring vertices at `point_index`.
    fn add_cap(
        &self,
        rings: &[Ring],
        point_index: usize,
        outward: Vec2,
        ring_start: u32,
        mesh: &mut Mesh,
    ) {
        let p = self.path[point_index];
        let normal = self.normals[point_index];
        let cap_start = mesh.next_index();
        for ring in rings {
            let pos = p + normal * ring.offset + outward * (self.feather / 2.0);
            self.push_vertex(mesh, pos, TRANSPARENT);
        }
        for k in 0..rings.len() as u32 - 1 {
            mesh.quad(
                ring_start + k,
                ring_start + k + 1,
                cap_start + k + 1,
                cap_start + k,
            );
        }
    }

    fn push_vertex(&self, mesh: &mut Mesh, pos: Point, color: [f32; 4]) {
        mesh.vertices.push(Vertex {
            pos,
            uv: self.white_uv,
            color,
        });
    }

    fn add_rounded_rect(&mut self, rect: Rect, corner_radius: f32) {
        let r = corner_radius
            .min(rect.width() / 2.0)
            .min(rect.height() / 2.0)
            .max(0.0);
        if r == 0.0 {
            self.path.extend_from_slice(&[
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
            ]);
            return;
        }
        use std::f32::consts::{FRAC_PI_2, PI};
        let inset = |p: Point, dx: f32, dy: f32| Point::new(p.x + dx, p.y + dy);
        // Clockwise on screen, starting at the top-left corner.
        self.add_arc(inset(rect.left_top(), r, r), r, PI, PI + FRAC_PI_2, true);
        self.add_arc(inset(rect.right_top(), -r, r), r, -FRAC_PI_2, 0.0, true);
        self.add_arc(inset(rect.right_bottom(), -r, -r), r, 0.0, FRAC_PI_2, true);
        self.add_arc(inset(rect.left_bottom(), r, -r), r, FRAC_PI_2, PI, true);
    }

    /// Adds points along an arc from angle `start` to `end` (radians, 0 =
    /// +x, increasing clockwise on screen). With `include_end` false the end
    /// point is omitted, which is what a full circle needs.
    fn add_arc(&mut self, center: Point, radius: f32, start: f32, end: f32, include_end: bool) {
        let segments = self.segments_for(radius, end - start);
        let last = if include_end { segments } else { segments - 1 };
        for i in 0..=last {
            let angle = start + (end - start) * i as f32 / segments as f32;
            let (sin, cos) = angle.sin_cos();
            self.path
                .push(Point::new(center.x + radius * cos, center.y + radius * sin));
        }
    }

    /// Number of straight segments needed to approximate an arc of `sweep`
    /// radians within [`CURVE_TOLERANCE_PX`].
    fn segments_for(&self, radius: f32, sweep: f32) -> u32 {
        let radius_px = radius * self.pixels_per_point;
        if radius_px <= CURVE_TOLERANCE_PX {
            return 1;
        }
        // The sagitta of a chord spanning angle θ is r·(1 − cos(θ/2)).
        let max_angle = 2.0 * (1.0 - CURVE_TOLERANCE_PX / radius_px).acos();
        let n = (sweep.abs() / max_angle).ceil();
        // Bounded by the clamp, so the cast cannot truncate meaningfully.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = n.clamp(1.0, 256.0) as u32;
        n.max(if sweep.abs() >= TAU { 8 } else { 1 })
    }
}

fn premultiplied(c: Color) -> [f32; 4] {
    [c.r * c.a, c.g * c.a, c.b * c.a, c.a]
}

/// Removes consecutive (and, for closed paths, first/last) points closer
/// than `epsilon`, which would otherwise produce undefined normals.
fn dedup_points(path: &mut Vec<Point>, epsilon: f32) {
    path.dedup_by(|b, a| a.distance(*b) < epsilon);
    while path.len() > 2 && path[0].distance(path[path.len() - 1]) < epsilon {
        path.pop();
    }
}

/// Computes one normal per point such that offsetting every point by
/// `normal * d` moves every edge outward by exactly `d`.
///
/// For closed paths "outward" is away from the enclosed area regardless of
/// winding. For open paths it is to the left of the direction of travel.
fn compute_normals(path: &[Point], closed: bool, normals: &mut Vec<Vec2>) {
    let n = path.len();
    // Outward edge normal for a clockwise (on screen) path is direction
    // rotated counter-clockwise; flip it for counter-clockwise paths.
    let sign = if closed && signed_area(path) < 0.0 {
        -1.0
    } else {
        1.0
    };
    let edge_normal = |i: usize| {
        let d = (path[(i + 1) % n] - path[i]).normalized();
        Vec2::new(d.y, -d.x) * sign
    };

    normals.clear();
    for i in 0..n {
        let normal = if !closed && i == 0 {
            edge_normal(0)
        } else if !closed && i == n - 1 {
            edge_normal(n - 2)
        } else {
            let prev = edge_normal((i + n - 1) % n);
            let next = edge_normal(i);
            miter(prev, next)
        };
        normals.push(normal);
    }
}

/// Combines two unit edge normals into a vertex normal whose projection on
/// each of them is 1. Very sharp corners are limited to avoid spikes.
fn miter(a: Vec2, b: Vec2) -> Vec2 {
    let mid = (a + b) / 2.0;
    let len_sq = mid.length_sq();
    const MIN_LEN_SQ: f32 = 0.1; // limits the miter to ~3x the stroke
    if len_sq < MIN_LEN_SQ {
        // Nearly a U-turn: fall back to a bevel-like direction.
        return if len_sq > 0.0 {
            mid / MIN_LEN_SQ.sqrt() / len_sq.sqrt()
        } else {
            a
        };
    }
    mid / len_sq
}

/// Shoelace formula. Positive for paths that run clockwise on screen.
fn signed_area(path: &[Point]) -> f32 {
    let n = path.len();
    (0..n)
        .map(|i| {
            let (a, b) = (path[i], path[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f32>()
        / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point, vec2};

    fn tessellator(pixels_per_point: f32) -> Tessellator {
        Tessellator::new(pixels_per_point, &TextureAtlas::new(16))
    }

    fn tessellate(shape: Shape, pixels_per_point: f32) -> Mesh {
        let mut mesh = Mesh::default();
        tessellator(pixels_per_point).tessellate_shape(&shape, &mut mesh);
        mesh
    }

    fn triangle_area(mesh: &Mesh, tri: &[u32]) -> f32 {
        let [a, b, c] = [0, 1, 2].map(|i| mesh.vertices[tri[i] as usize].pos);
        ((b - a).x * (c - a).y - (c - a).x * (b - a).y) / 2.0
    }

    /// Sum of triangle areas weighted by their average alpha: approximates
    /// the covered area, so it should match the geometric area of the shape.
    fn coverage(mesh: &Mesh) -> f32 {
        mesh.indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|tri| {
                let alpha: f32 = tri
                    .iter()
                    .map(|&i| mesh.vertices[i as usize].color[3])
                    .sum::<f32>()
                    / 3.0;
                triangle_area(mesh, tri).abs() * alpha
            })
            .sum()
    }

    #[test]
    fn filled_rect_covers_its_area() {
        let rect = Rect::from_min_size(point(10.0, 10.0), vec2(100.0, 50.0));
        let mesh = tessellate(
            Shape::Rect {
                rect,
                corner_radius: 0.0,
                fill: Color::WHITE,
                stroke: Stroke::NONE,
            },
            1.0,
        );
        // 4 corners × 2 rings; 2 fan triangles + 4 feather quads.
        assert_eq!(mesh.vertices.len(), 8);
        assert_eq!(mesh.indices.len(), 3 * (2 + 8));
        let area = coverage(&mesh);
        assert!((area - 5000.0).abs() < 2.0, "coverage {area}");
    }

    #[test]
    fn winding_does_not_change_the_result() {
        let cw = vec![
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 10.0),
            point(0.0, 10.0),
        ];
        let ccw: Vec<Point> = cw.iter().rev().copied().collect();
        let poly = |points| Shape::Path {
            points,
            closed: true,
            fill: Color::WHITE,
            stroke: Stroke::NONE,
        };
        let a = coverage(&tessellate(poly(cw), 1.0));
        let b = coverage(&tessellate(poly(ccw), 1.0));
        assert!(
            (a - 100.0).abs() < 0.5 && (b - 100.0).abs() < 0.5,
            "{a} vs {b}"
        );
    }

    #[test]
    fn circle_coverage_matches_pi_r_squared() {
        for ppp in [1.0, 2.0] {
            let mesh = tessellate(
                Shape::Circle {
                    center: point(50.0, 50.0),
                    radius: 20.0,
                    fill: Color::WHITE,
                    stroke: Stroke::NONE,
                },
                ppp,
            );
            let expected = std::f32::consts::PI * 400.0;
            let area = coverage(&mesh);
            assert!(
                (area - expected).abs() / expected < 0.01,
                "ppp {ppp}: {area} vs {expected}"
            );
        }
    }

    #[test]
    fn rounded_rect_coverage() {
        let r = 10.0;
        let mesh = tessellate(
            Shape::Rect {
                rect: Rect::from_min_size(point(0.0, 0.0), vec2(100.0, 60.0)),
                corner_radius: r,
                fill: Color::WHITE,
                stroke: Stroke::NONE,
            },
            1.0,
        );
        let expected = 100.0 * 60.0 - (4.0 - std::f32::consts::PI) * r * r;
        let area = coverage(&mesh);
        assert!(
            (area - expected).abs() / expected < 0.005,
            "{area} vs {expected}"
        );
    }

    /// LAY-07: rounded images cover a rounded rectangle and map texture
    /// coordinates by position.
    #[test]
    fn rounded_images() {
        let r = 10.0;
        let rect = Rect::from_min_size(point(0.0, 0.0), vec2(100.0, 60.0));
        let mesh = tessellate(
            Shape::Image {
                rect,
                texture: TextureId::User(1),
                uv: Rect::from_min_max(point(0.0, 0.0), point(1.0, 1.0)),
                tint: Color::WHITE,
                corner_radius: r,
            },
            1.0,
        );
        let expected = 100.0 * 60.0 - (4.0 - std::f32::consts::PI) * r * r;
        let area = coverage(&mesh);
        assert!(
            (area - expected).abs() / expected < 0.005,
            "{area} vs {expected}"
        );
        for v in &mesh.vertices {
            let expect = [
                (v.pos.x / 100.0).clamp(0.0, 1.0),
                (v.pos.y / 60.0).clamp(0.0, 1.0),
            ];
            assert!((v.uv[0] - expect[0]).abs() < 1e-5 && (v.uv[1] - expect[1]).abs() < 1e-5);
        }
    }

    #[test]
    fn corner_radius_is_clamped() {
        // A 20×20 square with a huge radius becomes a circle of radius 10.
        let mesh = tessellate(
            Shape::Rect {
                rect: Rect::from_min_size(point(0.0, 0.0), vec2(20.0, 20.0)),
                corner_radius: 1000.0,
                fill: Color::WHITE,
                stroke: Stroke::NONE,
            },
            1.0,
        );
        let expected = std::f32::consts::PI * 100.0;
        let area = coverage(&mesh);
        assert!(
            (area - expected).abs() / expected < 0.02,
            "{area} vs {expected}"
        );
    }

    #[test]
    fn line_coverage_matches_length_times_width() {
        let mesh = tessellate(
            Shape::LineSegment {
                points: [point(0.0, 0.0), point(30.0, 40.0)],
                stroke: Stroke::new(4.0, Color::WHITE),
            },
            1.0,
        );
        let area = coverage(&mesh);
        assert!((area - 200.0).abs() < 2.0, "{area}");
    }

    #[test]
    fn hairline_is_faded_not_dropped() {
        let mesh = tessellate(
            Shape::LineSegment {
                points: [point(0.0, 0.0), point(100.0, 0.0)],
                stroke: Stroke::new(0.5, Color::WHITE),
            },
            1.0,
        );
        let area = coverage(&mesh);
        assert!((area - 50.0).abs() < 2.0, "{area}");
    }

    #[test]
    fn invisible_shapes_produce_no_triangles() {
        let rect = Rect::from_min_size(point(0.0, 0.0), vec2(10.0, 10.0));
        let none = Shape::Rect {
            rect,
            corner_radius: 0.0,
            fill: Color::TRANSPARENT,
            stroke: Stroke::NONE,
        };
        assert!(tessellate(none, 1.0).is_empty());
        let degenerate = Shape::LineSegment {
            points: [point(5.0, 5.0), point(5.0, 5.0)],
            stroke: Stroke::new(2.0, Color::WHITE),
        };
        assert!(tessellate(degenerate, 1.0).is_empty());
    }

    #[test]
    fn colors_are_premultiplied() {
        let mesh = tessellate(
            Shape::Circle {
                center: point(0.0, 0.0),
                radius: 5.0,
                fill: Color::new(1.0, 0.5, 0.0, 0.5),
                stroke: Stroke::NONE,
            },
            1.0,
        );
        assert!(
            mesh.vertices
                .iter()
                .any(|v| v.color == [0.5, 0.25, 0.0, 0.5])
        );
    }

    #[test]
    fn same_clip_shapes_share_a_mesh() {
        let mut list = DisplayList::new();
        let clip = Rect::from_min_size(point(0.0, 0.0), vec2(50.0, 50.0));
        list.circle_filled(point(10.0, 10.0), 5.0, Color::WHITE);
        list.circle_filled(point(20.0, 10.0), 5.0, Color::WHITE);
        list.with_clip(clip, |l| {
            l.circle_filled(point(10.0, 10.0), 5.0, Color::WHITE);
            // Outside the clip: culled entirely.
            l.circle_filled(point(100.0, 100.0), 5.0, Color::WHITE);
        });
        let meshes = tessellator(1.0).tessellate(&list);
        assert_eq!(meshes.len(), 2);
        assert_eq!(meshes[0].clip_rect, Rect::EVERYTHING);
        assert_eq!(meshes[1].clip_rect, clip);
        assert_eq!(
            meshes[0].mesh.vertices.len(),
            2 * meshes[1].mesh.vertices.len()
        );
    }

    #[test]
    fn text_glyphs_become_pixel_aligned_quads() {
        use crate::{AtlasRegion, GlyphQuad};
        let region = AtlasRegion {
            x: 8,
            y: 4,
            width: 4,
            height: 6,
        };
        let galley = Galley {
            size: vec2(5.0, 10.0),
            line_count: 1,
            pixels_per_point: 2.0,
            glyphs: vec![GlyphQuad {
                offset_px: [2, 3],
                region,
                colored: false,
            }],
            rows: Vec::new(),
        };
        let shape = Shape::Text {
            // 10.3 points = 20.6 px, snapped to 21 px.
            pos: point(10.3, 0.0),
            galley: std::sync::Arc::new(galley),
            color: Color::new(1.0, 0.0, 0.0, 0.5),
        };
        let mesh = tessellate(shape, 2.0);
        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices.len(), 6);
        let v0 = mesh.vertices[0];
        assert_eq!(v0.pos, point((21.0 + 2.0) / 2.0, 1.5));
        assert_eq!(mesh.vertices[2].pos, point((21.0 + 6.0) / 2.0, 4.5));
        assert_eq!(v0.uv, [8.0 / 16.0, 4.0 / 16.0]);
        assert_eq!(v0.color, [0.5, 0.0, 0.0, 0.5]);
    }

    #[test]
    fn images_get_their_own_mesh() {
        let mut list = DisplayList::new();
        list.circle_filled(point(5.0, 5.0), 4.0, Color::WHITE);
        list.image(
            Rect::from_min_size(point(0.0, 0.0), vec2(10.0, 10.0)),
            TextureId::User(7),
            Color::WHITE,
        );
        list.circle_filled(point(5.0, 5.0), 4.0, Color::WHITE);
        let meshes = tessellator(1.0).tessellate(&list);
        let textures: Vec<TextureId> = meshes.iter().map(|m| m.mesh.texture).collect();
        assert_eq!(
            textures,
            [TextureId::Atlas, TextureId::User(7), TextureId::Atlas]
        );
        let image = &meshes[1].mesh;
        assert_eq!(image.vertices.len(), 4);
        assert_eq!(image.vertices[2].uv, [1.0, 1.0]);
    }
}
