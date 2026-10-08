//! Transitions: retained tweens keyed by node identity.
//!
//! A node that declares [`NodeSpec::transition`](crate::spec::NodeSpec::transition)
//! has its animatable spec values (sizing amounts, colours, radius, opacity,
//! shadow) eased from whatever they were last frame toward what the view
//! declares this frame. The *inputs* to layout animate, so a subtree lays
//! out consistently every frame instead of children snapping to a target
//! size inside a still-moving parent. The view keeps declaring the target;
//! nothing else is needed.
//!
//! ```rust
//! use kui_core::{Easing, NodeSpec, Sizing, Transition};
//!
//! // A sidebar whose width eases over 200 ms, keyed so the tween survives.
//! let open = true;
//! let w = if open { 240.0 } else { 48.0 };
//! let sidebar = NodeSpec::column().width(Sizing::Fixed(w)).transition(200.0);
//!
//! // A spring instead of a curve, with a little overshoot.
//! let springy = Transition::ms(350.0).easing(Easing::Spring).bounce(0.3);
//! let card = NodeSpec::row().transition_with(springy).slide();
//!
//! assert_eq!(sidebar.transition.unwrap().easing, Easing::EaseOut);
//! assert!(card.transition.unwrap().curve().is_spring() && card.slide);
//! ```
//!
//! Two kinds of motion: timed curves ([`Easing::EaseOut`] and friends,
//! which replay a leg from wherever the value was over `duration_ms`) and
//! springs ([`Easing::Smooth`], [`Easing::Snappy`], [`Easing::Spring`],
//! [`Easing::Bouncy`]), integrated per frame with a velocity that survives
//! retargets — a value chased mid-flight keeps its momentum instead of
//! restarting, which is what dragged and reordered things want.
//!
//! A spring takes the two numbers a person tunes by eye, not the physics:
//! `duration_ms`, how long it takes to get there (the response time), and
//! a [`Bounce`], how far it overshoots — 0 glides in, 0.5 bounces. Each
//! spring easing is a named bounce, and [`Transition::bounce`] sets any
//! other; the stiffness and damping follow from the two.
//!
//! A node can also declare [`crate::NodeSpec::keyframes`]: CSS-style
//! stops for any of the same slots, cycled over `duration_ms` in one of
//! CSS's four directions ([`Repeat`]). A keyframed slot is sampled straight
//! off the clock instead of retained as a tween, so nothing drifts, siblings
//! offset by `delay_ms` stay in phase with each other, and a view that
//! declares a pulse never has to wake up to flip a target.
//!
//! The core stays clock-free: the frame driver injects the time with
//! [`crate::Core::set_time`]. A driver that never does (headless tests, a C
//! host without a clock) sees every transition snap to its target.

use rustc_hash::FxHashMap;

use crate::key::Key;

/// Easing curve for a [`Transition`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Easing {
    /// Cubic ease-out: fast start, gentle landing (the UI default).
    #[default]
    EaseOut,
    Linear,
    EaseIn,
    EaseInOut,
    /// A spring with a hint of overshoot (bounce 0.25). `duration_ms` is
    /// how long it takes to get there; velocity carries across retargets.
    Spring,
    /// A spring that visibly bounces (bounce 0.5).
    Bouncy,
    /// A spring that glides in without overshooting (bounce 0): an
    /// ease-out that keeps its momentum when retargeted.
    Smooth,
    /// A quick spring with a trace of overshoot (bounce 0.15).
    Snappy,
}

impl Easing {
    /// Every curve, in wire order: `schema::EASINGS` is `ALL` by `name`,
    /// and a binding sends the index.
    pub const ALL: &'static [Easing] = &[
        Easing::EaseOut,
        Easing::Linear,
        Easing::EaseIn,
        Easing::EaseInOut,
        Easing::Spring,
        Easing::Bouncy,
        Easing::Smooth,
        Easing::Snappy,
    ];

    /// The camelCase spelling every binding uses.
    pub fn name(self) -> &'static str {
        match self {
            Easing::EaseOut => "easeOut",
            Easing::Linear => "linear",
            Easing::EaseIn => "easeIn",
            Easing::EaseInOut => "easeInOut",
            Easing::Spring => "spring",
            Easing::Bouncy => "bouncy",
            Easing::Smooth => "smooth",
            Easing::Snappy => "snappy",
        }
    }

    /// The variant `schema::EASINGS` index `i` names; the default for an
    /// index this build lacks.
    pub fn from_index(i: usize) -> Easing {
        Self::ALL.get(i).copied().unwrap_or_default()
    }

    /// The bounce a spring easing has unless [`Transition::bounce`] says
    /// otherwise; None for the timed curves.
    pub fn bounce(self) -> Option<f32> {
        match self {
            Easing::Smooth => Some(0.0),
            Easing::Snappy => Some(0.15),
            Easing::Spring => Some(0.25),
            Easing::Bouncy => Some(0.5),
            Easing::EaseOut | Easing::Linear | Easing::EaseIn | Easing::EaseInOut => None,
        }
    }

    /// Whether this is a spring, integrated with momentum, rather than a
    /// timed curve.
    pub fn is_spring(self) -> bool {
        self.bounce().is_some()
    }

    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseOut => 1.0 - (1.0 - t).powi(3),
            Easing::EaseIn => t * t * t,
            Easing::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            // Springs are integrated, not sampled; as a curve, ease out.
            Easing::Spring | Easing::Bouncy | Easing::Smooth | Easing::Snappy => {
                1.0 - (1.0 - t).powi(3)
            }
        }
    }
}

/// How keyframes cycle: CSS's `animation-direction`, always infinite.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Repeat {
    /// Forward, then jump back and replay.
    #[default]
    Normal,
    /// Backward, then jump forward and replay.
    Reverse,
    /// Forward, then backward, forever (the easing reverses with it).
    Alternate,
    /// Backward first, then forward.
    AlternateReverse,
}

