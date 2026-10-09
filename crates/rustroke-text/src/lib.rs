//! Text layout and glyph rasterization, built on cosmic-text.
//!
//! [`Fonts`] lays out strings into [`Galley`]s (shaping, bidi, wrapping and
//! font fallback come from cosmic-text), rasterizes the glyphs into the
//! [`TextureAtlas`] it owns, and caches both glyphs and galleys.
//!
//! The bundled *Inter* font (SIL Open Font License, see `fonts/`) is the
//! default proportional font, so text looks the same on every platform.
//! A bundled symbol font (*Rustroke Symbols*, a subset of Noto Sans Math,
//! Symbols and Symbols 2, also OFL) covers arrows, math and technical
//! symbols, shapes and dingbats (↶ ⚓ ∥ ⊥ ⌀ ✓ ★ ⚙), even without system
//! fonts.
//! System fonts are used for monospace and as fallback (emoji, CJK, ...).

mod icons;

pub use icons::{ICON_ACCENT_SOURCE_COLOR, IconError, IconId, IconLayer, RasterizedIcon};

use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::Arc;

use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, SwashCache, SwashContent,
    Weight,
};
use rustroke_core::{
    AtlasRegion, Color, Galley, GalleyDecoration, GalleyRow, GlyphQuad, Point, Rect, TextureAtlas,
    Vec2,
};

/// Inter as a variable font: every weight from 100 to 900 in one file.
const INTER: &[u8] = include_bytes!("../fonts/InterVariable.ttf");
const SYMBOLS: &[u8] = include_bytes!("../fonts/RustrokeSymbols-Regular.ttf");
const PROPORTIONAL_FAMILY: &str = "Inter Variable";

/// Families tried, in order, for [`FontFamily::System`].
const SYSTEM_FAMILIES: &[&str] = if cfg!(target_os = "macos") {
    // SF Pro; the system file (SFNS.ttf) has a hidden family name.
    &[".SF NS", "SF Pro", "SF Pro Text", "Helvetica Neue"]
} else if cfg!(target_os = "windows") {
    &["Segoe UI Variable Text", "Segoe UI"]
} else {
    &["Cantarell", "Ubuntu", "Noto Sans", "DejaVu Sans"]
};

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
    /// The platform's user interface font: SF Pro on macOS, Segoe UI on
    /// Windows, a common desktop font on Linux. Falls back to Inter when it
    /// is not available (e.g. with `Fonts::bundled_only`).
    System,
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
    /// Weight from 100 (thin) to 900 (black): 400 regular, 500 medium,
    /// 600 semibold, 700 bold. Variable fonts (the bundled Inter, SF Pro)
    /// have every weight; others use the nearest one they have.
    pub weight: u16,
    /// Bold: at least weight 700, whatever `weight` says.
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
            weight: 400,
            bold: false,
            line_height: 1.3,
        }
    }

    /// The platform's user interface font at `size` points (see
    /// [`FontFamily::System`]).
    pub fn system(size: f32) -> Self {
        Self {
            family: FontFamily::System,
            ..Self::proportional(size)
        }
    }

    /// The same style with weight `weight` (e.g. 500 for medium, 600 for
    /// semibold).
    pub fn weight(self, weight: u16) -> Self {
        Self {
            weight: weight.clamp(1, 1000),
            ..self
        }
    }

    /// The weight used for drawing: `weight`, raised to 700 if `bold`.
    pub fn effective_weight(&self) -> u16 {
        if self.bold {
            self.weight.max(700)
        } else {
            self.weight
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
        self.effective_weight().hash(state);
        self.line_height.to_bits().hash(state);
    }
}

