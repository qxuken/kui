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
//!
//! A finger put down begins a gesture of its own, pause or none (backlog
//! F117). A glide's events come a frame apart, so a swipe started while
//! the last one still glides — over another scroller, the other way —
//! was the glide's gesture and went to the glide's target until a pause
//! came. winit names a glide's start `Started` as it names a finger's,
//! but the order tells them apart: a finger lifts (`Ended`) and the
//! glide starts in the frames after, where a finger comes down on
//! nothing lifted, or on a glide, which macOS ends as it lands. So a
//! `Started` within [`GLIDE_AFTER`] of a finger's `Ended` is the glide's,
//! and every other is a finger's.

use std::time::{Duration, Instant};

use winit::event::TouchPhase;

use crate::axis_lock::GAP;

/// How soon after the fingers lift a `Started` is the glide's rather
/// than a finger's: macOS starts the glide on the next event, and no hand
/// lifts and lands again this fast.
pub(crate) const GLIDE_AFTER: Duration = Duration::from_millis(100);

/// Where the hand is in a swipe, by the phases its events said.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Finger {
    /// No swipe, or one whose glide ended.
    #[default]
    Up,
    /// A finger on the surface.
    Down,
    /// The finger lifted at this instant; a glide may follow.
    Lifted(Instant),
    /// The glide after a lift.
    Glide,
}

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
    /// The hand, for telling a finger's `Started` from a glide's.
    finger: Finger,
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

    /// A pixel event's touch phase, before its delta is counted: whether
    /// it is a finger coming down, which begins a gesture of its own —
    /// and a swipe of its own, for the axis lock — however soon after the
    /// last event it comes.
    pub(crate) fn phase(&mut self, phase: TouchPhase, now: Instant) -> bool {
        let (finger, down) = match (phase, self.finger) {
            (TouchPhase::Started, Finger::Lifted(t))
                if now.saturating_duration_since(t) <= GLIDE_AFTER =>
            {
                (Finger::Glide, false)
            }
            // macOS says `MayBegin` and then `Began`: one finger.
            (TouchPhase::Started, Finger::Down) => (Finger::Down, false),
            (TouchPhase::Started, _) => (Finger::Down, true),
            (TouchPhase::Ended | TouchPhase::Cancelled, Finger::Down) => {
                (Finger::Lifted(now), false)
            }
            (TouchPhase::Ended | TouchPhase::Cancelled, Finger::Glide) => (Finger::Up, false),
            (_, f) => (f, false),
        };
        self.finger = finger;
        if down {
            self.last = None;
        }
        down
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

    /// Feeds one pixel event as the runner does, its phase first:
    /// whether it begins a gesture.
    fn event(g: &mut Gesture, phase: TouchPhase, t: Instant) -> bool {
        g.phase(phase, t);
        g.begins(Kind::Pixel, phase == TouchPhase::Started, t)
    }

    /// F117: a finger put down while the last swipe still glides begins a
    /// gesture of its own, pause or none; the glide after a lift does not.
    #[test]
    fn a_finger_down_mid_glide_begins_a_gesture() {
        use TouchPhase::*;
        let mut g = Gesture::default();
        let mut t = Instant::now();
        assert!(event(&mut g, Started, t), "a finger down");
        for _ in 0..10 {
            t += FRAME;
            assert!(!event(&mut g, Moved, t));
        }
        t += FRAME;
        assert!(!event(&mut g, Ended, t), "the lift");
        t += FRAME;
        assert!(!event(&mut g, Started, t), "the glide starting");
        for _ in 0..10 {
            t += FRAME;
            assert!(!event(&mut g, Moved, t));
        }
        // A finger lands on the glide: macOS ends it, then the touch.
        t += FRAME;
        assert!(!event(&mut g, Ended, t));
        t += FRAME;
        assert!(event(&mut g, Started, t), "a new swipe, no pause");
        t += FRAME;
        assert!(!event(&mut g, Started, t), "`MayBegin` then `Began`");
        assert!(!event(&mut g, Moved, t + FRAME));
    }

    /// A finger down on a glide whose end never came begins one too.
    #[test]
    fn a_finger_down_on_an_unended_glide_begins_a_gesture() {
        use TouchPhase::*;
        let mut g = Gesture::default();
        let mut t = Instant::now();
        event(&mut g, Started, t);
        t += FRAME;
        event(&mut g, Ended, t);
        t += FRAME;
        assert!(!event(&mut g, Started, t));
        t += FRAME;
        assert!(!event(&mut g, Moved, t));
        t += FRAME;
        assert!(event(&mut g, Started, t));
    }

    /// A lift with no glide, and a finger down again within the pause but
    /// past [`GLIDE_AFTER`]: two swipes.
    #[test]
    fn a_finger_down_after_a_glideless_lift_begins_a_gesture() {
        use TouchPhase::*;
        let mut g = Gesture::default();
        let mut t = Instant::now();
        event(&mut g, Started, t);
        t += FRAME;
        event(&mut g, Ended, t);
        // Within GAP of the lift, which alone would keep the gesture.
        t += GLIDE_AFTER + FRAME;
        assert!(event(&mut g, Started, t));
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
