//! A frame the surface would not take, asked for again.
//!
//! The surface skips a frame when it is occluded or its acquire times
//! out: there is nothing to present to, and the frame is dropped. Nothing
//! else asks for another — an app that draws on input has none coming —
//! so the window keeps the last frame it presented, which is not what
//! the view now says. A window ordered front reports itself occluded
//! for a beat or two before the platform catches up, so its first frame
//! was the case met first (a new window sat blank until a stray mouse
//! move); a window brought back with ⌘-Tab or Alt-Tab is the same beat,
//! and the frame its focus asked for was dropped the same way — it came
//! up showing the frame from before it went away.
//!
//! So a skipped frame is owed: the next one is asked for a retry apart,
//! and only so many times, until one lands. Paced rather than spun:
//! asking again the instant a try fails would burn every try in a
//! millisecond, which is exactly how long the platform has *not* had to
//! stop calling the window occluded. A window the platform says is
//! covered (`WindowEvent::Occluded(true)`, where it says so: macOS, X11)
//! owes nothing — it really is hidden, and retrying would only spin —
//! and when it says the window is uncovered, a frame is owed at once,
//! since what the window shows is whatever it presented before it was
//! covered.
//!
//! An animation is the other thing that asks while the surface skips,
//! and it asks every turn of the loop: a skip returns at once, with no
//! drawable to wait for and no vsync behind it, so an animating window
//! whose surface skips on a platform that never says it is covered
//! (Wayland), or whose acquires time out, built and dropped frames as
//! fast as the loop went round — about 8,300 in a second and a half, the
//! first time it was counted (backlog F103). Not Windows behind other
//! windows: DWM composes a covered window, so nothing is skipped and it
//! draws at the display's rate, as an uncovered one does (backlog RG128);
//! there it is an acquire that times out. So the animation's next frame after a skip
//! waits out the same retry interval ([`Retry::animation_waits`]): at
//! most one try a retry apart while the surface will not take them, and
//! back at the display's rate with the first frame that lands.

use std::time::{Duration, Instant};

/// How long to wait between tries, and how many to make: about a second
/// of a surface insisting there is nothing to present to, after which it
/// is taken at its word and the ordinary redraw path (a resize, an
/// expose, being uncovered, any input) is what wakes it.
pub(crate) const RETRY: Duration = Duration::from_millis(16);
pub(crate) const RETRIES: u32 = 60;

/// How long the skips must have stopped for the next one to be a new
/// frame asked for, not the same surface still insisting.
/// A window away long enough to spend its tries, brought back on a
/// platform that sends no `Occluded` (Windows, Wayland), skips the frame
/// its focus asks for as it comes up; that skip follows a quiet spell,
/// and gets tries of its own. A window skipping without pause — an
/// animation in a hidden window (F103) — never has one, and stays spent.
pub(crate) const REST: Duration = Duration::from_millis(500);

/// One window's owed frame.
#[derive(Debug)]
pub(crate) struct Retry {
    /// Tries left and when to make the next; `None` while nothing is
    /// owed. Kept at zero tries once they are spent, so the skips after
    /// do not start the count over.
    owed: Option<(u32, Instant)>,
    /// The platform said the window is covered.
    hidden: bool,
    /// A frame has been presented, ever.
    presented: bool,
    /// When the surface last skipped a frame.
    last_skip: Option<Instant>,
}

impl Retry {
    /// A window just made: its first frame is owed from now.
    pub(crate) fn new(now: Instant) -> Self {
        Retry {
            owed: Some((RETRIES, now)),
            hidden: false,
            presented: false,
            last_skip: None,
        }
    }

    /// Whether the window has ever presented a frame.
    pub(crate) fn presented_once(&self) -> bool {
        self.presented
    }

    /// A frame landed: nothing is owed, and an animation asks at the
    /// display's rate again.
    pub(crate) fn presented(&mut self) {
        self.owed = None;
        self.presented = true;
        self.last_skip = None;
    }

    /// Until when an animation's next frame waits, the surface having
    /// skipped the last one a retry ago or less: `None` once it has
    /// waited that long, or when the last frame landed. The loop wakes at
    /// the instant returned and asks then (backlog F103).
    pub(crate) fn animation_waits(&self, now: Instant) -> Option<Instant> {
        let at = self.last_skip? + RETRY;
        (now < at).then_some(at)
    }

    /// The surface skipped a frame: the next is owed a retry from now,
    /// unless one already is or the window is covered. Tries spent are
    /// given again to a skip that follows a [`REST`] with none.
    pub(crate) fn skipped(&mut self, now: Instant) {
        let rested = self
            .last_skip
            .is_none_or(|last| now.saturating_duration_since(last) >= REST);
        self.last_skip = Some(now);
        if self.hidden {
            return;
        }
        match self.owed {
            None => self.owed = Some((RETRIES, now + RETRY)),
            Some((0, _)) if rested => self.owed = Some((RETRIES, now + RETRY)),
            Some(_) => {}
        }
    }

    /// The platform says the window is covered, or no longer is.
    pub(crate) fn occluded(&mut self, covered: bool, now: Instant) {
        self.hidden = covered;
        self.owed = (!covered).then_some((RETRIES, now));
    }

