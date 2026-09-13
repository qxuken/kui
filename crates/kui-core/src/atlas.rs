//! CPU-side glyph atlas: a single RGBA page with shelf packing. Renderers
//! mirror it to a texture; `dirty`/`epoch` tell them when to re-upload.

use cosmic_text::CacheKey;
use rustc_hash::FxHashMap;

use crate::resources::ImageId;

pub const ATLAS_SIZE: u32 = 1024;
/// The atlas doubles up to this when content (typically images) won't fit.
pub const MAX_ATLAS_SIZE: u32 = 4096;

#[derive(Clone, Copy, Debug)]
pub struct GlyphSlot {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Raster offset from the glyph origin.
    pub left: i32,
    pub top: i32,
    /// Color bitmap (emoji) vs alpha mask.
    pub color_glyph: bool,
    /// LCD subpixel mask: rgb are per-channel coverages.
    pub subpixel: bool,
}

pub struct RasterGlyph {
    pub w: u32,
    pub h: u32,
    pub left: i32,
    pub top: i32,
    pub color: bool,
    pub subpixel: bool,
    /// RGBA, w*h*4 bytes.
    pub data: Vec<u8>,
}

struct Shelf {
    y: u32,
    h: u32,
    cursor_x: u32,
}

pub struct GlyphAtlas {
    pub size: u32,
    /// RGBA, size*size*4.
    pub pixels: Vec<u8>,
    /// Set when pixels changed since the renderer last consumed them.
    pub dirty: bool,
    /// Bumped when the atlas is reset; renderers drop cached state.
    pub epoch: u64,
    map: FxHashMap<CacheKey, Option<GlyphSlot>>,
    /// Registered images blitted into the same page (one texture, one draw
    /// call). Keyed by handle; re-blitted from `Resources` after a reset.
    images: FxHashMap<ImageId, Option<GlyphSlot>>,
    shelves: Vec<Shelf>,
    next_shelf_y: u32,
    /// How many times the page filled since `begin_frame`: once is a
    /// working set that turned over, twice is one that does not fit the
    /// page, which is when the page grows (AR19).
    resets_this_frame: u32,
}

impl GlyphAtlas {
    pub fn new() -> Self {
        Self::with_size(ATLAS_SIZE)
    }

    pub fn with_size(size: u32) -> Self {
        Self {
            size,
            pixels: vec![0; (size * size * 4) as usize],
            dirty: false,
            epoch: 0,
            map: FxHashMap::default(),
            images: FxHashMap::default(),
            shelves: Vec::new(),
            next_shelf_y: 0,
            resets_this_frame: 0,
        }
    }

    /// A frame begins: the count that decides between a reset and a
    /// growth starts over.
    pub fn begin_frame(&mut self) {
        self.resets_this_frame = 0;
    }

    /// Drops every cached glyph and image (they re-rasterize on demand) and
    /// bumps the epoch so renderers re-upload. Used when the raster mode
    /// changes under the cache.
    pub fn clear(&mut self) {
        self.reset();
    }

    fn reset(&mut self) {
        self.pixels.fill(0);
        self.map.clear();
        self.images.clear();
        self.shelves.clear();
        self.next_shelf_y = 0;
        self.epoch += 1;
        self.dirty = true;
    }

    /// Reset onto a bigger page (used when content outgrows the current
    /// one). Everything cached is dropped and re-inserts on demand.
    fn grow_to(&mut self, size: u32) {
        self.size = size;
        self.pixels = vec![0; (size * size * 4) as usize];
        self.map.clear();
        self.images.clear();
        self.shelves.clear();
        self.next_shelf_y = 0;
        self.epoch += 1;
        self.dirty = true;
    }

    /// Alloc with escalation: on a full page, reset and retry; still no fit,
    /// double the page (to `MAX_ATLAS_SIZE`) until it fits or can't. A
    /// page that fills *twice in one frame* holds a working set larger
    /// than itself — three atlas-backed 800×600 images, a code view with
    /// many sizes plus CJK and emoji — and doubles instead of resetting
    /// again (AR19): resetting alone left every such frame corrupt, since
    /// after a reset any one item fits and the page never grew, and every
    /// frame then reset mid-emit, invalidated every text template and
    /// sampled the overwritten page from the quads emitted before it.
    fn alloc_or_make_room(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if let Some(pos) = self.alloc(w, h) {
            return Some(pos);
        }
        // Quads emitted earlier this frame may sample stale UVs for one
        // frame; the epoch bump forces a re-upload — and `Core` asks for
        // the frame that rebuilds them — so it self-heals.
        self.resets_this_frame += 1;
        let bigger = (self.size * 2).min(MAX_ATLAS_SIZE);
        if self.resets_this_frame >= 2 && bigger != self.size {
            self.grow_to(bigger);
        } else {
            self.reset();
        }
        loop {
            if let Some(pos) = self.alloc(w, h) {
                return Some(pos);
            }
            let bigger = (self.size * 2).min(MAX_ATLAS_SIZE);
            if bigger == self.size {
                return None;
            }
            self.grow_to(bigger);
        }
    }

