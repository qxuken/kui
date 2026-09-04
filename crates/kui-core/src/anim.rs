//! Transitions: retained tweens keyed by node identity. A node that declares
//! `NodeSpec::transition` has its animatable spec values (sizing amounts,
//! colors, radius, opacity, shadow) eased from whatever they were last frame toward what the
//! view declares this frame — the *inputs* to layout animate, so a subtree
//! lays out consistently every frame instead of children snapping to a
//! target size inside a still-moving parent.
//!
//! Two kinds of motion: timed curves ([`Easing::EaseOut`] and friends,
//! which replay a leg from wherever the value was over `duration_ms`) and
//! springs ([`Easing::Spring`], [`Easing::Bouncy`]), integrated per frame
//! with a velocity that survives retargets — a value chased mid-flight
//! keeps its momentum instead of restarting, which is what dragged and
//! reordered things want. For springs `duration_ms` is the response time.
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
    /// A damped spring (damping ratio 0.75: a hint of overshoot).
    /// `duration_ms` is the response time; velocity carries across
    /// retargets.
    Spring,
    /// A springier spring (damping ratio 0.5).
    Bouncy,
}

impl Easing {
    /// Damping ratio for the spring easings; None for timed curves.
    fn damping(self) -> Option<f32> {
        match self {
            Easing::Spring => Some(0.75),
            Easing::Bouncy => Some(0.5),
            _ => None,
        }
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
            Easing::Spring | Easing::Bouncy => 1.0 - (1.0 - t).powi(3),
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

/// How a node's animatable values move when the view changes them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub duration_ms: f32,
    pub easing: Easing,
    /// How the node's keyframes cycle (nothing without keyframes).
    pub repeat: Repeat,
    /// Holds the keyframe cycle back by this many ms, so siblings given
    /// different delays run out of phase (CSS's `animation-delay`).
    pub delay_ms: f32,
}

impl Transition {
    pub fn ms(duration_ms: f32) -> Self {
        Self {
            duration_ms,
            easing: Easing::EaseOut,
            repeat: Repeat::Normal,
            delay_ms: 0.0,
        }
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn repeat(mut self, repeat: Repeat) -> Self {
        self.repeat = repeat;
        self
    }

    pub fn delay(mut self, delay_ms: f32) -> Self {
        self.delay_ms = delay_ms;
        self
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
}

const SLOTS: usize = 9;

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
    transition: Transition,
    last_used: u64,
}

impl Tween {
    /// Advances a spring toward `to` from `last_time` to `now`; returns
    /// whether it is still moving. Semi-implicit Euler on a unit-mass
    /// spring with stiffness and damping from the response time and ratio
    /// (SwiftUI's parametrization).
    fn spring_step(&mut self, now: f64, zeta: f32) -> bool {
        let response = (self.transition.duration_ms.max(1.0) / 1000.0) as f64;
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

/// Evict tweens not driven for this many frames.
const EVICT_AFTER_FRAMES: u64 = 300;

#[derive(Default)]
pub struct AnimStore {
    tweens: FxHashMap<Key, [Option<Tween>; SLOTS]>,
    /// Driver time in seconds; None until the driver first sets it.
    now: Option<f64>,
    frame_no: u64,
    /// Whether any tween driven this frame is still mid-flight.
    active: bool,
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

    /// True when the last frame left a transition mid-flight, i.e. the
    /// driver should schedule another frame without waiting for input.
    pub fn animating(&self) -> bool {
        self.active
    }

    pub(crate) fn begin_frame(&mut self) {
        self.frame_no += 1;
        self.active = false;
        if !self.tweens.is_empty() && self.frame_no.is_multiple_of(240) {
            let cutoff = self.frame_no.saturating_sub(EVICT_AFTER_FRAMES);
            self.tweens
                .retain(|_, slots| slots.iter().flatten().any(|t| t.last_used >= cutoff));
        }
    }

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
    pub(crate) fn sample(&mut self, track: &Track, transition: Transition) -> Option<[f32; 4]> {
        let dur = transition.duration_ms as f64 / 1000.0;
        let (Some(now), true, Some(&(_, first))) = (self.now, dur > 0.0, track.first()) else {
            return None;
        };
        // A cycle never finishes, so the driver owes another frame while
        // a keyframed node is on screen.
        self.active = true;
        let u = (now - transition.delay_ms as f64 / 1000.0) / dur;
        let p = transition.repeat.progress(u);
        let mut from = (0.0, first);
        for &(at, value) in track {
            if p < at {
                let (a, va) = from;
                let t = if at > a { (p - a) / (at - a) } else { 1.0 };
                let e = transition.easing.apply(t);
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
        let frame_no = self.frame_no;
        let now = self.now;
        let slots = self.tweens.entry(key).or_default();
        let entry = &mut slots[slot as usize];
        let stale = entry.is_none_or(|t| t.last_used + 1 < frame_no);
        let Some(now) = now else {
            *entry = Some(Tween {
                from: target,
                to: target,
                value: target,
                start: 0.0,
                velocity: [0.0; 4],
                last_time: 0.0,
                transition,
                last_used: frame_no,
            });
            return target;
        };
        if stale {
            match enter_from {
                // The entrance: a leg from the declared start, begun now.
                Some(from) if from != target => {
                    *entry = Some(Tween {
                        from,
                        to: target,
                        value: from,
                        start: now,
                        velocity: [0.0; 4],
                        last_time: now,
                        transition,
                        last_used: frame_no,
                    });
                }
                // Settled from the start: a leg that began infinitely long
                // ago is complete, so nothing is owed until a retarget.
                _ => {
                    *entry = Some(Tween {
                        from: target,
                        to: target,
                        value: target,
                        start: f64::NEG_INFINITY,
                        velocity: [0.0; 4],
                        last_time: now,
                        transition,
                        last_used: frame_no,
                    });
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
        if let Some(zeta) = transition.easing.damping() {
            // Springs retarget freely: the velocity carries over.
            tw.to = target;
            tw.transition = transition;
            if tw.spring_step(now, zeta) {
                self.active = true;
            }
            return tw.value;
        }
        if tw.to != target {
            tw.from = tw.value;
            tw.to = target;
            tw.start = now;
            tw.transition = transition;
        }
        let dur = tw.transition.duration_ms.max(0.0) as f64 / 1000.0;
        let p = if dur <= 0.0 {
            1.0
        } else {
            (((now - tw.start) / dur) as f32).clamp(0.0, 1.0)
        };
        if p < 1.0 {
            self.active = true;
            let e = tw.transition.easing.apply(p);
            for i in 0..4 {
                tw.value[i] = tw.from[i] + (tw.to[i] - tw.from[i]) * e;
            }
        } else {
            tw.value = tw.to;
        }
        tw.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        ] {
            assert_eq!(e.apply(0.0), 0.0);
            assert_eq!(e.apply(1.0), 1.0);
            assert!(e.apply(0.5) > 0.0 && e.apply(0.5) < 1.0);
        }
    }

    #[test]
    fn first_sight_snaps_then_retargets_ease() {
        let mut a = AnimStore::default();
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0).easing(Easing::Linear);
        a.set_time(0.0);
        a.begin_frame();
        assert_eq!(a.drive(k, Slot::Width, None, one(10.0), t, true)[0], 10.0);
        assert!(!a.animating());

        a.set_time(0.0);
        a.begin_frame();
        assert_eq!(a.drive(k, Slot::Width, None, one(20.0), t, true)[0], 10.0);
        assert!(a.animating(), "mid-flight after retarget");

        a.set_time(0.05);
        a.begin_frame();
        assert!((a.drive(k, Slot::Width, None, one(20.0), t, true)[0] - 15.0).abs() < 1e-4);

        a.set_time(0.2);
        a.begin_frame();
        assert_eq!(a.drive(k, Slot::Width, None, one(20.0), t, true)[0], 20.0);
        assert!(!a.animating(), "settled");
    }

    #[test]
    fn unchanged_targets_owe_no_frames() {
        let mut a = AnimStore::default();
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0);
        for i in 0..3 {
            a.set_time(i as f64 * 0.001);
            a.begin_frame();
            a.drive(k, Slot::Width, None, one(5.0), t, true);
            assert!(!a.animating(), "frame {i}: same value, nothing to animate");
        }
    }

    #[test]
    fn retarget_mid_flight_starts_from_current_value() {
        let mut a = AnimStore::default();
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0).easing(Easing::Linear);
        a.set_time(0.0);
        a.begin_frame();
        a.drive(k, Slot::Width, None, one(0.0), t, true);
        a.set_time(0.0);
        a.begin_frame();
        a.drive(k, Slot::Width, None, one(100.0), t, true);
        a.set_time(0.05);
        a.begin_frame();
        assert!((a.drive(k, Slot::Width, None, one(100.0), t, true)[0] - 50.0).abs() < 1e-4);
        // Reverse: eases back from 50, not from 100.
        a.set_time(0.05);
        a.begin_frame();
        assert!((a.drive(k, Slot::Width, None, one(0.0), t, true)[0] - 50.0).abs() < 1e-4);
        a.set_time(0.10);
        a.begin_frame();
        assert!((a.drive(k, Slot::Width, None, one(0.0), t, true)[0] - 25.0).abs() < 1e-4);
    }

    #[test]
    fn springs_overshoot_settle_and_keep_momentum() {
        let mut a = AnimStore::default();
        let k = Key::ROOT.str("x");
        let t = Transition::ms(200.0).easing(Easing::Bouncy);
        a.set_time(0.0);
        a.begin_frame();
        a.drive(k, Slot::Width, None, one(0.0), t, true);
        // Retarget to 100 and step at 60Hz.
        let mut max = 0.0f32;
        let mut settled_at = None;
        for i in 1..=180 {
            a.set_time(i as f64 / 60.0);
            a.begin_frame();
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
        a.begin_frame();
        assert_eq!(a.drive(k, Slot::Width, None, one(100.0), t, true)[0], 100.0);

        // Momentum: retargeting mid-flight continues from the current
        // velocity rather than restarting, so the value keeps moving up
        // for a moment even though the new target is behind it.
        a.set_time(4.0);
        a.begin_frame();
        a.drive(k, Slot::Width, None, one(200.0), t, true);
        let mut v_prev = 100.0;
        for i in 1..=2 {
            a.set_time(4.0 + i as f64 / 60.0);
            a.begin_frame();
            v_prev = a.drive(k, Slot::Width, None, one(200.0), t, true)[0];
        }
        assert!(v_prev > 100.0);
        a.set_time(4.0 + 3.0 / 60.0);
        a.begin_frame();
        let after = a.drive(k, Slot::Width, None, one(100.0), t, true)[0];
        assert!(
            after > v_prev,
            "momentum carries past the retarget: {v_prev} -> {after}"
        );
    }

    #[test]
    fn an_entrance_starts_its_first_leg_from_the_declared_value() {
        let mut a = AnimStore::default();
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0).easing(Easing::Linear);
        a.set_time(0.0);
        a.begin_frame();
        assert_eq!(
            a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, true)[0],
            -100.0,
            "first sight starts at the entrance"
        );
        assert!(a.animating(), "and owes a frame");
        a.set_time(0.05);
        a.begin_frame();
        assert!(
            (a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, true)[0] + 50.0).abs() < 1e-3
        );
        a.set_time(0.2);
        a.begin_frame();
        assert_eq!(
            a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, true)[0],
            0.0
        );
        assert!(!a.animating());
        // Once seen, the entrance is spent: a retarget eases from where it is.
        a.set_time(0.2);
        a.begin_frame();
        assert_eq!(
            a.drive(k, Slot::Pos, Some(one(-100.0)), one(40.0), t, true)[0],
            0.0
        );
        a.set_time(0.25);
        a.begin_frame();
        assert!(
            (a.drive(k, Slot::Pos, Some(one(-100.0)), one(40.0), t, true)[0] - 20.0).abs() < 1e-3
        );
    }

