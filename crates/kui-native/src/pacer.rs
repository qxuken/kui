//! Frames started by the display, not by a free drawable.
//!
//! `Launcher::frame_latency` queued a second frame ahead of the one on screen, and every vsync
//! got a frame. It cost a frame of latency while frames run back to back.
//! The runner asked for the next frame as soon as the last one was handed
//! over, so the view built it at once, from the input it had then, and the
//! frame waited out a vsync in the queue before it was drawn.
//! Sampling-to-photon on the bake-off grid went from 19.2 to 27.5 ms.
//!
//! Here a window's back-to-back frames wait for its display instead: a
//! `CADisplayLink` on the window's own view (macOS 14+) fires at each
//! vsync, and only then is the frame built — from everything that arrived
//! meanwhile — drawn into a free drawable, and presented for the next
//! vsync. The queued slot is left as slack for a frame that runs late,
//! which is what gpui does with its display link and three drawables.
//! Measured the same way on the same grid, three captures a launch: 17.5–
//! 19.2 ms once the window has settled (it can read ~26 for its first
//! seconds), against 19.2–19.5 with one queued frame and 27.5–27.9 with two
//! unpaced, and every vsync still delivered. The link's tick lands ~3.7 ms
//! after its vsync; winit hands the redraw it asks for over in the same
//! turn of the run loop.
//!
//! A frame asked for from idle is not held: nothing is queued, so drawing
//! it at once is the fastest it can reach the screen, and a keystroke into
//! a still editor is exactly as quick as before. The link runs only while
//! frames are asked for, and pauses a few vsyncs after the last one, so an
//! idle window stays at no wake-ups. Where there is no link — an older
//! macOS, another platform, a view the link will not attach to — every
//! frame draws when asked, as before.

use std::time::{Duration, Instant};

/// A frame asked for within this long of the last present is part of a
/// run and waits for the display. Wider than a 60 Hz frame (16.7 ms), so a
/// run at any common rate is paced, and far below a person's keystrokes.
// Read only where there is a link (macOS); the tests read it everywhere.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const RUN_GAP: Duration = Duration::from_millis(20);

/// A frame held for the display longer than this is drawn anyway: the link
/// stopped firing (the window left every screen, the display slept).
const HELD_MAX: Duration = Duration::from_millis(50);

/// Vsyncs the link keeps firing with nothing asked for before it pauses.
// Read only where there is a link (macOS); the tests read it everywhere.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const IDLE_TICKS: u32 = 3;

#[cfg(target_os = "macos")]
mod link {
    use super::IDLE_TICKS;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::NSView;
    use objc2_foundation::{NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes};
    use objc2_quartz_core::CADisplayLink;
    use std::cell::Cell;
    use std::sync::Arc;
    use winit::window::Window;

    /// What the link's target reads and writes, on the main thread.
    pub(super) struct State {
        /// A frame is held for the next vsync.
        pub(super) wanted: Cell<bool>,
        /// The redraw in flight is the link's: draw it, do not hold it.
        pub(super) fired: Cell<bool>,
        idle: Cell<u32>,
        window: Arc<Window>,
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "KuiFramePacer"]
        #[ivars = State]
        pub(super) struct Target;

        impl Target {
            #[unsafe(method(tick:))]
            fn tick(&self, link: &CADisplayLink) {
                let s = self.ivars();
                if s.wanted.take() {
                    s.idle.set(0);
                    s.fired.set(true);
                    s.window.request_redraw();
                } else {
                    let idle = s.idle.get() + 1;
                    s.idle.set(idle);
                    if idle >= IDLE_TICKS {
                        link.setPaused(true);
                    }
                }
            }
        }

        unsafe impl NSObjectProtocol for Target {}
    );

    pub(super) struct Link {
        pub(super) target: Retained<Target>,
        link: Retained<CADisplayLink>,
    }

    impl Link {
        pub(super) fn new(window: &Arc<Window>) -> Option<Link> {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let mtm = MainThreadMarker::new()?;
            let RawWindowHandle::AppKit(h) = window.window_handle().ok()?.as_raw() else {
                return None;
            };
            // SAFETY: a live winit content view, on the event loop's thread.
            let view = unsafe { Retained::retain(h.ns_view.as_ptr().cast::<NSView>()) }?;
            // `-[NSView displayLinkWithTarget:selector:]` is macOS 14's.
            let responds: bool = unsafe {
                msg_send![&*view, respondsToSelector: sel!(displayLinkWithTarget:selector:)]
            };
            if !responds {
                return None;
            }
            let target = Target::alloc(mtm).set_ivars(State {
                wanted: Cell::new(false),
                fired: Cell::new(false),
                idle: Cell::new(0),
                window: window.clone(),
            });
            let target: Retained<Target> = unsafe { msg_send![super(target), init] };
            // SAFETY: `tick:` is `Target`'s, taking the link.
            let link = unsafe {
                view.displayLinkWithTarget_selector(
                    &*(Retained::as_ptr(&target) as *const AnyObject),
                    sel!(tick:),
                )
            };
            link.setPaused(true);
            // Common modes, so it fires while a resize or a menu tracks.
            unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
            Some(Link { target, link })
        }

        pub(super) fn state(&self) -> &State {
            self.target.ivars()
        }

        pub(super) fn run(&self) {
            self.state().idle.set(0);
            self.link.setPaused(false);
        }
    }

    impl Drop for Link {
        /// The link retains its target and the target the window; the
        /// link is what lets go.
        fn drop(&mut self) {
            self.link.invalidate();
        }
    }
}

