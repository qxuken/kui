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
//!
//! Pixels are not always a touch surface. A mouse whose driver scrolls
//! smoothly on macOS, and a high-resolution wheel's steps between notches
//! on Wayland, report `PixelDelta` too, and winit does not say which
//! device sent them (backlog RG73). What does is the phase: a trackpad's
//! or a Magic Mouse's stream opens with `TouchPhase::Started` (a finger
//! down, or a momentum run starting), and a wheel's never does. So a
//! pixel gesture is a swipe once its stream has said `Started`, and until
//! then it is aimed like a wheel: moving the pointer ends it.

use std::time::Instant;

use crate::axis_lock::GAP;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A wheel's `LineDelta` notches.
    Line,
    /// `PixelDelta`s: a trackpad's or a Magic Mouse's, or a smooth
    /// wheel's (see the module's note).
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
    /// The gesture's events have said `TouchPhase::Started`: a finger on
    /// a touch surface, whose glide the pointer does not re-aim.
    touch: bool,
}

impl Gesture {
    /// Whether an event of `kind` at `now`, dispatched, begins a
    /// gesture: the first event, one after a pause of [`GAP`], one of the
    /// other kind — or the first dispatched since one of those. `started`
    /// is the event's `TouchPhase::Started`.
    pub(crate) fn begins(&mut self, kind: Kind, started: bool, now: Instant) -> bool {
        self.note(kind, started, now);
        std::mem::take(&mut self.owed)
    }

    /// An event of `kind` at `now` that is not dispatched: it can begin
    /// a gesture, which the next dispatched one then says.
    pub(crate) fn note(&mut self, kind: Kind, started: bool, now: Instant) {
        let begins = match self.last {
            Some((k, t)) => k != kind || now.saturating_duration_since(t) > GAP,
            None => true,
        };
        self.last = Some((kind, now));
        self.owed |= begins;
        self.touch = started || (self.touch && !begins);
    }

    /// The pointer moved: a wheel's gesture is over, and so is a pixel
    /// one no touch began; a swipe's is not.
    pub(crate) fn pointer_moved(&mut self) {
        match self.last {
            Some((Kind::Line, _)) => self.last = None,
            Some((Kind::Pixel, _)) if !self.touch => self.last = None,
            _ => {}
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
        assert!(g.begins(Kind::Pixel, true, t), "a finger down");
        for i in 0..50 {
            t += FRAME;
            // The momentum run says `Started` again as it takes over.
            assert!(!g.begins(Kind::Pixel, i == 20, t));
        }
        // Moving the pointer during the glide does not end it.
        g.pointer_moved();
        t += FRAME;
        assert!(!g.begins(Kind::Pixel, false, t));
        t += GAP + FRAME;
        assert!(g.begins(Kind::Pixel, true, t), "a pause begins the next");
    }

    /// A spun wheel is one gesture until it pauses, the pointer moves, or
    /// the trackpad takes over.
    #[test]
    fn a_spun_wheel_is_one_gesture_until_the_pointer_moves() {
        let mut g = Gesture::default();
        let mut t = Instant::now();
        assert!(g.begins(Kind::Line, false, t));
        t += Duration::from_millis(40);
        assert!(!g.begins(Kind::Line, false, t));
        g.pointer_moved();
        t += Duration::from_millis(40);
        assert!(g.begins(Kind::Line, false, t), "the hand aimed anew");
        t += Duration::from_millis(40);
        assert!(g.begins(Kind::Pixel, true, t), "another device");
        t += Duration::from_millis(40);
        assert!(g.begins(Kind::Line, false, t));
        t += GAP + FRAME;
        assert!(g.begins(Kind::Line, false, t));
    }

    /// A gesture whose first event was not dispatched (kept to zero)
    /// begins with the first one that is.
    #[test]
    fn an_undispatched_first_event_leaves_the_beginning_owed() {
        let mut g = Gesture::default();
        let mut t = Instant::now();
        assert!(g.begins(Kind::Pixel, true, t));
        t += GAP + FRAME;
        g.note(Kind::Pixel, true, t);
        t += FRAME;
        assert!(g.begins(Kind::Pixel, false, t));
        t += FRAME;
        assert!(!g.begins(Kind::Pixel, false, t));
    }

    /// RG73: pixels no touch began — a smooth-scrolling mouse on macOS,
    /// a high-resolution wheel between notches on Wayland — are aimed as
    /// a wheel is: moving the pointer ends the gesture. A swipe's glide
    /// still keeps its target, and a touch that begins later in a
    /// phaseless stream makes it a swipe from there.
    #[test]
    fn pixels_no_touch_began_end_when_the_pointer_moves() {
        let mut g = Gesture::default();
        let mut t = Instant::now();
        assert!(g.begins(Kind::Pixel, false, t));
        t += FRAME;
        assert!(!g.begins(Kind::Pixel, false, t), "spun on: one gesture");
        g.pointer_moved();
        t += FRAME;
        assert!(g.begins(Kind::Pixel, false, t), "the hand aimed anew");
        // A finger down within the gesture: a swipe from here on.
        t += FRAME;
        assert!(!g.begins(Kind::Pixel, true, t));
        g.pointer_moved();
        t += FRAME;
        assert!(!g.begins(Kind::Pixel, false, t), "a swipe keeps its target");
        // A pause, and phaseless pixels again: a wheel's gesture.
        t += GAP + FRAME;
        assert!(g.begins(Kind::Pixel, false, t));
        g.pointer_moved();
        t += FRAME;
        assert!(g.begins(Kind::Pixel, false, t));
    }
}
