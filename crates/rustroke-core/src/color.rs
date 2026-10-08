/// An RGBA color in **linear** space with straight (non-premultiplied) alpha.
///
/// Colors are usually written in sRGB (e.g. from a design tool), so use
/// [`Color::from_srgb8`] / [`Color::from_srgba8`] for those. Blending and the
/// GPU pipeline work in linear space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    /// Red, linear (0..1).
    pub r: f32,
    /// Green, linear (0..1).
    pub g: f32,
    /// Blue, linear (0..1).
    pub b: f32,
    /// Opacity (0 = transparent, 1 = opaque).
    pub a: f32,
}

impl Color {
    /// Fully transparent.
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);
    /// Opaque black.
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0, 1.0);
    /// Opaque white.
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0, 1.0);

    /// Creates a color from linear components.
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Creates an opaque color from 8-bit sRGB components.
    pub fn from_srgb8(r: u8, g: u8, b: u8) -> Self {
        Self::from_srgba8(r, g, b, 255)
    }

    /// Creates a color from 8-bit sRGB components and 8-bit linear alpha.
    pub fn from_srgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            srgb8_to_linear(r),
            srgb8_to_linear(g),
            srgb8_to_linear(b),
            f32::from(a) / 255.0,
        )
    }

    /// Converts to 8-bit sRGB components with 8-bit alpha.
    pub fn to_srgba8(self) -> [u8; 4] {
        [
            linear_to_srgb8(self.r),
            linear_to_srgb8(self.g),
            linear_to_srgb8(self.b),
            unit_to_u8(self.a),
        ]
    }

    /// Linear interpolation: `self` at `t = 0`, `other` at `t = 1`.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Self::new(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
            mix(self.a, other.a),
        )
    }

    /// Returns the same color with a different alpha.
    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }
}

fn srgb8_to_linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb8(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    unit_to_u8(s)
}

fn unit_to_u8(c: f32) -> u8 {
    // Clamped to [0, 255] before the cast, so truncation cannot occur.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let v = (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb8_roundtrip_is_lossless() {
        for v in 0..=255u8 {
            let c = Color::from_srgba8(v, v, v, v);
            assert_eq!(c.to_srgba8(), [v, v, v, v]);
        }
    }

    #[test]
    fn known_srgb_midpoint() {
        // sRGB 188 is ~50% linear intensity.
        let c = Color::from_srgb8(188, 188, 188);
        assert!((c.r - 0.5).abs() < 0.005, "got {}", c.r);
    }

    #[test]
    fn lerp_mixes_and_clamps() {
        let mid = Color::BLACK.lerp(Color::WHITE, 0.5);
        assert_eq!(mid, Color::new(0.5, 0.5, 0.5, 1.0));
        assert_eq!(Color::BLACK.lerp(Color::WHITE, 2.0), Color::WHITE);
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        assert_eq!(
            Color::new(2.0, -1.0, 0.0, 1.5).to_srgba8(),
            [255, 0, 0, 255]
        );
    }
}
