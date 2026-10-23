//! The first press on a pumped window's edge, on macOS 26 and earlier
//! (backlog RG168).
//!
//! A press on the resize edge of a window whose app is not active goes to
//! AppKit's delayed first-click path: `-[NSWindow _handleMouseDownEvent:
//! isDelayedEvent:]` waits for the activation to come back, 78–230 ms, and
//! only then starts the resize. A native run loop is there the whole time.
//! A pumped one — the Node driver's, which runs AppKit a slice at a time
//! and sleeps in libuv between — saw the activation arrive after a quick
//! click's release, and the resize it then started waited for a release
//! that had come and gone: the window stuck to the pointer until the next
//! click, and a press held and let go outside the window hung the process
//! inside AppKit for good, ⌘Q included. Measured on macOS 26.6.2; macOS 27
//! activates in time and resizes on the first press, as Finder does.
//!
//! So under a pump, on macOS 26 and earlier, such a press only activates the
//! app: the monitor below drops it and brings the window forward, and the
//! drag or release that follows reaches the window as an ordinary event. The
//! second press resizes. A press anywhere else, a press while the app is
//! active, and every press on macOS 27 go on as before. Posting a release
//! back into the queue after activation instead was tried and did not end
//! AppKit's resize.

use std::cell::RefCell;
use std::ptr::NonNull;
use std::sync::Once;
use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSEvent, NSEventMask, NSEventType, NSWindowStyleMask};

/// How far inside the window's frame a press still counts as its edge, in
/// points: AppKit's resize band reaches a few points in along an edge and
/// the corner's 19 × 19 zone about ten, and the presses that stuck were all
/// within eleven.
pub(crate) const EDGE: f64 = 12.0;

/// Whether a press at `x`, `y` (window coordinates, origin bottom left) on a
/// window `w` × `h` is on its edge: within [`EDGE`] of any side, or outside.
pub(crate) fn on_edge(x: f64, y: f64, w: f64, h: f64) -> bool {
    x < EDGE || y < EDGE || x > w - EDGE || y > h - EDGE
}

thread_local! {
    /// The press the monitor held back, and when, until the app is active.
    static HELD: RefCell<Option<(Retained<NSEvent>, Instant)>> = const { RefCell::new(None) };
}

/// How long a held press waits for the activation before it is let go.
const WAIT: Duration = Duration::from_millis(500);

fn note(what: std::fmt::Arguments<'_>) {
    if std::env::var_os("KUI_FIRST_CLICK_LOG").is_some() {
        eprintln!("kui first-click: {what}");
    }
}

/// After a pump: once the app is active, the held press goes back to AppKit
/// if the button is still down — the window is active now, so AppKit starts
/// the resize at once and the drag in progress resizes, one gesture as
/// anywhere else — and is let go if it came up, which is the release the
/// delayed path missed.
pub(crate) fn after_pump(mtm: MainThreadMarker) {
    let Some((press, at)) = HELD.with(|h| h.borrow_mut().take()) else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    if !app.isActive() {
        if at.elapsed() < WAIT {
            HELD.with(|h| *h.borrow_mut() = Some((press, at)));
        } else {
            note(format_args!("let a press go: the app never became active"));
        }
        return;
    }
    if NSEvent::pressedMouseButtons() & 1 != 0 {
        app.postEvent_atStart(&press, true);
        note(format_args!(
            "replayed the press {:.0} ms on, button down",
            at.elapsed().as_secs_f64() * 1000.0
        ));
    } else {
        note(format_args!(
            "dropped the press {:.0} ms on, button up",
            at.elapsed().as_secs_f64() * 1000.0
        ));
    }
}

/// Installs the monitor once per process, on macOS 26 and earlier; from the
/// pumped runner only, since a native run loop never has the gap.
pub(crate) fn install(mtm: MainThreadMarker) {
    static ONCE: Once = Once::new();
    let major = objc2_foundation::NSProcessInfo::processInfo()
        .operatingSystemVersion()
        .majorVersion;
    if major >= 27 {
        return;
    }
    ONCE.call_once(|| {
        let handler = block2::RcBlock::new(move |ev: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit hands the monitor a live event.
            let e = unsafe { ev.as_ref() };
            if e.r#type() != NSEventType::LeftMouseDown {
                return ev.as_ptr();
            }
            let app = NSApplication::sharedApplication(mtm);
            if app.isActive() {
                return ev.as_ptr();
            }
            let Some(window) = e.window(mtm) else {
                return ev.as_ptr();
            };
            let style = window.styleMask();
            if !style.contains(NSWindowStyleMask::Resizable)
                || style.contains(NSWindowStyleMask::FullScreen)
            {
                return ev.as_ptr();
            }
            let (p, size) = (e.locationInWindow(), window.frame().size);
            if !on_edge(p.x, p.y, size.width, size.height) {
                return ev.as_ptr();
            }
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
            window.makeKeyAndOrderFront(None);
            // SAFETY: a live event, retained to be posted again later.
            if let Some(press) = unsafe { Retained::retain(ev.as_ptr()) } {
                HELD.with(|h| *h.borrow_mut() = Some((press, Instant::now())));
            }
            note(format_args!("held an edge press at {:.0},{:.0}", p.x, p.y));
            std::ptr::null_mut()
        });
        // SAFETY: the handler returns the event it was given, or nil to drop
        // it, as the monitor's contract asks; it lives for the process.
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::LeftMouseDown,
                &handler,
            )
        };
        std::mem::forget(monitor);
        std::mem::forget(handler);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_within_the_band_is_the_edge() {
        let (w, h) = (1100.0, 780.0);
        // The presses that stuck on the work Mac, all bottom edge or corner.
        for (x, y) in [
            (1096.0, -1.0),
            (1098.0, 3.0),
            (1066.0, 4.0),
            (979.0, 8.0),
            (1101.0, 11.0),
            (1099.0, 7.0),
        ] {
            assert!(on_edge(x, y, w, h), "{x},{y}");
        }
        assert!(on_edge(5.0, 400.0, w, h), "left");
        assert!(on_edge(600.0, 775.0, w, h), "top");
        assert!(on_edge(1095.0, 400.0, w, h), "right");
        // A press in the content is a click, activation and all.
        for (x, y) in [(942.0, 62.0), (866.0, 268.0), (550.0, 390.0), (13.0, 13.0)] {
            assert!(!on_edge(x, y, w, h), "{x},{y}");
        }
    }
}