/// How one section of rich text looks. Unset fields use the defaults of
/// the widget showing the text (its style and color).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextFormat {
    /// Typeface, size and weight; `None`: the default style.
    pub style: Option<TextStyle>,
    /// Slanted (the font's italic, or an oblique version of it).
    pub italic: bool,
    /// Text color; `None`: the default color.
    pub color: Option<Color>,
    /// Highlight behind the text.
    pub background: Option<Color>,
    /// A line under the text.
    pub underline: bool,
    /// A line through the text.
    pub strikethrough: bool,
    /// A URL the section links to (see `rustroke_widgets::Label::rich`).
    pub link: Option<String>,
}

impl TextFormat {
    /// The default format.
    pub fn new() -> Self {
        Self::default()
    }

    /// With typeface, size and weight `style`.
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// With color `color`.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Italic.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Underlined.
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Struck through.
    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// With a highlight of color `color` behind it.
    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// Linking to `url`.
    pub fn link(mut self, url: impl Into<String>) -> Self {
        self.link = Some(url.into());
        self
    }

    fn hash_into(&self, state: &mut impl Hasher) {
        match &self.style {
            Some(style) => {
                1u8.hash(state);
                style.hash_into(state);
            }
            None => 0u8.hash(state),
        }
        self.italic.hash(state);
        for c in [self.color, self.background] {
            c.map(|c| [c.r, c.g, c.b, c.a].map(f32::to_bits))
                .hash(state);
        }
        self.underline.hash(state);
        self.strikethrough.hash(state);
        self.link.hash(state);
    }
}

/// Text made of sections with different formats ("rich text"): bold
/// words, colored parts, links. Build it with [`LayoutJob::append`] and
/// lay it out with [`Fonts::layout_job`].
///
/// ```
/// use rustroke_text::{LayoutJob, TextFormat, TextStyle};
/// let mut job = LayoutJob::default();
/// job.append("Press ", TextFormat::new());
/// job.append("Save", TextFormat::new().style(TextStyle::proportional(14.0).bold()));
/// job.append(" to keep your changes.", TextFormat::new().italic());
/// assert_eq!(job.text, "Press Save to keep your changes.");
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutJob {
    /// All the text.
    pub text: String,
    /// Consecutive sections: byte ranges of `text` and their format.
    pub sections: Vec<(std::ops::Range<usize>, TextFormat)>,
}

impl LayoutJob {
    /// Text in a single format.
    pub fn simple(text: impl Into<String>, format: TextFormat) -> Self {
        let mut job = Self::default();
        job.append(&text.into(), format);
        job
    }

    /// Adds `text` in `format` at the end.
    pub fn append(&mut self, text: &str, format: TextFormat) {
        let start = self.text.len();
        self.text.push_str(text);
        self.sections.push((start..self.text.len(), format));
    }

    /// Whether any section is a link.
    pub fn has_links(&self) -> bool {
        self.sections.iter().any(|(_, f)| f.link.is_some())
    }

    fn hash_into(&self, state: &mut impl Hasher) {
        self.text.hash(state);
        for (range, format) in &self.sections {
            range.hash(state);
            format.hash_into(state);
        }
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
    icons: Vec<icons::IconSource>,
    icon_cache: icons::IconCache,
    /// What [`FontFamily::System`] means here.
    system_family: String,
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
        let system_family = SYSTEM_FAMILIES
            .iter()
            .find(|name| {
                db.faces()
                    .any(|f| f.families.iter().any(|(family, _)| family == *name))
            })
            .map_or(PROPORTIONAL_FAMILY, |name| name)
            .to_owned();
        let mut fonts = Self::with_font_system(system);
        fonts.system_family = system_family;
        fonts
    }

    /// The family [`FontFamily::System`] resolves to on this machine.
    pub fn system_family(&self) -> &str {
        &self.system_family
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
            icons: Vec::new(),
            icon_cache: HashMap::new(),
            system_family: PROPORTIONAL_FAMILY.to_owned(),
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
            0u8.hash(&mut h);
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
        let spans = [(0..text.len(), TextFormat::default())];
        let galley =
            Arc::new(self.layout_uncached(text, &spans, style, wrap_width, pixels_per_point));
        self.galleys.insert(key, (Arc::clone(&galley), self.frame));
        galley
    }

