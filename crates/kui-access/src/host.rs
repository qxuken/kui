//! The bridge a host attaches to a window it made itself (backlog F269):
//! GLFW, SDL3, a game engine's own. The runner's windows have theirs in
//! kui-native; this one needs no winit and no event loop of kui's, only
//! the window's native handle.
//!
//! **Threading.** The platform calls back while the host's loop pumps its
//! events (on the main thread on macOS; on the window's thread, or UIA's,
//! on Windows; on a thread of `accesskit_unix`'s own on Linux). Nothing
//! runs against the core from inside those calls: they are queued here,
//! and the host's next call that may change the core applies them
//! ([`HostBridge::apply`]). A client's first ask is answered lazily, as
//! the runner's is: the adapter shows a bare window until the host
//! finishes a frame, and [`HostBridge::publish`] sends the whole tree
//! then, and after that only a tree that changed. Until a client asks,
//! the core derives no tree at all.
//!
//! **Waking the host.** AccessKit's contract is that the tree it asked
//! for arrives "no later than the next display refresh, even if a frame
//! would not normally be rendered". A host asleep in `glfwWaitEvents` or
//! `SDL_WaitEvent` would sleep through the ask, so on macOS and Windows
//! the bridge posts the platform's empty event to the host's loop (what
//! `glfwPostEmptyEvent` posts), which wakes the wait and is ignored by
//! everything that reads it; [`HostBridge::owed`] then says a turn is
//! owed. On Linux the requests arrive on another thread with nothing to
//! post to, so a host there waits with a timeout while attached.

use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use kui_core::{Core, InputEvent, UiEvent};

use crate::{Feed, request_of};

/// What the platform said, in the order it said it, waiting for the
/// host's next call.
enum Note {
    /// A client asked for the tree.
    Asked,
    /// The platform says the client left (AT-SPI alone says so).
    #[cfg_attr(not(unix_atspi), allow(dead_code))]
    Left,
    Action(ActionRequest),
}

type Inbox = Arc<Mutex<Vec<Note>>>;

fn post(inbox: &Inbox, note: Note, wake: &platform::Wake) {
    if let Ok(mut q) = inbox.lock() {
        q.push(note);
    }
    wake.wake();
}

struct Asked(Inbox, platform::Wake);
impl ActivationHandler for Asked {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        // `None` is "later": the adapter stands a bare window in until
        // `publish` sends the tree from the host's own turn.
        post(&self.0, Note::Asked, &self.1);
        None
    }
}

struct Acted(Inbox, platform::Wake);
impl ActionHandler for Acted {
    fn do_action(&mut self, request: ActionRequest) {
        post(&self.0, Note::Action(request), &self.1);
    }
}

// Only `accesskit_unix` takes one; elsewhere nothing reports a client
// leaving, which is the platform's limit and not this bridge's.
#[cfg_attr(not(unix_atspi), allow(dead_code))]
struct Left(Inbox, platform::Wake);
impl DeactivationHandler for Left {
    fn deactivate_accessibility(&mut self) {
        post(&self.0, Note::Left, &self.1);
    }
}

/// The platform accessibility bridge of one window a host made: its
/// adapter, what the platform has said since the host last called, and
/// the [`Feed`] that decides what to send. Drop it to detach; on macOS
/// and Windows that restores the window as it was. Like the window, it
/// belongs to the thread that made it.
///
/// A host's turn with it, around the core's own:
///
/// ```no_run
/// # use kui_core::{Core, Size};
/// # fn turn(core: &mut Core, access: &mut kui_access::HostBridge) {
/// // Before input: what a client asked for, as the input it is.
/// for ev in access.apply(core) {
///     // ...handle as any event from `Core::handle_input`
/// #   let _ = ev;
/// }
/// let ui = core.frame(Size::new(640.0, 480.0), 2.0);
/// // ...the view
/// ui.finish();
/// // After the frame: the tree when it changed, and its announcements.
/// access.publish(core);
/// # }
/// ```
pub struct HostBridge {
    adapter: platform::Adapter,
    inbox: Inbox,
    feed: Feed,
}

impl HostBridge {
    /// Attaches to the window `handle` names, or `None` where this
    /// platform has no adapter or the handle is not one it takes:
    ///
    /// - **macOS**: an `NSView *` (GLFW's `glfwGetCocoaView`) or an
    ///   `NSWindow *` (`glfwGetCocoaWindow`, SDL3's
    ///   `SDL_PROP_WINDOW_COCOA_WINDOW_POINTER`), whose content view is
    ///   taken. On the main thread.
    /// - **Windows**: the `HWND`, on the thread that made it, **before
    ///   the window is first shown** — AccessKit's subclass refuses a
    ///   visible window (GLFW's `GLFW_VISIBLE` false, then
    ///   `glfwShowWindow` after attaching).
    /// - **Linux and the BSDs**: ignored. AT-SPI is per application,
    ///   not per window.
    ///
    /// One bridge a window: a second attach to the same view is refused.
    ///
    /// # Safety
    ///
    /// `handle` must be null or a live handle of the kind above, and stay
    /// live until the bridge is dropped.
    pub unsafe fn attach(handle: *mut c_void) -> Option<HostBridge> {
        let inbox: Inbox = Arc::default();
        let adapter = unsafe { platform::Adapter::attach(handle, &inbox)? };
        Some(HostBridge {
            adapter,
            inbox,
            feed: Feed::new(),
        })
    }

