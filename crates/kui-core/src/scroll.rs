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

impl ScrollGeometry {
    /// `{x, y, w, h, content_w, content_h, offset: {x, y}, max_offset:
    /// {x, y}}` — the box's rect flattened, its content's size beside it.
    pub fn to_value(&self) -> crate::value::Value {
        use crate::value::Value;
        Value::map([
            ("x", Value::float(self.rect.x)),
            ("y", Value::float(self.rect.y)),
            ("w", Value::float(self.rect.w)),
            ("h", Value::float(self.rect.h)),
            ("content_w", Value::float(self.content.w)),
            ("content_h", Value::float(self.content.h)),
            ("offset", self.offset.to_value()),
            ("max_offset", self.max_offset.to_value()),
        ])
    }
}

/// One container's retained state. The offset exists from the moment anything
/// writes one; the rest only once a layout has resolved the key *as a scroll
/// container*, which is why it is optional and the offset is not.
#[derive(Default, Clone, Copy)]
struct Entry {
    offset: Vec2,
    geom: Option<(Rect, Size, Vec2)>,
    /// The offset the last layout *placed the content at* — `offset` as
    /// `resolve` clamped it, before anything wrote a newer one. A wheel
    /// notch between two frames moves `offset` at once and this only at
    /// the next layout, which is the difference a drag following its
    /// scroller reads (ADR 0029, decision 1): the frame on screen is at
    /// this offset, whatever the store already holds.
    laid: Vec2,
    /// The frame a layout last resolved this key, or an offset was last
    /// written at it. Only the budget reads it (see
    /// [`MAX_UNDECLARED_SCROLLS`]).
    last_declared: u64,
    /// For an `auto` bar: the scroll state (offset, max) the bar was last
    /// emitted for, and the clock reading it last changed at — or was
    /// otherwise active — so the bar knows how long it has been quiet.
    /// `None` until the bar is first emitted.
    bar: Option<(Vec2, Vec2, f64)>,
    /// For an `anchor` container: the child first in view at the last
    /// layout and where its leading edge was in the content, on the main
    /// axis (backlog C26 step 3). What the next layout keeps still.
    anchor: Option<(Key, f32)>,
    /// A programmatic offset change waiting to be eased — a `set_scroll`
    /// or a `reveal` since the last layout (F80). The pointer's own
    /// writes leave it false: a scroll under the thumb or the wheel is
    /// the hand's, and easing it would lag behind the finger.
    asked_smooth: bool,
    /// The leg an eased offset is on: where the content was when it
    /// started, and when that was. The target is `offset`, and
    /// `resolve` samples between the two while it runs.
    smooth: Option<(Vec2, f64)>,
}

/// How many *undeclared* scroll entries the store keeps before the longest
/// undeclared one is dropped (backlog F26). An entry a layout resolved in
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
    /// The clock an eased offset reads (`Core::set_time`); `None` until
    /// a driver sets one, which is a driver that shows no animation.
    now: Option<f64>,
    /// Whether the last layout left an eased offset mid-flight.
    owes: bool,
    /// The frame being built, stamped onto every entry touched.
    frame_no: u64,
    /// The geometries read during this build ([`Self::geometry`]) — what
    /// a view sliced its rows by — as the box size and the clamped
    /// offset it was handed, so that after layout the core can tell
    /// whether the frame came out as the view assumed
    /// ([`Self::resliced`]). Interior mutability because a read is a
    /// read.
    reads: std::cell::RefCell<Vec<(Key, Size, Vec2)>>,
}