    /// Lays out rich text: like [`Fonts::layout`], with each section of
    /// `job` in its own format; sections without a style use `style`.
    /// The galley's glyphs carry the sections' colors, its decorations
    /// the backgrounds and lines, and [`Galley::sections`] where each
    /// section is (section indices are those of `job.sections`).
    pub fn layout_job(
        &mut self,
        job: &LayoutJob,
        style: &TextStyle,
        wrap_width: Option<f32>,
        pixels_per_point: f32,
    ) -> Arc<Galley> {
        let key = {
            let mut h = self.hasher.build_hasher();
            1u8.hash(&mut h);
            job.hash_into(&mut h);
            style.hash_into(&mut h);
            wrap_width.map(f32::to_bits).hash(&mut h);
            pixels_per_point.to_bits().hash(&mut h);
            h.finish()
        };
        if let Some((galley, last_used)) = self.galleys.get_mut(&key) {
            *last_used = self.frame;
            return Arc::clone(galley);
        }
        let galley = Arc::new(self.layout_uncached(
            &job.text,
            &job.sections,
            style,
            wrap_width,
            pixels_per_point,
        ));
        self.galleys.insert(key, (Arc::clone(&galley), self.frame));
        galley
    }

    fn layout_uncached(
        &mut self,
        text: &str,
        sections: &[(std::ops::Range<usize>, TextFormat)],
        style: &TextStyle,
        wrap_width: Option<f32>,
        pixels_per_point: f32,
    ) -> Galley {
        let line_height = style.size * style.line_height;
        let mut buffer = Buffer::new(&mut self.system, Metrics::new(style.size, line_height));
        buffer.set_size(wrap_width, None);
        let system_family = self.system_family.clone();
        let attrs_of = |index: usize, format| attrs_for(index, format, style, &system_family);
        let plain = TextFormat::default();
        let default_attrs = attrs_of(usize::MAX, &plain);
        let spans: Vec<(&str, Attrs<'_>)> = sections
            .iter()
            .enumerate()
            .filter(|(_, (r, _))| !r.is_empty())
            .map(|(i, (r, f))| (&text[r.clone()], attrs_of(i, f)))
            .collect();
        buffer.set_rich_text(spans, &default_attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.system, false);

        // cosmic-text numbers bytes from the start of each paragraph
        // (line separated by '\n'); cursor positions use whole-text offsets.
        let paragraph_starts: Vec<usize> = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect();

        // Collect first: rasterizing needs `self` mutably.
        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;
        let mut line_count = 0;
        let mut any_rtl = false;
        let mut placed = Vec::new();
        let mut rows = Vec::new();
        let mut decorations = Vec::new();
        let mut section_rects: Vec<(usize, Rect)> = Vec::new();
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height = height.max(run.line_top + run.line_height);
            any_rtl |= run.rtl;
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
                let format = sections.get(glyph.metadata).map(|(_, f)| f);
                placed.push((physical, format.and_then(|f| f.color)));
                let Some(format) = format else { continue };
                // Areas of the sections, merged along the row.
                let area = Rect::from_min_max(
                    Point::new(glyph.x, run.line_top),
                    Point::new(glyph.x + glyph.w, run.line_top + run.line_height),
                );
                match section_rects.last_mut() {
                    Some((i, r)) if *i == glyph.metadata && (r.min.y - area.min.y).abs() < 0.01 => {
                        *r = r.union(area);
                    }
                    _ => section_rects.push((glyph.metadata, area)),
                }
                let size = glyph.font_size;
                let thickness = (size / 14.0).max(1.0 / pixels_per_point);
                let x = (glyph.x, glyph.x + glyph.w);
                let line =
                    |y: f32| Rect::from_min_max(Point::new(x.0, y), Point::new(x.1, y + thickness));
                if let Some(bg) = format.background {
                    decorations.push(GalleyDecoration {
                        rect: area,
                        color: Some(bg),
                        behind: true,
                    });
                }
                if format.underline {
                    decorations.push(GalleyDecoration {
                        rect: line(run.line_y + size * 0.12),
                        color: format.color,
                        behind: false,
                    });
                }
                if format.strikethrough {
                    decorations.push(GalleyDecoration {
                        rect: line(run.line_y - size * 0.3),
                        color: format.color,
                        behind: false,
                    });
                }
            }
        }
        merge_decorations(&mut decorations);
        // Rich text has no line for an empty last paragraph (text ending
        // with a newline); the cursor still needs one.
        if text.ends_with('\n')
            && let Some(last) = rows.last()
            && last.end() < text.len()
        {
            let top = last.top + last.height;
            rows.push(GalleyRow {
                top,
                height: line_height,
                carets: vec![(text.len(), 0.0)],
            });
            height = height.max(top + line_height);
            line_count += 1;
        }
        let line_count = line_count.max(1);
        if rows.is_empty() {
            rows.push(GalleyRow {
                top: 0.0,
                height: line_height,
                carets: vec![(0, 0.0)],
            });
            height = line_height;
        }
        // Right-to-left paragraphs are aligned to the right of the wrap
        // width: the galley spans it, so they stay inside.
        if any_rtl && let Some(w) = wrap_width {
            width = width.max(w);
        }