    /// Applies what the platform said since the last call: a client's
    /// arrival or departure as `env.system.assistive`, and each action
    /// it asked for as `InputEvent::Access` — the events that input
    /// raised are returned, for the host to handle as any input's. Call
    /// it where input goes, before a frame is begun; a call with nothing
    /// queued costs a lock.
    pub fn apply(&mut self, core: &mut Core) -> Vec<UiEvent> {
        let notes = match self.inbox.lock() {
            Ok(mut q) if !q.is_empty() => std::mem::take(&mut *q),
            _ => {
                self.sync_assistive(core);
                return Vec::new();
            }
        };
        let mut events = Vec::new();
        for note in notes {
            match note {
                Note::Asked => self.feed.activate(),
                Note::Left => self.feed.deactivate(),
                Note::Action(req) => {
                    if let Some(req) = request_of(req) {
                        events.extend(core.handle_input(InputEvent::Access(req)));
                    }
                }
            }
        }
        self.sync_assistive(core);
        events
    }

    /// After a finished frame: takes the frame's announcements (every
    /// frame, so nothing is said minutes late) and, while a client
    /// listens, sends the tree when it or they changed. While nobody
    /// listens the tree is not derived.
    pub fn publish(&mut self, core: &mut Core) {
        let said = core.take_announcements();
        self.adapter.sync_focus();
        if !self.feed.active() {
            return;
        }
        let scale = core.scale();
        let adapter = &mut self.adapter;
        self.feed
            .publish(core.access_tree(), scale, &said, |u| adapter.update(u));
    }

    /// Whether the host owes a turn for the bridge's sake: something the
    /// platform said is waiting for [`HostBridge::apply`], or a client
    /// asked and has not had its tree from [`HostBridge::publish`].
    pub fn owed(&self) -> bool {
        self.feed.owes_tree() || self.inbox.lock().is_ok_and(|q| !q.is_empty())
    }

    /// Whether a client has asked for the tree (and not left).
    pub fn listening(&self) -> bool {
        self.feed.active()
    }

