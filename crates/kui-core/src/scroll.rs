//! Retained scroll state: the offset of every scroll container, keyed by
//! node `Key`, and the geometry the last layout resolved for it.
//!
//! A view makes a scroll container with `NodeSpec::scroll_y` /
//! `scroll_x` and never touches this store directly: the wheel, the
//! scrollbar and the keys move the offset, `Core::set_scroll` and
//! `Core::reveal` move it from the app, and layout clamps it to the
//! content every frame. What a view does read is [`ScrollGeometry`]
//! through `Ui::scroll_geometry` / `Core::scroll_geometry`: where the
//! container is, how big its content came out and where it is scrolled
//! to, which is what a virtual list needs to build only the visible rows.
//!
//! ```rust
//! use kui_core::{Core, NodeSpec, Size, TextStyle};
//!
//! let mut core = Core::new();
//! let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
//! ui.configure_root(NodeSpec::column().fill());
//! let list = ui.with_keyed("list", NodeSpec::column().fill().scroll_y(), |ui| {
//!     for i in 0..50 {
//!         ui.text(&format!("row {i}"), TextStyle::new(14.0));
//!     }
//! });
//! ui.finish();
//!
//! // Read during the next frame's build (or between frames): last layout's numbers.
//! let g = core.scroll_geometry(list).expect("laid out as a scroll container");
//! assert!(g.content.h > g.rect.h);
//! assert!(g.max_offset.y > 0.0 && g.offset.y == 0.0);
//! ```

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
    /// scroller reads: the frame on screen is at
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
    /// axis. What the next layout keeps still.
    anchor: Option<(Key, f32)>,
    /// For an `anchor` container the last layout left at its end on the
    /// main axis: the retained offset it left there. While the offset is
    /// still that — nobody scrolled since — the next layout keeps it at
    /// the end, whatever grew or shrank: a tailing log stays tailing when
    /// its box or its rows change size, in the same frame.
    tail: Option<f32>,
    /// A programmatic offset change waiting to be eased — a `set_scroll`
    /// or a `reveal` since the last layout. The pointer's own
    /// writes leave it false: a scroll under the thumb or the wheel is
    /// the hand's, and easing it would lag behind the finger.
    asked_smooth: bool,
    /// The leg an eased offset is on: where the content was when it
    /// started, when that was, and the container's transition. The
    /// target is `offset`, and `resolve` samples between the two while
    /// it runs — as does `geometry`, so a view slicing rows during the
    /// leg slices by the place this frame will draw.
    smooth: Option<(Vec2, f64, crate::anim::Transition)>,
    /// A wheel's glide (ADR 0050): the drawn place (`laid`) chasing the
    /// target (`offset`) every notch moves. Never beside `smooth`: a
    /// notch ends a leg and a programmatic write ends a glide.
    glide: Option<Glide>,
    /// The frame a layout last resolved this key as a scroll container
    /// — unlike `last_declared`, which a write also stamps. What
    /// `take_resliced` asks before it compares a read with a layout.
    laid_frame: u64,
}

/// A glide's spring: how fast the drawn place is moving, and the clock it
/// was last stepped at — `None` until the first layout after the notch
/// that started it, which steps it one nominal frame ([`GLIDE_FIRST_DT`])
/// whenever the notch came.
#[derive(Clone, Copy, Debug)]
struct Glide {
    velocity: Vec2,
    last: Option<f64>,
}

/// How a wheel's notch scrolls (ADR 0050, amended 2026-10-24): how far a
/// line goes, and how the glide eases it there. The app's input feel
/// rather than a view's, set with `Core::set_wheel_feel`; the native
/// runner copies the main window's to every other, and `KUI_WHEEL` in
/// the environment overrides it there for comparing without a rebuild.
///
/// [`WheelFeel::STANDARD`] is the default. The presets, from slow to
/// fast, at 60 Hz for one notch:
///
/// | preset     | a notch | first frame | nine tenths | landed  |
/// |------------|---------|-------------|-------------|---------|
/// | `gentle`   | 40 px   | 6.5 px      | 183 ms      | 333 ms  |
/// | `standard` | 40 px   | 7.8 px      | 167 ms      | 283 ms  |
/// | `brisk`    | 48 px   | 10.2 px     | 150 ms      | 267 ms  |
/// | `swift`    | 64 px   | 15.5 px     | 133 ms      | 267 ms  |
///
/// Every one keeps notches a slow hand turns 150 ms apart running into
/// each other: the glide is still moving over a pixel a frame when the
/// next one comes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelFeel {
    /// How far one line of a wheel goes, in logical px. A plain mouse's
    /// notch is one line on Windows, X11 and Wayland; on macOS it sends
    /// lines too, as many as the system's acceleration makes it. Also what a fraction of a
    /// line and an `on_scroll` handler's pixels are measured by, and what
    /// a notch jumps when it does not glide.
    pub line_px: f32,
    /// The glide's response time in seconds, in SwiftUI's
    /// parametrisation (the spring's period undamped). Critically
    /// damped, so shorter lands sooner and never overshoots.
    pub response: f32,
    /// How much of the speed that would make a notch's share a pure
    /// ease-out, `1 - e^(-ωt)`, the notch gives the spring, from 0 to 1.
    /// A whole kick starts each notch at its top speed and stops it
    /// short, so a slow hand's notches each lurch off and halt; none
    /// starts it from rest, which reads as lag. Between, the start is
    /// still the fastest frame and the tail is long enough to meet the
    /// next notch.
    pub kick: f32,
}

