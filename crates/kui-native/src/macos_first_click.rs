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
//! app: the monitor below drops it, and the drags and the release of the
//! same gesture with it, and brings the window forward. The second press
//! resizes. Letting the rest of the gesture through made it a drag across
//! the app, under the app's cursor, where the user meant a resize. A press
//! anywhere else, a press while the app is active, and every press on
//! macOS 27 go on as before.
//!
//! Tried on macOS 26.6.2 and not enough: posting a release back into the
//! queue after activation (the resize ignored it), and handing the dropped
//! press back to AppKit once the app was active with the button still down,
//! 2–6 ms on (the resize stuck and hung as before). Resizing in one gesture
//! from another app wants AppKit's loop running the whole time under a pump
//! (backlog C54).

use std::cell::Cell;
use std::ptr::NonNull;
use std::sync::Once;

use objc2::MainThreadMarker;
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
    /// A dropped press's gesture is still down: its drags and its release
    /// are dropped too.
    static SWALLOWING: Cell<bool> = const { Cell::new(false) };
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
            match e.r#type() {
                NSEventType::LeftMouseDown => SWALLOWING.with(|s| s.set(false)),
                NSEventType::LeftMouseDragged if SWALLOWING.with(Cell::get) => {
                    return std::ptr::null_mut();
                }
                NSEventType::LeftMouseUp if SWALLOWING.with(|s| s.replace(false)) => {
                    return std::ptr::null_mut();
                }
                _ => return ev.as_ptr(),
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
            SWALLOWING.with(|s| s.set(true));
            std::ptr::null_mut()
        });
        // SAFETY: the handler returns the event it was given, or nil to drop
        // it, as the monitor's contract asks; it lives for the process.
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::LeftMouseDown
                    | NSEventMask::LeftMouseDragged
                    | NSEventMask::LeftMouseUp,
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
