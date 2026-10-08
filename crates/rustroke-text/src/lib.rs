//! Text layout and glyph rasterization, built on cosmic-text.
//!
//! [`Fonts`] lays out strings into [`Galley`]s (shaping, bidi, wrapping and
//! font fallback come from cosmic-text), rasterizes the glyphs into the
//! [`TextureAtlas`] it owns, and caches both glyphs and galleys.
//!
//! The bundled *Inter* font (SIL Open Font License, see `fonts/`) is the
//! default proportional font, so text looks the same on every platform.
//! System fonts are used for monospace and as fallback (emoji, CJK, ...).

use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::Arc;

use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent, Weight,
};
use rustroke_core::{AtlasRegion, Color, Galley, GalleyRow, GlyphQuad, TextureAtlas, Vec2};

const INTER_REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");
const INTER_BOLD: &[u8] = include_bytes!("../fonts/Inter-Bold.ttf");
const PROPORTIONAL_FAMILY: &str = "Inter";

/// Initial atlas side; it doubles when full, up to [`MAX_ATLAS_SIZE`].
const INITIAL_ATLAS_SIZE: u32 = 1024;
const MAX_ATLAS_SIZE: u32 = 8192;

/// Which typeface to use.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontFamily {
    /// The bundled proportional font (Inter).
    #[default]
    Proportional,
    /// The platform's monospace font.
    Monospace,
    /// Any installed font, by family name.
    Name(String),
}

/// How text should look: typeface, size and weight.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// Typeface.
    pub family: FontFamily,
    /// Font size in logical points.
    pub size: f32,
    /// Use the bold weight.
    pub bold: bool,
    /// Distance between baselines, as a multiple of `size`.
    pub line_height: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::proportional(14.0)
    }
}

impl TextStyle {
    /// The bundled proportional font (Inter) at `size` points.
    pub fn proportional(size: f32) -> Self {
        Self {
            family: FontFamily::Proportional,
            size,
            bold: false,
            line_height: 1.3,
        }
    }

    /// The platform's monospace font at `size` points.
    pub fn monospace(size: f32) -> Self {
        Self {
            family: FontFamily::Monospace,
            ..Self::proportional(size)
        }
    }

    /// The same style, bold.
    pub fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    fn hash_into(&self, state: &mut impl Hasher) {
        self.family.hash(state);
        self.size.to_bits().hash(state);
        self.bold.hash(state);
        self.line_height.to_bits().hash(state);
    }
}

/// A rasterized glyph stored in the atlas. `None` in the cache means the
/// glyph has no pixels (e.g. a space).
#[derive(Clone, Copy, Debug)]
struct CachedGlyph {
    region: AtlasRegion,
    /// Offset of the bitmap's top-left from the glyph's pen position.
    left: i32,
    top: i32,
    colored: bool,
}

/// Font database, glyph atlas and layout caches. Create one per app and
/// reuse it: creating it loads system fonts, which can take a while.
pub struct Fonts {
    system: FontSystem,
    swash: SwashCache,
    atlas: TextureAtlas,
    glyphs: HashMap<CacheKey, Option<CachedGlyph>>,
    /// Galleys by content hash, with the frame they were last used in.
    galleys: HashMap<u64, (Arc<Galley>, u64)>,
    frame: u64,
    hasher: std::hash::RandomState,
}

impl std::fmt::Debug for Fonts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fonts")
            .field("atlas_size", &self.atlas.size())
            .field("cached_glyphs", &self.glyphs.len())
            .field("cached_galleys", &self.galleys.len())
            .finish_non_exhaustive()
    }
}

impl Default for Fonts {
    fn default() -> Self {
        Self::new()
    }
}

impl Fonts {
    /// Bundled fonts plus all system fonts (for monospace and fallback).
    pub fn new() -> Self {
        let mut system = FontSystem::new();
        let db = system.db_mut();
        load_bundled(db);
        db.set_monospace_family(platform_monospace());
        db.set_sans_serif_family(PROPORTIONAL_FAMILY);
        Self::with_font_system(system)
    }

    /// Only the bundled fonts: deterministic output on every machine, and
    /// fast to create. Intended for tests; missing glyphs show as boxes.
    pub fn bundled_only() -> Self {
        let mut db = cosmic_text::fontdb::Database::new();
        load_bundled(&mut db);
        db.set_sans_serif_family(PROPORTIONAL_FAMILY);
        db.set_monospace_family(PROPORTIONAL_FAMILY);
        Self::with_font_system(FontSystem::new_with_locale_and_db("en-US".to_owned(), db))
    }

    fn with_font_system(system: FontSystem) -> Self {
        Self {
            system,
            swash: SwashCache::new(),
            atlas: TextureAtlas::new(INITIAL_ATLAS_SIZE),
            glyphs: HashMap::new(),
            galleys: HashMap::new(),
            frame: 0,
            hasher: std::hash::RandomState::new(),
        }
    }