impl WheelFeel {
    /// Softer than the default: the same 40 px a notch, eased a little
    /// longer.
    pub const GENTLE: Self = Self {
        line_px: 40.0,
        response: 0.34,
        kick: 0.55,
    };
    /// The default. A notch is 40 px, nine tenths there by 167 ms and
    /// within half a pixel by about 283 ms; at 0.21 s and a whole kick a
    /// slow hand's notches had each stopped before the next and lurched
    /// off at full speed (the 2026-10-24 amendment).
    pub const STANDARD: Self = Self {
        line_px: 40.0,
        response: 0.30,
        kick: 0.6,
    };
    /// A touch faster: a fifth further a notch, and quicker to land.
    pub const BRISK: Self = Self {
        line_px: 48.0,
        response: 0.27,
        kick: 0.6,
    };
    /// Faster still: 64 px a notch, landed as soon as `brisk` lands 48.
    pub const SWIFT: Self = Self {
        line_px: 64.0,
        response: 0.24,
        kick: 0.62,
    };

    /// The presets by name, slow to fast — what [`Self::preset`] reads,
    /// and the names the bindings and `KUI_WHEEL` take.
    pub const PRESETS: [(&'static str, Self); 4] = [
        ("gentle", Self::GENTLE),
        ("standard", Self::STANDARD),
        ("brisk", Self::BRISK),
        ("swift", Self::SWIFT),
    ];

    /// The preset called `name`, ignoring case; `None` for any other.
    pub fn preset(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::PRESETS
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|&(_, f)| f)
    }

    /// The feel with each number brought into what the glide can use: a
    /// line from 1 to 1000 px, a response from 0.05 to 2 s, a kick from 0
    /// to 1. A number that is not finite takes the standard one.
    pub fn clamped(self) -> Self {
        let pick = |v: f32, lo: f32, hi: f32, or: f32| {
            if v.is_finite() { v.clamp(lo, hi) } else { or }
        };
        let s = Self::STANDARD;
        Self {
            line_px: pick(self.line_px, 1.0, 1000.0, s.line_px),
            response: pick(self.response, 0.05, 2.0, s.response),
            kick: pick(self.kick, 0.0, 1.0, s.kick),
        }
    }

    /// The glide's angular frequency, from [`Self::response`].
    fn omega(&self) -> f64 {
        std::f64::consts::TAU / self.response as f64
    }
}

impl Default for WheelFeel {
    fn default() -> Self {
        Self::STANDARD
    }
}

/// The step a glide's first layout takes. The store's clock is the last
/// frame's when a notch arrives, which may be minutes old; stepping from
/// it would land most of the notch on the first frame.
const GLIDE_FIRST_DT: f64 = 1.0 / 60.0;

/// The glide stepped from `last` to `now` toward `to`: the drawn place,
/// the velocity, and whether it has settled — within half a pixel and
/// nearly still, where it lands on the target exactly so that a settled
/// view is never a subpixel off. Semi-implicit Euler at the animation
/// store's step, as its springs are. `omega` is the feel's
/// ([`WheelFeel::omega`]).
fn glide_step(at: Vec2, g: Glide, to: Vec2, now: f64, omega: f64) -> (Vec2, Vec2, bool) {
    use crate::anim::{MAX_FRAME_DT, SPRING_STEP};
    let dt = match g.last {
        None => GLIDE_FIRST_DT,
        Some(last) => (now - last).clamp(0.0, MAX_FRAME_DT),
    };
    let (k, c) = ((omega * omega) as f32, (2.0 * omega) as f32);
    let steps = (dt / SPRING_STEP).ceil().max(1.0);
    let h = (dt / steps) as f32;
    let (mut p, mut v) = (at, g.velocity);
    for _ in 0..steps as usize {
        v.x += (-k * (p.x - to.x) - c * v.x) * h;
        v.y += (-k * (p.y - to.y) - c * v.y) * h;
        p.x += v.x * h;
        p.y += v.y * h;
    }
    let settled = (p.x - to.x).abs() < 0.5
        && (p.y - to.y).abs() < 0.5
        && v.x.abs() < 20.0
        && v.y.abs() < 20.0;
    if settled {
        (to, Vec2::ZERO, true)
    } else {
        (p, v, false)
    }
}

