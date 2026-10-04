//! CPU-side glyph atlas: a single RGBA page with shelf packing. Renderers
//! mirror it to a texture; `dirty`/`epoch` tell them when to re-upload.

use cosmic_text::CacheKey;
use rustc_hash::{FxHashMap, FxHashSet};

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

/// A glyph or shape refused for room on a page that began the frame
/// empty.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Refusal {
    Glyph(CacheKey),
    Synth(char, u32, u32),
}

struct Shelf {
    y: u32,
    h: u32,
    cursor_x: u32,
}

/// The page as it was when `begin_frame` last emptied it, kept for that
/// one frame: its pixels, and the glyphs and shapes it
/// held, where.
struct Prev {
    size: u32,
    pixels: Vec<u8>,
    map: FxHashMap<CacheKey, Option<GlyphSlot>>,
    synth: FxHashMap<(char, u32, u32), Option<GlyphSlot>>,
}

impl Prev {
    /// The texels of the `w × h` rect at (`x`, `y`), row after row.
    fn texels(&self, x: u32, y: u32, w: u32, h: u32) -> Vec<u8> {
        let stride = (self.size * 4) as usize;
        let row = (w * 4) as usize;
        let mut out = Vec::with_capacity(row * h as usize);
        for r in 0..h as usize {
            let at = (y as usize + r) * stride + (x * 4) as usize;
            out.extend_from_slice(&self.pixels[at..at + row]);
        }
        out
    }
}

