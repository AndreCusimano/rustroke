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

    /// Creates an opaque color from 8-bit sRGB components. Usable in
    /// constants: `const ACCENT: Color = Color::from_srgb8(137, 180, 250);`
    pub const fn from_srgb8(r: u8, g: u8, b: u8) -> Self {
        Self::from_srgba8(r, g, b, 255)
    }

    /// Creates a color from 8-bit sRGB components and 8-bit linear alpha.
    pub const fn from_srgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            srgb8_to_linear(r),
            srgb8_to_linear(g),
            srgb8_to_linear(b),
            a as f32 / 255.0,
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

    /// Hue (0..1, red at 0), saturation and value of the sRGB color, as in
    /// color pickers, and alpha.
    pub fn to_hsva(self) -> [f32; 4] {
        let [r, g, b] = [self.r, self.g, self.b].map(linear_to_srgb);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let hue = if delta <= 0.0 {
            0.0
        } else if max == r {
            ((g - b) / delta).rem_euclid(6.0) / 6.0
        } else if max == g {
            ((b - r) / delta + 2.0) / 6.0
        } else {
            ((r - g) / delta + 4.0) / 6.0
        };
        let saturation = if max <= 0.0 { 0.0 } else { delta / max };
        [hue, saturation, max, self.a]
    }

    /// A color from hue (0..1, wraps), saturation and value (0..1) in sRGB
    /// space, and alpha: the inverse of [`Color::to_hsva`].
    pub fn from_hsva(h: f32, s: f32, v: f32, a: f32) -> Self {
        let h = h.rem_euclid(1.0) * 6.0;
        let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
        let c = v * s;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = v - c;
        Self::new(
            srgb_to_linear(r + m),
            srgb_to_linear(g + m),
            srgb_to_linear(b + m),
            a,
        )
    }
}