impl Repeat {
    /// Every direction, in wire order: `schema::REPEATS` is `ALL` by
    /// `name`.
    pub const ALL: &'static [Repeat] = &[
        Repeat::Normal,
        Repeat::Reverse,
        Repeat::Alternate,
        Repeat::AlternateReverse,
    ];

    /// CSS's `animation-direction` spelling, camelCased.
    pub fn name(self) -> &'static str {
        match self {
            Repeat::Normal => "normal",
            Repeat::Reverse => "reverse",
            Repeat::Alternate => "alternate",
            Repeat::AlternateReverse => "alternateReverse",
        }
    }

    /// The variant `schema::REPEATS` index `i` names; the default for an
    /// index this build lacks.
    pub fn from_index(i: usize) -> Repeat {
        Self::ALL.get(i).copied().unwrap_or_default()
    }

    /// Where in the keyframe cycle (0..=1) a clock reading lands, with `u`
    /// in periods.
    fn progress(self, u: f64) -> f32 {
        // `rem_euclid` so a delay past the clock's origin still lands in
        // the cycle rather than running it backwards.
        let p = match self {
            Repeat::Normal => u.rem_euclid(1.0),
            Repeat::Reverse => 1.0 - u.rem_euclid(1.0),
            Repeat::Alternate => {
                let c = u.rem_euclid(2.0);
                if c < 1.0 { c } else { 2.0 - c }
            }
            Repeat::AlternateReverse => {
                let c = u.rem_euclid(2.0);
                if c < 1.0 { 1.0 - c } else { c - 1.0 }
            }
        };
        p as f32
    }
}

/// Spring integration step (seconds); a frame is split into steps this
/// long so a stiff spring stays stable at any frame rate.
const SPRING_STEP: f64 = 0.004;
/// A pause longer than this (window hidden, debugger) doesn't get replayed
/// as one giant step.
const MAX_FRAME_DT: f64 = 0.1;

/// The most bounce a spring takes: at 1 it would never settle, and past
/// 0.9 it rings for seconds.
pub const MAX_BOUNCE: f32 = 0.9;

/// How far a spring overshoots, 0 (glides in, no overshoot) to
/// [`MAX_BOUNCE`] (rings a while): the one number that shapes a spring
/// besides its duration. A bounce `b` is a damping ratio of `1 - b`
/// (SwiftUI's `Spring(duration:bounce:)`).
///
/// Held in ten-thousandths in two bytes, because it rides in
/// [`Transition`], which rides inline in every `NodeSpec` — and the two
/// bytes of padding `Transition` had spare are all the room there is
/// (`node_spec_stays_small`). The niche keeps `Option<Bounce>` at two.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bounce(std::num::NonZeroU16);

impl Bounce {
    /// `b` clamped to 0..=[`MAX_BOUNCE`]; NaN reads as 0.
    pub fn new(b: f32) -> Self {
        let q = (b.clamp(0.0, MAX_BOUNCE) * 10_000.0).round() as u16;
        Bounce(std::num::NonZeroU16::MIN.saturating_add(q))
    }

    pub fn get(self) -> f32 {
        (self.0.get() - 1) as f32 / 10_000.0
    }
}

/// How a node's animatable values move when the view changes them: a
/// duration, an easing (a timed curve or a spring), and for keyframes a
/// repeat direction and a delay. Built with [`Transition::ms`] and the
/// builders, or through the `NodeSpec` shorthands (`transition`, `easing`,
/// `bounce`, `repeat`, `delay`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    /// How long a timed curve takes; for a spring, its response time —
    /// about how long it takes to get there.
    pub duration_ms: f32,
    pub easing: Easing,
    /// How the node's keyframes cycle (nothing without keyframes).
    pub repeat: Repeat,
    /// A spring's bounce in place of its easing's own; on a timed curve
    /// it makes the transition a spring ([`Self::curve`]). None is the
    /// easing's.
    pub bounce: Option<Bounce>,
    /// Holds the keyframe cycle back by this many ms, so siblings given
    /// different delays run out of phase (CSS's `animation-delay`).
    pub delay_ms: f32,
}

impl Transition {
    /// A cubic ease-out over `duration_ms`.
    pub fn ms(duration_ms: f32) -> Self {
        Self {
            duration_ms,
            easing: Easing::EaseOut,
            repeat: Repeat::Normal,
            bounce: None,
            delay_ms: 0.0,
        }
    }

    /// The curve or spring to move by.
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// How keyframes cycle (CSS's `animation-direction`).
    pub fn repeat(mut self, repeat: Repeat) -> Self {
        self.repeat = repeat;
        self
    }

    /// Holds a keyframe cycle back by `delay_ms` (CSS's `animation-delay`).
    pub fn delay(mut self, delay_ms: f32) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// Springs with this bounce, 0 (no overshoot) to [`MAX_BOUNCE`]: on a
    /// spring easing in place of its own, and on a timed one in place of
    /// the curve, since a bounce is only a spring's to have.
    pub fn bounce(mut self, bounce: f32) -> Self {
        self.bounce = Some(Bounce::new(bounce));
        self
    }

    /// The easing this transition moves by: its `easing`, or
    /// [`Easing::Spring`] when a bounce was given to a timed curve.
    pub fn curve(&self) -> Easing {
        match self.bounce {
            Some(_) if !self.easing.is_spring() => Easing::Spring,
            _ => self.easing,
        }
    }

    /// The damping ratio when this transition is a spring (`1 - bounce`);
    /// None for a timed curve.
    fn damping(&self) -> Option<f32> {
        let own = self.curve().bounce()?;
        Some(1.0 - self.bounce.map_or(own, Bounce::get))
    }
}

/// A keyframed slot: `(offset, value)` stops with offsets ascending from 0
/// to 1 (see [`crate::keyframes`], which builds them).
pub(crate) type Track = [(f32, [f32; 4])];

/// Which spec value a tween drives. One node can animate several at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Slot {
    Width = 0,
    Height = 1,
    Bg = 2,
    Border = 3,
    Radius = 4,
    Pos = 5,
    Opacity = 6,
    /// The shadow's geometry as one vector: dx, dy, blur, spread.
    Shadow = 7,
    ShadowColor = 8,
    /// The node's turn in turns and its scale, as one vector: rotate,
    /// scale, two spare (ADR 0043). Not one of the [`SLOTS`] every
    /// transitioning node carries: retained beside them, in
    /// `AnimStore::turns`, for the nodes that turn.
    Transform = 9,
}

/// The slots retained per transitioning node, in a fixed array: every slot
/// but [`Slot::Transform`]. A slot here is 96 bytes on every node that
/// declares a `transition`, whether or not it ever drives that slot, and
/// the tenth cost a frame of 10,000 transitioning boxes 2.6% when it was
/// one of them (ADR 0043's amendment).
const SLOTS: usize = 9;

