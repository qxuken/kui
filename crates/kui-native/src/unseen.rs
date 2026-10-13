//! A frame built for a window that cannot draw, and not drawn (backlog
//! F169).
//!
//! The app's own work runs in its frames: a `view` that takes what a
//! sync thread or a folder watcher left and calls [`Waker::wake`] to be
//! run, or that asks to run again in two seconds with
//! `Ui::request_frame_at`. A frame is a redraw the platform delivers, and
//! a window that cannot be seen is delivered none: on Windows a minimized
//! window has no `WM_PAINT`, and the runner asks it for no animation
//! frames (RG45). So a minimized app heard nothing a thread brought it until it
//! was restored — Carnet's notes from another device waited on the
//! taskbar, and so did its timers.
//!
//! So a wake, or a time the view asked for, reaching a window the runner
//! knows cannot draw — minimized on Windows, dark where the platform says
//! (`Occluded`, macOS and X11; `Causes::went_dark`) — builds that
//! window's frame at the end of the loop's turn and does not draw it: the
//! view runs, the events it made reach the app, the title, the windows,
//! the sounds and the access tree it declared are applied, and nothing is
//! presented. The frame that brings the window back draws as before.
//!
//! Where the platform still delivers a dark window's redraws — macOS and
//! X11 deliver a covered window's, and its surface skips them (`mod
//! retry`) — the wake still asks for one, and the view runs in it at the
//! pace it always did; the unseen frame is the floor under it, built only
//! when no frame came within a [`GAP`] of the ask: macOS stops delivering
//! them some while after a minimize. A minimized window on Windows is
//! delivered none, so there the frame is built at once.
//!
//! Why the frame and not a hook of its own (an `App::woken`): what a wake
//! means already is "run `view`", in all five bindings, and an app that
//! keeps its work in the view keeps it there. A hook would be a second
//! door every binding grows, and an app whose work is in the view would
//! have to move it there to be heard while minimized. The frame is laid
//! out against the size the window last drew at, not the nothing a
//! minimized window reports on Windows, so no `resize` reaches the app
//! from a window it cannot see.
//!
//! Paced: an unseen frame has no display to wait for, and a thread can
//! wake the loop a thousand times a second, so a window builds at most
//! one every [`GAP`]. A wake inside it is held to the gap's end, where
//! the loop wakes for it, and every wake held there is one frame.
//!
//! Wayland reports no `Occluded`, so a window there is never dark to the
//! runner, and needs not be: winit delivers the redraw a wake asks for
//! whether or not the window is seen — the runner never asks it to wait
//! for the compositor's frame callback — and the surface skips a frame
//! it cannot present (`mod retry`). The view already runs there.
//!
//! [`Waker::wake`]: crate::Waker::wake

use std::time::{Duration, Instant};

use kui_core::{FrameCause, Size};

/// The least time between two frames built for a window that cannot
/// draw, and how long one waits for a redraw the platform may yet
/// deliver: four a second — enough for a hidden window's work, and more
/// than a browser gives a background tab's timers.
pub(crate) const GAP: Duration = Duration::from_millis(250);

/// What a window that cannot draw builds a frame for: the app asked for
/// its view to run (a `Waker`, a `wake_at`, the view's own
/// `request_frame_at`), or the last frame's events reached it and its
/// answer is the next view's. Not input, which a window nobody sees does
/// not get, and not what only a drawn frame would show — a caret's blink,
/// a retry, an animation, whose frames wait for the window to come back.
pub(crate) const ASKS: FrameCause = FrameCause::WAKE.union(FrameCause::AFTER_FRAME);

/// One window's unseen frames.
#[derive(Debug, Default)]
pub(crate) struct Unseen {
    /// A frame is asked for and none built since: when it was first asked.
    owed: Option<Instant>,
    /// When the last unseen one was built.
    last: Option<Instant>,
    /// What the window last drew at: its logical size and scale.
    drawn: Option<(Size, f32)>,
}

impl Unseen {
    /// A frame is asked for at `now`, to be built at a turn's end
    /// ([`Self::poll`]) unless one is built first.
    pub(crate) fn ask(&mut self, now: Instant) {
        self.owed.get_or_insert(now);
    }

    /// A frame was built for the window — a drawn one, a redraw the
    /// platform delivered, or the one that brings the window back: it
    /// answers whatever was asked here.
    pub(crate) fn answered(&mut self) {
        self.owed = None;
    }

    /// At the loop's end: whether to build the frame now, and when the
    /// loop must wake for it if not yet. `grace` is how long a frame the
    /// platform may yet deliver is waited for: none for a window it
    /// delivers none to, a [`GAP`] for one it may. Building it is the
    /// caller's.
    pub(crate) fn poll(&mut self, now: Instant, grace: Duration) -> (bool, Option<Instant>) {
        let Some(asked) = self.owed else {
            return (false, None);
        };
        let at = (asked + grace).max(self.last.map_or(asked, |t| t + GAP));
        if now < at {
            return (false, Some(at));
        }
        self.owed = None;
        self.last = Some(now);
        (true, None)
    }