/// How many *undeclared* scroll entries the store keeps before the longest
/// undeclared one is dropped. An entry a layout resolved in the frame
/// that just ended is never evicted, however many there are. An entry is
/// about 64 bytes, so a full budget is about 64 KB.
pub const MAX_UNDECLARED_SCROLLS: usize = 1024;

/// The retained scroll state of one window, owned by `Core`. Reach it
/// through `Core::scroll_offset`, `Core::set_scroll` and
/// `Core::scroll_geometry` rather than directly.
#[derive(Default)]
pub struct ScrollStore {
    entries: FxHashMap<Key, Entry>,
    /// The clock an eased offset reads (`Core::set_time`); `None` until
    /// a driver sets one, which is a driver that shows no animation.
    now: Option<f64>,
    /// The containers whose eased offset the last layout left
    /// mid-flight, kept as that layout found them: a wheel between two
    /// frames ends a leg in its entry, but the frame after is still owed
    /// by that container, and a trace names it.
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
    /// How a wheel's notch goes ([`WheelFeel`]), clamped.
    feel: WheelFeel,
}

impl ScrollStore {
    /// How a wheel's notch goes; see [`WheelFeel`]. A glide under way
    /// carries on at the new response from where it is.
    pub(crate) fn set_wheel_feel(&mut self, feel: WheelFeel) {
        self.feel = feel.clamped();
    }

    pub(crate) fn wheel_feel(&self) -> WheelFeel {
        self.feel
    }