impl Slot {
    /// Every slot, at its index: the [`SLOTS`] retained in the array, then
    /// the transform.
    const ALL: [Slot; SLOTS + 1] = [
        Slot::Width,
        Slot::Height,
        Slot::Bg,
        Slot::Border,
        Slot::Radius,
        Slot::Pos,
        Slot::Opacity,
        Slot::Shadow,
        Slot::ShadowColor,
        Slot::Transform,
    ];

    /// The names of the slots set in `mask` (bit `slot as u16`), in slot
    /// order.
    pub(crate) fn names(mask: u16) -> Vec<&'static str> {
        Self::ALL
            .into_iter()
            .filter(|s| mask & (1 << *s as u16) != 0)
            .map(Slot::name)
            .collect()
    }

    /// The prop the slot eases, as a person reads it in a trace
    /// ([`crate::runtime::cause::FrameHolder::slots`]).
    pub(crate) fn name(self) -> &'static str {
        match self {
            Slot::Width => "width",
            Slot::Height => "height",
            Slot::Bg => "bg",
            Slot::Border => "borderColor",
            Slot::Radius => "radius",
            Slot::Pos => "position",
            Slot::Opacity => "opacity",
            Slot::Shadow => "shadow",
            Slot::ShadowColor => "shadowColor",
            Slot::Transform => "transform",
        }
    }
}

/// One slot's retained motion. Ten of these per transitioning node, held
/// across frames, so what is *not* on it matters: a 10,000-node frame with
/// a transition on every node walks the lot of them.
///
/// Two fields of the [`Transition`] and not the transition itself. A leg
/// keeps the curve it started under — that is what makes retargeting a
/// running tween continuous, and why this cannot be one value on the node
/// — but a leg only ever reads `duration_ms` and `easing`. `repeat` and
/// `delay_ms` belong to the keyframe cycle, which is sampled straight off
/// the clock ([`sample_track`]) and never reaches a `Tween` at all. Both
/// were being copied into every slot of every node to be read by nobody.
///
/// `last_used` stays per slot, and is not the same redundancy: a node does
/// not drive all nine every frame — `Core::ease_positions` drives
/// `Slot::Pos` on its own, and a slot the node's keyframes name is sampled
/// instead of driven — so staleness, which is what makes a skipped slot
/// snap rather than resume, is per slot too.
#[derive(Clone, Copy, Debug)]
struct Tween {
    from: [f32; 4],
    to: [f32; 4],
    value: [f32; 4],
    /// Time the current leg started, in the driver's seconds.
    start: f64,
    /// Springs: velocity per component and the time last integrated to.
    velocity: [f32; 4],
    last_time: f64,
    /// The leg's own curve: `Transition::duration_ms` and `easing` as they
    /// were when it started.
    duration_ms: f32,
    easing: Easing,
    last_used: u64,
}

impl Tween {
    /// A tween at rest on `value`: a leg that began at `start` — `0.0`
    /// with no clock, infinitely long ago with one, so nothing is owed
    /// until a retarget.
    fn settled(value: [f32; 4], start: f64, last_time: f64, t: Transition, frame_no: u64) -> Self {
        Tween {
            from: value,
            to: value,
            value,
            start,
            velocity: [0.0; 4],
            last_time,
            duration_ms: t.duration_ms,
            easing: t.curve(),
            last_used: frame_no,
        }
    }

    /// A leg from `from` to `to`, begun `now` on the transition's curve.
    fn leg(from: [f32; 4], to: [f32; 4], now: f64, t: Transition, frame_no: u64) -> Self {
        Tween {
            from,
            to,
            value: from,
            start: now,
            velocity: [0.0; 4],
            last_time: now,
            duration_ms: t.duration_ms,
            easing: t.curve(),
            last_used: frame_no,
        }
    }

    /// How far along its leg the tween is at `now`, 0..=1: complete when
    /// the leg has no duration or began infinitely long ago.
    #[inline]
    fn progress_at(&self, now: f64) -> f32 {
        let dur = self.duration_ms.max(0.0) as f64 / 1000.0;
        if dur <= 0.0 {
            1.0
        } else {
            (((now - self.start) / dur) as f32).clamp(0.0, 1.0)
        }
    }

    /// The leg's value at progress `p`, eased.
    #[inline]
    fn at(&self, p: f32) -> [f32; 4] {
        if p >= 1.0 {
            return self.to;
        }
        let e = self.easing.apply(p);
        std::array::from_fn(|i| self.from[i] + (self.to[i] - self.from[i]) * e)
    }

    /// Where this tween's current leg stands at `now`, without touching it.
    /// A settled leg (`start` infinitely far back) reads as its target.
    fn eased_at(&self, now: f64) -> [f32; 4] {
        self.at(self.progress_at(now))
    }

    /// Advances a spring toward `to` from `last_time` to `now`; returns
    /// whether it is still moving. Semi-implicit Euler on a unit-mass
    /// spring with stiffness and damping from the response time and ratio
    /// (SwiftUI's parametrization).
    fn spring_step(&mut self, now: f64, zeta: f32) -> bool {
        let response = (self.duration_ms.max(1.0) / 1000.0) as f64;
        let omega = std::f64::consts::TAU / response;
        let k = (omega * omega) as f32;
        let c = (2.0 * zeta as f64 * omega) as f32;
        let dt = (now - self.last_time).clamp(0.0, MAX_FRAME_DT);
        self.last_time = now;
        let steps = (dt / SPRING_STEP).ceil().max(1.0);
        let h = (dt / steps) as f32;
        for _ in 0..steps as usize {
            for i in 0..4 {
                let a = -k * (self.value[i] - self.to[i]) - c * self.velocity[i];
                self.velocity[i] += a * h;
                self.value[i] += self.velocity[i] * h;
            }
        }
        // Settled: within a hair of the target and nearly still (units are
        // px or 0..1 color channels; per second for velocity).
        let moving = (0..4)
            .any(|i| (self.value[i] - self.to[i]).abs() > 1e-3 || self.velocity[i].abs() > 5e-2);
        if !moving {
            self.value = self.to;
            self.velocity = [0.0; 4];
        }
        moving
    }
}