    /// A drawn frame was laid out at `size` and `scale`: what an unseen
    /// one lays out at. A size with no area — what a minimized window
    /// reports on Windows — is not one to keep.
    pub(crate) fn drew(&mut self, size: Size, scale: f32) {
        if size.w > 0.0 && size.h > 0.0 {
            self.drawn = Some((size, scale));
        }
    }

    /// What an unseen frame lays out at: the size the window last drew
    /// at, or `window`'s own before it has drawn.
    pub(crate) fn size(&self, window: (Size, f32)) -> (Size, f32) {
        self.drawn.unwrap_or(window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wake to a window that cannot draw is built at the turn's end, at
    /// once after a quiet spell, and once however often it was asked.
    #[test]
    fn a_wake_is_one_frame_at_the_turns_end() {
        let t = Instant::now();
        let mut u = Unseen::default();
        assert_eq!(u.poll(t, Duration::ZERO), (false, None), "nothing asked");
        u.ask(t);
        u.ask(t);
        u.ask(t);
        assert_eq!(u.poll(t, Duration::ZERO), (true, None), "at once");
        assert_eq!(u.poll(t, Duration::ZERO), (false, None), "and once");
    }

    /// Wakes inside the gap are held to its end, where the loop wakes,
    /// and are one frame there: a thread waking without pause is four
    /// frames a second, not one a turn.
    #[test]
    fn wakes_without_pause_are_four_frames_a_second() {
        let t = Instant::now();
        let mut u = Unseen::default();
        let mut built = 0;
        let mut now = t;
        while now < t + Duration::from_secs(2) {
            u.ask(now);
            let (build, wake) = u.poll(now, Duration::ZERO);
            if build {
                built += 1;
            } else {
                let at = wake.expect("the loop wakes for a held frame");
                assert!(at > now && at <= now + GAP);
            }
            now += Duration::from_millis(1);
        }
        assert_eq!(built, 8, "two seconds, a frame a gap");
        // Held at the end: built when the gap is up, with no wake since.
        let (_, wake) = u.poll(now, Duration::ZERO);
        if let Some(at) = wake {
            assert_eq!(u.poll(at, Duration::ZERO), (true, None));
        }
    }

    /// A frame built for the window answers what was held for it: the
    /// drawn one that brings it back, or a redraw the platform delivered.
    #[test]
    fn a_frame_built_answers_a_held_one() {
        let t = Instant::now();
        let mut u = Unseen::default();
        u.ask(t);
        assert!(u.poll(t, Duration::ZERO).0);
        u.ask(t);
        assert!(u.poll(t + GAP / 2, Duration::ZERO).1.is_some(), "held");
        u.answered();
        assert_eq!(u.poll(t + GAP, Duration::ZERO), (false, None));
    }

    /// Where the platform may still deliver the redraw a wake asked for,
    /// the unseen frame waits a gap for it and is not built when it came;
    /// when none comes, it is the floor — four a second however long the
    /// platform stays silent.
    #[test]
    fn a_redraw_that_may_come_is_waited_for_a_gap() {
        let t = Instant::now();
        let mut u = Unseen::default();
        u.ask(t);
        assert_eq!(u.poll(t, GAP), (false, Some(t + GAP)), "waited for");
        // The platform delivered it: the view ran there.
        u.answered();
        assert_eq!(u.poll(t + GAP, GAP), (false, None));
        // It stopped delivering: the floor, a gap after each ask.
        let mut built = 0;
        let mut now = t + GAP;
        while now < t + GAP + Duration::from_secs(2) {
            u.ask(now);
            if u.poll(now, GAP).0 {
                built += 1;
            }
            now += Duration::from_millis(1);
        }
        assert!((7..=8).contains(&built), "{built} in two seconds");
    }

    /// An unseen frame lays out where the window last drew, not at the
    /// nothing a minimized window reports — and at the window's own size
    /// before it has drawn at all.
    #[test]
    fn an_unseen_frame_keeps_the_size_the_window_drew_at() {
        let mut u = Unseen::default();
        let window = (Size::new(800.0, 600.0), 2.0);
        assert_eq!(u.size(window), window, "not drawn yet");
        u.drew(Size::new(1024.0, 768.0), 2.0);
        u.drew(Size::new(0.0, 0.0), 2.0);
        assert_eq!(
            u.size((Size::new(0.0, 0.0), 2.0)),
            (Size::new(1024.0, 768.0), 2.0)
        );
    }
}