    /// How many entries are retained, declared and undeclared together.
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
        // reads are this frame's. `take_resliced` drains them.
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
    /// an eased leg the target is somewhere the content is not
    /// yet.
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
            if e.smooth.is_some() || e.glide.is_some() {
                e.laid
            } else {
                e.offset
            }
        })
    }

    /// Moves `key`'s scroll state by the content that moved under it —
    /// `drawn` for the drawn place and a leg's start, `target` for the
    /// offset — without asking for an ease or ending one: each means the
    /// same content it did, in a coordinate space that moved under it.
    /// Two deltas because mid-leg they are two places in the content, and
    /// rows measured between them move one and not the other. What
    /// `widgets::list`'s height correction is: a `set_scroll` there,
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
        // While an offset is easing this is where the content
        // *is*, not where it is going: the question this answers is
        // which rows can be seen, and during the leg that is the ones
        // around the drawn offset. The target is `Core::scroll_offset`.
        //
        // And where it *will be* drawn this frame, not where the last
        // frame drew it: the leg is sampled at the clock the coming
        // layout reads, so a view slices the rows that frame shows
        // rather than a frame behind with a blank band on every frame of
        // a long glide.
        let clamp = |v: Vec2| Vec2::new(v.x.clamp(0.0, max_offset.x), v.y.clamp(0.0, max_offset.y));
        // A glide the same way: stepped to the coming layout's clock, as
        // `resolve` will step it.
        let offset = match (e.smooth, e.glide, self.now) {
            (_, Some(g), Some(now)) => {
                glide_step(clamp(e.laid), g, clamp(e.offset), now, self.feel.omega()).0
            }
            (Some((from, start, t)), _, Some(now)) => {
                sample((clamp(from), start, t), clamp(e.offset), now).0
            }
            (Some(_), _, None) | (_, Some(_), None) => clamp(e.laid),
            (None, None, _) => clamp(e.offset),
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
    /// back.
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
    /// whether it can still move a way: the place a wheel's
    /// delta would be added to — the drawn place while an eased leg is in
    /// flight, which the delta ends, else the stored offset with
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
    /// into rather than jumping.
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
        // reveal meant. Taking the leg's place ends the leg; a
        // programmatic ask starts a new one from there at the next
        // layout, and a second delta before then adds up as it did.
        // A glide the same: a trackpad, a thumb or the autoscroll taking
        // over goes on from where the content is (ADR 0050, decision 5).
        let leg = e.smooth.take().is_some();
        if e.glide.take().is_some() || leg {
            e.offset = e.laid;
        }
        e.offset.x += delta.x;
        e.offset.y += delta.y;
        // An offset written between two frames counts as a declaration: it
        // is usually a `set_scroll` at a key this frame is about to build.
        e.last_declared = frame_no;
    }

    /// A wheel's delta (ADR 0050): `whole` when it is at least a notch on
    /// an axis it moves. A whole notch glides — added to the target,
    /// clamped to the travel, the drawn place chasing it from where it
    /// is — and so does any delta while a glide runs, so a fraction does
    /// not jump the content back. A fraction otherwise, a key no layout
    /// has resolved, and a store with no clock (a driver that shows no
    /// animation) take it as [`Self::scroll_by`] does.
    pub(crate) fn wheel_by(&mut self, key: Key, delta: Vec2, whole: bool) {
        let frame_no = self.frame_no;
        let feel = self.feel;
        let glides = self.now.is_some()
            && self
                .entries
                .get(&key)
                .is_some_and(|e| e.geom.is_some() && (whole || e.glide.is_some()));
        if !glides {
            return self.scroll_by(key, delta);
        }
        let e = self.entries.entry(key).or_default();
        let Some((_, _, max)) = e.geom else { return };
        e.asked_smooth = false;
        if e.smooth.take().is_some() {
            e.offset = e.laid;
        }
        let g = e.glide.get_or_insert(Glide {
            velocity: Vec2::ZERO,
            last: None,
        });
        // Clamped now, not at the next layout: a notch past the end is
        // travel a reversed wheel would have to undo before anything
        // moved.
        let from = Vec2::new(e.offset.x.clamp(0.0, max.x), e.offset.y.clamp(0.0, max.y));
        e.offset.x = (from.x + delta.x).clamp(0.0, max.x);
        e.offset.y = (from.y + delta.y).clamp(0.0, max.y);
        // The speed that makes this notch's own share ease out, times the
        // feel's kick (`WheelFeel::kick`): what the target moved, so a
        // notch the end swallowed adds none.
        let omega = feel.kick * feel.omega() as f32;
        g.velocity.x += omega * (e.offset.x - from.x);
        g.velocity.y += omega * (e.offset.y - from.y);
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
        // every frame asked for the next.
        if smooth && e.smooth.is_some() && e.offset == offset {
            return;
        }
        e.asked_smooth = smooth;
        if !smooth {
            e.smooth = None;
        }
        // A programmatic write ends a glide: an eased one starts its leg
        // from the drawn place at the next layout (ADR 0050, decision 5).
        e.glide = None;
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
    /// — what [`Self::animating`] is made of, for a trace.
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

    /// The retained offset an `anchor` container was left at its end with
    /// by the last layout, if it was (see the entry's `tail`).
    pub(crate) fn tail(&self, key: Key) -> Option<f32> {
        self.entries.get(&key).and_then(|e| e.tail)
    }

    /// Records whether this layout left `key` at its end, and the offset.
    pub(crate) fn set_tail(&mut self, key: Key, tail: Option<f32>) {
        self.entries.entry(key).or_default().tail = tail;
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
        let omega = self.feel.omega();
        let e = self.entries.entry(key).or_default();
        e.last_declared = frame_no;
        e.geom = Some((rect, content, Vec2::new(max.x.max(0.0), max.y.max(0.0))));
        e.offset.x = e.offset.x.clamp(0.0, max.x.max(0.0));
        e.offset.y = e.offset.y.clamp(0.0, max.y.max(0.0));
        // Where the content is drawn: the offset itself, unless the
        // container asked for a transition and the move was a
        // programmatic one — then it starts where the last frame left it
        // and eases to the offset over the leg.
        e.laid_frame = frame_no;
        // A wheel's glide, whatever the container's transition says: its
        // target clamped to the travel this layout found, so content that
        // shrank under it lands at its new end.
        if let (Some(g), Some(now)) = (e.glide, now) {
            let clamp = |v: Vec2| {
                Vec2::new(
                    v.x.clamp(0.0, max.x.max(0.0)),
                    v.y.clamp(0.0, max.y.max(0.0)),
                )
            };
            let (at, velocity, settled) = glide_step(clamp(e.laid), g, e.offset, now, omega);
            e.glide = (!settled).then_some(Glide {
                velocity,
                last: Some(now),
            });
            if !settled && !self.owing.contains(&key) {
                self.owing.push(key);
            }
            e.laid = at;
            return at;
        }
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
                        // for the rest of the leg.
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