    /// Finds space for a w*h glyph (padded by 1px to avoid sampling bleed).
    fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        let (pw, ph) = (w + 1, h + 1);
        if pw > self.size || ph > self.size {
            return None;
        }
        // Reuse a shelf that's tall enough but not wastefully so.
        for shelf in &mut self.shelves {
            if ph <= shelf.h && shelf.h <= ph.saturating_mul(2) && shelf.cursor_x + pw <= self.size
            {
                let pos = (shelf.cursor_x, shelf.y);
                shelf.cursor_x += pw;
                return Some(pos);
            }
        }
        // Open a new shelf, height quantized to reduce fragmentation.
        let shelf_h = ph.next_multiple_of(8);
        if self.next_shelf_y + shelf_h > self.size {
            return None;
        }
        let shelf = Shelf {
            y: self.next_shelf_y,
            h: shelf_h,
            cursor_x: pw,
        };
        self.next_shelf_y += shelf_h;
        let pos = (0, shelf.y);
        self.shelves.push(shelf);
        Some(pos)
    }

    fn blit(&mut self, x: u32, y: u32, w: u32, h: u32, data: &[u8]) {
        let stride = (self.size * 4) as usize;
        for row in 0..h as usize {
            let src = row * (w as usize) * 4;
            let dst = (y as usize + row) * stride + (x as usize) * 4;
            self.pixels[dst..dst + (w as usize) * 4]
                .copy_from_slice(&data[src..src + (w as usize) * 4]);
        }
        self.dirty = true;
    }

    /// Cached lookup; rasterizes on miss. `None` means unrasterizable (e.g.
    /// whitespace) and is cached as such.
    pub fn get_or_insert(
        &mut self,
        key: CacheKey,
        raster: impl FnOnce() -> Option<RasterGlyph>,
    ) -> Option<GlyphSlot> {
        if let Some(slot) = self.map.get(&key) {
            return *slot;
        }
        let Some(glyph) = raster() else {
            self.map.insert(key, None);
            return None;
        };
        let Some((x, y)) = self.alloc_or_make_room(glyph.w, glyph.h) else {
            self.map.insert(key, None);
            return None;
        };
        self.blit(x, y, glyph.w, glyph.h, &glyph.data);
        let slot = GlyphSlot {
            x,
            y,
            w: glyph.w,
            h: glyph.h,
            left: glyph.left,
            top: glyph.top,
            color_glyph: glyph.color,
            subpixel: glyph.subpixel,
        };
        self.map.insert(key, Some(slot));
        Some(slot)
    }

    /// Cached lookup for a registered image; blits `rgba` (w*h*4) on miss.
    /// `None` means it can't fit even a `MAX_ATLAS_SIZE` page.
    pub fn get_or_insert_image(
        &mut self,
        id: ImageId,
        w: u32,
        h: u32,
        rgba: &[u8],
    ) -> Option<GlyphSlot> {
        if let Some(slot) = self.images.get(&id) {
            return *slot;
        }
        debug_assert_eq!(rgba.len(), (w * h * 4) as usize);
        let Some((x, y)) = self.alloc_or_make_room(w, h) else {
            self.images.insert(id, None);
            return None;
        };
        self.blit(x, y, w, h, rgba);
        let slot = GlyphSlot {
            x,
            y,
            w,
            h,
            left: 0,
            top: 0,
            color_glyph: true,
            subpixel: false,
        };
        self.images.insert(id, Some(slot));
        Some(slot)
    }

    /// Forget an image's slot (its pixels are reclaimed at the next reset).
    /// Call when the host removes the image from `Resources`.
    pub fn evict_image(&mut self, id: ImageId) {
        self.images.remove(&id);
    }

    /// Keeps the slots of the images `live` says still exist and forgets
    /// the rest — how a window learns of removals made through another
    /// window of its session (AR8).
    pub fn retain_images(&mut self, live: impl Fn(ImageId) -> bool) {
        self.images.retain(|id, _| live(*id));
    }

    /// Whether the atlas holds a slot for `id`.
    pub fn has_image(&self, id: ImageId) -> bool {
        self.images.contains_key(&id)
    }
}

impl Default for GlyphAtlas {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_key(i: u32) -> CacheKey {
        // CacheKey is plain data; construct distinct ones via glyph id.
        CacheKey {
            font_id: cosmic_text::fontdb::ID::dummy(),
            glyph_id: i as u16,
            font_size_bits: (12.0f32 + (i / 65536) as f32).to_bits(),
            x_bin: cosmic_text::SubpixelBin::Zero,
            y_bin: cosmic_text::SubpixelBin::Zero,
            font_weight: cosmic_text::fontdb::Weight::NORMAL,
            flags: cosmic_text::CacheKeyFlags::empty(),
        }
    }

