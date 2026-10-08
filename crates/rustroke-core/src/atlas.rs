/// A region of a [`TextureAtlas`], in texels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasRegion {
    /// Left edge, in texels.
    pub x: u32,
    /// Top edge, in texels.
    pub y: u32,
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
}

/// A square RGBA texture that many small images (glyphs, icons) are packed
/// into, so everything can be drawn with a single GPU texture.
///
/// Pixels are sRGB-encoded with **premultiplied** alpha. A small white block
/// is always reserved so solid shapes can sample plain white (see
/// [`TextureAtlas::white_uv`]).
///
/// Packing uses simple shelves (rows): fast and good enough for glyphs,
/// which have similar heights.
#[derive(Clone, Debug)]
pub struct TextureAtlas {
    size: u32,
    pixels: Vec<[u8; 4]>,
    /// Next free position on the current shelf.
    cursor_x: u32,
    cursor_y: u32,
    shelf_height: u32,
    dirty: Option<AtlasRegion>,
}

/// Empty texels kept between packed images so linear filtering does not
/// bleed neighbors into each other.
const PADDING: u32 = 1;
/// Side of the reserved white block. Its center texel is sampled, so linear
/// filtering never reaches a non-white texel.
const WHITE_SIZE: u32 = 3;

impl TextureAtlas {
    /// Creates an atlas of `size × size` texels.
    ///
    /// # Panics
    /// If `size` is too small to hold the reserved white block.
    pub fn new(size: u32) -> Self {
        assert!(
            size >= WHITE_SIZE + PADDING,
            "atlas size {size} is too small"
        );
        let mut atlas = Self {
            size,
            pixels: vec![[0; 4]; (size * size) as usize],
            cursor_x: 0,
            cursor_y: 0,
            shelf_height: 0,
            dirty: None,
        };
        let white = atlas
            .allocate(WHITE_SIZE, WHITE_SIZE)
            .expect("white block fits by the assertion above");
        atlas.write(white, &[[255; 4]; (WHITE_SIZE * WHITE_SIZE) as usize]);
        atlas
    }

    /// Width and height in texels.
    pub fn size(&self) -> u32 {
        self.size
    }

    /// All texels, row-major.
    pub fn pixels(&self) -> &[[u8; 4]] {
        &self.pixels
    }

    /// Texture coordinate of a pure white texel.
    pub fn white_uv(&self) -> [f32; 2] {
        let center = WHITE_SIZE as f32 / 2.0 / self.size as f32;
        [center, center]
    }

    /// Reserves space for a `width × height` image. Returns `None` when the
    /// atlas is full.
    pub fn allocate(&mut self, width: u32, height: u32) -> Option<AtlasRegion> {
        if width + PADDING > self.size {
            return None;
        }
        if self.cursor_x + width + PADDING > self.size {
            // Start a new shelf.
            self.cursor_y += self.shelf_height;
            self.cursor_x = 0;
            self.shelf_height = 0;
        }
        if self.cursor_y + height + PADDING > self.size {
            return None;
        }
        let region = AtlasRegion {
            x: self.cursor_x,
            y: self.cursor_y,
            width,
            height,
        };
        self.cursor_x += width + PADDING;
        self.shelf_height = self.shelf_height.max(height + PADDING);
        Some(region)
    }

    /// Doubles the width and height, keeping existing contents (and so all
    /// regions handed out so far) in place. Texture coordinates must be
    /// derived from texel positions *after* growing.
    pub fn grow(&mut self) {
        let old_size = self.size as usize;
        let new_size = self.size * 2;
        let mut pixels = vec![[0; 4]; (new_size * new_size) as usize];
        for (row, src) in self.pixels.chunks_exact(old_size).enumerate() {
            let start = row * new_size as usize;
            pixels[start..start + old_size].copy_from_slice(src);
        }
        self.pixels = pixels;
        self.size = new_size;
        // Continue below the existing shelves, now with the full new width.
        self.cursor_y += self.shelf_height;
        self.cursor_x = 0;
        self.shelf_height = 0;
        self.dirty = Some(AtlasRegion {
            x: 0,
            y: 0,
            width: new_size,
            height: new_size,
        });
    }