#[derive(Default)]
pub struct AnimStore {
    tweens: FxHashMap<Key, [Option<Tween>; SLOTS]>,
    /// The transform slot's tweens (ADR 0043), by node, for the nodes that
    /// turn: kept apart from `tweens` so the nodes that do not — almost
    /// all of them — carry nothing for it.
    turns: FxHashMap<Key, Option<Tween>>,
    /// Driver time in seconds; None until the driver first sets it.
    now: Option<f64>,
    /// The core's frame counter as of `begin_frame`.
    frame_no: u64,
    /// The clock this frame drives at, as of `begin_frame`: what
    /// [`Self::owing`] reads a leg's progress at once the frame is over,
    /// since the driver sets the next frame's time before that frame
    /// begins.
    drove_at: Option<f64>,
    /// Whether any tween driven this frame is still mid-flight — a
    /// finite leg, or a spring not yet at rest.
    owes_transition: bool,
    /// Whether a keyframed slot was sampled this frame. A cycle has no
    /// end, so this is set on every frame the node is drawn; kept apart
    /// from the flag above so a test can wait for the transitions to run
    /// out under a cycle that never will.
    owes_cycle: bool,
}

impl AnimStore {
    /// Sets the frame clock (monotonic seconds, any origin). Drivers call it
    /// before each frame; never calling it means transitions snap.
    pub fn set_time(&mut self, now_secs: f64) {
        self.now = Some(now_secs);
    }

    pub fn time(&self) -> Option<f64> {
        self.now
    }

    /// True when the last frame left a transition mid-flight or a cycle
    /// running, i.e. the driver should schedule another frame without
    /// waiting for input.
    pub fn animating(&self) -> bool {
        self.owes_transition || self.owes_cycle
    }

    /// The two halves of [`animating`](Self::animating): a finite
    /// transition still mid-flight, and a keyframe cycle running.
    pub fn owes(&self) -> (bool, bool) {
        (self.owes_transition, self.owes_cycle)
    }

    /// Starts a frame: `frame_no` is the core's counter, stamped on what
    /// this frame drives.
    pub(crate) fn begin_frame(&mut self, frame_no: u64) {
        self.frame_no = frame_no;
        self.drove_at = self.now;
        self.owes_transition = false;
        self.owes_cycle = false;
        // Tweens nothing has driven for a while go (`retain::sweep_cutoff`).
        if !self.tweens.is_empty()
            && let Some(cutoff) = crate::retain::sweep_cutoff(self.frame_no)
        {
            self.tweens
                .retain(|_, slots| slots.iter().flatten().any(|t| t.last_used >= cutoff));
        }
        if !self.turns.is_empty()
            && let Some(cutoff) = crate::retain::sweep_cutoff(self.frame_no)
        {
            self.turns
                .retain(|_, t| t.is_some_and(|t| t.last_used >= cutoff));
        }
    }

    /// Every slot the last frame drove and left mid-flight, by node: what
    /// [`Self::owes`]'s `transition` half is made of, read back off the
    /// tweens rather than recorded while they were driven, so a frame
    /// nobody traces pays nothing for it. Asked between
    /// frames, or before this store's `begin_frame`: the same test
    /// `drive` made — a leg short of its end at the frame's clock, a
    /// spring not yet snapped to rest.
    pub(crate) fn owing(&self, mut each: impl FnMut(Key, Slot)) {
        let Some(now) = self.drove_at.or(self.now) else {
            return;
        };
        let moving = |tw: &Tween| {
            tw.last_used == self.frame_no
                && if tw.easing.is_spring() {
                    tw.value != tw.to || tw.velocity != [0.0; 4]
                } else {
                    tw.progress_at(now) < 1.0
                }
        };
        for (&key, slots) in &self.tweens {
            for (i, tw) in slots.iter().enumerate() {
                if tw.as_ref().is_some_and(moving) {
                    each(key, Slot::ALL[i]);
                }
            }
        }
        for (&key, tw) in &self.turns {
            if tw.as_ref().is_some_and(moving) {
                each(key, Slot::Transform);
            }
        }
    }

    /// Eases `key`'s `slot` toward `target`, returning the value to use this
    /// frame. A slot not driven last frame (node just appeared, or rendered
    /// without a transition in between) snaps: nothing animates in from
    /// nowhere, and a drag that disabled the transition doesn't replay —
    /// unless `enter_from` says where such a slot starts, in which case its
    /// first leg runs from there (an `enter` prop). `follow` off means the
    /// slot only eases that entrance: once it has settled, a new target
    /// snaps as it would without a transition — a node whose position
    /// enters but doesn't `slide`.
    pub(crate) fn drive(
        &mut self,
        key: Key,
        slot: Slot,
        enter_from: Option<[f32; 4]>,
        target: [f32; 4],
        transition: Transition,
        follow: bool,
    ) -> [f32; 4] {
        self.node(key)
            .drive(slot, enter_from, target, transition, follow)
    }

    /// The tween slots of one node, borrowed once.
    ///
    /// A node that declares a transition drives up to ten slots in a row
    /// (`Core::ease_transitioning`), and each of those used to ask the map
    /// for the same key — nine hashes and nine probes per node per frame,
    /// which on a frame where every node transitions was the largest single
    /// entry in the profile. The lookup happens here instead and the borrow
    /// serves every slot. `sample` rides along because a keyframed slot is
    /// reached from the same walk and sets a flag beside `active`'s, not
    /// because it needs the slots: a sampled track is not retained.
    pub(crate) fn node(&mut self, key: Key) -> NodeAnim<'_> {
        NodeAnim {
            slots: self.tweens.entry(key).or_default(),
            now: self.now,
            frame_no: self.frame_no,
            active: &mut self.owes_transition,
            cycling: &mut self.owes_cycle,
        }
    }

    /// [`NodeAnim::drive`] for the transform slot, whose tweens are kept
    /// apart (`turns`): the same leg, the same rules.
    pub(crate) fn drive_turn(
        &mut self,
        key: Key,
        enter_from: Option<[f32; 4]>,
        target: [f32; 4],
        transition: Transition,
    ) -> [f32; 4] {
        let entry = self.turns.entry(key).or_default();
        drive_entry(
            entry,
            self.now,
            self.frame_no,
            &mut self.owes_transition,
            enter_from,
            target,
            transition,
            true,
        )
    }

    /// [`NodeAnim::sample`] for the transform slot.
    pub(crate) fn sample_turn(
        &mut self,
        track: &Track,
        transition: Transition,
    ) -> Option<[f32; 4]> {
        let v = sample_track(track, transition, self.now);
        if v.is_some() {
            self.owes_cycle = true;
        }
        v
    }
}

