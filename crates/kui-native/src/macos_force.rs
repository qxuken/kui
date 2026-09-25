//! Force Touch: asking AppKit for the deep-click stage
//! (`docs/adr/0017-selection-as-a-scope.md`, decision 6).
//!
//! winit delivers `WindowEvent::TouchpadPressure` from the view's
//! `pressureChangeWithEvent:` and never touches the view's *pressure
//! configuration*, which is what decides whether the ramp ever reaches
//! stage 2 — the deepened press macOS calls a force click. So the driver
//! asks for it: `NSPressureBehaviorPrimaryDeepClick` is the behaviour
//! Quick Look and every force-clickable text view use, and it is the one
//! that gives the second stage its haptic.
//!
//! **Unverified.** A force click needs a Force Touch trackpad and a real
//! finger; nothing synthesises one, so this is the one call in the driver
//! that no test and no scripted run can reach. It is also the ADR's
//! second open question — whether the default configuration reaches stage
//! 2 on its own — asked and answered by declaring the behaviour we
//! actually want rather than by relying on a default we cannot observe.

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{NSPressureBehavior, NSPressureConfiguration, NSView};
use winit::window::Window;

/// Asks `window`'s content view for the deep-click pressure behaviour.
/// Returns false when there is no AppKit view to ask.
pub fn configure(window: &Window) -> bool {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else {
        return false;
    };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else {
        return false;
    };
    // SAFETY: a live winit content view, on the event loop's thread.
    let Some(view) = (unsafe { Retained::retain(h.ns_view.as_ptr().cast::<NSView>()) }) else {
        return false;
    };
    let config = NSPressureConfiguration::initWithPressureBehavior(
        NSPressureConfiguration::alloc(),
        NSPressureBehavior::PrimaryDeepClick,
    );
    view.setPressureConfiguration(Some(&config));
    true
}