/// How the page makes room. A slot handed out during a
/// frame is never moved or overwritten before that frame is presented:
/// the quads already emitted, the text templates built and the cell
/// tables filled all carry its texel rect, and nothing walks them again.
/// So a page that fills mid-frame is *extended* — doubled with its
/// pixels kept where they are, which leaves every texel rect valid, since
/// `uv` is in texels and the renderer divides by the page's size at draw
/// time — and the reset that reclaims it waits for the next
/// `begin_frame`, before anything is emitted. The page then starts that
/// frame empty at its base size and holds that frame's set alone; if the
/// set does not fit it, it is larger than the page and the page keeps the
/// growth. Most fills never get as far as mid-frame:
/// `begin_frame` sees one coming in the rows the last frames opened and
/// empties the page first. A page at `MAX_ATLAS_SIZE` cannot extend: there
/// the request is refused for this frame (the glyph is not drawn, an
/// image draws from a texture of its own) and the next frame, which
/// `short` asks for, starts on an empty page.
///
/// A page that began the frame empty and still refuses holds a set bigger
/// than itself, and the next frame would refuse the same. Its
/// refusals are kept, and until the page is next emptied the atlas
/// measures what each frame looks up: `stamp` moves every frame, so the
/// caches that keep slots look theirs up again, and each distinct slot's
/// texels are counted once. A frame that wanted a refused glyph and whose
/// set fits in what the page held is `short`, and the next frame begins
/// on an empty page that takes it — the view has scrolled to a part of
/// the set. One that did not fit keeps the page as it is.
///
/// An emptied page is not drawn from again, but it is kept for the frame
/// that begins on the empty one. A glyph or shape that
/// frame looks up and the old page held is copied across, not
/// rasterized again: the frame after a reset is the whole visible set
/// looked up at once — kawoosh's window, ~600 glyphs, was 2.5–5.3 ms of
/// rasterizing where the copy is a fraction of one. Only what the frame
/// looks up is copied, so the page still holds that frame's set alone.
/// `clear`, for a raster mode that changed, keeps nothing.
pub struct GlyphAtlas {
    pub size: u32,
    /// RGBA, size*size*4.
    pub pixels: Vec<u8>,
    /// Set when pixels changed since the renderer last consumed them.
    pub dirty: bool,
    /// Bumped whenever the page is replaced — reset, or resized — so
    /// renderers re-upload it whole and caches that stamped it re-look
    /// their slots up. A resize keeps every slot where it was.
    pub epoch: u64,
    /// What caches that keep slots across frames — text templates, cell
    /// tables — key them on: it moves with `epoch`, and on every frame
    /// while refusals are pending, so that those frames look every
    /// slot they use up again and are measured.
    pub stamp: u64,
    map: FxHashMap<CacheKey, Option<GlyphSlot>>,
    /// Registered images blitted into the same page (one texture, one draw
    /// call). Keyed by handle; re-blitted from `Resources` after a reset.
    images: FxHashMap<ImageId, Option<GlyphSlot>>,
    /// Shapes drawn from a cell box rather than a font — box drawing,
    /// blocks, Powerline — keyed on the character and the
    /// cell size in physical px, so one cell size shares one slot and
    /// another size does not. Plain masks, tinted like a glyph's.
    synth: FxHashMap<(char, u32, u32), Option<GlyphSlot>>,
    shelves: Vec<Shelf>,
    next_shelf_y: u32,
    /// The size the page settles at: what `begin_frame` resets an
    /// extended page to. It grows when one frame's set does not fit it —
    /// the page filled on a frame it began empty — or one item alone is
    /// bigger than it, and never shrinks.
    base: u32,
    /// The page held nothing when this frame began, so whatever fills it
    /// is this frame's own set.
    fresh: bool,
    /// A request was refused for room this frame on a page that could not
    /// extend and still held earlier frames' glyphs: the next frame
    /// starts on an empty page and should come.
    short: bool,
    /// The shelf rows a frame has been opening lately — the last frame's,
    /// or a quarter less than the figure before, whichever is more; a
    /// frame that began on an empty page counts none — and the page's
    /// rows when this frame began, which the next frame's figure is read
    /// against.
    rows_per_frame: u32,
    rows_at_begin: u32,
    /// Frames begun since the page was last emptied. A page that needs
    /// emptying again within two is too small for its set and how fast
    /// it turns over, and grows instead (F83's thrash, measured).
    frames_since_reset: u32,
    /// A frame since the page was last emptied began on a page holding
    /// something and opened rows on it: the set is turning over. Until
    /// one has, `rows_per_frame` is what the frames before the reset
    /// opened — a burst, a view's worth of new glyphs, that says nothing
    /// of how fast the set that followed turns over — and the page is not
    /// read as filling (otherwise a burst that fit would double the page
    /// for good two frames later).
    turned: bool,
    /// The refusals pending: what a page that began the frame empty
    /// could not take, with the texels each would use and the frame it
    /// was last looked up on. Emptied with the page.
    refused: FxHashMap<Refusal, (u64, u64)>,
    /// Texels handed out since the page was last emptied, padding
    /// included, and what they came to on the frame that began it empty
    /// and refused: the part of its set the page was seen to hold.
    placed: u64,
    held: u64,
    /// While refusals are pending: the texels of the distinct slots this
    /// frame looked up, refused ones included, the slots already counted,
    /// and whether a refused one was among them.
    demand: u64,
    seen: FxHashSet<(u32, u32)>,
    wanted: bool,
    /// Frames begun, for `refused`'s once-a-frame count.
    frame: u64,
    /// The page `begin_frame` emptied, for this frame only (see the type's
    /// note).
    prev: Option<Prev>,
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
            stamp: 0,
            map: FxHashMap::default(),
            images: FxHashMap::default(),
            synth: FxHashMap::default(),
            shelves: Vec::new(),
            next_shelf_y: 0,
            base: size,
            fresh: true,
            short: false,
            rows_per_frame: 0,
            rows_at_begin: 0,
            frames_since_reset: u32::MAX,
            turned: false,
            refused: FxHashMap::default(),
            placed: 0,
            held: 0,
            demand: 0,
            seen: FxHashSet::default(),
            wanted: false,
            frame: 0,
            prev: None,
        }
    }

    /// A frame begins, before anything is emitted — the one point where
    /// the page can be emptied without a quad sampling what it dropped.
    /// It is emptied, back at its base size, when the last frame extended
    /// it or was refused room (`short`) — and when it is about to fill:
    /// fewer rows free than two frames open at the rate they lately have.
    /// A set that turns
    /// over a little each frame, a list scrolling through fonts, fills
    /// the page every so often, and the rows foresee it, so the frame
    /// that would have extended the page mid-emit — a page four times
    /// the size, its rows copied, a texture made and uploaded twice —
    /// begins on an empty one instead. A fill it does not foresee still
    /// extends.
    ///
    /// A page that needs emptying within two frames of the last time is
    /// too small for its set and the rate it turns over at, and grows
    /// instead: an extension is kept, and a page about to fill doubles
    /// with its slots in place. That is F83's thrash, a set between one
    /// page and two, measured by what the page does rather than by which
    /// glyphs come back; a fill long after the last empties it.
    ///
    /// A page with refusals pending is emptied when the last frame wanted
    /// one and its set fits (see the type's note); otherwise the frame
    /// ahead is measured.
    pub fn begin_frame(&mut self) {
        // The page emptied a frame ago has served the frame it was kept for.
        self.prev = None;
        if self.fresh && !self.refused.is_empty() {
            self.held = self.placed;
        }
        let refit = self.refit();
        let rows = self.next_shelf_y;
        // A frame that began on an empty page opened its whole set, which
        // says nothing of how fast the set turns over.
        let opened = if self.fresh {
            0
        } else {
            rows.saturating_sub(self.rows_at_begin)
        };
        self.rows_per_frame = opened.max(self.rows_per_frame - self.rows_per_frame / 4);
        self.frames_since_reset = self.frames_since_reset.saturating_add(1);
        self.turned |= opened > 0;
        let extended = self.size > self.base;
        let filling = self.turned && self.size - rows < 2 * self.rows_per_frame;
        let thrash = self.frames_since_reset <= 2;
        if self.short || refit {
            self.reset_to(self.base, true);
        } else if extended && thrash {
            self.base = self.size;
        } else if extended {
            self.reset_to(self.base, true);
        } else if filling && thrash && self.size < MAX_ATLAS_SIZE {
            self.extend_to((self.size * 2).min(MAX_ATLAS_SIZE));
            self.base = self.size;
        } else if filling {
            self.reset_to(self.base, true);
        }
        self.short = false;
        self.fresh = self.next_shelf_y == 0;
        self.rows_at_begin = self.next_shelf_y;
        self.frame += 1;
        self.demand = 0;
        self.seen.clear();
        self.wanted = false;
        if !self.refused.is_empty() {
            self.stamp += 1;
        }
    }

    /// The frame's glyphs are all looked up: the page `begin_frame`
    /// emptied has served it. Dropped here rather than at the next
    /// `begin_frame`, which on an idle window may never come — the frame
    /// that shrinks an extended page, or empties a full 4096 one, would
    /// otherwise hold up to 64 MiB for as long as nothing redraws.
    pub(crate) fn end_frame(&mut self) {
        self.prev = None;
    }

    /// For a frame-level test: the next `begin_frame` empties the page,
    /// keeping the old one, as a refusal would.
    #[cfg(test)]
    pub(crate) fn reset_next_frame(&mut self) {
        self.short = true;
    }

    /// For a frame-level test: whether an emptied page is still kept.
    #[cfg(test)]
    pub(crate) fn keeps_prev(&self) -> bool {
        self.prev.is_some()
    }

    /// Whether this frame was refused room (see `short`), or wanted a
    /// glyph refused earlier while its set fits the page: it drew
    /// without some glyph, and the next frame, on an empty page, draws it.
    pub(crate) fn short(&self) -> bool {
        self.short || self.refit()
    }

    /// The frame wanted a pending refusal, and what it looked up fits in
    /// what the page was seen to hold. Not on a frame that began empty,
    /// which was measured only from its first refusal on.
    fn refit(&self) -> bool {
        !self.fresh && self.wanted && self.demand <= self.held
    }

    /// Counts a looked-up slot into the frame's demand, once a frame,
    /// while refusals are pending.
    fn note_slot(&mut self, slot: GlyphSlot) {
        if !self.refused.is_empty() && self.seen.insert((slot.x, slot.y)) {
            self.demand += texels(slot.w, slot.h);
        }
    }

    /// Counts a looked-up refusal into the frame's demand, once a frame.
    /// Not a refusal — a glyph with nothing to draw, or larger than any
    /// page — counts nothing.
    fn note_refusal(&mut self, refusal: Refusal) {
        if let Some((cost, last)) = self.refused.get_mut(&refusal)
            && *last != self.frame
        {
            *last = self.frame;
            self.demand += *cost;
            self.wanted = true;
        }
    }

    /// Keeps a refusal for room made on a page that began the frame
    /// empty, the first time it is refused.
    fn refuse(&mut self, refusal: Refusal, w: u32, h: u32) {
        if self.fresh && w < MAX_ATLAS_SIZE && h < MAX_ATLAS_SIZE {
            self.refused.insert(refusal, (texels(w, h), self.frame));
            self.demand += texels(w, h);
            self.wanted = true;
        }
    }

    /// Drops every cached glyph and image (they re-rasterize on demand) and
    /// bumps the epoch so renderers re-upload. Used when the raster mode
    /// changes under the cache — between frames, never during one.
    pub fn clear(&mut self) {
        self.prev = None;
        self.reset_to(self.size, false);
    }

    /// Empties the page onto a `size` one: every slot dropped, the epoch
    /// bumped. Only between frames. `keep` keeps the old page for the
    /// frame ahead to copy from (see the type's note).
    fn reset_to(&mut self, size: u32, keep: bool) {
        if keep {
            let pixels = std::mem::replace(&mut self.pixels, vec![0; (size * size * 4) as usize]);
            self.prev = Some(Prev {
                size: self.size,
                pixels,
                map: std::mem::take(&mut self.map),
                synth: std::mem::take(&mut self.synth),
            });
            self.size = size;
        } else if size == self.size {
            self.pixels.fill(0);
        } else {
            self.size = size;
            self.pixels = vec![0; (size * size * 4) as usize];
        }
        self.map.clear();
        self.images.clear();
        self.synth.clear();
        self.shelves.clear();
        self.next_shelf_y = 0;
        self.epoch += 1;
        self.stamp += 1;
        self.dirty = true;
        self.frames_since_reset = 0;
        self.turned = false;
        self.refused.clear();
        self.placed = 0;
        self.held = 0;
    }

    /// Doubles the page with every slot kept where it is: the rows copy
    /// into the top-left of the bigger page, the shelves run on into the
    /// new width and new ones open below. Nothing handed out is
    /// invalidated, so it is safe mid-frame.
    fn extend_to(&mut self, size: u32) {
        let old = self.size as usize;
        let mut pixels = vec![0; (size * size * 4) as usize];
        for (row, src) in self.pixels.chunks_exact(old * 4).enumerate() {
            let at = row * size as usize * 4;
            pixels[at..at + old * 4].copy_from_slice(src);
        }
        self.pixels = pixels;
        self.size = size;
        self.epoch += 1;
        self.stamp += 1;
        self.dirty = true;
    }

    /// Alloc with room made: on a full page, extend it (see the type's
    /// note) until the item fits or the page is at `MAX_ATLAS_SIZE`. The
    /// growth is kept — `base` moves — when the page began the frame
    /// empty, since then this frame's set alone overflowed it (AR19: a
    /// set larger than the page; F83: one between one page and two), or
    /// when the item alone does not fit the base page; otherwise it is
    /// this frame's, and `begin_frame` resets to the base. Resetting here
    /// instead, as this did until F99, overwrote the slots of every quad
    /// the frame had emitted before the fill, and that frame was
    /// presented with them blank or scrambled.
    fn alloc_or_make_room(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        let pos = self.make_room(w, h);
        if pos.is_some() {
            self.placed += texels(w, h);
        }
        pos
    }

    fn make_room(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if let Some(pos) = self.alloc(w, h) {
            return Some(pos);
        }
        if w + 1 > MAX_ATLAS_SIZE || h + 1 > MAX_ATLAS_SIZE {
            return None; // no page holds it: nothing to make room for
        }
        loop {
            let bigger = (self.size * 2).min(MAX_ATLAS_SIZE);
            if bigger == self.size {
                // No room without dropping a slot this frame may have
                // used. A page that began the frame empty would refuse
                // this on any frame; one that held earlier frames' glyphs
                // will not, once the next frame begins on an empty page.
                if !self.fresh {
                    self.short = true;
                }
                return None;
            }
            self.extend_to(bigger);
            if self.fresh || w + 1 > self.base || h + 1 > self.base {
                self.base = self.size;
            }
            if let Some(pos) = self.alloc(w, h) {
                return Some(pos);
            }
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

    /// A glyph the page held before `begin_frame` emptied it, its texels
    /// copied out of the old page. Only one that was drawn: a
    /// `None` there may be a refusal, which the empty page is for.
    fn carried(&self, key: &CacheKey) -> Option<RasterGlyph> {
        let prev = self.prev.as_ref()?;
        let slot = (*prev.map.get(key)?)?;
        Some(RasterGlyph {
            w: slot.w,
            h: slot.h,
            left: slot.left,
            top: slot.top,
            color: slot.color_glyph,
            subpixel: slot.subpixel,
            data: prev.texels(slot.x, slot.y, slot.w, slot.h),
        })
    }

    /// Cached lookup; rasterizes on miss — or, the frame after the page
    /// was emptied, copies what the old page held. `None` means
    /// unrasterizable (e.g. whitespace) and is cached as such.
    pub fn get_or_insert(
        &mut self,
        key: CacheKey,
        raster: impl FnOnce() -> Option<RasterGlyph>,
    ) -> Option<GlyphSlot> {
        if let Some(&slot) = self.map.get(&key) {
            match slot {
                Some(slot) => self.note_slot(slot),
                None => self.note_refusal(Refusal::Glyph(key)),
            }
            return slot;
        }
        let Some(glyph) = self.carried(&key).or_else(raster) else {
            self.map.insert(key, None);
            return None;
        };
        let Some((x, y)) = self.alloc_or_make_room(glyph.w, glyph.h) else {
            self.refuse(Refusal::Glyph(key), glyph.w, glyph.h);
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
        self.note_slot(slot);
        self.map.insert(key, Some(slot));
        Some(slot)
    }

    /// Cached lookup for a shape drawn to a `w × h` cell;
    /// `coverage` is called on a miss for `w * h` alpha bytes, which land
    /// as a white mask the renderer tints like any glyph's. `None` means
    /// the cell does not fit a `MAX_ATLAS_SIZE` page.
    pub fn get_or_insert_synth(
        &mut self,
        ch: char,
        w: u32,
        h: u32,
        coverage: impl FnOnce() -> Vec<u8>,
    ) -> Option<GlyphSlot> {
        if let Some(&slot) = self.synth.get(&(ch, w, h)) {
            match slot {
                Some(slot) => self.note_slot(slot),
                None => self.note_refusal(Refusal::Synth(ch, w, h)),
            }
            return slot;
        }
        let Some((x, y)) = self.alloc_or_make_room(w, h) else {
            self.refuse(Refusal::Synth(ch, w, h), w, h);
            self.synth.insert((ch, w, h), None);
            return None;
        };
        let carried = self.prev.as_ref().and_then(|prev| {
            let slot = (*prev.synth.get(&(ch, w, h))?)?;
            Some(prev.texels(slot.x, slot.y, w, h))
        });
        let rgba = carried.unwrap_or_else(|| {
            let mask = coverage();
            debug_assert_eq!(mask.len(), (w * h) as usize);
            let mut rgba = Vec::with_capacity(mask.len() * 4);
            for &a in &mask {
                rgba.extend_from_slice(&[255, 255, 255, a]);
            }
            rgba
        });
        self.blit(x, y, w, h, &rgba);
        let slot = GlyphSlot {
            x,
            y,
            w,
            h,
            left: 0,
            top: 0,
            color_glyph: false,
            subpixel: false,
        };
        self.note_slot(slot);
        self.synth.insert((ch, w, h), Some(slot));
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
        if let Some(&slot) = self.images.get(&id) {
            // A refused image draws from a texture of its own: it takes
            // nothing of the page and wants nothing of it.
            if let Some(slot) = slot {
                self.note_slot(slot);
            }
            return slot;
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
        self.note_slot(slot);
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
    /// window of its session.
    pub fn retain_images(&mut self, live: impl Fn(ImageId) -> bool) {
        self.images.retain(|id, _| live(*id));
    }

    /// Whether the atlas holds a slot for `id`.
    pub fn has_image(&self, id: ImageId) -> bool {
        self.images.contains_key(&id)
    }
}

/// The texels a `w × h` item takes of the page, its padding included.
fn texels(w: u32, h: u32) -> u64 {
    u64::from(w + 1) * u64::from(h + 1)
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

    fn filled(w: u32, h: u32, v: u8) -> RasterGlyph {
        RasterGlyph {
            data: vec![v; (w * h * 4) as usize],
            ..raster(w, h)
        }
    }

    /// Whether every texel of `slot` is `v`: the slot still holds what
    /// was put there.
    fn holds(atlas: &GlyphAtlas, slot: GlyphSlot, v: u8) -> bool {
        (slot.y..slot.y + slot.h).all(|y| {
            let at = ((y * atlas.size + slot.x) * 4) as usize;
            atlas.pixels[at..at + (slot.w * 4) as usize]
                .iter()
                .all(|&p| p == v)
        })
    }

    /// F99: a page that fills mid-frame makes room without moving or
    /// overwriting a slot the frame already has — it extends, the pixels
    /// kept in place — and resets only when the next frame begins, back
    /// to the size it had. It used to reset on the spot, and every quad
    /// emitted before the fill sampled the page packed over it.
    #[test]
    fn a_page_that_fills_mid_frame_keeps_the_frames_slots_until_it_ends() {
        let mut atlas = GlyphAtlas::with_size(64);
        // 30×30 glyphs; a 64 page holds four.
        atlas.begin_frame();
        for i in 0..4u32 {
            atlas.get_or_insert(fake_key(i), || Some(filled(30, 30, i as u8 + 1)));
        }
        assert_eq!(atlas.size, 64);
        // The next frame draws one glyph it had and four new ones: the
        // fifth does not fit the page with the four old ones in it.
        atlas.begin_frame();
        let mut used = Vec::new();
        for i in [0u32, 4, 5, 6, 7] {
            let v = i as u8 + 1;
            let slot = atlas
                .get_or_insert(fake_key(i), || Some(filled(30, 30, v)))
                .expect("room is made");
            used.push((slot, v));
            for &(slot, v) in &used {
                assert!(
                    holds(&atlas, slot, v),
                    "a slot this frame used was overwritten"
                );
            }
        }
        assert_eq!(atlas.size, 128, "extended for the rest of the frame");
        assert!(!atlas.short());
        let epoch = atlas.epoch;
        // Between frames the extension goes: the page is its old size,
        // empty, and the frame's glyphs are copied from the page it
        // replaced (DX26), not rasterized again.
        atlas.begin_frame();
        assert_eq!(atlas.size, 64);
        assert!(atlas.epoch > epoch, "reset before anything is emitted");
        let mut rasterized = 0;
        for i in [4u32, 5, 6, 7] {
            let slot = atlas
                .get_or_insert(fake_key(i), || {
                    rasterized += 1;
                    Some(filled(30, 30, i as u8 + 1))
                })
                .expect("room");
            assert!(holds(&atlas, slot, i as u8 + 1), "copied whole");
        }
        assert_eq!(rasterized, 0);
        assert_eq!(atlas.size, 64, "the set fits the page: no growth kept");
    }

    /// AR19: a set that does not fit the page grows it, and from the
    /// next frame on nothing resets. A set that turns over — one page's
    /// worth of new keys a frame — never keeps a growth.
    #[test]
    fn a_working_set_larger_than_the_page_grows_it_and_then_holds() {
        let mut atlas = GlyphAtlas::with_size(64);
        atlas.begin_frame();
        for i in 0..100 {
            atlas.get_or_insert(fake_key(i), || Some(raster(30, 30)));
        }
        assert!(atlas.size >= 512, "grown to hold the set: {}", atlas.size);
        let (size, epoch) = (atlas.size, atlas.epoch);
        for _ in 0..3 {
            atlas.begin_frame();
            for i in 0..100 {
                atlas.get_or_insert(fake_key(i), || panic!("cached"));
            }
        }
        assert_eq!(
            (atlas.size, atlas.epoch),
            (size, epoch),
            "still from then on"
        );
        // A set that turns over whole each frame — a page's worth of new
        // keys every frame — grows only until a page lasts past two
        // frames, and then holds its size, emptied between frames.
        let mut atlas = GlyphAtlas::with_size(64);
        let mut sizes = Vec::new();
        for frame in 0..40u32 {
            atlas.begin_frame();
            let (size, epoch) = (atlas.size, atlas.epoch);
            for i in 0..4 {
                atlas.get_or_insert(fake_key(frame * 4 + i), || Some(raster(30, 30)));
            }
            if frame >= 20 {
                assert_eq!(
                    (atlas.size, atlas.epoch),
                    (size, epoch),
                    "no fill mid-frame"
                );
            }
            sizes.push(atlas.size);
        }
        assert!(sizes[20..].iter().all(|&s| s == sizes[39]), "{sizes:?}");
        assert!(sizes[39] <= 256, "bounded: {sizes:?}");
    }

    /// F99: a set that turns over a little each frame — a list scrolling
    /// through fonts — is emptied at `begin_frame`, when fewer rows are
    /// free than a frame has lately opened, and never fills mid-frame.
    #[test]
    fn a_turnover_the_rows_foresee_is_emptied_between_frames() {
        // 30×30 glyphs; a 256 page's row of 32 holds eight, and it has
        // eight rows. Eight new keys a frame is a row a frame.
        let mut atlas = GlyphAtlas::with_size(256);
        let mut resets = 0;
        for frame in 0..40u32 {
            let epoch = atlas.epoch;
            atlas.begin_frame();
            if atlas.epoch != epoch {
                resets += 1;
            }
            let (size, epoch) = (atlas.size, atlas.epoch);
            for i in 0..8 {
                atlas.get_or_insert(fake_key(frame * 8 + i), || Some(raster(30, 30)));
            }
            assert_eq!(
                (atlas.size, atlas.epoch),
                (size, epoch),
                "frame {frame}: the page filled mid-frame"
            );
        }
        assert_eq!(atlas.size, 256);
        assert!(resets >= 4, "emptied between frames: {resets}");
    }

    /// F83's thrash, measured: a page about to fill within two frames of
    /// being emptied is too small for its set and its turnover, and
    /// doubles at `begin_frame` with every slot in place — the set it
    /// holds is not rasterized again.
    #[test]
    fn a_page_emptied_again_within_two_frames_grows_with_its_slots() {
        let mut atlas = GlyphAtlas::with_size(256);
        // Six rows that stay, a new row a frame, on a page of eight.
        let mut next = 1000;
        let mut frame = |atlas: &mut GlyphAtlas| {
            atlas.begin_frame();
            for i in 0..48 {
                atlas.get_or_insert(fake_key(i), || Some(raster(30, 30)));
            }
            for _ in 0..8 {
                next += 1;
                atlas.get_or_insert(fake_key(next), || Some(raster(30, 30)));
            }
        };
        frame(&mut atlas); // on an empty page: seven rows
        frame(&mut atlas); // eight: full, a row a frame measured
        let epoch = atlas.epoch;
        frame(&mut atlas); // emptied before it: seven rows again
        assert_eq!((atlas.size, atlas.epoch), (256, epoch + 1));
        frame(&mut atlas); // a row turned over on it: full again
        assert_eq!(atlas.size, 256);
        atlas.begin_frame(); // no row free, a row a frame: again, so soon
        assert_eq!(atlas.size, 512, "grown instead");
        for i in 0..48 {
            atlas.get_or_insert(fake_key(i), || panic!("kept in place"));
        }
        let epoch = atlas.epoch;
        for _ in 0..40 {
            frame(&mut atlas);
            assert_eq!(atlas.size, 512, "and it holds");
        }
        assert!(
            atlas.epoch > epoch,
            "emptied between frames as it turns over"
        );
    }

    /// F83: a set between one page and two — the glyphs of a big font —
    /// that first arrives on a page holding older glyphs: the frame it
    /// fills extends the page for itself, the next begins empty at the
    /// old size, fills again with that set alone, and keeps the growth;
    /// from then on the page is still.
    #[test]
    fn a_set_between_one_page_and_two_grows_on_its_second_frame() {
        let mut atlas = GlyphAtlas::with_size(64);
        let frame = |atlas: &mut GlyphAtlas, keys: std::ops::Range<u32>| {
            atlas.begin_frame();
            for i in keys {
                atlas.get_or_insert(fake_key(i), || Some(raster(30, 30)));
            }
        };
        frame(&mut atlas, 100..102);
        // Six 30×30 glyphs; a 64 page holds four.
        frame(&mut atlas, 0..6);
        assert_eq!(atlas.size, 128, "extended for the frame");
        frame(&mut atlas, 0..6);
        assert_eq!(atlas.size, 128, "the set alone overflowed: kept");
        let epoch = atlas.epoch;
        frame(&mut atlas, 0..6);
        frame(&mut atlas, 0..6);
        assert_eq!(atlas.epoch, epoch, "and still from then on");
    }

    /// RG23: a fill long after the last, one that happens to want back a
    /// glyph an earlier reset dropped — the chrome's letters are in every
    /// set — is an ordinary turnover, and the page keeps its size.
    #[test]
    fn a_fill_long_after_a_reset_resets_even_with_a_dropped_glyph_back() {
        let mut atlas = GlyphAtlas::with_size(64);
        // 30×30 glyphs; a 64 page holds four.
        let frame = |atlas: &mut GlyphAtlas, keys: &[u32]| {
            atlas.begin_frame();
            for &i in keys {
                atlas.get_or_insert(fake_key(i), || Some(raster(30, 30)));
            }
        };
        frame(&mut atlas, &[0, 1, 2, 3]);
        frame(&mut atlas, &[4, 5, 6, 7]);
        for _ in 0..3 {
            frame(&mut atlas, &[4, 5, 6, 7]);
        }
        assert_eq!(atlas.size, 64, "one turnover, one reset");
        frame(&mut atlas, &[0, 8, 9, 10]);
        frame(&mut atlas, &[0, 8, 9, 10]);
        assert_eq!(atlas.size, 64, "a later turnover resets, it does not grow");
    }

    /// F99: a page at `MAX_ATLAS_SIZE` cannot extend, so a fill there
    /// refuses the glyph for this frame rather than drop a slot the frame
    /// used, says so (`short`, which asks for the next frame), and the
    /// next frame begins on an empty page that takes it. A page that
    /// began the frame empty and still cannot take it is not short:
    /// nothing the next frame does would change that.
    #[test]
    fn a_full_page_at_the_largest_size_refuses_for_one_frame() {
        let mut atlas = GlyphAtlas::with_size(MAX_ATLAS_SIZE);
        // 1000×1000 items: a 4096 page holds sixteen.
        atlas.begin_frame();
        let mut used = Vec::new();
        for i in 0..16u32 {
            let v = i as u8 + 1;
            let slot = atlas
                .get_or_insert(fake_key(i), || Some(filled(1000, 1000, v)))
                .expect("fits");
            used.push((slot, v));
        }
        assert!(!atlas.short());
        atlas.begin_frame();
        assert!(
            atlas
                .get_or_insert(fake_key(16), || Some(raster(1000, 1000)))
                .is_none()
        );
        assert!(atlas.short(), "refused on a page holding the last frame");
        for &(slot, v) in &used {
            assert!(holds(&atlas, slot, v), "nothing dropped under the frame");
        }
        let epoch = atlas.epoch;
        atlas.begin_frame();
        assert!(atlas.epoch > epoch && !atlas.short());
        assert!(
            atlas
                .get_or_insert(fake_key(16), || Some(raster(1000, 1000)))
                .is_some()
        );
        for i in 0..16u32 {
            atlas.get_or_insert(fake_key(i), || Some(raster(1000, 1000)));
        }
        assert!(
            !atlas.short(),
            "a set bigger than the page on an empty one is not short"
        );
    }

    /// RG56: a set bigger than a `MAX_ATLAS_SIZE` page on a frame that
    /// began it empty refuses what does not fit, and nothing the next
    /// frame does would change that, so it is not `short`. When the view
    /// then shows a part of the set that wants a refused glyph and fits,
    /// that frame is short and the next one empties the page and draws
    /// the glyph. It used to stay blank for as long as the page lived.
    #[test]
    fn a_glyph_refused_on_a_fresh_full_page_gets_room_once_its_set_fits() {
        let mut atlas = GlyphAtlas::with_size(MAX_ATLAS_SIZE);
        // 1000×1000 items: a 4096 page holds sixteen.
        let frame = |atlas: &mut GlyphAtlas, keys: std::ops::Range<u32>| {
            atlas.begin_frame();
            keys.map(|i| atlas.get_or_insert(fake_key(i), || Some(raster(1000, 1000))))
                .collect::<Vec<_>>()
        };
        let slots = frame(&mut atlas, 0..17);
        assert!(slots[16].is_none() && !atlas.short(), "refused, not short");
        // The view scrolls to the last four: they fit, one is refused.
        let slots = frame(&mut atlas, 13..17);
        assert!(slots[3].is_none(), "the cached refusal, this frame");
        assert!(atlas.short(), "and the next frame is owed");
        let epoch = atlas.epoch;
        let slots = frame(&mut atlas, 13..17);
        assert!(atlas.epoch > epoch, "emptied before anything is emitted");
        assert!(slots.iter().all(Option::is_some), "and drawn: {slots:?}");
        assert!(!atlas.short());
        let (epoch, stamp) = (atlas.epoch, atlas.stamp);
        for _ in 0..3 {
            frame(&mut atlas, 13..17);
        }
        assert_eq!((atlas.epoch, atlas.stamp), (epoch, stamp), "then still");
    }

    /// RG56: while the whole of a set bigger than the page stays on
    /// screen, the frames that measure it find it does not fit and keep
    /// the page — nothing is emptied or asked for, however many frames —
    /// while the stamp moves, so the caches that keep slots look them up
    /// and are counted.
    #[test]
    fn a_set_bigger_than_the_largest_page_keeps_it_still() {
        let mut atlas = GlyphAtlas::with_size(MAX_ATLAS_SIZE);
        let frame = |atlas: &mut GlyphAtlas| {
            atlas.begin_frame();
            for i in 0..17u32 {
                atlas.get_or_insert(fake_key(i), || Some(raster(1000, 1000)));
            }
        };
        frame(&mut atlas);
        let epoch = atlas.epoch;
        for _ in 0..5 {
            let stamp = atlas.stamp;
            frame(&mut atlas);
            assert!(!atlas.short(), "the set does not fit: nothing owed");
            assert_eq!(atlas.epoch, epoch, "the page is kept");
            assert!(atlas.stamp > stamp, "measured again");
        }
        // A page with nothing refused keeps its stamp from frame to frame.
        let mut atlas = GlyphAtlas::with_size(256);
        atlas.begin_frame();
        atlas.get_or_insert(fake_key(0), || Some(raster(30, 30)));
        let stamp = atlas.stamp;
        atlas.begin_frame();
        atlas.get_or_insert(fake_key(0), || panic!("cached"));
        assert_eq!(atlas.stamp, stamp);
    }

    /// One view's worth of new glyphs arriving at once — a tab of other
    /// fonts opened beside a steady chrome — fits the page, and a page
    /// whose set then holds still keeps its size: a burst is not a
    /// turnover (the regression pass over F99: the burst's rows, decayed,
    /// read as a page about to fill again within two frames of emptying).
    #[test]
    fn a_burst_that_fits_does_not_grow_the_page() {
        // 30×30 glyphs: 33 to a 32 px row of a 1024 page.
        let mut atlas = GlyphAtlas::with_size(1024);
        let frame = |atlas: &mut GlyphAtlas, burst: bool| {
            atlas.begin_frame();
            for i in 0..297 {
                atlas.get_or_insert(fake_key(i), || Some(raster(30, 30)));
            }
            if burst {
                for i in 1000..1627 {
                    atlas.get_or_insert(fake_key(i), || Some(raster(30, 30)));
                }
            }
        };
        for _ in 0..4 {
            frame(&mut atlas, false);
        }
        for n in 0..12 {
            frame(&mut atlas, true);
            assert_eq!(atlas.size, 1024, "frame {n} after the burst");
        }
        atlas.begin_frame();
        assert_eq!(atlas.size, 1024);
    }

    /// F66: a synthesized shape is one slot per character and cell size —
    /// the same size twice shares, another size does not — and a reset
    /// drops it with the glyphs.
    /// DX26: the frame that begins on an emptied page copies what the old
    /// page held and it looks up — texels, offsets, kind — rather than
    /// rasterizing it again, and only for that frame. What it did not look
    /// up is gone with the old page, and `clear` keeps nothing.
    #[test]
    fn the_frame_after_a_reset_copies_what_the_old_page_held() {
        let mut atlas = GlyphAtlas::with_size(64);
        atlas.begin_frame();
        let glyph = |v: u8| RasterGlyph {
            left: -2,
            top: 9,
            subpixel: true,
            ..filled(30, 30, v)
        };
        for i in 0..4u32 {
            atlas.get_or_insert(fake_key(i), || Some(glyph(i as u8 + 1)));
        }
        atlas.get_or_insert_synth('─', 8, 16, || vec![200; 8 * 16]);
        atlas.short = true; // what a refusal leaves: the next frame resets
        let epoch = atlas.epoch;
        atlas.begin_frame();
        assert!(atlas.epoch > epoch, "the page was emptied");

        let mut rasterized = 0;
        for i in [2u32, 0] {
            let slot = atlas
                .get_or_insert(fake_key(i), || {
                    rasterized += 1;
                    Some(glyph(0))
                })
                .unwrap();
            assert!(holds(&atlas, slot, i as u8 + 1), "glyph {i}'s texels");
            assert_eq!((slot.left, slot.top, slot.subpixel), (-2, 9, true));
        }
        let mut drawn = 0;
        let shape = atlas
            .get_or_insert_synth('─', 8, 16, || {
                drawn += 1;
                vec![0; 8 * 16]
            })
            .unwrap();
        assert_eq!(
            atlas.pixels[((shape.y * atlas.size + shape.x) * 4 + 3) as usize],
            200
        );
        assert_eq!((rasterized, drawn), (0, 0), "copied, not drawn again");
        assert_eq!(
            atlas.placed,
            2 * texels(30, 30) + texels(8, 16),
            "only what was looked up is placed"
        );

        // The frame's end: the old page is gone, and in the next frame a
        // glyph the reset frame did not ask for is rasterized.
        atlas.end_frame();
        assert!(atlas.prev.is_none(), "not held past its frame");
        atlas.begin_frame();
        atlas.get_or_insert(fake_key(1), || {
            rasterized += 1;
            Some(glyph(2))
        });
        assert_eq!(rasterized, 1);

        // `clear` (a raster mode that changed) keeps nothing.
        atlas.clear();
        atlas.begin_frame();
        atlas.get_or_insert(fake_key(1), || {
            rasterized += 1;
            Some(glyph(2))
        });
        assert_eq!(rasterized, 2);
    }

    #[test]
    fn a_synthesized_shape_is_keyed_on_its_cell_size() {
        use std::cell::Cell;
        let mut atlas = GlyphAtlas::with_size(128);
        let calls = Cell::new(0);
        let draw = |atlas: &mut GlyphAtlas, w, h| {
            atlas
                .get_or_insert_synth('│', w, h, || {
                    calls.set(calls.get() + 1);
                    vec![255; (w * h) as usize]
                })
                .expect("fits")
        };
        let a = draw(&mut atlas, 7, 16);
        let b = draw(&mut atlas, 7, 16);
        let c = draw(&mut atlas, 7, 20);
        assert_eq!((a.x, a.y), (b.x, b.y), "the same size shares a slot");
        assert_ne!((a.x, a.y), (c.x, c.y), "another size does not");
        assert_eq!(calls.get(), 2);
        assert!(!a.color_glyph && !a.subpixel && a.left == 0 && a.top == 0);
        // The mask landed as a tinted white mask.
        let i = ((a.y * atlas.size + a.x) * 4) as usize;
        assert_eq!(&atlas.pixels[i..i + 4], &[255, 255, 255, 255]);
        atlas.clear();
        draw(&mut atlas, 7, 16);
        assert_eq!(calls.get(), 3, "a reset drops the shape with the glyphs");
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
