use crate::Color;

/// Which GPU texture a mesh samples from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextureId {
    /// The shared [`crate::TextureAtlas`] (glyphs and solid shapes).
    #[default]
    Atlas,
    /// An image uploaded by the application.
    User(u64),
}

/// An RGBA image in memory: sRGB colors with straight (not premultiplied)
/// alpha, rows top to bottom. This is what PNG/JPEG decoders produce.
#[derive(Clone, PartialEq, Eq)]
pub struct ColorImage {
    /// Width and height in pixels.
    pub size: [u32; 2],
    /// Pixels row by row, `size[0] * size[1]` of them.
    pub pixels: Vec<[u8; 4]>,
}

impl std::fmt::Debug for ColorImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColorImage")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl ColorImage {
    /// # Panics
    /// If `rgba` is not `width * height * 4` bytes long.
    pub fn from_rgba_unmultiplied(size: [u32; 2], rgba: &[u8]) -> Self {
        assert_eq!(
            rgba.len(),
            (size[0] * size[1] * 4) as usize,
            "wrong image data length"
        );
        Self {
            size,
            pixels: rgba.as_chunks::<4>().0.to_vec(),
        }
    }

    /// An image computed pixel by pixel: `f(x, y)` returns the color.
    pub fn from_fn(size: [u32; 2], f: impl Fn(u32, u32) -> Color) -> Self {
        let pixels = (0..size[1])
            .flat_map(|y| (0..size[0]).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y).to_srgba8())
            .collect();
        Self { size, pixels }
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.size[0]
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.size[1]
    }

    /// Pixels converted to what GPU textures expect here: sRGB-encoded
    /// with alpha premultiplied in linear space.
    pub fn to_premultiplied(&self) -> Vec<[u8; 4]> {
        self.pixels
            .iter()
            .map(|&[r, g, b, a]| {
                if a == 255 {
                    return [r, g, b, a];
                }
                let c = Color::from_srgba8(r, g, b, a);
                Color::new(c.r * c.a, c.g * c.a, c.b * c.a, c.a).to_srgba8()
            })
            .collect()
    }
}

/// Changes to user textures since the last frame, for the renderer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexturesDelta {
    /// Textures to create (or replace).
    pub set: Vec<(TextureId, ColorImage)>,
    /// Textures that are no longer used.
    pub free: Vec<TextureId>,
}

impl TexturesDelta {
    /// True if there is nothing to upload or free.
    pub fn is_empty(&self) -> bool {
        self.set.is_empty() && self.free.is_empty()
    }

    /// Adds the changes of `other` after those of `self`.
    pub fn append(&mut self, mut other: Self) {
        self.set.append(&mut other.set);
        self.free.append(&mut other.free);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiplication() {
        let img = ColorImage::from_rgba_unmultiplied([2, 1], &[255, 0, 0, 255, 255, 255, 255, 0]);
        let p = img.to_premultiplied();
        assert_eq!(p[0], [255, 0, 0, 255]);
        assert_eq!(p[1], [0, 0, 0, 0]);
    }

    #[test]
    fn from_fn_fills_row_by_row() {
        let img = ColorImage::from_fn([2, 2], |x, y| {
            if x == 1 && y == 0 {
                Color::WHITE
            } else {
                Color::BLACK
            }
        });
        assert_eq!(img.pixels[1], [255; 4]);
        assert_eq!(img.pixels[2], [0, 0, 0, 255]);
    }
}