impl ScrollStore {
    /// How many entries are retained — declared and undeclared together.
    /// What a test watches the budget through (backlog F26).
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
        self.reads.get_mut().clear();
        self.owes = false;
        if self.entries.len() > MAX_UNDECLARED_SCROLLS {
            self.evict(frame_no.saturating_sub(1));
        }
    }

    fn evict(&mut self, declared_at: u64) {
        crate::retain::evict_undeclared(
            &mut self.entries,
            MAX_UNDECLARED_SCROLLS,
            declared_at,
            |e| e.last_declared,
            |_| false,
            |_| {},
        );
    }

    pub fn offset(&self, key: Key) -> Vec2 {
        self.entries.get(&key).map_or(Vec2::ZERO, |e| e.offset)
    }

    /// The last layout's geometry for `key`, or `None` for a key no layout
    /// has ever resolved as a scroll container — including one that only
    /// ever had an offset written at it.
    /// The offset the last layout placed `key`'s content at, or `None`
    /// for a key no layout has resolved as a scroll container. Unlike
    /// [`Self::geometry`]'s `offset`, an offset written since is not in
    /// it: this is where the frame on screen *is*.
    pub(crate) fn laid_offset(&self, key: Key) -> Option<Vec2> {
        let e = self.entries.get(&key)?;
        e.geom?;
        Some(e.laid)
    }

    pub fn geometry(&self, key: Key) -> Option<ScrollGeometry> {
        let e = self.entries.get(&key)?;
        let (rect, content, max_offset) = e.geom?;
        // Clamped here, not just at layout: a `set_scroll` between two
        // frames leaves a raw number in the store, and a view slicing
        // its data by it would index far off the end.
        //
        // While an offset is easing (F80) this is where the content
        // *is*, not where it is going: the question this answers is
        // which rows can be seen, and during the leg that is the ones
        // around the drawn offset. The target is `Core::scroll_offset`.
        let offset = match e.smooth {
            Some(_) => e.laid,
            None => Vec2::new(
                e.offset.x.clamp(0.0, max_offset.x),
                e.offset.y.clamp(0.0, max_offset.y),
            ),
        };
        if let Ok(mut reads) = self.reads.try_borrow_mut() {
            reads.push((key, Size::new(rect.w, rect.h), offset));
        }
        Some(ScrollGeometry {
            rect,
            content,
            offset,
            max_offset,
        })
    }

    /// Whether a container whose geometry this build read came out of
    /// layout with another box size or another placed offset than the
    /// one it was handed: the view sliced by the frame before, and that
    /// frame is not this one — a resize, a split sliding open, a reveal
    /// — so one more frame is owed, built against what is on screen now.
    /// Without it the rows a virtual list built for the old box stay on
    /// screen until the next event, a screenful short.
    pub(crate) fn resliced(&self) -> bool {
        self.reads.borrow().iter().any(|(key, size, offset)| {
            match self
                .entries
                .get(key)
                .and_then(|e| e.geom.map(|g| (g, e.laid)))
            {
                Some(((rect, _, _), laid)) => {
                    (rect.w - size.w).abs() > 0.5
                        || (rect.h - size.h).abs() > 0.5
                        || (laid.x - offset.x).abs() > 0.5
                        || (laid.y - offset.y).abs() > 0.5
                }
                // Read, and then not laid out as a container at all.
                None => true,
            }
        })
    }

    /// Adds a delta (positive = scroll content further down/right).
    /// Clamping happens in the next layout pass.
    pub fn scroll_by(&mut self, key: Key, delta: Vec2) {
        self.scroll_by_from(key, delta, false);
    }

    /// The same, from a programmatic source — a `reveal`, an app's
    /// `set_scroll` — which a container declaring a `transition` eases
    /// into rather than jumping (F80).
    pub fn scroll_by_smooth(&mut self, key: Key, delta: Vec2) {
        self.scroll_by_from(key, delta, true);
    }

    fn scroll_by_from(&mut self, key: Key, delta: Vec2, smooth: bool) {
        let frame_no = self.frame_no;
        let e = self.entries.entry(key).or_default();
        e.asked_smooth = smooth;
        if !smooth {
            // The hand takes the content where it is now, mid-ease or
            // not: an ease the pointer interrupts has no leg left to run.
            e.smooth = None;
        }
        e.offset.x += delta.x;
        e.offset.y += delta.y;
        // An offset written between two frames counts as a declaration: it
        // is usually a `set_scroll` at a key this frame is about to build.
        e.last_declared = frame_no;
    }

    pub fn set(&mut self, key: Key, offset: Vec2) {
        self.set_from(key, offset, false);
    }

    /// The same, from a programmatic source; see
    /// [`scroll_by_smooth`](Self::scroll_by_smooth).
    pub fn set_smooth(&mut self, key: Key, offset: Vec2) {
        self.set_from(key, offset, true);
    }

    fn set_from(&mut self, key: Key, offset: Vec2, smooth: bool) {
        let frame_no = self.frame_no;
        let e = self.entries.entry(key).or_default();
        e.asked_smooth = smooth;
        if !smooth {
            e.smooth = None;
        }
        e.offset = offset;
        e.last_declared = frame_no;
    }

    /// The clock the eased offsets read, fed by `Core::set_time` beside
    /// the animation store's.
    pub(crate) fn set_time(&mut self, now: f64) {
        self.now = Some(now);
    }

    /// Whether an eased offset was still mid-flight at the last layout,
    /// so the driver owes another frame.
    pub fn animating(&self) -> bool {
        self.owes
    }

    /// How long `key`'s scroll state has been quiet, in seconds of the
    /// driver's clock, as of `now` — zero on the frame the offset or the
    /// travel moved, on the first frame the bar is asked about, and
    /// whenever `active` says the pointer or a drag is holding it. What an
    /// `auto` scrollbar fades by (`crate::spec::ScrollbarMode::Auto`).
    pub(crate) fn bar_idle(&mut self, key: Key, now: f64, active: bool) -> f64 {
        let e = self.entries.entry(key).or_default();
        let max = e.geom.map_or(Vec2::ZERO, |(_, _, m)| m);
        let state = (e.offset, max);
        match e.bar {
            Some((off, m, at)) if !active && (off, m) == state => (now - at).max(0.0),
            _ => {
                e.bar = Some((state.0, state.1, now));
                0.0
            }
        }
    }

    /// The anchor the last layout recorded for `key`: the child first in
    /// view and its leading edge's content position on the main axis.
    pub(crate) fn anchor(&self, key: Key) -> Option<(Key, f32)> {
        self.entries.get(&key).and_then(|e| e.anchor)
    }

    /// Records this layout's anchor for `key` (see [`Self::anchor`]);
    /// `None` when the container had nothing in view.
    pub(crate) fn set_anchor(&mut self, key: Key, anchor: Option<(Key, f32)>) {
        self.entries.entry(key).or_default().anchor = anchor;
    }

    /// What layout calls on a scroll container: records the geometry it
    /// resolved, clamps the stored offset to `[0, max]` per axis, and
    /// returns it. `max` is passed rather than derived from `rect` and
    /// `content` because an axis that does not scroll has no travel however
    /// far its content overflows.
    pub fn resolve(
        &mut self,
        key: Key,
        rect: Rect,
        content: Size,
        max: Vec2,
        smooth: Option<crate::anim::Transition>,
    ) -> Vec2 {
        let frame_no = self.frame_no;
        let now = self.now;
        let e = self.entries.entry(key).or_default();
        e.last_declared = frame_no;
        e.geom = Some((rect, content, Vec2::new(max.x.max(0.0), max.y.max(0.0))));
        e.offset.x = e.offset.x.clamp(0.0, max.x.max(0.0));
        e.offset.y = e.offset.y.clamp(0.0, max.y.max(0.0));
        // Where the content is drawn: the offset itself, unless the
        // container asked for a transition and the move was a
        // programmatic one — then it starts where the last frame left it
        // and eases to the offset over the leg (F80).
        let drawn = match (smooth, now) {
            (Some(t), Some(now)) if t.duration_ms > 0.0 => {
                if std::mem::take(&mut e.asked_smooth) && e.laid != e.offset {
                    // From where the content is *now*, so a second
                    // reveal mid-flight carries on from what is on
                    // screen rather than snapping back to start.
                    e.smooth = Some((e.laid, now));
                }
                match e.smooth {
                    Some((from, start)) => {
                        let p = (((now - start) / (t.duration_ms as f64 / 1000.0)) as f32)
                            .clamp(0.0, 1.0);
                        let k = t.easing.apply(p);
                        let at = Vec2::new(
                            from.x + (e.offset.x - from.x) * k,
                            from.y + (e.offset.y - from.y) * k,
                        );
                        if p >= 1.0 {
                            e.smooth = None;
                        } else {
                            self.owes = true;
                        }
                        at
                    }
                    None => e.offset,
                }
            }
            _ => {
                e.asked_smooth = false;
                e.smooth = None;
                e.offset
            }
        };
        e.laid = drawn;
        drawn
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
            None,
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
            None,
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