        let glyphs = placed
            .into_iter()
            .filter_map(|(p, color)| {
                let cached = self.glyph(p.cache_key)?;
                Some(GlyphQuad {
                    offset_px: [p.x + cached.left, p.y - cached.top],
                    region: cached.region,
                    colored: cached.colored,
                    color,
                })
            })
            .collect();

        Galley {
            size: Vec2::new(width, height),
            line_count,
            pixels_per_point,
            glyphs,
            rows,
            decorations,
            sections: section_rects,
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
                self.icon_cache.clear();
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

/// cosmic-text attributes for section `index` in `format`, with `style`
/// for what the format leaves unset.
fn attrs_for<'a>(
    index: usize,
    format: &'a TextFormat,
    style: &'a TextStyle,
    system_family: &'a str,
) -> Attrs<'a> {
    let s = format.style.as_ref().unwrap_or(style);
    let family = match &s.family {
        FontFamily::Proportional => Family::Name(PROPORTIONAL_FAMILY),
        FontFamily::System => Family::Name(system_family),
        FontFamily::Monospace => Family::Monospace,
        FontFamily::Name(name) => Family::Name(name),
    };
    let mut attrs = Attrs::new()
        .family(family)
        .weight(Weight(s.effective_weight()))
        .metadata(index);
    if format.italic {
        attrs = attrs.style(Style::Italic);
    }
    if format.style.is_some() {
        attrs = attrs.metrics(Metrics::new(s.size, s.size * s.line_height));
    }
    attrs
}

/// Joins decorations of consecutive glyphs into one rectangle each, so
/// underlines have no seams.
fn merge_decorations(decorations: &mut Vec<GalleyDecoration>) {
    let mut merged: Vec<GalleyDecoration> = Vec::with_capacity(decorations.len());
    for d in decorations.drain(..) {
        if let Some(last) = merged.iter_mut().rev().take(3).find(|m| {
            m.behind == d.behind
                && m.color == d.color
                && m.rect.min.y == d.rect.min.y
                && m.rect.max.y == d.rect.max.y
                && (m.rect.max.x - d.rect.min.x).abs() < 0.5
        }) {
            last.rect = last.rect.union(d.rect);
        } else {
            merged.push(d);
        }
    }
    *decorations = merged;
}