/// Where `now` lands in `track`'s cycle, shaped by `transition`'s easing.
/// The walk behind [`NodeAnim::sample`]. None without a clock or a
/// duration.
fn sample_track(track: &Track, transition: Transition, now: Option<f64>) -> Option<[f32; 4]> {
    let dur = transition.duration_ms as f64 / 1000.0;
    let (Some(now), true, Some(&(_, first))) = (now, dur > 0.0, track.first()) else {
        return None;
    };
    let u = (now - transition.delay_ms as f64 / 1000.0) / dur;
    let p = transition.repeat.progress(u);
    let mut from = (0.0, first);
    for &(at, value) in track {
        if p < at {
            let (a, va) = from;
            let t = if at > a { (p - a) / (at - a) } else { 1.0 };
            let e = transition.curve().apply(t);
            let mut out = [0.0; 4];
            for i in 0..4 {
                out[i] = va[i] + (value[i] - va[i]) * e;
            }
            return Some(out);
        }
        from = (at, value);
    }
    Some(from.1)
}

/// One node's animation state for this frame: its retained tween slots,
/// the clock, and the store's "another frame is owed" flag. Made by
/// [`AnimStore::node`].
pub(crate) struct NodeAnim<'a> {
    slots: &'a mut [Option<Tween>; SLOTS],
    /// Driver time in seconds; None until the driver first sets it.
    now: Option<f64>,
    frame_no: u64,
    /// The store's "a transition is mid-flight" flag.
    active: &'a mut bool,
    /// The store's "a cycle is running" flag (F64).
    cycling: &'a mut bool,
}

impl NodeAnim<'_> {
    /// Samples a keyframed slot: where the clock lands in the cycle picks
    /// a segment of `track`, and the transition's easing shapes that
    /// segment (CSS applies the timing function per keyframe interval, so
    /// an alternate cycle traverses the same curve backwards on its way
    /// home). Sampled, not retained: this needs no key and cannot drift,
    /// so two nodes with the same duration stay locked together. Spring
    /// easings sample as ease-out — there is no leg to carry momentum
    /// across.
    ///
    /// None without a clock or a duration: the caller keeps the declared
    /// value, the same snap a plain transition gets.
    ///
    /// It is reached from a node borrow only because the caller holds one
    /// (`Core::ease_transitioning` walks a node's slots and its keyframed
    /// ones together) and because the flag it sets lives behind that
    /// borrow — the *cycle* flag, not the transition one: a cycle never
    /// ends, and a wait for the transitions to run out must not wait on
    /// it. The walk itself is [`sample_track`] and takes no
    /// key.
    pub(crate) fn sample(&mut self, track: &Track, transition: Transition) -> Option<[f32; 4]> {
        let v = sample_track(track, transition, self.now);
        if v.is_some() {
            *self.cycling = true;
        }
        v
    }

    pub(crate) fn drive(
        &mut self,
        slot: Slot,
        enter_from: Option<[f32; 4]>,
        target: [f32; 4],
        transition: Transition,
        follow: bool,
    ) -> [f32; 4] {
        // The transform slot is not in the array (see `SLOTS`); its
        // tweens are driven through `AnimStore::drive_turn`.
        debug_assert!(slot != Slot::Transform);
        drive_entry(
            &mut self.slots[slot as usize],
            self.now,
            self.frame_no,
            self.active,
            enter_from,
            target,
            transition,
            follow,
        )
    }
}

