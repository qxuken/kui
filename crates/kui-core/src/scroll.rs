//! Retained scroll state: offsets keyed by widget `Key`, surviving the
//! per-frame tree rebuild. The layout pass clamps each offset to the current
//! content overflow, so state stays valid as content changes — and, at the
//! same moment, records the geometry it clamped against, which is the only
//! copy of it that outlives the frame (`Core::scroll_geometry`).

use rustc_hash::FxHashMap;

use crate::geom::{Rect, Size, Vec2};
use crate::key::Key;

/// What the last layout resolved for a scroll container: where the container
/// landed, how big its content came out, and the offset it clamped. All
/// logical px, in the same viewport coordinates `on_layout` reports.
///
/// This is the geometry a view needs to build only the rows that can be seen
/// — it is read during the *next* frame's build, so it describes the frame
/// before. See `Core::scroll_geometry` for what that costs and
/// `widgets::virtual_column` for the uniform-row case done for you.
///
/// The four numbers describe one moment, which is what makes arithmetic on
/// them safe: `offset` is always within `max_offset`, even immediately after
/// a `set_scroll` wrote something wilder (the documented "a huge value jumps
/// to the end" would otherwise hand a view an index a million rows past its
/// data).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollGeometry {
    /// The container's own box, as the last layout placed and sized it —
    /// the same rect an `on_layout` on that node would have reported.
    pub rect: Rect,
    /// Its laid-out content, padding included. Bigger than `rect` on an axis
    /// exactly when that axis has somewhere left to scroll.
    pub content: Size,
    /// Where the container is scrolled to (positive = content moved up /
    /// left): the retained offset clamped to `max_offset`, which is what the
    /// next layout will resolve it to unless the content changes size in the
    /// same frame. `Core::scroll_offset` is the unclamped retained number.
    pub offset: Vec2,
    /// How far `offset` can travel on each axis — zero on an axis that does
    /// not scroll, or whose content fits. `offset == max_offset` is "at the
    /// end", which is how a log view asks whether it is still tailing.
    pub max_offset: Vec2,
}

/// One container's retained state. The offset exists from the moment anything
/// writes one; the rest only once a layout has resolved the key *as a scroll
/// container*, which is why it is optional and the offset is not.
#[derive(Default, Clone, Copy)]
struct Entry {
    offset: Vec2,
    geom: Option<(Rect, Size, Vec2)>,
    /// The frame a layout last resolved this key, or an offset was last
    /// written at it. Only the budget reads it (see
    /// [`MAX_UNDECLARED_SCROLLS`]).
    last_declared: u64,
}

/// How many *undeclared* scroll entries the store keeps before the longest
/// undeclared one is dropped (backlog F25). An entry a layout resolved in
/// the frame that just ended is never evicted, however many there are.
///
/// Why 1024 where the editors get 256: an entry is a pair of offsets, an
/// optional geometry and a stamp — 56 bytes, 64 with its key in the map,
/// so a full budget is ~64 KB against the editors' megabytes. A view with a thousand
/// scroll containers it no longer declares is already generating keys.
pub const MAX_UNDECLARED_SCROLLS: usize = 1024;

#[derive(Default)]
pub struct ScrollStore {
    entries: FxHashMap<Key, Entry>,
    /// The frame being built, stamped onto every entry touched.
    frame_no: u64,
}

impl ScrollStore {
    /// How many entries are retained — declared and undeclared together.
    /// What a test watches the budget through (backlog F25).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Stamps the frame being built and, if the map has grown past the
    /// budget, drops the longest-undeclared entries
    /// ([`MAX_UNDECLARED_SCROLLS`]). Same rule as the editors': an entry
    /// the frame that just ended resolved (or that was written at since)
    /// is never evicted, so retention across absence still holds — this is
    /// a ceiling, not a prune. A store inside the budget never walks
    /// itself.
    pub(crate) fn begin_frame(&mut self, frame_no: u64) {
        self.frame_no = frame_no;
        if self.entries.len() > MAX_UNDECLARED_SCROLLS {
            self.evict(frame_no.saturating_sub(1));
        }
    }

    fn evict(&mut self, declared_at: u64) {
        let mut undeclared: Vec<(u64, Key)> = self
            .entries
            .iter()
            .filter(|(_, e)| e.last_declared < declared_at)
            .map(|(k, e)| (e.last_declared, *k))
            .collect();
        let Some(excess) = undeclared.len().checked_sub(MAX_UNDECLARED_SCROLLS) else {
            return;
        };
        if excess == 0 {
            return;
        }
        undeclared.sort_unstable();
        for (_, key) in &undeclared[..excess] {
            self.entries.remove(key);
        }
    }

    pub fn offset(&self, key: Key) -> Vec2 {
        self.entries.get(&key).map_or(Vec2::ZERO, |e| e.offset)
    }

    /// The last layout's geometry for `key`, or `None` for a key no layout
    /// has ever resolved as a scroll container — including one that only
    /// ever had an offset written at it.
    pub fn geometry(&self, key: Key) -> Option<ScrollGeometry> {
        let e = self.entries.get(&key)?;
        let (rect, content, max_offset) = e.geom?;
        Some(ScrollGeometry {
            rect,
            content,
            // Clamped here, not just at layout: a `set_scroll` between two
            // frames leaves a raw number in the store, and a view slicing
            // its data by it would index far off the end.
            offset: Vec2::new(
                e.offset.x.clamp(0.0, max_offset.x),
                e.offset.y.clamp(0.0, max_offset.y),
            ),
            max_offset,
        })
    }