    fn raster(w: u32, h: u32) -> RasterGlyph {
        RasterGlyph {
            w,
            h,
            left: 0,
            top: 0,
            color: false,
            subpixel: false,
            data: vec![0xff; (w * h * 4) as usize],
        }
    }

    #[test]
    fn slots_stay_in_bounds_and_do_not_overlap() {
        let mut atlas = GlyphAtlas::with_size(256);
        let mut slots = Vec::new();
        for i in 0..200 {
            let (w, h) = (5 + (i % 13), 7 + (i % 9));
            if let Some(s) = atlas.get_or_insert(fake_key(i), || Some(raster(w, h))) {
                assert!(
                    s.x + s.w <= 256 && s.y + s.h <= 256,
                    "slot out of bounds: {s:?}"
                );
                slots.push(s);
            }
        }
        assert!(!slots.is_empty());
        for (i, a) in slots.iter().enumerate() {
            for b in slots.iter().skip(i + 1) {
                let disjoint =
                    a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assert!(disjoint, "overlap: {a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn lookup_is_cached() {
        let mut atlas = GlyphAtlas::with_size(128);
        let mut calls = 0;
        let key = fake_key(1);
        for _ in 0..3 {
            atlas.get_or_insert(key, || {
                calls += 1;
                Some(raster(10, 10))
            });
        }
        assert_eq!(calls, 1);
    }

    #[test]
    fn unrasterizable_is_cached_as_none() {
        let mut atlas = GlyphAtlas::with_size(128);
        let mut calls = 0;
        for _ in 0..3 {
            let slot = atlas.get_or_insert(fake_key(2), || {
                calls += 1;
                None
            });
            assert!(slot.is_none());
        }
        assert_eq!(calls, 1);
    }

    #[test]
    fn overflow_resets_and_bumps_epoch() {
        let mut atlas = GlyphAtlas::with_size(64);
        for i in 0..100 {
            atlas.get_or_insert(fake_key(i), || Some(raster(30, 30)));
        }
        assert!(atlas.epoch > 0, "expected at least one reset");
        // Still functional after reset.
        let s = atlas.get_or_insert(fake_key(1000), || Some(raster(20, 20)));
        assert!(s.is_some());
    }

    /// AR19: a set that does not fit the page grows it, once — and from
    /// the next frame on, nothing resets. Before, the page reset on every
    /// overflow and never grew for a set of items that each fit, so a
    /// frame whose glyphs outnumbered the page reset mid-emit every time.
    #[test]
    fn a_working_set_larger_than_the_page_grows_it_and_then_holds() {
        let mut atlas = GlyphAtlas::with_size(64);
        atlas.begin_frame();
        for i in 0..100 {
            atlas.get_or_insert(fake_key(i), || Some(raster(30, 30)));
        }
        assert!(atlas.size >= 512, "grown to hold the set: {}", atlas.size);
        let epoch = atlas.epoch;
        // The same set next frame: what the last growth dropped is
        // rasterized again and fits, and the page is still.
        atlas.begin_frame();
        for i in 0..100 {
            atlas.get_or_insert(fake_key(i), || Some(raster(30, 30)));
        }
        assert_eq!(atlas.epoch, epoch, "no reset on the second frame");
        atlas.begin_frame();
        for i in 0..100 {
            atlas.get_or_insert(fake_key(i), || panic!("cached by now"));
        }
        assert_eq!(atlas.epoch, epoch);
        // A turned-over set — one page's worth of new keys per frame —
        // still resets rather than growing without bound.
        let mut atlas = GlyphAtlas::with_size(64);
        for frame in 0..10u32 {
            atlas.begin_frame();
            for i in 0..4 {
                atlas.get_or_insert(fake_key(frame * 4 + i), || Some(raster(30, 30)));
            }
        }
        assert_eq!(atlas.size, 64, "one page's worth a frame never grows it");
    }

    #[test]
    fn oversized_content_grows_the_page() {
        let mut atlas = GlyphAtlas::with_size(64);
        let s = atlas.get_or_insert(fake_key(5), || Some(raster(200, 200)));
        assert!(s.is_some(), "the page should double until it fits");
        assert!(atlas.size >= 256, "size is {}", atlas.size);
        assert!(
            atlas
                .get_or_insert(fake_key(6), || Some(raster(10, 10)))
                .is_some()
        );
    }

    #[test]
    fn impossible_content_is_rejected() {
        let mut atlas = GlyphAtlas::with_size(64);
        let big = MAX_ATLAS_SIZE + 1;
        let s = atlas.get_or_insert(fake_key(7), || Some(raster(big, 1)));
        assert!(s.is_none());
        // Still functional afterwards.
        assert!(
            atlas
                .get_or_insert(fake_key(8), || Some(raster(10, 10)))
                .is_some()
        );
    }
}