    /// The glyph atlas (shared with images of solid shapes).
    pub fn atlas(&self) -> &TextureAtlas {
        &self.atlas
    }

    /// Mutable access to the glyph atlas, e.g. for the renderer to upload changes.
    pub fn atlas_mut(&mut self) -> &mut TextureAtlas {
        &mut self.atlas
    }

    /// Marks the end of a frame: galleys not used during it are dropped.
    pub fn end_frame(&mut self) {
        let frame = self.frame;
        self.galleys.retain(|_, (_, last_used)| *last_used == frame);
        self.frame += 1;
    }

    /// Lays out `text`, wrapping lines longer than `wrap_width` points
    /// (`None`: only explicit newlines break lines), and rasterizes its
    /// glyphs for `pixels_per_point`. Results are cached, so calling this
    /// every frame with the same arguments is cheap.
    pub fn layout(
        &mut self,
        text: &str,
        style: &TextStyle,
        wrap_width: Option<f32>,
        pixels_per_point: f32,
    ) -> Arc<Galley> {
        let key = {
            let mut h = self.hasher.build_hasher();
            text.hash(&mut h);
            style.hash_into(&mut h);
            wrap_width.map(f32::to_bits).hash(&mut h);
            pixels_per_point.to_bits().hash(&mut h);
            h.finish()
        };
        if let Some((galley, last_used)) = self.galleys.get_mut(&key) {
            *last_used = self.frame;
            return Arc::clone(galley);
        }
        let galley = Arc::new(self.layout_uncached(text, style, wrap_width, pixels_per_point));
        self.galleys.insert(key, (Arc::clone(&galley), self.frame));
        galley
    }

    fn layout_uncached(
        &mut self,
        text: &str,
        style: &TextStyle,
        wrap_width: Option<f32>,
        pixels_per_point: f32,
    ) -> Galley {
        let line_height = style.size * style.line_height;
        let mut buffer = Buffer::new(&mut self.system, Metrics::new(style.size, line_height));
        buffer.set_size(wrap_width, None);
        let family = match &style.family {
            FontFamily::Proportional => Family::Name(PROPORTIONAL_FAMILY),
            FontFamily::Monospace => Family::Monospace,
            FontFamily::Name(name) => Family::Name(name),
        };
        let weight = if style.bold {
            Weight::BOLD
        } else {
            Weight::NORMAL
        };
        let attrs = Attrs::new().family(family).weight(weight);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.system, false);

        // cosmic-text numbers bytes from the start of each paragraph
        // (line separated by '\n'); cursor positions use whole-text offsets.
        let paragraph_starts: Vec<usize> = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect();

        // Collect first: rasterizing needs `self` mutably.
        let mut width: f32 = 0.0;
        let mut line_count = 0;
        let mut placed = Vec::new();
        let mut rows = Vec::new();
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            line_count += 1;
            let base = paragraph_starts
                .get(run.line_i)
                .copied()
                .unwrap_or(text.len());
            rows.push(row_carets(base, &run));
            for glyph in run.glyphs {
                // Offsets passed to `physical` are in pixels, after scaling.
                let physical =
                    glyph.physical((0.0, run.line_y * pixels_per_point), pixels_per_point);
                placed.push(physical);
            }
        }
        let line_count = line_count.max(1);
        if rows.is_empty() {
            rows.push(GalleyRow {
                top: 0.0,
                height: line_height,
                carets: vec![(0, 0.0)],
            });
        }

        let glyphs = placed
            .into_iter()
            .filter_map(|p| {
                let cached = self.glyph(p.cache_key)?;
                Some(GlyphQuad {
                    offset_px: [p.x + cached.left, p.y - cached.top],
                    region: cached.region,
                    colored: cached.colored,
                })
            })
            .collect();

        Galley {
            size: Vec2::new(width, line_count as f32 * line_height),
            line_count,
            pixels_per_point,
            glyphs,
            rows,
        }
    }

    /// Returns the cached glyph, rasterizing it into the atlas on first use.
    fn glyph(&mut self, key: CacheKey) -> Option<CachedGlyph> {
        if let Some(cached) = self.glyphs.get(&key) {
            return *cached;
        }
        let cached = self.rasterize(key);
        self.glyphs.insert(key, cached);
        cached
    }

    fn rasterize(&mut self, key: CacheKey) -> Option<CachedGlyph> {
        let image = self.swash.get_image_uncached(&mut self.system, key)?;
        let (w, h) = (image.placement.width, image.placement.height);
        if w == 0 || h == 0 {
            return None;
        }
        let (texels, colored) = match image.content {
            SwashContent::Mask => (
                image.data.iter().map(|&a| coverage_texel(a)).collect(),
                false,
            ),
            SwashContent::Color => (
                image
                    .data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| premultiply_srgba(c[0], c[1], c[2], c[3]))
                    .collect::<Vec<_>>(),
                true,
            ),
            // Not requested (cosmic-text renders with Format::Alpha); use
            // the green channel as plain coverage just in case.
            SwashContent::SubpixelMask => (
                image
                    .data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| coverage_texel(c[1]))
                    .collect(),
                false,
            ),
        };
        let region = self.allocate(w, h)?;
        self.atlas.write(region, &texels);
        Some(CachedGlyph {
            region,
            left: image.placement.left,
            top: image.placement.top,
            colored,
        })
    }

    /// Allocates atlas space, growing the atlas when it is full. At the
    /// maximum size the atlas is cleared instead, so text may flicker for a
    /// frame while glyphs are re-rasterized.
    fn allocate(&mut self, w: u32, h: u32) -> Option<AtlasRegion> {
        loop {
            if let Some(region) = self.atlas.allocate(w, h) {
                return Some(region);
            }
            if self.atlas.size() < MAX_ATLAS_SIZE {
                self.atlas.grow();
                log::debug!("glyph atlas grown to {0}x{0}", self.atlas.size());
            } else if self.glyphs.is_empty() {
                log::warn!("glyph of {w}x{h} px does not fit in the atlas");
                return None;
            } else {
                log::warn!("glyph atlas full; clearing it");
                self.atlas.clear();
                self.glyphs.clear();
                self.galleys.clear();
            }
        }
    }
}