    /// Removes everything except the white block, keeping the size.
    pub fn clear(&mut self) {
        *self = Self::new(self.size);
    }

    /// Copies `data` (row-major, `region.width * region.height` texels)
    /// into `region`.
    ///
    /// # Panics
    /// If `data` has the wrong length or `region` lies outside the atlas.
    pub fn write(&mut self, region: AtlasRegion, data: &[[u8; 4]]) {
        assert_eq!(data.len(), (region.width * region.height) as usize);
        assert!(region.x + region.width <= self.size && region.y + region.height <= self.size);
        let width = region.width as usize;
        for (row, src) in data.chunks_exact(width.max(1)).enumerate() {
            let start = (region.y as usize + row) * self.size as usize + region.x as usize;
            self.pixels[start..start + width].copy_from_slice(src);
        }
        self.mark_dirty(region);
    }

    /// Returns the area changed since the last call, if any. The renderer
    /// uses this to upload only what changed.
    pub fn take_dirty(&mut self) -> Option<AtlasRegion> {
        self.dirty.take()
    }

    fn mark_dirty(&mut self, r: AtlasRegion) {
        self.dirty = Some(match self.dirty {
            None => r,
            Some(d) => {
                let x = d.x.min(r.x);
                let y = d.y.min(r.y);
                AtlasRegion {
                    x,
                    y,
                    width: (d.x + d.width).max(r.x + r.width) - x,
                    height: (d.y + d.height).max(r.y + r.height) - y,
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_block_is_reserved_and_dirty() {
        let mut atlas = TextureAtlas::new(64);
        let [u, v] = atlas.white_uv();
        let (x, y) = ((u * 64.0) as usize, (v * 64.0) as usize);
        assert_eq!(atlas.pixels()[y * 64 + x], [255; 4]);
        assert_eq!(
            atlas.take_dirty(),
            Some(AtlasRegion {
                x: 0,
                y: 0,
                width: 3,
                height: 3
            })
        );
        assert_eq!(atlas.take_dirty(), None);
    }

    #[test]
    fn allocations_do_not_overlap() {
        let mut atlas = TextureAtlas::new(64);
        let mut regions = Vec::new();
        while let Some(r) = atlas.allocate(10, 7) {
            regions.push(r);
        }
        assert!(regions.len() > 20, "only {} regions fit", regions.len());
        for (i, a) in regions.iter().enumerate() {
            assert!(a.x + a.width <= 64 && a.y + a.height <= 64);
            for b in &regions[i + 1..] {
                let disjoint = a.x + a.width <= b.x
                    || b.x + b.width <= a.x
                    || a.y + a.height <= b.y
                    || b.y + b.height <= a.y;
                assert!(disjoint, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn grow_keeps_contents_and_regions() {
        let mut atlas = TextureAtlas::new(16);
        let a = atlas.allocate(4, 4).unwrap();
        atlas.write(a, &[[7; 4]; 16]);
        atlas.grow();
        assert_eq!(atlas.size(), 32);
        assert_eq!(
            atlas.pixels()[(a.y as usize + 3) * 32 + a.x as usize + 3],
            [7; 4]
        );
        assert_eq!(
            atlas.take_dirty().map(|d| (d.width, d.height)),
            Some((32, 32))
        );
        // New allocations don't overlap the old ones.
        let b = atlas.allocate(20, 4).unwrap();
        assert!(b.y >= a.y + a.height);
    }

    #[test]
    fn too_large_allocation_fails() {
        let mut atlas = TextureAtlas::new(64);
        assert_eq!(atlas.allocate(64, 1), None);
        assert_eq!(atlas.allocate(1, 64), None);
    }

    #[test]
    fn dirty_region_grows_to_cover_writes() {
        let mut atlas = TextureAtlas::new(64);
        atlas.take_dirty();
        let a = atlas.allocate(4, 4).unwrap();
        let b = atlas.allocate(4, 8).unwrap();
        atlas.write(a, &[[1; 4]; 16]);
        atlas.write(b, &[[2; 4]; 32]);
        let d = atlas.take_dirty().unwrap();
        assert_eq!((d.x, d.y), (a.x, a.y));
        assert_eq!(
            (d.x + d.width, d.y + d.height),
            (b.x + b.width, b.y + b.height)
        );
    }
}
