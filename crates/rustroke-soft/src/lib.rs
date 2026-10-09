//! A software renderer for rustroke: draws tessellated meshes on the CPU,
//! with the same conventions as the GPU renderer (linear blending with
//! premultiplied alpha, sRGB textures and target, bilinear sampling,
//! scissor rectangles from clip rectangles).
//!
//! It needs no GPU, so it works in CI machines, virtual machines and
//! servers (e.g. for screenshots in tests). It is much slower than the GPU
//! and not meant for interactive drawing of large windows.
//!
//! ```
//! use rustroke_core::{Color, DisplayList, PhysicalSize, Rect, Tessellator, TextureAtlas, point};
//! use rustroke_soft::SoftwareRenderer;
//!
//! let atlas = TextureAtlas::new(64);
//! let mut list = DisplayList::new();
//! list.rect_filled(Rect::from_min_max(point(2.0, 2.0), point(6.0, 6.0)), 0.0, Color::WHITE);
//! let meshes = Tessellator::new(1.0, &atlas).tessellate(&list);
//! let image = SoftwareRenderer::new().render(&meshes, &atlas, PhysicalSize::new(8, 8), 1.0, Color::BLACK);
//! assert_eq!(image.pixel(4, 4), [255, 255, 255, 255]);
//! assert_eq!(image.pixel(0, 0), [0, 0, 0, 255]);
//! ```

use std::collections::HashMap;

use rustroke_core::{
    ClippedMesh, Color, PhysicalSize, Rect, TextureAtlas, TextureId, TexturesDelta, Vertex,
};

/// An image drawn by [`SoftwareRenderer::render`]: sRGB RGBA8 pixels, row
/// by row, top first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Width and height in pixels.
    pub size: [u32; 2],
    /// `size[0] * size[1] * 4` bytes.
    pub pixels: Vec<u8>,
}

impl Image {
    /// Pixel `(x, y)` as sRGB RGBA bytes.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.size[0] + x) * 4) as usize;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }
}

/// A texture as linear, premultiplied RGBA.
#[derive(Clone, Debug)]
struct Texture {
    size: [u32; 2],
    texels: Vec<[f32; 4]>,
}

/// Draws meshes into images on the CPU. Keep one around: it remembers the
/// application's textures (see [`SoftwareRenderer::update_textures`]).
#[derive(Debug, Default)]
pub struct SoftwareRenderer {
    textures: HashMap<TextureId, Texture>,
}

/// sRGB byte → linear intensity, for decoding texels.
fn srgb_to_linear_lut() -> [f32; 256] {
    let mut lut = [0.0; 256];
    for (i, v) in lut.iter_mut().enumerate() {
        let c = i as f32 / 255.0;
        *v = if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        };
    }
    lut
}