fn load_bundled(db: &mut cosmic_text::fontdb::Database) {
    db.load_font_data(INTER.to_vec());
    // Found by font fallback for characters Inter doesn't have.
    db.load_font_data(SYMBOLS.to_vec());
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

    /// TXT-04: symbols render without system fonts (no repeated "tofu"
    /// box: each symbol gets its own bitmap).
    #[test]
    fn bundled_symbols_have_glyphs() {
        let mut fonts = fonts();
        let style = TextStyle::proportional(16.0);
        let symbols = "↶↷⚓∥⊥⌀⚙✓★";
        let galley = fonts.layout(symbols, &style, None, 1.0);
        assert_eq!(galley.glyphs.len(), symbols.chars().count());
        let mut regions: Vec<_> = galley
            .glyphs
            .iter()
            .map(|g| (g.region.x, g.region.y))
            .collect();
        regions.sort_unstable();
        regions.dedup();
        assert_eq!(regions.len(), symbols.chars().count(), "all distinct");
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
    fn weights_between_regular_and_bold() {
        let mut fonts = fonts();
        let width = |fonts: &mut Fonts, w: u16| {
            let style = TextStyle::proportional(16.0).weight(w);
            fonts.layout("Weight", &style, None, 1.0).size.x
        };
        let (w400, w500, w600, w700) = (
            width(&mut fonts, 400),
            width(&mut fonts, 500),
            width(&mut fonts, 600),
            width(&mut fonts, 700),
        );
        assert!(
            w400 < w500 && w500 < w600 && w600 < w700,
            "{w400} {w500} {w600} {w700}"
        );
        assert_eq!(TextStyle::proportional(16.0).bold().effective_weight(), 700);
        // Without system fonts, the system family is Inter.
        let system = fonts.layout("Weight", &TextStyle::system(16.0), None, 1.0);
        assert_eq!(system.size.x, w400);
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

    #[test]
    fn rich_text_has_colors_decorations_and_sections() {
        let mut fonts = fonts();
        let red = Color::from_srgb8(255, 0, 0);
        let mut job = LayoutJob::default();
        job.append("plain ", TextFormat::new());
        job.append("red", TextFormat::new().color(red).underline());
        job.append(
            " big",
            TextFormat::new().style(TextStyle::proportional(28.0)),
        );
        job.append(
            " link",
            TextFormat::new()
                .link("https://example.com")
                .background(red),
        );
        let style = TextStyle::proportional(14.0);
        let g = fonts.layout_job(&job, &style, None, 1.0);
        assert!(g.glyphs.iter().any(|q| q.color == Some(red)));
        assert!(g.glyphs.iter().any(|q| q.color.is_none()));
        // One underline (merged across the glyphs) and one background.
        assert_eq!(g.decorations.iter().filter(|d| !d.behind).count(), 1);
        assert_eq!(g.decorations.iter().filter(|d| d.behind).count(), 1);
        // The big section makes the line taller than plain text.
        let plain = fonts.layout("plain", &style, None, 1.0);
        assert!(g.size.y > plain.size.y * 1.5);
        // Sections can be found by position, in order along the line.
        let x_of = |i: usize| g.sections.iter().find(|(s, _)| *s == i).unwrap().1;
        assert!(x_of(0).max.x <= x_of(1).min.x + 0.5);
        let link = x_of(3);
        assert_eq!(g.section_at(link.center()), Some(3));
        assert!(job.has_links());
    }

    #[test]
    fn right_to_left_text_is_right_aligned_and_selectable() {
        let mut fonts = fonts();
        let style = TextStyle::proportional(14.0);
        let g = fonts.layout("שלום", &style, Some(300.0), 1.0);
        assert_eq!(g.size.x, 300.0, "spans the wrap width");
        let row = &g.rows[0];
        // The first character is on the right.
        assert!(row.x_of(0) > row.x_of("שלום".len()));
        let sel = g.selection_rects(0, "של".len());
        assert_eq!(sel.len(), 1);
        assert!(sel[0].width() > 1.0);
        assert!(
            sel[0].max.x > 250.0,
            "the selection is on the right: {:?}",
            sel[0]
        );
    }
}