/// Whether a frame asked for at `now`, at surface `size`, is part of a
/// run of frames: the last one was presented less than [`RUN_GAP`] ago, at
/// the same size.
// Read only where there is a link (macOS); the tests read it everywhere.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn in_run(
    last_present: Option<Instant>,
    last_size: (u32, u32),
    now: Instant,
    size: (u32, u32),
) -> bool {
    size == last_size && last_present.is_some_and(|at| now.duration_since(at) < RUN_GAP)
}

/// One window's frame pacing; see the module docs.
pub(crate) struct Pacer {
    #[cfg(target_os = "macos")]
    link: Option<link::Link>,
    last_present: Option<Instant>,
    /// The surface size the last present had, physical px.
    last_size: (u32, u32),
    held_since: Option<Instant>,
}

impl Pacer {
    /// Pacing for `window`: a display link where the platform has one on
    /// the window's view, none elsewhere, and none when `KUI_FRAME_PACING=0`
    /// asks for frames as soon as they are asked for — how C47's numbers
    /// without it were taken.
    ///
    /// None either for a `run_loop` that is not `kui_native::run`'s own — the
    /// `PumpRunner` a Node window turns from a JavaScript timer. The link
    /// fires only while the run loop runs, which there is only inside a
    /// pump, so a held frame waited for a pump that happened to meet a
    /// vsync: an animating Node window drew 50 frames a second paced
    /// against 95 unpaced.
    pub(crate) fn new(window: &std::sync::Arc<winit::window::Window>, run_loop: bool) -> Pacer {
        let wanted =
            run_loop && std::env::var("KUI_FRAME_PACING").map_or(true, |v| v.trim() != "0");
        #[cfg(not(target_os = "macos"))]
        let _ = (window, wanted);
        Pacer {
            #[cfg(target_os = "macos")]
            link: if wanted {
                link::Link::new(window)
            } else {
                None
            },
            last_present: None,
            last_size: (0, 0),
            held_since: None,
        }
    }

    /// Whether this redraw draws now. The link's own always does; one
    /// asked for from idle does, and so does one at a new surface size —
    /// a live resize wants its frame in the redraw AppKit asked for, or the
    /// old one is stretched to the new bounds; one asked for while frames
    /// run back to back is held for the next vsync, and the link is started
    /// to bring it back. Without a link every redraw draws.
    pub(crate) fn admit(&mut self, now: Instant, size: (u32, u32)) -> bool {
        #[cfg(target_os = "macos")]
        if let Some(link) = &self.link {
            let s = link.state();
            if s.fired.take() {
                self.held_since = None;
                return true;
            }
            let running = in_run(self.last_present, self.last_size, now, size);
            if running {
                s.wanted.set(true);
                self.held_since.get_or_insert(now);
                link.run();
                return false;
            }
        }
        let _ = (now, size);
        self.held_since = None;
        true
    }

    /// A frame reached the surface, at this size.
    pub(crate) fn presented(&mut self, now: Instant, size: (u32, u32)) {
        self.last_present = Some(now);
        self.last_size = size;
    }

    /// When a held frame should be drawn whether or not the link fired —
    /// the deadline `about_to_wait` wakes for — and whether it is past.
    pub(crate) fn overdue(&mut self, now: Instant) -> Option<(Instant, bool)> {
        let since = self.held_since?;
        let at = since + HELD_MAX;
        if now >= at {
            #[cfg(target_os = "macos")]
            if let Some(link) = &self.link {
                let s = link.state();
                s.wanted.set(false);
                s.fired.set(true);
            }
            self.held_since = None;
            return Some((at, true));
        }
        Some((at, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame right after the last present is part of a run; one from
    /// idle, one at a new size, and the first one ever are not.
    #[test]
    fn a_run_is_frames_close_together_at_one_size() {
        let t0 = Instant::now();
        let at = |ms| t0 + Duration::from_millis(ms);
        assert!(in_run(Some(t0), (800, 600), at(8), (800, 600)));
        assert!(
            !in_run(Some(t0), (800, 600), at(25), (800, 600)),
            "from idle"
        );
        assert!(
            !in_run(Some(t0), (800, 600), at(8), (801, 600)),
            "a live resize"
        );
        assert!(!in_run(None, (0, 0), t0, (800, 600)), "the first frame");
    }

    /// Without a link — any platform but macOS 14+, or pacing turned off —
    /// every redraw draws, and nothing is ever held.
    #[test]
    fn without_a_link_every_redraw_draws() {
        let mut p = Pacer {
            #[cfg(target_os = "macos")]
            link: None,
            last_present: None,
            last_size: (0, 0),
            held_since: None,
        };
        let t0 = Instant::now();
        p.presented(t0, (800, 600));
        assert!(p.admit(t0 + Duration::from_millis(4), (800, 600)));
        assert!(p.overdue(t0 + Duration::from_secs(1)).is_none());
    }
}