    /// The bridge's reading wins over whatever the host set: attached,
    /// nobody asking is `None` rather than unknown.
    fn sync_assistive(&self, core: &mut Core) {
        let now = self.feed.assistive();
        if core.env.system.assistive != now {
            core.env.system.assistive = now;
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;

    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::{ClassType, msg_send};
    use objc2_app_kit::{
        NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSView, NSWindow,
    };
    use objc2_foundation::{MainThreadMarker, NSPoint};

    use super::{Acted, Asked, Inbox};

    pub struct Adapter {
        inner: accesskit_macos::SubclassingAdapter,
        view: Retained<NSView>,
        /// Whether the view's window was key when last told, so the
        /// adapter hears a change once (`accesskit_winit` tells it on
        /// winit's `Focused`; a host's window has no such event here).
        key: Option<bool>,
    }

    /// The empty event that wakes a host's `nextEventMatchingMask`
    /// (glfwWaitEvents, SDL_WaitEvent): an application-defined one,
    /// which is what `glfwPostEmptyEvent` posts and what every reader
    /// passes over. The handlers run on the main thread here.
    #[derive(Clone, Copy)]
    pub struct Wake;

    impl Wake {
        pub fn wake(&self) {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let ev = unsafe {
                NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
                    NSEventType::ApplicationDefined,
                    NSPoint::new(0.0, 0.0),
                    NSEventModifierFlags(0),
                    0.0,
                    0,
                    None,
                    0,
                    0,
                    0,
                )
            };
            if let Some(ev) = ev {
                NSApplication::sharedApplication(mtm).postEvent_atStart(&ev, false);
            }
        }
    }

    impl Adapter {
        pub unsafe fn attach(handle: *mut c_void, inbox: &Inbox) -> Option<Adapter> {
            // AppKit's objects are the main thread's, and the adapter
            // asserts it is on it.
            MainThreadMarker::new()?;
            let obj = unsafe { (handle as *const AnyObject).as_ref()? };
            let is = |class| -> bool { unsafe { msg_send![obj, isKindOfClass: class] } };
            let view: Retained<NSView> = if is(NSWindow::class()) {
                let window = unsafe { &*(handle as *const NSWindow) };
                window.contentView()?
            } else if is(NSView::class()) {
                unsafe { Retained::retain(handle as *mut NSView)? }
            } else {
                return None;
            };
            // The adapter panics on a view that already has one; a
            // second attach is refused here instead. Its subclass names
            // itself, which is the one sign of it from outside; the catch
            // stands behind that.
            let class: &AnyClass = unsafe { msg_send![&*view, class] };
            if class.name().starts_with("AccessKitSubclassOf") {
                return None;
            }
            let inner = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                accesskit_macos::SubclassingAdapter::new(
                    Retained::as_ptr(&view) as *mut c_void,
                    Asked(inbox.clone(), Wake),
                    Acted(inbox.clone(), Wake),
                )
            }))
            .ok()?;
            let mut adapter = Adapter {
                inner,
                view,
                key: None,
            };
            adapter.sync_focus();
            Some(adapter)
        }

        pub fn update(&mut self, update: accesskit::TreeUpdate) {
            if let Some(events) = self.inner.update_if_active(|| update) {
                events.raise();
            }
        }

        pub fn sync_focus(&mut self) {
            let key = self.view.window().is_some_and(|w| w.isKeyWindow());
            if self.key != Some(key) {
                self.key = Some(key);
                if let Some(events) = self.inner.update_view_focus_state(key) {
                    events.raise();
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::ffi::c_void;

    use accesskit_windows::HWND;

    use super::{Acted, Asked, Inbox};

    // user32 and kernel32, which every Windows program links: four
    // calls are not worth a binding crate.
    #[link(name = "user32")]
    unsafe extern "system" {
        fn IsWindow(hwnd: *mut c_void) -> i32;
        fn IsWindowVisible(hwnd: *mut c_void) -> i32;
        fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
        fn PostMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    const WM_NULL: u32 = 0;

    pub struct Adapter {
        inner: accesskit_windows::SubclassingAdapter,
    }

    /// A `WM_NULL` posted to the window, which wakes the host's
    /// `GetMessage` / `MsgWaitForMultipleObjects` (glfwWaitEvents,
    /// SDL_WaitEvent) and does nothing. Posting is any thread's to do,
    /// and UIA calls the action handler on its own.
    #[derive(Clone, Copy)]
    pub struct Wake(usize);

    impl Wake {
        pub fn wake(&self) {
            unsafe { PostMessageW(self.0 as *mut c_void, WM_NULL, 0, 0) };
        }
    }

    impl Adapter {
        pub unsafe fn attach(handle: *mut c_void, inbox: &Inbox) -> Option<Adapter> {
            // The subclass is the window's thread's to install, and
            // AccessKit panics on a window already shown: refused here.
            let ours = unsafe {
                !handle.is_null()
                    && IsWindow(handle) != 0
                    && IsWindowVisible(handle) == 0
                    && GetWindowThreadProcessId(handle, std::ptr::null_mut())
                        == GetCurrentThreadId()
            };
            if !ours {
                return None;
            }
            let wake = Wake(handle as usize);
            let inner = accesskit_windows::SubclassingAdapter::new(
                HWND(handle),
                Asked(inbox.clone(), wake),
                Acted(inbox.clone(), wake),
            );
            Some(Adapter { inner })
        }

        pub fn update(&mut self, update: accesskit::TreeUpdate) {
            if let Some(events) = self.inner.update_if_active(|| update) {
                events.raise();
            }
        }

        /// The subclass watches `WM_SETFOCUS` / `WM_KILLFOCUS` itself.
        pub fn sync_focus(&mut self) {}
    }
}

#[cfg(unix_atspi)]
mod platform {
    use std::ffi::c_void;

    use super::{Acted, Asked, Inbox, Left};

    pub struct Adapter {
        inner: accesskit_unix::Adapter,
    }

    /// Nothing to post to: AT-SPI's requests arrive on the adapter's own
    /// thread, and a host's loop is not reachable from it. A host waits
    /// with a timeout while attached.
    #[derive(Clone, Copy)]
    pub struct Wake;

    impl Wake {
        pub fn wake(&self) {}
    }

    impl Adapter {
        pub unsafe fn attach(_handle: *mut c_void, inbox: &Inbox) -> Option<Adapter> {
            Some(Adapter {
                inner: accesskit_unix::Adapter::new(
                    Asked(inbox.clone(), Wake),
                    Acted(inbox.clone(), Wake),
                    Left(inbox.clone(), Wake),
                ),
            })
        }

        pub fn update(&mut self, update: accesskit::TreeUpdate) {
            self.inner.update_if_active(|| update);
        }

        pub fn sync_focus(&mut self) {}
    }
}

/// A platform AccessKit has no desktop adapter for: nothing attaches.
#[cfg(not(any(target_os = "macos", target_os = "windows", unix_atspi)))]
mod platform {
    use std::ffi::c_void;

    use super::Inbox;

    pub enum Adapter {}

    #[derive(Clone, Copy)]
    pub struct Wake;

    impl Wake {
        pub fn wake(&self) {}
    }

    impl Adapter {
        pub unsafe fn attach(_handle: *mut c_void, _inbox: &Inbox) -> Option<Adapter> {
            None
        }

        pub fn update(&mut self, _update: accesskit::TreeUpdate) {
            match *self {}
        }

        pub fn sync_focus(&mut self) {
            match *self {}
        }
    }
}
