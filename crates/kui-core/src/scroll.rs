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
/// `widgets::uniform_list` for the uniform-row case done for you.
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
    /// started, when that was, and the container's transition. The
    /// target is `offset`, and `resolve` samples between the two while
    /// it runs — as does `geometry`, so a view slicing rows during the
    /// leg slices by the place this frame will draw (RG19).
    smooth: Option<(Vec2, f64, crate::anim::Transition)>,
    /// The frame a layout last resolved this key as a scroll container
    /// — unlike `last_declared`, which a write also stamps. What
    /// `take_resliced` asks before it compares a read with a layout
    /// (RG25).
    laid_frame: u64,
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
    /// The containers whose eased offset the last layout left
    /// mid-flight, kept as that layout found them: a wheel between two
    /// frames ends a leg in its entry, but the frame after is still owed
    /// by that container, and a trace names it (backlog RG89,
    /// from the F111 regression pass).
    owing: Vec<Key>,
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
        // Not the reads: a binding that runs its view before the frame
        // begins — Node's, which calls `scrollGeometry` while the JS
        // view builds its tree — reads between two frames, and those
        // reads are this frame's (RG24). `take_resliced` drains them.
        self.owing.clear();
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

    /// Where `key`'s content is drawn: the offset the last layout placed
    /// it at, for a key a layout has resolved, else the stored offset.
    /// What the thumb, the access tree and the inspector show — during
    /// an eased leg (F80) the target is somewhere the content is not
    /// yet (RG21).
    pub(crate) fn drawn(&self, key: Key) -> Vec2 {
        self.entries.get(&key).map_or(
            Vec2::ZERO,
            |e| {
                if e.geom.is_some() { e.laid } else { e.offset }
            },
        )
    }

    /// Where a write that takes the content "where it stands" starts
    /// from: the drawn place while a leg is in flight, else the stored
    /// offset — which between two frames may already hold a wheel
    /// notch the frame has not drawn, and that notch must add up.
    pub(crate) fn standing(&self, key: Key) -> Vec2 {
        self.entries.get(&key).map_or(Vec2::ZERO, |e| {
            if e.smooth.is_some() { e.laid } else { e.offset }
        })
    }

    /// Moves `key`'s scroll state by the content that moved under it —
    /// `drawn` for the drawn place and a leg's start, `target` for the
    /// offset — without asking for an ease or ending one: each means the
    /// same content it did, in a coordinate space that moved under it.
    /// Two deltas because mid-leg they are two places in the content, and
    /// rows measured between them move one and not the other. What
    /// `widgets::list`'s height correction is (RG18): a `set_scroll` there,
    /// eased on a container with a `transition`, showed the uncorrected
    /// frame the correction exists to hide, and mid-glide it moved the
    /// target to where the content stood and ended a long jump a screen
    /// along.
    pub(crate) fn shift(&mut self, key: Key, drawn: Vec2, target: Vec2) {
        let e = self.entries.entry(key).or_default();
        // From the offset as the last layout clamped it — the number the
        // view read — or a raw "jump to the end" (`f32::MAX`) plus a
        // shift would still be the end, and the rows would not hold.
        if let Some((_, _, max)) = e.geom {
            e.offset.x = e.offset.x.clamp(0.0, max.x);
            e.offset.y = e.offset.y.clamp(0.0, max.y);
        }
        e.offset.x += target.x;
        e.offset.y += target.y;
        e.laid.x += drawn.x;
        e.laid.y += drawn.y;
        if let Some((from, _, _)) = &mut e.smooth {
            from.x += drawn.x;
            from.y += drawn.y;
        }
        e.last_declared = self.frame_no;
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
        //
        // And where it *will be* drawn this frame, not where the last
        // frame drew it: the leg is sampled at the clock the coming
        // layout reads, so a view slices the rows that frame shows
        // rather than a frame behind with a blank band on every frame of
        // a long glide (RG19).
        let clamp = |v: Vec2| Vec2::new(v.x.clamp(0.0, max_offset.x), v.y.clamp(0.0, max_offset.y));
        let offset = match (e.smooth, self.now) {
            (Some((from, start, t)), Some(now)) => {
                sample((clamp(from), start, t), clamp(e.offset), now).0
            }
            (Some(_), None) => clamp(e.laid),
            (None, _) => clamp(e.offset),
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
    ///
    /// Drains the reads: the next frame's are whatever is read from here
    /// on. A container read and then not laid out this frame — a pane in
    /// a hidden tab — owes nothing: there is no frame of it on screen to
    /// correct, and comparing its stale placement with a `set_scroll`
    /// written since asked for a frame on every frame until the tab came
    /// back (RG25).
    pub(crate) fn take_resliced(&mut self) -> bool {
        let reads = std::mem::take(self.reads.get_mut());
        let frame_no = self.frame_no;
        reads.iter().any(|(key, size, offset)| {
            match self
                .entries
                .get(key)
                .filter(|e| e.laid_frame == frame_no)
                .and_then(|e| e.geom.map(|g| (g, e.laid)))
            {
                Some(((rect, _, _), laid)) => {
                    (rect.w - size.w).abs() > 0.5
                        || (rect.h - size.h).abs() > 0.5
                        || (laid.x - offset.x).abs() > 0.5
                        || (laid.y - offset.y).abs() > 0.5
                }
                None => false,
            }
        })
    }

    /// Where `key` stands against its travel, for a scroll gesture asking
    /// whether it can still move a way (backlog F107): the place a wheel's
    /// delta would be added to — the drawn place while an eased leg is in
    /// flight, which the delta ends (RG17), else the stored offset with
    /// any notch since the last frame in it — clamped to the last
    /// layout's `max_offset`, and that `max_offset`. `None` for a key no
    /// layout has resolved as a scroll container. A read that records
    /// nothing, unlike [`Self::geometry`]'s: a wheel asking is not a view
    /// slicing rows.
    pub(crate) fn room(&self, key: Key) -> Option<(Vec2, Vec2)> {
        let e = self.entries.get(&key)?;
        let (_, _, max) = e.geom?;
        let from = if e.smooth.is_some() { e.laid } else { e.offset };
        let at = Vec2::new(from.x.clamp(0.0, max.x), from.y.clamp(0.0, max.y));
        Some((at, max))
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
        // A delta is measured against what is on screen — the wheel's
        // notch, a reveal's gap from the drawn node — so mid-leg it
        // moves from the drawn place, not from the target: adding it to
        // the target jumped the content past where the hand or the
        // reveal meant (RG17). Taking the leg's place ends the leg; a
        // programmatic ask starts a new one from there at the next
        // layout, and a second delta before then adds up as it did.
        if e.smooth.take().is_some() {
            e.offset = e.laid;
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
        e.last_declared = frame_no;
        // Asked again for where a leg under way is already going — a view
        // that calls `reveal_row` every frame until the row shows — the
        // leg goes on. Asked anew, it would start over from where it is
        // drawn, sampled at the same instant: the content never moved and
        // every frame asked for the next (the alpha.22 regression pass).
        if smooth && e.smooth.is_some() && e.offset == offset {
            return;
        }
        e.asked_smooth = smooth;
        if !smooth {
            e.smooth = None;
        }
        e.offset = offset;
    }

    /// The clock the eased offsets read, fed by `Core::set_time` beside
    /// the animation store's.
    pub(crate) fn set_time(&mut self, now: f64) {
        self.now = Some(now);
    }

    /// Whether an eased offset was still mid-flight at the last layout,
    /// so the driver owes another frame.
    pub fn animating(&self) -> bool {
        !self.owing.is_empty()
    }

    /// The containers whose eased offset the last layout left mid-flight
    /// — what [`Self::animating`] is made of, for a trace (backlog F111).
    /// As that layout left them, not as the entries read now: a leg a
    /// wheel ended since still owes the frame it was mid-flight for.
    pub(crate) fn easing(&self) -> impl Iterator<Item = Key> + '_ {
        self.owing.iter().copied()
    }

    /// How long `key`'s scroll state has been quiet, in seconds of the
    /// driver's clock, as of `now` — zero on the frame the offset or the
    /// travel moved, on the first frame the bar is asked about, and
    /// whenever `active` says the pointer or a drag is holding it. What an
    /// `auto` scrollbar fades by (`crate::spec::ScrollbarMode::Auto`).
    pub(crate) fn bar_idle(&mut self, key: Key, now: f64, active: bool) -> f64 {
        let e = self.entries.entry(key).or_default();
        let max = e.geom.map_or(Vec2::ZERO, |(_, _, m)| m);
        // The drawn place: an eased leg is the content moving, and a bar
        // that faded while it moved would be quiet through a scroll.
        let state = (if e.geom.is_some() { e.laid } else { e.offset }, max);
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
        e.laid_frame = frame_no;
        let drawn = match (smooth, now) {
            (Some(t), Some(now)) if t.duration_ms > 0.0 => {
                if std::mem::take(&mut e.asked_smooth) && e.laid != e.offset {
                    // From where the content is *now*, so a second
                    // reveal mid-flight carries on from what is on
                    // screen rather than snapping back to start.
                    e.smooth = Some((e.laid, now, t));
                }
                match e.smooth {
                    Some((from, start, _)) => {
                        // The start clamped too: content that shrank
                        // under a leg must not be drawn past its new end
                        // for the rest of the leg (RG22).
                        let from = Vec2::new(
                            from.x.clamp(0.0, max.x.max(0.0)),
                            from.y.clamp(0.0, max.y.max(0.0)),
                        );
                        let (at, done) = sample((from, start, t), e.offset, now);
                        if done {
                            e.smooth = None;
                        } else {
                            e.smooth = Some((from, start, t));
                            if !self.owing.contains(&key) {
                                self.owing.push(key);
                            }
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

/// A leg sampled at `now`: the drawn place, and whether the leg is over.
fn sample(
    (from, start, t): (Vec2, f64, crate::anim::Transition),
    to: Vec2,
    now: f64,
) -> (Vec2, bool) {
    let p = (((now - start) / (t.duration_ms as f64 / 1000.0)) as f32).clamp(0.0, 1.0);
    let k = t.curve().apply(p);
    (
        Vec2::new(from.x + (to.x - from.x) * k, from.y + (to.y - from.y) * k),
        p >= 1.0,
    )
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
