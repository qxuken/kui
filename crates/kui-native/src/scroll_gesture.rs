//! Where one scroll gesture ends and the next begins (backlog F107).
//!
//! The core latches a gesture's target when it begins and keeps it until
//! the next begins (`InputEvent::ScrollGesture`), but it keeps no clock:
//! the boundaries are the runner's to draw. The same ones F104's axis
//! lock draws for a swipe — a pause of [`GAP`] — so a swipe and the
//! momentum after it are one gesture, as they are one axis.
//!
//! A wheel's notches latch too, as a browser's do: a wheel spun without
//! a pause moves one scroller, and a list reaching its end under a
//! spinning wheel does not hand the rest of the spin to the page around
//! it. Two things end a wheel's gesture that do not end a swipe's: the
//! pointer moving — a hand on a mouse that moves it is aiming somewhere
//! new, where a swipe's momentum outlives the hand and must not jump to
//! whatever the pointer is moved over — and a switch between notches and
//! pixels, which are two devices.

use std::time::Instant;

use crate::axis_lock::GAP;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A wheel's `LineDelta` notches.
    Line,
    /// A trackpad's (or a Magic Mouse's) `PixelDelta`s.
    Pixel,
}

/// One window's scroll gesture: its kind, when its last event came, and
/// whether the next event dispatched begins one.
#[derive(Debug, Default)]
pub(crate) struct Gesture {
    last: Option<(Kind, Instant)>,
    /// A gesture began and no event of it has reached the core yet — an
    /// event the axis lock kept to zero is not dispatched, and the one
    /// after it must still say it begins.
    owed: bool,
}

impl Gesture {
    /// Whether an event of `kind` at `now`, dispatched, begins a
    /// gesture: the first event, one after a pause of [`GAP`], one of the
    /// other kind — or the first dispatched since one of those.
    pub(crate) fn begins(&mut self, kind: Kind, now: Instant) -> bool {
        self.note(kind, now);
        std::mem::take(&mut self.owed)
    }

    /// An event of `kind` at `now` that is not dispatched: it can begin
    /// a gesture, which the next dispatched one then says.
    pub(crate) fn note(&mut self, kind: Kind, now: Instant) {
        let begins = match self.last {
            Some((k, t)) => k != kind || now.saturating_duration_since(t) > GAP,
            None => true,
        };
        self.last = Some((kind, now));
        self.owed |= begins;
    }

    /// The pointer moved: a wheel's gesture is over, a swipe's is not.
    pub(crate) fn pointer_moved(&mut self) {
        if matches!(self.last, Some((Kind::Line, _))) {
            self.last = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const FRAME: Duration = Duration::from_millis(8);

    /// A swipe and its glide are one gesture; a pause ends it.
    #[test]
    fn a_swipe_and_its_momentum_are_one_gesture() {
        let mut g = Gesture::default();
        let mut t = Instant::now();
        assert!(g.begins(Kind::Pixel, t));
        for _ in 0..50 {
            t += FRAME;
            assert!(!g.begins(Kind::Pixel, t));
        }
        // Moving the pointer during the glide does not end it.
        g.pointer_moved();
        t += FRAME;
        assert!(!g.begins(Kind::Pixel, t));
        t += GAP + FRAME;
        assert!(g.begins(Kind::Pixel, t), "a pause begins the next");
    }

    /// A spun wheel is one gesture until it pauses, the pointer moves, or
    /// the trackpad takes over.
    #[test]
    fn a_spun_wheel_is_one_gesture_until_the_pointer_moves() {
        let mut g = Gesture::default();
        let mut t = Instant::now();
        assert!(g.begins(Kind::Line, t));
        t += Duration::from_millis(40);
        assert!(!g.begins(Kind::Line, t));
        g.pointer_moved();
        t += Duration::from_millis(40);
        assert!(g.begins(Kind::Line, t), "the hand aimed anew");
        t += Duration::from_millis(40);
        assert!(g.begins(Kind::Pixel, t), "another device");
        t += Duration::from_millis(40);
        assert!(g.begins(Kind::Line, t));
        t += GAP + FRAME;
        assert!(g.begins(Kind::Line, t));
    }

    /// A gesture whose first event was not dispatched (kept to zero)
    /// begins with the first one that is.
    #[test]
    fn an_undispatched_first_event_leaves_the_beginning_owed() {
        let mut g = Gesture::default();
        let mut t = Instant::now();
        assert!(g.begins(Kind::Pixel, t));
        t += GAP + FRAME;
        g.note(Kind::Pixel, t);
        t += FRAME;
        assert!(g.begins(Kind::Pixel, t));
        t += FRAME;
        assert!(!g.begins(Kind::Pixel, t));
    }
}
