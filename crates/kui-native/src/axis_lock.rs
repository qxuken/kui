//! A trackpad swipe keeps to the axis it started on (backlog F104).
//!
//! A finger on a trackpad never moves along one axis alone: a swipe
//! meant straight down carries a few pixels sideways with it, and the
//! momentum after it carries them on. Handed through as they come, they
//! move whatever scrolls sideways under the pointer — since DX13 a
//! vertical list passes the x it cannot use to the strip it sits in, so
//! scrolling a list nudged the strip, and an editor's text column taking
//! the whole delta drifted off its left edge. macOS's own scroll views
//! lock to the predominant axis for the same reason.
//!
//! So a swipe is locked to one axis once it has travelled a few pixels,
//! the larger of the two it travelled, and the other axis is dropped for
//! as long as the swipe and its momentum last — a pause of [`GAP`] ends
//! it. Sticky, not fixed: when the other axis carries [`SWITCH_RATIO`]
//! times the locked one's recent travel, and at least [`SWITCH_MIN`], the
//! hand has turned and the lock turns with it. Only pixel deltas are
//! locked — a trackpad's, a Magic Mouse's; a wheel's notches are lines,
//! one axis at a time already (Shift turns them sideways on purpose), and
//! pass whole.

use std::time::{Duration, Instant};

use kui_core::Vec2;

/// A pause this long between two events ends the swipe: the next one
/// chooses its axis afresh. Momentum events come a frame apart, so a
/// swipe's glide keeps the lock its hand set.
pub(crate) const GAP: Duration = Duration::from_millis(200);
/// Travel (logical px, both axes summed) before an axis is chosen. Until
/// then each event keeps its own larger component.
pub(crate) const DECIDE: f32 = 4.0;
/// How much of the recent travel each event keeps: about the last half
/// dozen events count.
pub(crate) const DECAY: f32 = 0.75;
/// The other axis turns the lock when its recent travel is this many
/// times the locked axis's…
pub(crate) const SWITCH_RATIO: f32 = 3.0;
/// …and at least this much (logical px).
pub(crate) const SWITCH_MIN: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

/// One window's current swipe.
#[derive(Debug, Default)]
pub(crate) struct AxisLock {
    /// The axis the swipe keeps to; `None` until it has travelled
    /// [`DECIDE`].
    axis: Option<Axis>,
    /// When the last event came.
    last: Option<Instant>,
    /// Travel since the swipe began, per axis, while undecided.
    travel: (f32, f32),
    /// Recent travel per axis, [`DECAY`]ed an event.
    recent: (f32, f32),
}

impl AxisLock {
    /// A wheel's notch: nothing to lock, and any swipe before it is over.
    pub(crate) fn line(&mut self) {
        *self = AxisLock::default();
    }

