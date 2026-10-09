use crate::{AtlasRegion, Color, Point, Rect, Vec2};

/// A block of text that has been laid out and whose glyphs have been
/// rasterized into a [`crate::TextureAtlas`]. Produced by the text crate,
/// drawn with [`crate::Shape::Text`].
///
/// Glyph positions are stored in **physical pixels** relative to the galley
/// origin, so text stays pixel-aligned (crisp) once the origin is snapped
/// to the pixel grid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Galley {
    /// Layout size in logical points: width of the widest line and total
    /// height of all lines. Use this to measure text.
    pub size: Vec2,
    /// Number of laid out lines (at least 1, even for empty text).
    pub line_count: usize,
    /// Scale the glyphs were rasterized for.
    pub pixels_per_point: f32,
    /// Rasterized glyphs to draw, in no particular order.
    pub glyphs: Vec<GlyphQuad>,
    /// Visual lines, top to bottom, with cursor positions. Always at least
    /// one (an empty text has one empty row).
    pub rows: Vec<GalleyRow>,
    /// Backgrounds, underlines and strike-through lines of rich text.
    pub decorations: Vec<GalleyDecoration>,
    /// Where each section of rich text is, one rectangle per row it
    /// covers, relative to the origin: `(section index, area)`. Used to
    /// find links under the pointer.
    pub sections: Vec<(usize, Rect)>,
}

/// A rectangle drawn with a galley: a text background (behind the
/// glyphs) or an underline or strike-through line (over them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GalleyDecoration {
    /// Relative to the galley origin, in points.
    pub rect: Rect,
    /// `None`: the color the galley is drawn with.
    pub color: Option<Color>,
    /// Drawn before the glyphs (backgrounds).
    pub behind: bool,
}

/// One visual line of a [`Galley`] (a paragraph may wrap into several).
#[derive(Clone, Debug, PartialEq)]
pub struct GalleyRow {
    /// Top of the row, in points from the galley origin.
    pub top: f32,
    /// Row height in points.
    pub height: f32,
    /// Where a text cursor can stand in this row: `(byte index in the
    /// text, x in points)`, sorted by index. The last entry is the end of
    /// the row. Indices are always on character (cluster) boundaries.
    pub carets: Vec<(usize, f32)>,
}

impl GalleyRow {
    /// Index of the first cursor position in the row.
    pub fn start(&self) -> usize {
        self.carets.first().map_or(0, |c| c.0)
    }

    /// Index of the last cursor position in the row.
    pub fn end(&self) -> usize {
        self.carets.last().map_or(0, |c| c.0)
    }

    /// The cursor position closest to `x`.
    pub fn index_at_x(&self, x: f32) -> usize {
        self.carets
            .iter()
            .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
            .map_or(0, |c| c.0)
    }

    /// The x of the cursor at `index` (or the nearest one before it).
    pub fn x_of(&self, index: usize) -> f32 {
        self.carets
            .iter()
            .rev()
            .find(|c| c.0 <= index)
            .or(self.carets.first())
            .map_or(0.0, |c| c.1)
    }
}

/// One rasterized glyph of a [`Galley`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphQuad {
    /// Top-left of the glyph bitmap, in physical pixels from the galley origin.
    pub offset_px: [i32; 2],
    /// Where the bitmap lives in the atlas (its size is the quad size).
    pub region: AtlasRegion,
    /// Color glyph (e.g. emoji): drawn with its own colors instead of the
    /// text color.
    pub colored: bool,
    /// Its own color (rich text), instead of the color the galley is drawn
    /// with; that color's alpha still applies (e.g. to fade disabled text).
    pub color: Option<Color>,
}

impl Galley {
    /// The area covered by glyph bitmaps, relative to the origin, in points.
    /// May extend slightly beyond [`Galley::size`] (e.g. italic overhang).
    pub fn ink_rect(&self) -> Rect {
        let ppp = self.pixels_per_point;
        self.glyphs.iter().fold(Rect::NOTHING, |acc, g| {
            let min = Point::new(g.offset_px[0] as f32 / ppp, g.offset_px[1] as f32 / ppp);
            let size = Vec2::new(g.region.width as f32 / ppp, g.region.height as f32 / ppp);
            acc.union(Rect::from_min_size(min, size))
        })
    }

    /// The row a cursor at `index` is shown in. At a wrap point the cursor
    /// belongs to the start of the next row.
    pub fn row_of(&self, index: usize) -> usize {
        let last = self.rows.len().saturating_sub(1);
        (0..self.rows.len())
            .find(|&r| {
                index < self.rows[r].end()
                    || (r < last && index == self.rows[r].end() && self.rows[r + 1].start() > index)
            })
            .unwrap_or(last)
    }

    /// Where to draw a text cursor at byte `index`: a zero-width rect as
    /// tall as the row, relative to the galley origin.
    pub fn cursor_rect(&self, index: usize) -> Rect {
        let Some(row) = self.rows.get(self.row_of(index)) else {
            return Rect::from_min_size(Point::ZERO, Vec2::new(0.0, self.size.y));
        };
        Rect::from_min_size(
            Point::new(row.x_of(index), row.top),
            Vec2::new(0.0, row.height),
        )
    }