/// Cursor positions of one laid out row: the start of every glyph cluster,
/// plus the end of the last one.
fn row_carets(base: usize, run: &cosmic_text::LayoutRun<'_>) -> GalleyRow {
    let mut carets: Vec<(usize, f32)> = Vec::with_capacity(run.glyphs.len() + 1);
    for glyph in run.glyphs {
        carets.push((base + glyph.start, glyph.x));
    }
    match run.glyphs.iter().max_by_key(|g| g.end) {
        Some(last) => carets.push((base + last.end, last.x + last.w)),
        None => carets.push((base, 0.0)),
    }
    // Several glyphs can share a cluster (e.g. combining marks): keep the
    // first position for each index.
    carets.sort_by_key(|c| c.0);
    carets.dedup_by_key(|c| c.0);
    GalleyRow {
        top: run.line_top,
        height: run.line_height,
        carets,
    }
}

fn load_bundled(db: &mut cosmic_text::fontdb::Database) {
    db.load_font_data(INTER_REGULAR.to_vec());
    db.load_font_data(INTER_BOLD.to_vec());
}

fn platform_monospace() -> &'static str {
    if cfg!(target_os = "macos") {
        "Menlo"
    } else if cfg!(target_os = "windows") {
        "Consolas"
    } else {
        "DejaVu Sans Mono"
    }
}

/// A white texel with coverage `a`, premultiplied. The atlas is sRGB, so
/// the color channels are encoded such that the GPU decodes them back to
/// exactly `a` in linear space.
fn coverage_texel(a: u8) -> [u8; 4] {
    let c = Color::new(f32::from(a) / 255.0, 0.0, 0.0, 1.0).to_srgba8()[0];
    [c, c, c, a]
}