/// sRGB-encoded intensity (0..1) → linear.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear intensity → sRGB-encoded (0..1).
fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB-encoded 8-bit value → linear intensity. Precomputed so that the
/// sRGB constructors can be `const fn` (`powf` is not available in const).
#[rustfmt::skip]
const SRGB8_TO_LINEAR: [f32; 256] = [
    0.0, 0.000303527, 0.000607054, 0.000910581, 0.001214108, 0.001517635, 0.001821162, 0.0021246888,
    0.002428216, 0.0027317428, 0.00303527, 0.0033465358, 0.0036765074, 0.004024717, 0.004391442, 0.0047769533,
    0.0051815165, 0.0056053917, 0.006048833, 0.0065120906, 0.00699541, 0.007499032, 0.008023193, 0.008568126,
    0.009134059, 0.009721218, 0.010329823, 0.010960094, 0.011612245, 0.012286488, 0.0129830325, 0.013702083,
    0.014443844, 0.015208514, 0.015996294, 0.016807375, 0.017641954, 0.01850022, 0.019382361, 0.020288562,
    0.02121901, 0.022173885, 0.023153367, 0.024157632, 0.02518686, 0.026241222, 0.027320892, 0.02842604,
    0.029556835, 0.030713445, 0.031896032, 0.033104766, 0.034339808, 0.035601314, 0.03688945, 0.038204372,
    0.039546236, 0.0409152, 0.04231141, 0.04373503, 0.045186203, 0.046665087, 0.048171826, 0.049706567,
    0.051269457, 0.052860647, 0.054480277, 0.05612849, 0.05780543, 0.059511237, 0.061246052, 0.063010015,
    0.064803265, 0.06662594, 0.06847817, 0.070360094, 0.07227185, 0.07421357, 0.07618538, 0.07818742,
    0.08021982, 0.08228271, 0.08437621, 0.08650046, 0.08865558, 0.09084171, 0.093058966, 0.09530747,
    0.09758735, 0.099898726, 0.10224173, 0.104616486, 0.107023105, 0.10946171, 0.11193243, 0.114435375,
    0.116970666, 0.11953843, 0.122138776, 0.12477182, 0.12743768, 0.13013647, 0.13286832, 0.13563333,
    0.13843161, 0.14126329, 0.14412847, 0.14702727, 0.14995979, 0.15292615, 0.15592647, 0.15896083,
    0.16202937, 0.1651322, 0.1682694, 0.17144111, 0.1746474, 0.17788842, 0.18116425, 0.18447499,
    0.18782078, 0.19120169, 0.19461784, 0.19806932, 0.20155625, 0.20507874, 0.20863687, 0.21223076,
    0.2158605, 0.2195262, 0.22322796, 0.22696587, 0.23074006, 0.23455058, 0.23839757, 0.24228112,
    0.24620132, 0.25015828, 0.2541521, 0.25818285, 0.26225066, 0.2663556, 0.2704978, 0.2746773,
    0.27889428, 0.28314874, 0.28744084, 0.29177064, 0.29613826, 0.30054379, 0.3049873, 0.30946892,
    0.31398872, 0.31854677, 0.3231432, 0.3277781, 0.33245152, 0.33716363, 0.34191442, 0.34670407,
    0.3515326, 0.35640013, 0.3613068, 0.3662526, 0.3712377, 0.37626213, 0.38132602, 0.38642943,
    0.39157248, 0.39675522, 0.40197778, 0.4072402, 0.4125426, 0.41788507, 0.42326766, 0.4286905,
    0.43415365, 0.43965718, 0.4452012, 0.4507858, 0.45641103, 0.462077, 0.4677838, 0.47353148,
    0.47932017, 0.48514995, 0.49102086, 0.49693298, 0.5028865, 0.50888133, 0.5149177, 0.52099556,
    0.5271151, 0.5332764, 0.5394795, 0.54572445, 0.55201143, 0.5583404, 0.5647115, 0.57112485,
    0.57758045, 0.58407843, 0.59061885, 0.59720176, 0.60382736, 0.61049557, 0.6172066, 0.6239604,
    0.63075715, 0.63759685, 0.6444797, 0.65140563, 0.65837485, 0.6653873, 0.67244315, 0.6795425,
    0.6866853, 0.69387174, 0.7011019, 0.70837575, 0.7156935, 0.7230551, 0.73046076, 0.7379104,
    0.7454042, 0.7529422, 0.7605245, 0.76815116, 0.7758222, 0.7835378, 0.7912979, 0.7991027,
    0.80695224, 0.8148466, 0.82278574, 0.8307699, 0.838799, 0.8468732, 0.8549926, 0.8631572,
    0.8713671, 0.8796224, 0.8879231, 0.8962694, 0.9046612, 0.91309863, 0.92158186, 0.9301109,
    0.9386857, 0.9473065, 0.9559733, 0.9646863, 0.9734453, 0.9822506, 0.9911021, 1.0,
];

const fn srgb8_to_linear(c: u8) -> f32 {
    SRGB8_TO_LINEAR[c as usize]
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
    fn lookup_table_matches_the_srgb_formula() {
        for i in 0..=255u8 {
            let c = f64::from(i) / 255.0;
            let exact = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
            assert!((f64::from(srgb8_to_linear(i)) - exact).abs() < 1e-7, "{i}");
        }
        const ACCENT: Color = Color::from_srgb8(137, 180, 250);
        assert_eq!(ACCENT, Color::from_srgba8(137, 180, 250, 255));
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

    #[test]
    fn hsva_roundtrips() {
        for c in [
            Color::from_srgb8(255, 0, 0),
            Color::from_srgb8(18, 200, 77),
            Color::from_srgba8(137, 180, 250, 128),
            Color::WHITE,
            Color::BLACK,
        ] {
            let [h, s, v, a] = c.to_hsva();
            assert_eq!(Color::from_hsva(h, s, v, a).to_srgba8(), c.to_srgba8());
        }
        assert_eq!(
            Color::from_hsva(1.0 / 3.0, 1.0, 1.0, 1.0).to_srgba8(),
            [0, 255, 0, 255]
        );
    }
}
