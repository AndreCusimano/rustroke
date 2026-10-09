//! SVG icons, rasterized on demand into the glyph atlas at the size and
//! screen density they are drawn at.

use std::sync::Arc;

use resvg::{tiny_skia, usvg};
use rustroke_core::{Galley, GalleyRow, GlyphQuad, Vec2};

use crate::{Fonts, coverage_texel, premultiply_srgba};

/// The color that marks the accent parts of a two-tone icon in its SVG
/// source (`#1E6FFF`). Everything else is the line color.
pub const ICON_ACCENT_SOURCE_COLOR: [u8; 3] = [0x1E, 0x6F, 0xFF];

/// An icon loaded with [`Fonts::add_svg_icon`] (or its variants).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IconId(u32);

/// An SVG that could not be loaded.
#[derive(Debug)]
pub struct IconError(usvg::Error);

impl std::fmt::Display for IconError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid SVG icon: {}", self.0)
    }
}

impl std::error::Error for IconError {}

/// How an icon's colors are used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IconMode {
    /// Recolored: line parts and accent parts take colors from the theme.
    TwoTone,
    /// Drawn with the colors of the SVG.
    Original,
}

pub(crate) struct IconSource {
    light: usvg::Tree,
    dark: Option<usvg::Tree>,
    mode: IconMode,
}

/// Which colors a layer of a rasterized icon takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconLayer {
    /// The main strokes and shapes: draw with the text color.
    Line,
    /// Parts drawn in the accent color in the SVG: draw with the theme's
    /// accent color.
    Accent,
    /// An icon with its own colors: draw with white (or a tint to fade
    /// it).
    Original,
}

/// An icon rasterized for one size and screen density: one or two
/// layers, each a [`Galley`] drawn like text (pixel-aligned) with the
/// color its [`IconLayer`] asks for.
#[derive(Clone, Debug)]
pub struct RasterizedIcon {
    /// Size in points.
    pub size: Vec2,
    /// The layers, back to front. Empty layers are left out.
    pub layers: Vec<(IconLayer, Arc<Galley>)>,
}

/// Rasterized icons, by icon, pixel size and theme.
pub(crate) type IconCache = std::collections::HashMap<(IconId, u32, bool), RasterizedIcon>;

fn parse(svg: &[u8]) -> Result<usvg::Tree, IconError> {
    usvg::Tree::from_data(svg, &usvg::Options::default()).map_err(IconError)
}

impl Fonts {
    fn push_icon(&mut self, source: IconSource) -> IconId {
        self.icons.push(source);
        IconId(self.icons.len() as u32 - 1)
    }

    /// Loads a two-tone SVG icon: its parts in [`ICON_ACCENT_SOURCE_COLOR`]
    /// are drawn with the theme's accent color, everything else (draw it in
    /// black) with the text color. So one file works with light and dark
    /// themes and in every widget state.
    pub fn add_svg_icon(&mut self, svg: &[u8]) -> Result<IconId, IconError> {
        let light = parse(svg)?;
        Ok(self.push_icon(IconSource {
            light,
            dark: None,
            mode: IconMode::TwoTone,
        }))
    }

    /// Like [`Fonts::add_svg_icon`], with a separate drawing for dark
    /// themes, for the few icons that don't work by recoloring alone.
    pub fn add_svg_icon_themed(&mut self, light: &[u8], dark: &[u8]) -> Result<IconId, IconError> {
        let (light, dark) = (parse(light)?, parse(dark)?);
        Ok(self.push_icon(IconSource {
            light,
            dark: Some(dark),
            mode: IconMode::TwoTone,
        }))
    }

    /// Loads an SVG icon drawn with its own colors (logos, colorful
    /// illustrations).
    pub fn add_svg_icon_colored(&mut self, svg: &[u8]) -> Result<IconId, IconError> {
        let light = parse(svg)?;
        Ok(self.push_icon(IconSource {
            light,
            dark: None,
            mode: IconMode::Original,
        }))
    }

    /// The icon rasterized to fit a `size` × `size` point square (keeping
    /// its proportions) at `pixels_per_point`, using the dark variant if
    /// `dark` and there is one. Cached, so calling it every frame is cheap.
    pub fn icon(
        &mut self,
        id: IconId,
        size: f32,
        pixels_per_point: f32,
        dark: bool,
    ) -> Option<RasterizedIcon> {
        let source = self.icons.get(id.0 as usize)?;
        let dark = dark && source.dark.is_some();
        let px = (size * pixels_per_point).round().max(1.0) as u32;
        if let Some(icon) = self.icon_cache.get(&(id, px, dark)) {
            return Some(icon.clone());
        }
        let source = &self.icons[id.0 as usize];
        let tree = match (&source.dark, dark) {
            (Some(tree), true) => tree,
            _ => &source.light,
        };
        let mode = source.mode;
        let tree_size = tree.size();
        let scale = px as f32 / tree_size.width().max(tree_size.height());
        let w = (tree_size.width() * scale).round().max(1.0) as u32;
        let h = (tree_size.height() * scale).round().max(1.0) as u32;
        let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
        resvg::render(
            tree,
            tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );

        let pixels = pixmap.data().as_chunks::<4>().0;
        let layers: Vec<(IconLayer, Vec<[u8; 4]>)> = match mode {
            IconMode::Original => vec![(
                IconLayer::Original,
                pixels
                    .iter()
                    .map(|&p| {
                        let [r, g, b, a] = unpremultiply(p);
                        premultiply_srgba(r, g, b, a)
                    })
                    .collect(),
            )],
            IconMode::TwoTone => {
                let (line, accent): (Vec<_>, Vec<_>) = pixels
                    .iter()
                    .map(|&p| {
                        let [r, g, b, a] = unpremultiply(p);
                        let t = accent_weight([r, g, b]);
                        let accent = (f32::from(a) * t).round() as u8;
                        (coverage_texel(a - accent), coverage_texel(accent))
                    })
                    .unzip();
                vec![(IconLayer::Line, line), (IconLayer::Accent, accent)]
            }
        };

        let mut out = Vec::new();
        for (layer, texels) in layers {
            if texels.iter().all(|t| t[3] == 0) {
                continue;
            }
            let region = self.allocate(w, h)?;
            self.atlas.write(region, &texels);
            let galley = Galley {
                size: Vec2::new(w as f32, h as f32) / pixels_per_point,
                line_count: 1,
                pixels_per_point,
                glyphs: vec![GlyphQuad {
                    offset_px: [0, 0],
                    region,
                    colored: layer == IconLayer::Original,
                    color: None,
                }],
                rows: vec![GalleyRow {
                    top: 0.0,
                    height: h as f32 / pixels_per_point,
                    carets: vec![(0, 0.0)],
                }],
                ..Default::default()
            };
            out.push((layer, Arc::new(galley)));
        }
        let icon = RasterizedIcon {
            size: Vec2::new(w as f32, h as f32) / pixels_per_point,
            layers: out,
        };
        self.icon_cache.insert((id, px, dark), icon.clone());
        Some(icon)
    }
}