/// Converts a straight-alpha sRGB pixel to premultiplied (in linear space)
/// sRGB, as the atlas expects.
fn premultiply_srgba(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    let c = Color::from_srgba8(r, g, b, a);
    Color::new(c.r * c.a, c.g * c.a, c.b * c.a, c.a).to_srgba8()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonts() -> Fonts {
        Fonts::bundled_only()
    }

    #[test]
    fn longer_text_is_wider() {
        let mut fonts = fonts();
        let style = TextStyle::proportional(16.0);
        let short = fonts.layout("Hello", &style, None, 1.0);
        let long = fonts.layout("Hello, world", &style, None, 1.0);
        assert!(short.size.x > 20.0, "{:?}", short.size);
        assert!(long.size.x > short.size.x);
        assert_eq!(short.size.y, long.size.y);
        assert_eq!(short.line_count, 1);
        // One quad per visible glyph: no quad for the space.
        assert_eq!(long.glyphs.len(), "Hello,world".len());
    }

    #[test]
    fn empty_text_has_one_line_of_height() {
        let mut fonts = fonts();
        let style = TextStyle::proportional(10.0);
        let galley = fonts.layout("", &style, None, 1.0);
        assert_eq!(galley.size, Vec2::new(0.0, 13.0));
        assert_eq!(galley.line_count, 1);
        assert!(galley.glyphs.is_empty());
    }

    #[test]
    fn newlines_and_wrapping_add_lines() {
        let mut fonts = fonts();
        let style = TextStyle::proportional(14.0);
        assert_eq!(fonts.layout("a\nb\nc", &style, None, 1.0).line_count, 3);

        let text = "the quick brown fox jumps over the lazy dog";
        let unwrapped = fonts.layout(text, &style, None, 1.0);
        let wrapped = fonts.layout(text, &style, Some(100.0), 1.0);
        assert!(wrapped.line_count >= 3, "{}", wrapped.line_count);
        assert!(wrapped.size.x <= 100.0);
        assert_eq!(wrapped.glyphs.len(), unwrapped.glyphs.len());
    }

    #[test]
    fn size_is_in_points_regardless_of_scale() {
        let mut fonts = fonts();
        let style = TextStyle::proportional(14.0);
        let a = fonts.layout("Scale", &style, None, 1.0);
        let b = fonts.layout("Scale", &style, None, 2.0);
        assert!(
            (a.size.x - b.size.x).abs() < 0.5,
            "{:?} vs {:?}",
            a.size,
            b.size
        );
        // ...but glyphs are rasterized at twice the resolution.
        let height = |g: &Galley| g.glyphs.iter().map(|q| q.region.height).max().unwrap();
        assert!(height(&b) >= 2 * height(&a) - 1);
    }

    #[test]
    fn bold_is_wider() {
        let mut fonts = fonts();
        let regular = fonts.layout("Bold text", &TextStyle::proportional(16.0), None, 1.0);
        let bold = fonts.layout(
            "Bold text",
            &TextStyle::proportional(16.0).bold(),
            None,
            1.0,
        );
        assert!(bold.size.x > regular.size.x);
    }

    #[test]
    fn galleys_and_glyphs_are_cached() {
        let mut fonts = fonts();
        let style = TextStyle::proportional(14.0);
        let a = fonts.layout("cache me", &style, None, 1.0);
        let b = fonts.layout("cache me", &style, None, 1.0);
        assert!(Arc::ptr_eq(&a, &b));

        // The same glyphs at the same sub-pixel positions in another string
        // reuse the atlas: no new rasterization.
        let cached_glyphs = fonts.glyphs.len();
        let c = fonts.layout("cache", &style, None, 1.0);
        assert_eq!(fonts.glyphs.len(), cached_glyphs);
        assert_eq!(c.glyphs[..], a.glyphs[..5]);

        // Unused galleys are evicted at the end of the next frame.
        fonts.end_frame();
        fonts.end_frame();
        let d = fonts.layout("cache me", &style, None, 1.0);
        assert!(!Arc::ptr_eq(&a, &d));
    }

    #[test]
    fn atlas_grows_when_full() {
        let mut fonts = fonts();
        let start = fonts.atlas().size();
        let style = TextStyle::proportional(120.0);
        let text: String = ('A'..='Z').chain('a'..='z').collect();
        fonts.layout(&text, &style, None, 2.0);
        assert!(fonts.atlas().size() > start);
    }

    #[test]
    fn rows_have_cursor_positions_with_whole_text_offsets() {
        let mut fonts = fonts();
        let style = TextStyle::proportional(14.0);
        let g = fonts.layout("ab\ncd\n", &style, None, 1.0);
        assert_eq!(g.rows.len(), 3);
        let indices = |r: usize| g.rows[r].carets.iter().map(|c| c.0).collect::<Vec<_>>();
        assert_eq!(indices(0), [0, 1, 2]);
        assert_eq!(indices(1), [3, 4, 5]);
        // The empty last line still has a cursor position.
        assert_eq!(indices(2), [6]);
        assert!(g.rows[1].top > g.rows[0].top);
        // x grows along the row.
        let xs: Vec<f32> = g.rows[0].carets.iter().map(|c| c.1).collect();
        assert!(xs[0] == 0.0 && xs[1] > 0.0 && xs[2] > xs[1]);

        // Multi-byte characters: positions are byte offsets on char boundaries.
        let g = fonts.layout("è€x", &style, None, 1.0);
        let idx: Vec<usize> = g.rows[0].carets.iter().map(|c| c.0).collect();
        assert_eq!(idx, [0, 2, 5, 6]);
        assert_eq!(
            fonts.layout("", &style, None, 1.0).rows[0].carets,
            [(0, 0.0)]
        );
    }

    #[test]
    fn coverage_texels_decode_to_linear_coverage() {
        assert_eq!(coverage_texel(0), [0, 0, 0, 0]);
        assert_eq!(coverage_texel(255), [255; 4]);
        // 50% coverage must be stored as sRGB 188 (≈ linear 0.5).
        assert_eq!(coverage_texel(128), [188, 188, 188, 128]);
    }
}