    /// The cursor position nearest to `pos` (relative to the origin).
    pub fn index_at(&self, pos: Point) -> usize {
        let row = self
            .rows
            .iter()
            .position(|r| pos.y < r.top + r.height)
            .unwrap_or(self.rows.len().saturating_sub(1));
        self.rows.get(row).map_or(0, |r| r.index_at_x(pos.x))
    }

    /// Rectangles covering the text between two byte indices (in any
    /// order), relative to the origin, for drawing selections: one per row,
    /// or more where right-to-left and left-to-right text mix.
    pub fn selection_rects(&self, a: usize, b: usize) -> Vec<Rect> {
        let (start, end) = (a.min(b), a.max(b));
        if start == end {
            return Vec::new();
        }
        let mut rects = Vec::new();
        for r in self
            .rows
            .iter()
            .filter(|r| r.start() <= end && start <= r.end())
        {
            // Every character between two cursor positions inside the
            // range is selected, wherever it is drawn.
            let mut spans: Vec<(f32, f32)> = r
                .carets
                .windows(2)
                .filter(|w| w[0].0 >= start && w[1].0 <= end)
                .map(|w| (w[0].1.min(w[1].1), w[0].1.max(w[1].1)))
                .collect();
            // A selection continuing past the row end (newline): show a
            // little space so empty lines are visibly selected.
            if end > r.end() {
                let x = r.x_of(r.end());
                spans.push((x, x + 4.0));
            }
            spans.sort_by(|p, q| p.0.total_cmp(&q.0));
            let mut merged: Vec<(f32, f32)> = Vec::new();
            for (x0, x1) in spans {
                match merged.last_mut() {
                    Some(last) if x0 <= last.1 + 0.5 => last.1 = last.1.max(x1),
                    _ => merged.push((x0, x1)),
                }
            }
            rects.extend(
                merged
                    .into_iter()
                    .filter(|(x0, x1)| x1 > x0)
                    .map(|(x0, x1)| {
                        Rect::from_min_max(Point::new(x0, r.top), Point::new(x1, r.top + r.height))
                    }),
            );
        }
        rects
    }

    /// The rich-text section at `pos` (relative to the origin), if any.
    pub fn section_at(&self, pos: Point) -> Option<usize> {
        self.sections
            .iter()
            .find(|(_, r)| r.contains(pos))
            .map(|(i, _)| *i)
    }

    /// Everything this galley may draw on, relative to its origin.
    pub fn bounding_rect(&self) -> Rect {
        Rect::from_min_size(Point::ZERO, self.size).union(self.ink_rect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "ab cd\nef" laid out as rows "ab " / "cd" (wrapped) / "ef",
    /// 10 points per character, rows 20 points tall.
    fn galley() -> Galley {
        let row = |top: f32, carets: &[(usize, f32)]| GalleyRow {
            top,
            height: 20.0,
            carets: carets.to_vec(),
        };
        Galley {
            size: Vec2::new(30.0, 60.0),
            line_count: 3,
            pixels_per_point: 1.0,
            glyphs: Vec::new(),
            rows: vec![
                row(0.0, &[(0, 0.0), (1, 10.0), (2, 20.0), (3, 30.0)]),
                row(20.0, &[(3, 0.0), (4, 10.0), (5, 20.0)]),
                row(40.0, &[(6, 0.0), (7, 10.0), (8, 20.0)]),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn cursor_rows_and_positions() {
        let g = galley();
        assert_eq!(g.row_of(0), 0);
        assert_eq!(g.row_of(2), 0);
        // Wrap point: the cursor goes to the start of the next row.
        assert_eq!(g.row_of(3), 1);
        // End of a paragraph stays on its row.
        assert_eq!(g.row_of(5), 1);
        assert_eq!(g.row_of(6), 2);
        assert_eq!(g.row_of(8), 2);
        assert_eq!(g.cursor_rect(4).min, Point::new(10.0, 20.0));
        assert_eq!(g.cursor_rect(8).min, Point::new(20.0, 40.0));
    }

    #[test]
    fn hit_testing_picks_the_nearest_caret() {
        let g = galley();
        assert_eq!(g.index_at(Point::new(14.0, 5.0)), 1);
        assert_eq!(g.index_at(Point::new(16.0, 5.0)), 2);
        assert_eq!(g.index_at(Point::new(100.0, 25.0)), 5);
        assert_eq!(g.index_at(Point::new(-5.0, 500.0)), 6);
    }

    #[test]
    fn selection_spans_rows() {
        let g = galley();
        let rects = g.selection_rects(7, 1);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].min.x, 10.0);
        assert_eq!(rects[2].max.x, 10.0);
        assert!(g.selection_rects(4, 4).is_empty());
    }
}