/// Straight-alpha color of a premultiplied tiny-skia pixel.
fn unpremultiply([r, g, b, a]: [u8; 4]) -> [u8; 4] {
    if a == 0 {
        return [0; 4];
    }
    let un = |c: u8| ((u32::from(c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8;
    [un(r), un(g), un(b), a]
}

/// How much a color is the accent color (1) rather than the line color,
/// black (0). Anti-aliased edges between the two get partial weights.
fn accent_weight(rgb: [u8; 3]) -> f32 {
    let accent = ICON_ACCENT_SOURCE_COLOR.map(f32::from);
    let c = rgb.map(f32::from);
    let dot: f32 = (0..3).map(|i| c[i] * accent[i]).sum();
    let len2: f32 = accent.iter().map(|a| a * a).sum();
    // Only colors close to the accent's hue count (grays are lines).
    let projected = accent.map(|a| a * dot / len2);
    let off: f32 = (0..3).map(|i| (c[i] - projected[i]).abs()).sum::<f32>() / 255.0;
    ((dot / len2) * (1.0 - off * 2.0)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICON: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">
        <rect x="1" y="1" width="6" height="14" fill="#000"/>
        <rect x="9" y="1" width="6" height="14" fill="#1E6FFF"/>
    </svg>"##;

    #[test]
    fn two_tone_icons_split_into_line_and_accent_layers() {
        let mut fonts = Fonts::bundled_only();
        let id = fonts.add_svg_icon(ICON).unwrap();
        let icon = fonts.icon(id, 16.0, 2.0, false).unwrap();
        assert_eq!(icon.size, Vec2::new(16.0, 16.0));
        let layers: Vec<_> = icon.layers.iter().map(|(l, _)| *l).collect();
        assert_eq!(layers, [IconLayer::Line, IconLayer::Accent]);
        let quad = icon.layers[0].1.glyphs[0];
        assert_eq!((quad.region.width, quad.region.height), (32, 32), "2x");

        // The left half is line, the right half accent.
        let atlas = fonts.atlas();
        let texel = |layer: usize, x: u32, y: u32| {
            let r = icon.layers[layer].1.glyphs[0].region;
            atlas.pixels()[((r.y + y) * atlas.size() + r.x + x) as usize][3]
        };
        assert_eq!(texel(0, 8, 16), 255);
        assert_eq!(texel(1, 8, 16), 0);
        assert_eq!(texel(0, 24, 16), 0);
        assert_eq!(texel(1, 24, 16), 255);

        // Cached: same regions on the next call.
        let again = fonts.icon(id, 16.0, 2.0, false).unwrap();
        assert_eq!(again.layers[0].1.glyphs[0].region, quad.region);
    }

    #[test]
    fn colored_and_themed_icons() {
        let mut fonts = Fonts::bundled_only();
        let colored = fonts.add_svg_icon_colored(ICON).unwrap();
        let icon = fonts.icon(colored, 8.0, 1.0, false).unwrap();
        assert_eq!(icon.layers.len(), 1);
        assert_eq!(icon.layers[0].0, IconLayer::Original);
        assert!(icon.layers[0].1.glyphs[0].colored);

        let dark_svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 8">
            <rect width="16" height="8"/></svg>"##;
        let themed = fonts.add_svg_icon_themed(ICON, dark_svg).unwrap();
        let light = fonts.icon(themed, 16.0, 1.0, false).unwrap();
        let dark = fonts.icon(themed, 16.0, 1.0, true).unwrap();
        assert_eq!(light.size, Vec2::new(16.0, 16.0));
        assert_eq!(dark.size, Vec2::new(16.0, 8.0), "the dark drawing");
        assert!(fonts.add_svg_icon(b"not svg").is_err());
    }

    #[test]
    fn accent_weight_separates_colors() {
        assert_eq!(accent_weight([0, 0, 0]), 0.0);
        assert_eq!(accent_weight([0x1E, 0x6F, 0xFF]), 1.0);
        assert_eq!(accent_weight([128, 128, 128]), 0.0, "grays are lines");
        let half = accent_weight([0x0F, 0x37, 0x80]);
        assert!((0.3..0.7).contains(&half), "{half}");
    }
}
