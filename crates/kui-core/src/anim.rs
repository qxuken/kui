//! Transitions: retained tweens keyed by node identity. A node that declares
//! `NodeSpec::transition` has its animatable spec values (sizing amounts,
//! colors, radius) eased from whatever they were last frame toward what the
//! view declares this frame — the *inputs* to layout animate, so a subtree
//! lays out consistently every frame instead of children snapping to a
//! target size inside a still-moving parent.
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
}

impl Easing {
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
        }
    }
}

/// How a node's animatable values move when the view changes them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub duration_ms: f32,
    pub easing: Easing,
}

impl Transition {
    pub fn ms(duration_ms: f32) -> Self {
        Self {
            duration_ms,
            easing: Easing::EaseOut,
        }
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

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
}

const SLOTS: usize = 6;

#[derive(Clone, Copy, Debug)]
struct Tween {
    from: [f32; 4],
    to: [f32; 4],
    value: [f32; 4],
    /// Time the current leg started, in the driver's seconds.
    start: f64,
    transition: Transition,
    last_used: u64,
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

    /// Eases `key`'s `slot` toward `target`, returning the value to use this
    /// frame. A slot not driven last frame (node just appeared, or rendered
    /// without a transition in between) snaps: nothing animates in from
    /// nowhere, and a drag that disabled the transition doesn't replay.
    pub(crate) fn drive(
        &mut self,
        key: Key,
        slot: Slot,
        target: [f32; 4],
        transition: Transition,
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
                transition,
                last_used: frame_no,
            });
            return target;
        };
        if stale {
            // Settled from the start: a leg that began infinitely long ago
            // is complete, so nothing is owed until a retarget.
            *entry = Some(Tween {
                from: target,
                to: target,
                value: target,
                start: f64::NEG_INFINITY,
                transition,
                last_used: frame_no,
            });
            return target;
        }
        let tw = entry.as_mut().expect("checked above");
        tw.last_used = frame_no;
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
        assert_eq!(a.drive(k, Slot::Width, one(10.0), t)[0], 10.0);
        assert!(!a.animating());

        a.set_time(0.0);
        a.begin_frame();
        assert_eq!(a.drive(k, Slot::Width, one(20.0), t)[0], 10.0);
        assert!(a.animating(), "mid-flight after retarget");

        a.set_time(0.05);
        a.begin_frame();
        assert!((a.drive(k, Slot::Width, one(20.0), t)[0] - 15.0).abs() < 1e-4);

        a.set_time(0.2);
        a.begin_frame();
        assert_eq!(a.drive(k, Slot::Width, one(20.0), t)[0], 20.0);
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
            a.drive(k, Slot::Width, one(5.0), t);
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
        a.drive(k, Slot::Width, one(0.0), t);
        a.set_time(0.0);
        a.begin_frame();
        a.drive(k, Slot::Width, one(100.0), t);
        a.set_time(0.05);
        a.begin_frame();
        assert!((a.drive(k, Slot::Width, one(100.0), t)[0] - 50.0).abs() < 1e-4);
        // Reverse: eases back from 50, not from 100.
        a.set_time(0.05);
        a.begin_frame();
        assert!((a.drive(k, Slot::Width, one(0.0), t)[0] - 50.0).abs() < 1e-4);
        a.set_time(0.10);
        a.begin_frame();
        assert!((a.drive(k, Slot::Width, one(0.0), t)[0] - 25.0).abs() < 1e-4);
    }

    #[test]
    fn a_slot_skipped_for_a_frame_snaps() {
        let mut a = AnimStore::default();
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0);
        a.set_time(0.0);
        a.begin_frame();
        a.drive(k, Slot::Width, one(0.0), t);
        // A frame without this node (or without its transition).
        a.set_time(0.01);
        a.begin_frame();
        a.set_time(0.02);
        a.begin_frame();
        assert_eq!(a.drive(k, Slot::Width, one(100.0), t)[0], 100.0);
        assert!(!a.animating());
    }

    #[test]
    fn no_clock_means_no_animation() {
        let mut a = AnimStore::default();
        let k = Key::ROOT.str("x");
        let t = Transition::ms(100.0);
        a.begin_frame();
        a.drive(k, Slot::Width, one(0.0), t);
        a.begin_frame();
        assert_eq!(a.drive(k, Slot::Width, one(100.0), t)[0], 100.0);
        assert!(!a.animating());
    }
}