    /// At the loop's end: whether to ask for a frame now, and when the
    /// loop must wake for the next try.
    pub(crate) fn poll(&mut self, now: Instant) -> (bool, Option<Instant>) {
        let Some((left, at)) = &mut self.owed else {
            return (false, None);
        };
        if *left == 0 {
            return (false, None);
        }
        let ask = now >= *at;
        if ask {
            *left -= 1;
            *at = now + RETRY;
        }
        (ask, Some(*at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asks(r: &mut Retry, now: Instant) -> bool {
        r.poll(now).0
    }

    /// A new window's first frame is asked for at once, then a retry
    /// apart while the surface skips, and no more once one lands.
    #[test]
    fn a_new_windows_first_frame_is_asked_for_until_it_lands() {
        let t = Instant::now();
        let mut r = Retry::new(t);
        assert!(asks(&mut r, t), "at once");
        r.skipped(t);
        assert!(!asks(&mut r, t + RETRY / 2), "not before the retry");
        assert!(asks(&mut r, t + RETRY), "a retry later");
        r.presented();
        assert!(r.presented_once());
        assert_eq!(r.poll(t + RETRY * 3), (false, None), "nothing owed");
    }

    /// The case F102 is about: a window that has presented skips a frame
    /// — the one its focus asked for as ⌘-Tab brought it back — and
    /// that frame is asked for again, where only a first frame was.
    #[test]
    fn a_frame_skipped_after_the_first_is_asked_for_again() {
        let t = Instant::now();
        let mut r = Retry::new(t);
        r.presented();
        r.skipped(t);
        let (ask, wake) = r.poll(t);
        assert!(!ask);
        assert_eq!(wake, Some(t + RETRY), "the loop wakes for it");
        assert!(asks(&mut r, t + RETRY));
        r.presented();
        assert_eq!(r.poll(t + RETRY * 2), (false, None));
    }

    /// The tries are bounded, and skips after them do not start the
    /// count over: a surface that stays unpresentable is not spun on.
    #[test]
    fn the_tries_run_out_and_stay_out() {
        let t = Instant::now();
        let mut r = Retry::new(t);
        r.presented();
        r.skipped(t);
        let mut now = t;
        let mut n = 0;
        for _ in 0..RETRIES * 2 {
            now += RETRY;
            if asks(&mut r, now) {
                n += 1;
            }
            // The surface goes on refusing every frame asked of it.
            r.skipped(now);
        }
        assert_eq!(n, RETRIES);
        r.skipped(now);
        assert_eq!(r.poll(now + RETRY), (false, None), "spent");
        // Skipping on without a pause — an animation in a hidden window
        // — stays spent.
        for i in 1..100u32 {
            r.skipped(now + RETRY * i);
        }
        assert_eq!(r.poll(now + RETRY * 100), (false, None), "still spent");
    }

    /// RG74: spent tries come back for a skip after a quiet spell. On a
    /// platform with no `Occluded`, a window away long enough to spend
    /// them came back behind a skipped frame and showed the stale one,
    /// F102's own symptom.
    #[test]
    fn a_skip_after_a_rest_is_owed_tries_of_its_own() {
        let t = Instant::now();
        let mut r = Retry::new(t);
        r.presented();
        r.skipped(t);
        let mut now = t;
        for _ in 0..RETRIES {
            now += RETRY;
            assert!(asks(&mut r, now));
            r.skipped(now);
        }
        assert_eq!(r.poll(now + RETRY), (false, None), "spent");
        // Seconds later the window comes back, and the frame its focus
        // asks for is skipped.
        let back = now + Duration::from_secs(10);
        r.skipped(back);
        assert!(!asks(&mut r, back + RETRY / 2));
        assert!(asks(&mut r, back + RETRY), "asked for again");
        r.presented();
        assert_eq!(r.poll(back + RETRY * 2), (false, None));
    }

    /// F103: an animation asks for its next frame a retry after a skip,
    /// not at once, however long the surface goes on skipping — and at
    /// once again from the first frame that lands.
    #[test]
    fn an_animation_waits_a_retry_after_a_skip() {
        let t = Instant::now();
        let mut r = Retry::new(t);
        r.presented();
        assert_eq!(r.animation_waits(t), None, "presenting: no wait");
        // Long past the tries: the surface never takes a frame, and an
        // animation is what goes on asking.
        let mut now = t;
        let mut asked = 0;
        while now < t + Duration::from_secs(3) {
            match r.animation_waits(now) {
                Some(at) => {
                    assert!(at > now && at <= now + RETRY);
                    now = at;
                }
                // A skip returns at once; the loop's next turn is tens of
                // microseconds on.
                None => {
                    asked += 1;
                    r.skipped(now);
                    now += Duration::from_micros(50);
                }
            }
        }
        let most = (Duration::from_secs(3).as_millis() / RETRY.as_millis()) as u32 + 1;
        assert!(asked <= most, "{asked} tries in 3 s, at most {most}");
        r.presented();
        assert_eq!(r.animation_waits(now), None, "landed: the display's rate");
    }

    /// Covered, a skip owes nothing; uncovered, a frame is owed at once,
    /// with the tries fresh, whatever was spent before.
    #[test]
    fn covered_owes_nothing_and_uncovered_owes_a_frame_at_once() {
        let t = Instant::now();
        let mut r = Retry::new(t);
        r.presented();
        r.occluded(true, t);
        r.skipped(t);
        assert_eq!(r.poll(t + RETRY), (false, None), "hidden: no retry");
        let back = t + Duration::from_secs(5);
        r.occluded(false, back);
        assert!(asks(&mut r, back), "uncovered: at once");
        r.skipped(back);
        assert!(asks(&mut r, back + RETRY), "and again while it skips");
    }
}
