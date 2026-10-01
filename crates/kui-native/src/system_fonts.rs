//! The system's installed fonts changing while the app runs.
//!
//! kui scans the system's fonts once a process (`kui_core`'s
//! `text::new_font_system`), so a font the user installs afterwards is not
//! in any session until `Core::reload_system_fonts` scans again. Where the
//! platform says the set changed, the runner does that itself: this module
//! turns the platform's signal into one `UserEvent::FontsChanged` on the
//! loop, and the shell rescans the session and asks every window for a
//! frame.
//!
//! - macOS: CoreText posts `kCTFontManagerRegisteredFontsChangedNotification`
//!   to the process's local notification center when the registered fonts
//!   change, in any scope — a font installed through Font Book or dropped
//!   into a Fonts folder included.
//! - Windows: whatever adds or removes a font resource broadcasts
//!   `WM_FONTCHANGE` to every top-level window; the window subclass
//!   `windows_theme` installs hears it.
//! - Elsewhere fontconfig has no change notification, so nothing is
//!   watched; an app calls `reload_system_fonts` when it has reason to.
//!
//! One install can arrive as several signals (a family's faces, a copy to
//! each window on Windows), and a rescan opens every font file, so they
//! are coalesced: a signal while one is waiting on the loop sends nothing.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use winit::event_loop::EventLoopProxy;

use crate::access_bridge::UserEvent;

/// Where a signal goes: the loop's proxy, once the shell has one.
static PROXY: Mutex<Option<EventLoopProxy<UserEvent>>> = Mutex::new(None);

/// A `FontsChanged` is on the loop and not yet handled.
static PENDING: AtomicBool = AtomicBool::new(false);

/// Starts listening, sending to `proxy`. Called once the loop exists; a
/// later call (a second runner on the same loop) only replaces the proxy.
pub fn watch(proxy: EventLoopProxy<UserEvent>) {
    *PROXY.lock().unwrap_or_else(|e| e.into_inner()) = Some(proxy);
    #[cfg(target_os = "macos")]
    macos::observe();
}

/// The platform said the installed fonts changed: one event on the loop,
/// unless one is already waiting.
pub fn changed() {
    if PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let sent = PROXY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|proxy| proxy.send_event(UserEvent::FontsChanged).is_ok());
    if !sent {
        PENDING.store(false, Ordering::Release);
    }
}

/// The shell took the event: the next signal sends again. Cleared before
/// the rescan, so a font installed while it runs is not lost.
pub fn handled() {
    PENDING.store(false, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A signal with no loop to send to is dropped without leaving the
    /// next one blocked behind a `FontsChanged` that never went out.
    #[test]
    fn a_signal_with_nowhere_to_go_does_not_block_the_next() {
        *PROXY.lock().unwrap() = None;
        changed();
        assert!(!PENDING.load(Ordering::Acquire));
        PENDING.store(true, Ordering::Release);
        handled();
        assert!(!PENDING.load(Ordering::Acquire));
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    use std::sync::Once;

    type CFNotificationCenterRef = *mut c_void;
    type CFStringRef = *const c_void;
    type Callback = extern "C" fn(
        center: CFNotificationCenterRef,
        observer: *mut c_void,
        name: CFStringRef,
        object: *const c_void,
        user_info: *const c_void,
    );

    /// `CFNotificationSuspensionBehaviorDeliverImmediately`: the local
    /// center does not suspend, but say so.
    const DELIVER_IMMEDIATELY: isize = 4;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFNotificationCenterGetLocalCenter() -> CFNotificationCenterRef;
        fn CFNotificationCenterAddObserver(
            center: CFNotificationCenterRef,
            observer: *const c_void,
            callback: Callback,
            name: CFStringRef,
            object: *const c_void,
            behavior: isize,
        );
    }

    #[link(name = "CoreText", kind = "framework")]
    unsafe extern "C" {
        static kCTFontManagerRegisteredFontsChangedNotification: CFStringRef;
    }

    extern "C" fn fonts_changed(
        _center: CFNotificationCenterRef,
        _observer: *mut c_void,
        _name: CFStringRef,
        _object: *const c_void,
        _user_info: *const c_void,
    ) {
        super::changed();
    }

    /// Adds the observer, once a process: the proxy it reaches is swapped
    /// under it by `watch`, so a second runner needs no second observer.
    pub fn observe() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| unsafe {
            CFNotificationCenterAddObserver(
                CFNotificationCenterGetLocalCenter(),
                // Any non-null token: it names this observer for a removal
                // that never comes, the observer living as the process does.
                c"kui.system-fonts".as_ptr().cast(),
                fonts_changed,
                kCTFontManagerRegisteredFontsChangedNotification,
                std::ptr::null(),
                DELIVER_IMMEDIATELY,
            );
        });
    }
}