fn linear_to_srgb8(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

fn decode(texel: [u8; 4], lut: &[f32; 256]) -> [f32; 4] {
    [
        lut[texel[0] as usize],
        lut[texel[1] as usize],
        lut[texel[2] as usize],
        f32::from(texel[3]) / 255.0,
    ]
}

impl SoftwareRenderer {
    /// A renderer without application textures.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies uploads and frees of application textures (from
    /// `FrameOutput::textures`).
    pub fn update_textures(&mut self, delta: &TexturesDelta) {
        let lut = srgb_to_linear_lut();
        for (id, image) in &delta.set {
            let texels = image
                .to_premultiplied()
                .into_iter()
                .map(|t| decode(t, &lut))
                .collect();
            self.textures.insert(
                *id,
                Texture {
                    size: image.size,
                    texels,
                },
            );
        }
        for id in &delta.free {
            self.textures.remove(id);
        }
    }

    /// Draws `meshes` (in points, `pixels_per_point` physical pixels per
    /// point) over `clear` into an image of `size` pixels. Paint callbacks
    /// are skipped.
    pub fn render(
        &self,
        meshes: &[ClippedMesh],
        atlas: &TextureAtlas,
        size: PhysicalSize,
        pixels_per_point: f32,
        clear: Color,
    ) -> Image {
        let (w, h) = (size.width as usize, size.height as usize);
        let clear = [
            clear.r * clear.a,
            clear.g * clear.a,
            clear.b * clear.a,
            clear.a,
        ];
        let mut target = vec![clear; w * h];
        let lut = srgb_to_linear_lut();
        let atlas_size = atlas.size();
        let atlas_texels = atlas.pixels();
        for clipped in meshes {
            if clipped.callback.is_some() {
                continue;
            }
            let Some(scissor) = scissor(clipped.clip_rect, pixels_per_point, size) else {
                continue;
            };
            let mesh = &clipped.mesh;
            let sample = |uv: [f32; 2]| -> [f32; 4] {
                match mesh.texture {
                    TextureId::Atlas => bilinear(uv, [atlas_size, atlas_size], |x, y| {
                        decode(atlas_texels[(y * atlas_size + x) as usize], &lut)
                    }),
                    id => match self.textures.get(&id) {
                        Some(t) => {
                            bilinear(uv, t.size, |x, y| t.texels[(y * t.size[0] + x) as usize])
                        }
                        None => [1.0; 4],
                    },
                }
            };
            for tri in mesh.indices.as_chunks::<3>().0 {
                let v = [tri[0], tri[1], tri[2]].map(|i| mesh.vertices[i as usize]);
                draw_triangle(&mut target, w, scissor, pixels_per_point, v, &sample);
            }
        }
        let pixels = target
            .iter()
            .flat_map(|c| {
                [
                    linear_to_srgb8(c[0]),
                    linear_to_srgb8(c[1]),
                    linear_to_srgb8(c[2]),
                    (c[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                ]
            })
            .collect();
        Image {
            size: [size.width, size.height],
            pixels,
        }
    }
}

/// The clip rectangle in pixels (as the GPU scissor): `[x0, y0, x1, y1)`.
fn scissor(clip: Rect, ppp: f32, target: PhysicalSize) -> Option<[usize; 4]> {
    let (tw, th) = (target.width as f32, target.height as f32);
    let x0 = (clip.min.x * ppp).floor().clamp(0.0, tw);
    let y0 = (clip.min.y * ppp).floor().clamp(0.0, th);
    let x1 = (clip.max.x * ppp).ceil().clamp(0.0, tw);
    let y1 = (clip.max.y * ppp).ceil().clamp(0.0, th);
    (x1 > x0 && y1 > y0).then_some([x0 as usize, y0 as usize, x1 as usize, y1 as usize])
}

/// Bilinear filtering with clamped edges; `uv` in 0..1.
fn bilinear(uv: [f32; 2], size: [u32; 2], texel: impl Fn(u32, u32) -> [f32; 4]) -> [f32; 4] {
    let (w, h) = (size[0].max(1), size[1].max(1));
    let x = uv[0] * w as f32 - 0.5;
    let y = uv[1] * h as f32 - 0.5;
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let clamp = |v: f32, max: u32| (v.max(0.0) as u32).min(max - 1);
    let (xa, xb) = (clamp(x0, w), clamp(x0 + 1.0, w));
    let (ya, yb) = (clamp(y0, h), clamp(y0 + 1.0, h));
    let (a, b, c, d) = (texel(xa, ya), texel(xb, ya), texel(xa, yb), texel(xb, yb));
    let mut out = [0.0; 4];
    for i in 0..4 {
        let top = a[i] + (b[i] - a[i]) * fx;
        let bottom = c[i] + (d[i] - c[i]) * fx;
        out[i] = top + (bottom - top) * fy;
    }
    out
}

/// Fills one triangle into `target` (linear premultiplied RGBA), sampling
/// pixel centers, with the top-left rule so shared edges aren't drawn
/// twice.
fn draw_triangle(
    target: &mut [[f32; 4]],
    width: usize,
    scissor: [usize; 4],
    ppp: f32,
    v: [Vertex; 3],
    sample: &impl Fn([f32; 2]) -> [f32; 4],
) {
    let p = v.map(|v| (v.pos.x * ppp, v.pos.y * ppp));
    let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
    if area.abs() < 1e-12 {
        return;
    }
    // Edge functions, positive inside for either winding.
    let sign = area.signum();
    let edge = |a: (f32, f32), b: (f32, f32), x: f32, y: f32| {
        ((b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0)) * sign
    };
    // Top-left rule: pixels exactly on an edge belong to the triangle on
    // one side only.
    let top_left = |a: (f32, f32), b: (f32, f32)| {
        let (dx, dy) = ((b.0 - a.0) * sign, (b.1 - a.1) * sign);
        (dy == 0.0 && dx < 0.0) || dy > 0.0
    };
    let edges = [(p[1], p[2]), (p[2], p[0]), (p[0], p[1])];
    let rules = edges.map(|(a, b)| top_left(a, b));
    let min_x = p
        .iter()
        .map(|q| q.0)
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(scissor[0] as f32) as usize;
    let max_x = p
        .iter()
        .map(|q| q.0)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(scissor[2] as f32) as usize;
    let min_y = p
        .iter()
        .map(|q| q.1)
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(scissor[1] as f32) as usize;
    let max_y = p
        .iter()
        .map(|q| q.1)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(scissor[3] as f32) as usize;
    let total = area.abs();
    for y in min_y..max_y {
        let cy = y as f32 + 0.5;
        for x in min_x..max_x {
            let cx = x as f32 + 0.5;
            let mut w = [0.0f32; 3];
            let mut inside = true;
            for (k, (a, b)) in edges.iter().enumerate() {
                let e = edge(*a, *b, cx, cy);
                if e < 0.0 || (e == 0.0 && !rules[k]) {
                    inside = false;
                    break;
                }
                w[k] = e / total;
            }
            if !inside {
                continue;
            }
            // Barycentric interpolation of color and texture coordinates.
            let mut color = [0.0f32; 4];
            let mut uv = [0.0f32; 2];
            for (vertex, weight) in v.iter().zip(w) {
                for (c, vc) in color.iter_mut().zip(vertex.color) {
                    *c += vc * weight;
                }
                uv[0] += vertex.uv[0] * weight;
                uv[1] += vertex.uv[1] * weight;
            }
            let t = sample(uv);
            let src = [
                color[0] * t[0],
                color[1] * t[1],
                color[2] * t[2],
                color[3] * t[3],
            ];
            let dst = &mut target[y * width + x];
            for c in 0..4 {
                dst[c] = src[c] + dst[c] * (1.0 - src[3]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustroke_core::{DisplayList, Tessellator, point};

    fn draw(list: &DisplayList, size: u32) -> Image {
        let atlas = TextureAtlas::new(64);
        let meshes = Tessellator::new(1.0, &atlas).tessellate(list);
        SoftwareRenderer::new().render(
            &meshes,
            &atlas,
            PhysicalSize::new(size, size),
            1.0,
            Color::BLACK,
        )
    }

    #[test]
    fn shared_edges_are_not_drawn_twice() {
        // A half-transparent square made of two triangles: the diagonal
        // must not be darker or lighter than the rest.
        let mut list = DisplayList::new();
        list.rect_filled(
            Rect::from_min_max(point(0.0, 0.0), point(16.0, 16.0)),
            0.0,
            Color::WHITE.with_alpha(0.5),
        );
        let image = draw(&list, 16);
        let inside = image.pixel(3, 9);
        assert_eq!(image.pixel(8, 8), inside);
        assert_eq!(image.pixel(5, 5), inside);
        assert_eq!(inside[3], 255);
        assert!(
            (i32::from(inside[0]) - 188).abs() <= 1,
            "50% white over black is sRGB 188: {inside:?}"
        );
    }

    #[test]
    fn clip_rectangles_cut_shapes() {
        let mut list = DisplayList::new();
        list.with_clip(
            Rect::from_min_max(point(0.0, 0.0), point(8.0, 16.0)),
            |list| {
                list.rect_filled(
                    Rect::from_min_max(point(0.0, 0.0), point(16.0, 16.0)),
                    0.0,
                    Color::WHITE,
                );
            },
        );
        let image = draw(&list, 16);
        assert_eq!(image.pixel(4, 8), [255, 255, 255, 255]);
        assert_eq!(image.pixel(12, 8), [0, 0, 0, 255]);
    }
}