/// One slot's leg driven toward `target` this frame — what
/// [`NodeAnim::drive`] and [`AnimStore::drive_turn`] both are, over the
/// entry each keeps. Inlined: it is the body of the first, which is asked
/// up to nine times per transitioning node per frame.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn drive_entry(
    entry: &mut Option<Tween>,
    now: Option<f64>,
    frame_no: u64,
    active: &mut bool,
    enter_from: Option<[f32; 4]>,
    target: [f32; 4],
    transition: Transition,
    follow: bool,
) -> [f32; 4] {
    {
        let stale = entry.is_none_or(|t| t.last_used + 1 < frame_no);
        let Some(now) = now else {
            *entry = Some(Tween::settled(target, 0.0, 0.0, transition, frame_no));
            return target;
        };
        if stale {
            match enter_from {
                // The entrance: a leg from the declared start, begun now.
                Some(from) if from != target => {
                    *entry = Some(Tween::leg(from, target, now, transition, frame_no));
                }
                // Settled from the start: a leg that began infinitely long
                // ago is complete, so nothing is owed until a retarget.
                _ => {
                    *entry = Some(Tween::settled(
                        target,
                        f64::NEG_INFINITY,
                        now,
                        transition,
                        frame_no,
                    ));
                    return target;
                }
            }
        }
        let tw = entry.as_mut().expect("checked above");
        tw.last_used = frame_no;
        if !follow && tw.to != target && tw.value == tw.to && tw.velocity == [0.0; 4] {
            // Off the leash: settled, and the view moved it — snap.
            tw.from = target;
            tw.to = target;
            tw.value = target;
            tw.start = f64::NEG_INFINITY;
            tw.last_time = now;
            return target;
        }
        if let Some(zeta) = transition.damping() {
            // Springs retarget freely: the velocity carries over.
            tw.to = target;
            tw.duration_ms = transition.duration_ms;
            tw.easing = transition.curve();
            if tw.spring_step(now, zeta) {
                *active = true;
            }
            return tw.value;
        }
        if tw.to != target {
            // Where the running leg stands *at `now`*, not where it stood
            // when it was last sampled. The difference is the whole frame:
            // a new leg starts at `p == 0`, so retargeting from the stale
            // value spends none of the elapsed time — and a target the view
            // moves every frame (a canvas of `slide` floats under a drag)
            // then retargets every frame and never advances at all. Backlog
            // F15: the pan was live in the model and frozen on screen.
            tw.from = tw.eased_at(now);
            tw.to = target;
            tw.start = now;
            tw.duration_ms = transition.duration_ms;
            tw.easing = transition.curve();
        }
        let p = tw.progress_at(now);
        if p < 1.0 {
            *active = true;
        }
        tw.value = tw.at(p);
        tw.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The core's frame counter, stood in for: each test's frames count
    /// from one.
    struct Frames(u64);
    impl Frames {
        fn next(&mut self) -> u64 {
            self.0 += 1;
            self.0
        }
    }

    /// `Tween` is retained per slot, nine slots per transitioning node, and
    /// walked in full every frame the node is driven: a 10,000-node frame
    /// with a transition on every node carries 8.6 MB of it. So a field
    /// added here is paid nine times per node forever, and this is the
    /// number a review can fail — the same guard `NodeSpec` carries, for
    /// the same reason (C15).
    ///
    /// Raise it only for something a *leg* reads. What belongs to the node
    /// rather than the leg goes on the node, and what belongs to the
    /// keyframe cycle goes nowhere near here: `Transition::repeat` and
    /// `delay_ms` used to ride along and were read by nobody.
    #[test]
    fn a_tween_stays_small() {
        const BOUND: usize = 96;
        let size = std::mem::size_of::<Option<Tween>>();
        assert!(
            size <= BOUND,
            "Option<Tween> is {size} bytes, over the {BOUND}-byte bound. \
             It is held per slot per node across frames; put what the node \
             owns on the node and what the cycle owns in the Transition."
        );
        // The discriminant rides in `Easing`'s niche rather than widening
        // the struct; a field ordering that loses that is worth noticing.
        assert_eq!(size, std::mem::size_of::<Tween>());
    }

    fn one(v: f32) -> [f32; 4] {
        [v, 0.0, 0.0, 0.0]
    }

    #[test]
    fn easings_hit_their_endpoints() {
        for e in [
            Easing::Linear,
            Easing::EaseOut,
            Easing::EaseIn,
            Easing::EaseInOut,
            Easing::Spring,
            Easing::Bouncy,
            Easing::Smooth,
            Easing::Snappy,
        ] {
            assert_eq!(e.apply(0.0), 0.0);
            assert_eq!(e.apply(1.0), 1.0);
            assert!(e.apply(0.5) > 0.0 && e.apply(0.5) < 1.0);
        }
    }

    #[test]
    fn first_sight_snaps_then_retargets_ease() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0).easing(Easing::Linear);
        a.set_time(0.0);
        a.begin_frame(frame.next());
        assert_eq!(a.drive(k, Slot::Width, None, one(10.0), t, true)[0], 10.0);
        assert!(!a.animating());

        a.set_time(0.0);
        a.begin_frame(frame.next());
        assert_eq!(a.drive(k, Slot::Width, None, one(20.0), t, true)[0], 10.0);
        assert!(a.animating(), "mid-flight after retarget");

        a.set_time(0.05);
        a.begin_frame(frame.next());
        assert!((a.drive(k, Slot::Width, None, one(20.0), t, true)[0] - 15.0).abs() < 1e-4);

        a.set_time(0.2);
        a.begin_frame(frame.next());
        assert_eq!(a.drive(k, Slot::Width, None, one(20.0), t, true)[0], 20.0);
        assert!(!a.animating(), "settled");
    }

    #[test]
    fn unchanged_targets_owe_no_frames() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0);
        for i in 0..3 {
            a.set_time(i as f64 * 0.001);
            a.begin_frame(frame.next());
            a.drive(k, Slot::Width, None, one(5.0), t, true);
            assert!(!a.animating(), "frame {i}: same value, nothing to animate");
        }
    }

    #[test]
    fn retarget_mid_flight_starts_from_current_value() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0).easing(Easing::Linear);
        a.set_time(0.0);
        a.begin_frame(frame.next());
        a.drive(k, Slot::Width, None, one(0.0), t, true);
        a.set_time(0.0);
        a.begin_frame(frame.next());
        a.drive(k, Slot::Width, None, one(100.0), t, true);
        a.set_time(0.05);
        a.begin_frame(frame.next());
        assert!((a.drive(k, Slot::Width, None, one(100.0), t, true)[0] - 50.0).abs() < 1e-4);
        // Reverse: eases back from 50, not from 100.
        a.set_time(0.05);
        a.begin_frame(frame.next());
        assert!((a.drive(k, Slot::Width, None, one(0.0), t, true)[0] - 50.0).abs() < 1e-4);
        a.set_time(0.10);
        a.begin_frame(frame.next());
        assert!((a.drive(k, Slot::Width, None, one(0.0), t, true)[0] - 25.0).abs() < 1e-4);
    }

    #[test]
    fn springs_overshoot_settle_and_keep_momentum() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let k = Key::ROOT.str("x");
        let t = Transition::ms(200.0).easing(Easing::Bouncy);
        a.set_time(0.0);
        a.begin_frame(frame.next());
        a.drive(k, Slot::Width, None, one(0.0), t, true);
        // Retarget to 100 and step at 60Hz.
        let mut max = 0.0f32;
        let mut settled_at = None;
        for i in 1..=180 {
            a.set_time(i as f64 / 60.0);
            a.begin_frame(frame.next());
            let v = a.drive(k, Slot::Width, None, one(100.0), t, true)[0];
            max = max.max(v);
            if !a.animating() && settled_at.is_none() {
                settled_at = Some(i);
            }
        }
        assert!(max > 101.0, "bouncy overshoots: peak {max}");
        let settled = settled_at.expect("settles within 3s");
        assert!(settled > 6, "not instant: {settled}");
        a.set_time(4.0);
        a.begin_frame(frame.next());
        assert_eq!(a.drive(k, Slot::Width, None, one(100.0), t, true)[0], 100.0);

        // Momentum: retargeting mid-flight continues from the current
        // velocity rather than restarting, so the value keeps moving up
        // for a moment even though the new target is behind it.
        a.set_time(4.0);
        a.begin_frame(frame.next());
        a.drive(k, Slot::Width, None, one(200.0), t, true);
        let mut v_prev = 100.0;
        for i in 1..=2 {
            a.set_time(4.0 + i as f64 / 60.0);
            a.begin_frame(frame.next());
            v_prev = a.drive(k, Slot::Width, None, one(200.0), t, true)[0];
        }
        assert!(v_prev > 100.0);
        a.set_time(4.0 + 3.0 / 60.0);
        a.begin_frame(frame.next());
        let after = a.drive(k, Slot::Width, None, one(100.0), t, true)[0];
        assert!(
            after > v_prev,
            "momentum carries past the retarget: {v_prev} -> {after}"
        );
    }

    /// The highest a slot eased from 0 to 100 under `t` reads, stepped at
    /// 60Hz for three seconds; and whether it came to rest by then.
    fn peak(t: Transition) -> (f32, bool) {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let k = Key::ROOT.str("x");
        a.set_time(0.0);
        a.begin_frame(frame.next());
        a.drive(k, Slot::Width, None, one(0.0), t, true);
        let mut max = 0.0f32;
        for i in 1..=180 {
            a.set_time(i as f64 / 60.0);
            a.begin_frame(frame.next());
            max = max.max(a.drive(k, Slot::Width, None, one(100.0), t, true)[0]);
        }
        (max, !a.animating())
    }

    /// Each spring easing is a bounce, and the bounce is what a person
    /// reads off the screen: none glides in under the target, and more
    /// overshoots further. `spring` and `bouncy` keep the damping ratios
    /// they had before they were named bounces (0.75 and 0.5).
    #[test]
    fn a_spring_s_bounce_is_how_far_it_overshoots() {
        let t = Transition::ms(200.0);
        assert_eq!(t.easing(Easing::Spring).damping(), Some(0.75));
        assert_eq!(t.easing(Easing::Bouncy).damping(), Some(0.5));
        assert_eq!(t.damping(), None, "a timed curve is no spring");

        let (smooth, settled) = peak(t.easing(Easing::Smooth));
        assert!(settled, "smooth comes to rest");
        assert!(smooth <= 100.0 + 1e-3, "smooth never overshoots: {smooth}");
        let mut last = smooth;
        for e in [Easing::Snappy, Easing::Spring, Easing::Bouncy] {
            let (p, settled) = peak(t.easing(e));
            assert!(settled, "{e:?} comes to rest");
            assert!(
                p > last,
                "{e:?} overshoots past the one before: {p} <= {last}"
            );
            last = p;
        }
        // A bounce of its own replaces the easing's, either way.
        assert_eq!(
            peak(t.easing(Easing::Bouncy).bounce(0.0)).0,
            smooth,
            "bouncy with no bounce is smooth"
        );
        assert_eq!(
            peak(t.easing(Easing::Smooth).bounce(0.5)).0,
            last,
            "smooth with bouncy's bounce is bouncy"
        );
    }

    /// A bounce is only a spring's to have, so one given to a timed curve
    /// makes the transition a spring rather than being dropped: `transition`
    /// and `bounce` alone are a spring of that length and bounce.
    #[test]
    fn a_bounce_on_a_timed_curve_makes_it_a_spring() {
        let t = Transition::ms(200.0).easing(Easing::Linear).bounce(0.5);
        assert_eq!(t.curve(), Easing::Spring);
        assert_eq!(t.damping(), Some(0.5));
        assert_eq!(
            peak(t).0,
            peak(Transition::ms(200.0).easing(Easing::Bouncy)).0
        );
        assert_eq!(Transition::ms(200.0).curve(), Easing::EaseOut);
    }

    #[test]
    fn a_bounce_holds_its_range() {
        assert!((Bounce::new(0.3).get() - 0.3).abs() < 1e-4);
        assert_eq!(Bounce::new(0.0).get(), 0.0);
        assert_eq!(Bounce::new(-1.0).get(), 0.0);
        assert_eq!(Bounce::new(f32::NAN).get(), 0.0);
        assert_eq!(Bounce::new(1.0).get(), MAX_BOUNCE, "1 would never settle");
        let (_, settled) = peak(Transition::ms(100.0).easing(Easing::Spring).bounce(1.0));
        assert!(settled, "the most bounce there is still comes to rest");
    }

    /// `Transition` rides inline in every `NodeSpec`, which sits at its
    /// bound (`node_spec_stays_small`): the bounce had to fit in the two
    /// bytes of padding it had spare.
    #[test]
    fn a_transition_carries_its_bounce_in_its_padding() {
        assert_eq!(std::mem::size_of::<Option<Bounce>>(), 2);
        assert_eq!(std::mem::size_of::<Option<Transition>>(), 12);
    }

    #[test]
    fn an_entrance_starts_its_first_leg_from_the_declared_value() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0).easing(Easing::Linear);
        a.set_time(0.0);
        a.begin_frame(frame.next());
        assert_eq!(
            a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, true)[0],
            -100.0,
            "first sight starts at the entrance"
        );
        assert!(a.animating(), "and owes a frame");
        a.set_time(0.05);
        a.begin_frame(frame.next());
        assert!(
            (a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, true)[0] + 50.0).abs() < 1e-3
        );
        a.set_time(0.2);
        a.begin_frame(frame.next());
        assert_eq!(
            a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, true)[0],
            0.0
        );
        assert!(!a.animating());
        // Once seen, the entrance is spent: a retarget eases from where it is.
        a.set_time(0.2);
        a.begin_frame(frame.next());
        assert_eq!(
            a.drive(k, Slot::Pos, Some(one(-100.0)), one(40.0), t, true)[0],
            0.0
        );
        a.set_time(0.25);
        a.begin_frame(frame.next());
        assert!(
            (a.drive(k, Slot::Pos, Some(one(-100.0)), one(40.0), t, true)[0] - 20.0).abs() < 1e-3
        );
    }

    #[test]
    fn an_entrance_equal_to_the_target_is_no_entrance() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0);
        a.set_time(0.0);
        a.begin_frame(frame.next());
        assert_eq!(
            a.drive(k, Slot::Bg, Some(one(5.0)), one(5.0), t, true)[0],
            5.0
        );
        assert!(!a.animating());
    }

    #[test]
    fn a_spring_entrance_carries_no_velocity_in() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0).easing(Easing::Spring);
        a.set_time(0.0);
        a.begin_frame(frame.next());
        assert_eq!(
            a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, true)[0],
            -100.0
        );
        a.set_time(0.016);
        a.begin_frame(frame.next());
        let first = a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, true)[0];
        assert!(
            first > -100.0 && first < 0.0,
            "leaves the entrance toward the target: {first}"
        );
        assert!(a.animating());
        let mut v = first;
        for i in 2..=90 {
            a.set_time(i as f64 * 0.016);
            a.begin_frame(frame.next());
            v = a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, true)[0];
        }
        assert!(v.abs() < 1.0, "settled on the target: {v}");
        assert!(!a.animating());
    }

    #[test]
    fn off_the_leash_a_settled_slot_snaps_to_a_new_target() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0).easing(Easing::Linear);
        a.set_time(0.0);
        a.begin_frame(frame.next());
        a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, false);
        // Retargeted mid-entrance: still eases, a fresh leg from where it is.
        a.set_time(0.05);
        a.begin_frame(frame.next());
        let at = a.drive(k, Slot::Pos, Some(one(-100.0)), one(20.0), t, false)[0];
        a.set_time(0.1);
        a.begin_frame(frame.next());
        let mid = a.drive(k, Slot::Pos, Some(one(-100.0)), one(20.0), t, false)[0];
        assert!(
            at < mid && mid < 20.0,
            "mid-flight retarget keeps easing: {at} -> {mid}"
        );
        a.set_time(0.3);
        a.begin_frame(frame.next());
        assert_eq!(
            a.drive(k, Slot::Pos, Some(one(-100.0)), one(20.0), t, false)[0],
            20.0
        );
        assert!(!a.animating());
        // Settled and moved by layout: a node that doesn't slide snaps.
        a.set_time(0.3);
        a.begin_frame(frame.next());
        assert_eq!(
            a.drive(k, Slot::Pos, Some(one(-100.0)), one(300.0), t, false)[0],
            300.0
        );
        assert!(!a.animating());
    }

    /// A canvas of `slide` floats panned by
    /// a drag retargets every slot on every frame. Each retarget starts a
    /// fresh leg at `p == 0`, so a tween that reads its stale value spends
    /// none of the frame's time and never moves — the pan is live in the
    /// model and frozen on screen. It has to follow, a fixed distance back.
    #[test]
    fn a_target_that_moves_every_frame_still_advances() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let k = Key::ROOT.str("card");
        let t = Transition::ms(160.0);
        a.set_time(0.0);
        a.begin_frame(frame.next());
        a.drive(k, Slot::Pos, None, one(0.0), t, true);
        // 16 ms frames, 9.6 px of pan each: a second of a drag in flight.
        let mut behind = Vec::new();
        let mut drawn = 0.0;
        for f in 1..=60 {
            let target = f as f32 * 9.6;
            a.set_time(f as f64 * 0.016);
            a.begin_frame(frame.next());
            drawn = a.drive(k, Slot::Pos, None, one(target), t, true)[0];
            behind.push(target - drawn);
        }
        let target = 60.0 * 9.6;
        assert!(
            drawn > target * 0.8,
            "the pan reaches the screen: drawn {drawn} of {target}"
        );
        // And it trails by a fixed distance rather than falling further
        // behind every frame — one transition's worth of travel, no more.
        let (early, late) = (behind[29], behind[59]);
        assert!(
            (early - late).abs() < 1.0,
            "the gap stops growing: {early} then {late}"
        );
    }

    #[test]
    fn a_slot_skipped_for_a_frame_snaps() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0);
        a.set_time(0.0);
        a.begin_frame(frame.next());
        a.drive(k, Slot::Width, None, one(0.0), t, true);
        // A frame without this node (or without its transition).
        a.set_time(0.01);
        a.begin_frame(frame.next());
        a.set_time(0.02);
        a.begin_frame(frame.next());
        assert_eq!(a.drive(k, Slot::Width, None, one(100.0), t, true)[0], 100.0);
        assert!(!a.animating());
    }

    /// Two stops, linear: the value reads as a straight function of the
    /// clock in each of CSS's four directions.
    #[test]
    fn keyframes_cycle_in_every_direction() {
        let track = [(0.0, one(0.0)), (1.0, one(100.0))];
        let mut frame = Frames(0);
        let mut at = |a: &mut AnimStore, repeat: Repeat, now: f64| {
            let t = Transition::ms(1000.0).easing(Easing::Linear).repeat(repeat);
            a.set_time(now);
            a.begin_frame(frame.next());
            let v = a.node(Key::ROOT).sample(&track, t).unwrap()[0];
            assert!(a.animating(), "a keyframed slot always owes a frame");
            v
        };
        let mut a = AnimStore::default();
        for (repeat, expect) in [
            (Repeat::Normal, [0.0, 25.0, 50.0, 75.0, 0.0, 25.0]),
            (Repeat::Reverse, [100.0, 75.0, 50.0, 25.0, 100.0, 75.0]),
            (Repeat::Alternate, [0.0, 25.0, 50.0, 75.0, 100.0, 75.0]),
            (
                Repeat::AlternateReverse,
                [100.0, 75.0, 50.0, 25.0, 0.0, 25.0],
            ),
        ] {
            for (i, e) in expect.iter().enumerate() {
                let v = at(&mut a, repeat, i as f64 * 0.25);
                assert!((v - e).abs() < 1e-3, "{repeat:?} at {i}/4: {v} != {e}");
            }
        }
    }

    #[test]
    fn delay_shifts_the_cycle_and_easing_shapes_each_segment() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let track = [(0.0, one(0.0)), (0.5, one(10.0)), (1.0, one(0.0))];
        let t = Transition::ms(1000.0).easing(Easing::Linear);
        a.set_time(0.25);
        a.begin_frame(frame.next());
        assert!((a.node(Key::ROOT).sample(&track, t).unwrap()[0] - 5.0).abs() < 1e-3);
        // Held back 250ms: reads what an undelayed node read at 0.
        a.set_time(0.25);
        a.begin_frame(frame.next());
        assert!((a.node(Key::ROOT).sample(&track, t.delay(250.0)).unwrap()[0]).abs() < 1e-3);
        // Ease-in per segment: a quarter of the way through the first leg
        // sits well below linear's 5.
        a.set_time(0.125);
        a.begin_frame(frame.next());
        let v = a
            .node(Key::ROOT)
            .sample(&track, t.easing(Easing::EaseIn))
            .unwrap()[0];
        assert!(v > 0.0 && v < 2.0, "{v}");
        // Stops that don't start at 0 hold the first value until they do.
        let late = [(0.5, one(3.0)), (1.0, one(9.0))];
        a.set_time(0.1);
        a.begin_frame(frame.next());
        assert_eq!(a.node(Key::ROOT).sample(&late, t).unwrap()[0], 3.0);
    }

    #[test]
    fn keyframes_need_a_clock_and_a_duration() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let track = [(0.0, one(0.0)), (1.0, one(1.0))];
        a.begin_frame(frame.next());
        assert!(
            a.node(Key::ROOT)
                .sample(&track, Transition::ms(100.0))
                .is_none()
        );
        assert!(!a.animating());
        a.set_time(1.0);
        a.begin_frame(frame.next());
        assert!(
            a.node(Key::ROOT)
                .sample(&track, Transition::ms(0.0))
                .is_none()
        );
        assert!(
            a.node(Key::ROOT)
                .sample(&[], Transition::ms(100.0))
                .is_none()
        );
        assert!(!a.animating());
    }

    #[test]
    fn no_clock_means_no_animation() {
        let mut a = AnimStore::default();
        let mut frame = Frames(0);
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0);
        a.begin_frame(frame.next());
        a.drive(k, Slot::Width, None, one(0.0), t, true);
        a.begin_frame(frame.next());
        assert_eq!(a.drive(k, Slot::Width, None, one(100.0), t, true)[0], 100.0);
        assert!(!a.animating());
    }
}