    /// A trackpad's delta, with the axis it is not locked to dropped.
    pub(crate) fn pixel(&mut self, d: Vec2, now: Instant) -> Vec2 {
        if self
            .last
            .is_some_and(|t| now.saturating_duration_since(t) > GAP)
        {
            *self = AxisLock::default();
        }
        self.last = Some(now);
        let (ax, ay) = (d.x.abs(), d.y.abs());
        self.recent = (self.recent.0 * DECAY + ax, self.recent.1 * DECAY + ay);
        let axis = match self.axis {
            None => {
                self.travel = (self.travel.0 + ax, self.travel.1 + ay);
                let (tx, ty) = self.travel;
                if tx + ty >= DECIDE {
                    let a = if tx > ty { Axis::X } else { Axis::Y };
                    self.axis = Some(a);
                    a
                } else if ax > ay {
                    Axis::X
                } else {
                    Axis::Y
                }
            }
            Some(a) => {
                let (along, across, other) = match a {
                    Axis::X => (self.recent.0, self.recent.1, Axis::Y),
                    Axis::Y => (self.recent.1, self.recent.0, Axis::X),
                };
                if across >= SWITCH_MIN && across > along * SWITCH_RATIO {
                    self.axis = Some(other);
                    other
                } else {
                    a
                }
            }
        };
        match axis {
            Axis::X => Vec2::new(d.x, 0.0),
            Axis::Y => Vec2::new(0.0, d.y),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: Duration = Duration::from_millis(8);

    /// Feeds `n` equal events a frame apart from `t`, summing what passed.
    fn swipe(l: &mut AxisLock, t: &mut Instant, d: Vec2, n: usize) -> Vec2 {
        let mut out = Vec2::ZERO;
        for _ in 0..n {
            *t += FRAME;
            let p = l.pixel(d, *t);
            out = Vec2::new(out.x + p.x, out.y + p.y);
        }
        out
    }

    /// The case F104 is about: a swipe down that drifts sideways moves
    /// down only, its momentum too.
    #[test]
    fn a_vertical_swipe_drops_its_drift() {
        let mut t = Instant::now();
        let mut l = AxisLock::default();
        let hand = swipe(&mut l, &mut t, Vec2::new(1.5, -12.0), 20);
        assert_eq!(hand, Vec2::new(0.0, -240.0));
        let glide = swipe(&mut l, &mut t, Vec2::new(0.6, -3.0), 30);
        assert_eq!(glide.x, 0.0, "momentum keeps the hand's axis");
        let drift = swipe(&mut l, &mut t, Vec2::new(-4.0, -5.0), 10);
        assert_eq!(drift.x, 0.0, "sideways under the ratio stays dropped");
    }

    /// Sideways is locked the same way.
    #[test]
    fn a_sideways_swipe_drops_its_vertical_drift() {
        let mut t = Instant::now();
        let mut l = AxisLock::default();
        let out = swipe(&mut l, &mut t, Vec2::new(9.0, 2.0), 10);
        assert_eq!(out, Vec2::new(90.0, 0.0));
    }

    /// Before the swipe has travelled `DECIDE`, each event keeps its own
    /// larger component, so the first pixels are not lost.
    #[test]
    fn an_undecided_swipe_keeps_each_events_larger_axis() {
        let t = Instant::now();
        let mut l = AxisLock::default();
        assert_eq!(l.pixel(Vec2::new(0.5, 1.0), t), Vec2::new(0.0, 1.0));
        assert_eq!(l.pixel(Vec2::new(1.0, 0.5), t + FRAME), Vec2::new(1.0, 0.0));
        // Travel is now 3.0; the next event decides on the larger sum.
        assert_eq!(
            l.pixel(Vec2::new(0.2, 1.0), t + FRAME * 2),
            Vec2::new(0.0, 1.0)
        );
        assert_eq!(l.axis, Some(Axis::Y));
    }

    /// A hand that turns without lifting turns the lock once the other
    /// axis clearly carries the motion.
    #[test]
    fn a_turn_the_other_axis_clearly_carries_turns_the_lock() {
        let mut t = Instant::now();
        let mut l = AxisLock::default();
        swipe(&mut l, &mut t, Vec2::new(0.0, 10.0), 10);
        let turned = swipe(&mut l, &mut t, Vec2::new(10.0, 1.0), 10);
        assert!(turned.x > 0.0, "the turn goes through: {turned:?}");
        assert_eq!(l.axis, Some(Axis::X));
        let last = l.pixel(Vec2::new(10.0, 1.0), t + FRAME);
        assert_eq!(last, Vec2::new(10.0, 0.0), "and is locked in turn");
    }

    /// A pause ends the swipe: the next one chooses afresh.
    #[test]
    fn a_pause_ends_the_swipe() {
        let mut t = Instant::now();
        let mut l = AxisLock::default();
        swipe(&mut l, &mut t, Vec2::new(0.0, 10.0), 10);
        t += GAP + FRAME;
        let out = swipe(&mut l, &mut t, Vec2::new(6.0, 1.0), 3);
        assert_eq!(out, Vec2::new(18.0, 0.0));
    }

    /// A wheel's notch ends a swipe too.
    #[test]
    fn a_notch_ends_the_swipe() {
        let mut t = Instant::now();
        let mut l = AxisLock::default();
        swipe(&mut l, &mut t, Vec2::new(0.0, 10.0), 10);
        l.line();
        let out = swipe(&mut l, &mut t, Vec2::new(6.0, 1.0), 3);
        assert_eq!(out, Vec2::new(18.0, 0.0));
    }
}