    /// Adds a delta (positive = scroll content further down/right).
    /// Clamping happens in the next layout pass.
    pub fn scroll_by(&mut self, key: Key, delta: Vec2) {
        let frame_no = self.frame_no;
        let e = self.entries.entry(key).or_default();
        e.offset.x += delta.x;
        e.offset.y += delta.y;
        // An offset written between two frames counts as a declaration: it
        // is usually a `set_scroll` at a key this frame is about to build.
        e.last_declared = frame_no;
    }

    pub fn set(&mut self, key: Key, offset: Vec2) {
        let frame_no = self.frame_no;
        let e = self.entries.entry(key).or_default();
        e.offset = offset;
        e.last_declared = frame_no;
    }

    /// What layout calls on a scroll container: records the geometry it
    /// resolved, clamps the stored offset to `[0, max]` per axis, and
    /// returns it. `max` is passed rather than derived from `rect` and
    /// `content` because an axis that does not scroll has no travel however
    /// far its content overflows.
    pub fn resolve(&mut self, key: Key, rect: Rect, content: Size, max: Vec2) -> Vec2 {
        let frame_no = self.frame_no;
        let e = self.entries.entry(key).or_default();
        e.last_declared = frame_no;
        e.geom = Some((rect, content, Vec2::new(max.x.max(0.0), max.y.max(0.0))));
        e.offset.x = e.offset.x.clamp(0.0, max.x.max(0.0));
        e.offset.y = e.offset.y.clamp(0.0, max.y.max(0.0));
        e.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(s: &mut ScrollStore, k: Key, max: Vec2) -> Vec2 {
        // The geometry a 100x300 container with `max` of overflow would have.
        s.resolve(
            k,
            Rect::new(0.0, 0.0, 100.0, 300.0),
            Size::new(100.0 + max.x, 300.0 + max.y),
            max,
        )
    }

    #[test]
    fn accumulates_and_clamps() {
        let mut s = ScrollStore::default();
        let k = Key::ROOT.str("list");
        s.scroll_by(k, Vec2::new(0.0, 120.0));
        s.scroll_by(k, Vec2::new(0.0, 500.0));
        assert_eq!(
            resolve(&mut s, k, Vec2::new(0.0, 300.0)),
            Vec2::new(0.0, 300.0)
        );
        s.scroll_by(k, Vec2::new(0.0, -1000.0));
        assert_eq!(
            resolve(&mut s, k, Vec2::new(0.0, 300.0)),
            Vec2::new(0.0, 0.0)
        );
        // Content shrinking re-clamps existing offsets.
        s.scroll_by(k, Vec2::new(0.0, 250.0));
        assert_eq!(
            resolve(&mut s, k, Vec2::new(0.0, 100.0)),
            Vec2::new(0.0, 100.0)
        );
    }

    #[test]
    fn unknown_key_is_zero() {
        let mut s = ScrollStore::default();
        assert_eq!(s.offset(Key::ROOT.str("x")), Vec2::ZERO);
        assert_eq!(
            resolve(&mut s, Key::ROOT.str("x"), Vec2::new(0.0, 100.0)),
            Vec2::ZERO
        );
    }

    /// An offset written at a key is not evidence that the key is a
    /// container: only a layout resolving it fills the geometry in.
    #[test]
    fn geometry_needs_a_layout_not_just_an_offset() {
        let mut s = ScrollStore::default();
        let k = Key::ROOT.str("list");
        assert_eq!(s.geometry(k), None);
        s.set(k, Vec2::new(0.0, 40.0));
        assert_eq!(s.offset(k), Vec2::new(0.0, 40.0));
        assert_eq!(s.geometry(k), None);

        resolve(&mut s, k, Vec2::new(0.0, 700.0));
        let g = s.geometry(k).expect("resolved");
        assert_eq!(g.rect, Rect::new(0.0, 0.0, 100.0, 300.0));
        assert_eq!(g.content, Size::new(100.0, 1000.0));
        // The geometry carries the clamped offset, not the written one.
        assert_eq!(g.offset, Vec2::new(0.0, 40.0));
        assert_eq!(g.max_offset, Vec2::new(0.0, 700.0));
    }

    /// An axis that does not scroll has no travel however far its content
    /// overflows — which is why `max` is recorded rather than derived from
    /// the two sizes.
    #[test]
    fn max_offset_is_the_axis_travel_not_the_overflow() {
        let mut s = ScrollStore::default();
        let k = Key::ROOT.str("list");
        // Content overflows both axes; only y scrolls.
        s.resolve(
            k,
            Rect::new(0.0, 0.0, 100.0, 300.0),
            Size::new(400.0, 1000.0),
            Vec2::new(0.0, 700.0),
        );
        let g = s.geometry(k).unwrap();
        assert_eq!(g.max_offset, Vec2::new(0.0, 700.0));
        assert!(
            g.content.w > g.rect.w,
            "the x overflow is real, the travel is not"
        );
    }

    /// A `set_scroll` of "a huge value" means "the end"; a view slicing by
    /// the geometry sees the end, not the huge value.
    #[test]
    fn geometry_clamps_an_offset_no_layout_has_seen_yet() {
        let mut s = ScrollStore::default();
        let k = Key::ROOT.str("list");
        resolve(&mut s, k, Vec2::new(0.0, 700.0));
        s.set(k, Vec2::new(0.0, 1e9));
        // The raw retained number is what was written...
        assert_eq!(s.offset(k), Vec2::new(0.0, 1e9));
        // ...but the geometry is coherent.
        let g = s.geometry(k).unwrap();
        assert_eq!(g.offset, g.max_offset);
        assert_eq!(g.offset.y, 700.0);
    }
}