    #[test]
    fn an_entrance_equal_to_the_target_is_no_entrance() {
        let mut a = AnimStore::default();
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0);
        a.set_time(0.0);
        a.begin_frame();
        assert_eq!(
            a.drive(k, Slot::Bg, Some(one(5.0)), one(5.0), t, true)[0],
            5.0
        );
        assert!(!a.animating());
    }

    #[test]
    fn a_spring_entrance_carries_no_velocity_in() {
        let mut a = AnimStore::default();
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0).easing(Easing::Spring);
        a.set_time(0.0);
        a.begin_frame();
        assert_eq!(
            a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, true)[0],
            -100.0
        );
        a.set_time(0.016);
        a.begin_frame();
        let first = a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, true)[0];
        assert!(
            first > -100.0 && first < 0.0,
            "leaves the entrance toward the target: {first}"
        );
        assert!(a.animating());
        let mut v = first;
        for i in 2..=90 {
            a.set_time(i as f64 * 0.016);
            a.begin_frame();
            v = a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, true)[0];
        }
        assert!(v.abs() < 1.0, "settled on the target: {v}");
        assert!(!a.animating());
    }

    #[test]
    fn off_the_leash_a_settled_slot_snaps_to_a_new_target() {
        let mut a = AnimStore::default();
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0).easing(Easing::Linear);
        a.set_time(0.0);
        a.begin_frame();
        a.drive(k, Slot::Pos, Some(one(-100.0)), one(0.0), t, false);
        // Retargeted mid-entrance: still eases, a fresh leg from where it is.
        a.set_time(0.05);
        a.begin_frame();
        let at = a.drive(k, Slot::Pos, Some(one(-100.0)), one(20.0), t, false)[0];
        a.set_time(0.1);
        a.begin_frame();
        let mid = a.drive(k, Slot::Pos, Some(one(-100.0)), one(20.0), t, false)[0];
        assert!(
            at < mid && mid < 20.0,
            "mid-flight retarget keeps easing: {at} -> {mid}"
        );
        a.set_time(0.3);
        a.begin_frame();
        assert_eq!(
            a.drive(k, Slot::Pos, Some(one(-100.0)), one(20.0), t, false)[0],
            20.0
        );
        assert!(!a.animating());
        // Settled and moved by layout: a node that doesn't slide snaps.
        a.set_time(0.3);
        a.begin_frame();
        assert_eq!(
            a.drive(k, Slot::Pos, Some(one(-100.0)), one(300.0), t, false)[0],
            300.0
        );
        assert!(!a.animating());
    }

    #[test]
    fn a_slot_skipped_for_a_frame_snaps() {
        let mut a = AnimStore::default();
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0);
        a.set_time(0.0);
        a.begin_frame();
        a.drive(k, Slot::Width, None, one(0.0), t, true);
        // A frame without this node (or without its transition).
        a.set_time(0.01);
        a.begin_frame();
        a.set_time(0.02);
        a.begin_frame();
        assert_eq!(a.drive(k, Slot::Width, None, one(100.0), t, true)[0], 100.0);
        assert!(!a.animating());
    }

    /// Two stops, linear: the value reads as a straight function of the
    /// clock in each of CSS's four directions.
    #[test]
    fn keyframes_cycle_in_every_direction() {
        let track = [(0.0, one(0.0)), (1.0, one(100.0))];
        let at = |a: &mut AnimStore, repeat: Repeat, now: f64| {
            let t = Transition::ms(1000.0).easing(Easing::Linear).repeat(repeat);
            a.set_time(now);
            a.begin_frame();
            let v = a.sample(&track, t).unwrap()[0];
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
        let track = [(0.0, one(0.0)), (0.5, one(10.0)), (1.0, one(0.0))];
        let t = Transition::ms(1000.0).easing(Easing::Linear);
        a.set_time(0.25);
        a.begin_frame();
        assert!((a.sample(&track, t).unwrap()[0] - 5.0).abs() < 1e-3);
        // Held back 250ms: reads what an undelayed node read at 0.
        a.set_time(0.25);
        a.begin_frame();
        assert!((a.sample(&track, t.delay(250.0)).unwrap()[0]).abs() < 1e-3);
        // Ease-in per segment: a quarter of the way through the first leg
        // sits well below linear's 5.
        a.set_time(0.125);
        a.begin_frame();
        let v = a.sample(&track, t.easing(Easing::EaseIn)).unwrap()[0];
        assert!(v > 0.0 && v < 2.0, "{v}");
        // Stops that don't start at 0 hold the first value until they do.
        let late = [(0.5, one(3.0)), (1.0, one(9.0))];
        a.set_time(0.1);
        a.begin_frame();
        assert_eq!(a.sample(&late, t).unwrap()[0], 3.0);
    }

    #[test]
    fn keyframes_need_a_clock_and_a_duration() {
        let mut a = AnimStore::default();
        let track = [(0.0, one(0.0)), (1.0, one(1.0))];
        a.begin_frame();
        assert!(a.sample(&track, Transition::ms(100.0)).is_none());
        assert!(!a.animating());
        a.set_time(1.0);
        a.begin_frame();
        assert!(a.sample(&track, Transition::ms(0.0)).is_none());
        assert!(a.sample(&[], Transition::ms(100.0)).is_none());
        assert!(!a.animating());
    }

    #[test]
    fn no_clock_means_no_animation() {
        let mut a = AnimStore::default();
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0);
        a.begin_frame();
        a.drive(k, Slot::Width, None, one(0.0), t, true);
        a.begin_frame();
        assert_eq!(a.drive(k, Slot::Width, None, one(100.0), t, true)[0], 100.0);
        assert!(!a.animating());
    }
}
