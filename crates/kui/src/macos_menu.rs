//! The macOS context menu: an `NSMenu`, shown where the platform shows
//! one, with the platform's own wording, keyboard and Services
//! (`docs/adr/0017-selection-as-a-scope.md`, decision 5, step 3).
//!
//! **Why this is deferred rather than shown where the press is handled.**
//! `popUpMenuPositioningItem:atLocation:inView:` runs a nested modal run
//! loop and does not return until the menu closes. winit's macOS backend
//! refuses to be re-entered — its event handler `panic!`s with "tried to
//! handle event while another event is currently being handled"
//! (`winit-0.30.13/src/platform_impl/macos/event_handler.rs`) — and its
//! run-loop observers fire in `NSEventTrackingRunLoopMode` too, so a menu
//! popped from inside a `window_event` takes the whole app down the first
//! time the pointer moves over it.
//!
//! So the menu is scheduled instead: [`present`] asks AppKit to call back
//! with `performSelector:withObject:afterDelay:` at zero delay, which runs
//! on the next turn of the run loop with no winit handler on the stack.
//! The nested loop is entered from *there*, where re-entering winit is
//! ordinary — frames draw behind the menu the way they do in any AppKit
//! app — and the chosen row is handed back through the callback the
//! caller supplied.
//!
//! What this is not: a menu bar, a submenu, or a Services entry. Those are
//! `NSMenu` too and would arrive the same way; nothing here has needed
//! them yet.

use std::cell::{Cell, RefCell};

use kui_core::{MenuItem, MenuRole, Vec2};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSEventModifierFlags, NSMenu, NSMenuItem, NSView};
use objc2_foundation::{
    MainThreadMarker, NSAttributedString, NSObject, NSObjectProtocol, NSPoint, NSString,
};
use winit::window::Window;

/// What one presentation needs to know once it is finally shown: the view
/// to show over, where, the rows, and who to tell.
struct Pending {
    view: Retained<NSView>,
    at: Vec2,
    items: Vec<MenuItem>,
}

/// The ivars of [`MenuTarget`]: the presentation waiting for the run loop,
/// and the row the menu reported before it closed (`-1` for none).
struct TargetIvars {
    pending: RefCell<Option<Pending>>,
    chosen: Cell<isize>,
    /// True from the moment the nested modal loop is entered until it
    /// returns, so a caller can tell "the menu is up" from "the menu has
    /// closed and here is the answer".
    up: Cell<bool>,
}

define_class!(
    // SAFETY:
    // - `NSObject` has no subclassing requirements.
    // - `MenuTarget` does not implement `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "KuiMenuTarget"]
    #[ivars = TargetIvars]
    struct MenuTarget;

    impl MenuTarget {
        /// Every row's action. `tag` is the row's index in the list the
        /// core gave us, so the answer needs no lookup.
        #[unsafe(method(kuiMenuPick:))]
        fn pick(&self, sender: &NSMenuItem) {
            self.ivars().chosen.set(sender.tag());
        }

        /// The deferred presentation (see the module docs): entered from
        /// the run loop rather than from inside a winit callback.
        #[unsafe(method(kuiMenuShow))]
        fn show(&self) {
            let Some(p) = self.ivars().pending.borrow_mut().take() else {
                return;
            };
            let mtm = MainThreadMarker::from(self);
            let menu = build(mtm, self, &p.items);
            self.ivars().chosen.set(-1);
            // The view is flipped (winit's is), so a logical viewport
            // point is the view point, with no height to subtract.
            let at = NSPoint::new(p.at.x as f64, p.at.y as f64);
            self.ivars().up.set(true);
            // Does not return until the menu closes; the row, if any,
            // arrived through `kuiMenuPick:` before it did.
            menu.popUpMenuPositioningItem_atLocation_inView(None, at, Some(&p.view));
            self.ivars().up.set(false);
        }
    }

    unsafe impl NSObjectProtocol for MenuTarget {}
);

impl MenuTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TargetIvars {
            pending: RefCell::new(None),
            chosen: Cell::new(-1),
            up: Cell::new(false),
        });
        unsafe { msg_send![super(this), init] }
    }
}

/// Builds the `NSMenu` for `items`. Every row is the platform's own
/// wording for its role where it has one (`MenuRole::default_label`), and
/// the standard rows carry the accelerators a macOS user reads them by —
/// display only, since the app's own key handling is what actually runs
/// them.
fn build(mtm: MainThreadMarker, target: &MenuTarget, items: &[MenuItem]) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    // Ours to decide: without this AppKit greys out every row whose target
    // does not answer `validateMenuItem:`, which is all of them.
    menu.setAutoenablesItems(false);
    for (i, item) in items.iter().enumerate() {
        if item.role == MenuRole::Separator {
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            continue;
        }
        let title = NSString::from_str(item.text());
        let key = NSString::from_str(accelerator(item.role));
        let row = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &title,
                Some(sel!(kuiMenuPick:)),
                &key,
            )
        };
        unsafe {
            row.setTarget(Some(&*(target as *const MenuTarget as *const AnyObject)));
            row.setTag(i as isize);
            row.setEnabled(item.enabled);
            if !key.to_string().is_empty() {
                row.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
            }
        }
        menu.addItem(&row);
    }
    menu
}

/// The key equivalent a macOS user expects to see beside a standard row.
/// Empty for everything else: an app's own accelerator is the app's to
/// draw, and a wrong one here would be worse than none.
fn accelerator(role: MenuRole) -> &'static str {
    match role {
        MenuRole::Cut => "x",
        MenuRole::Copy => "c",
        MenuRole::Paste => "v",
        MenuRole::SelectAll => "a",
        _ => "",
    }
}

/// Shows the platform's definition panel for `text`, anchored at `at` —
/// the baseline origin of the text on screen, logical viewport px. macOS's own Look Up: the same panel a force
/// click opens in any AppKit text view, which is what a custom-drawn UI
/// otherwise has no way to offer (ADR 0017, decision 6).
///
/// Not deferred, unlike the menu: `showDefinitionForAttributedString:`
/// puts up an ordinary popover and returns, with no nested run loop to
/// re-enter winit from.
pub fn show_definition(window: &Window, at: kui_core::Vec2, text: &str) -> bool {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if text.is_empty() {
        return false;
    }
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
    let s = NSAttributedString::from_nsstring(&NSString::from_str(text));
    // The core hands over the baseline origin the panel wants, so this is
    // a straight conversion: the view is flipped, and logical viewport px
    // are its points.
    let at = NSPoint::new(at.x as f64, at.y as f64);
    view.showDefinitionForAttributedString_atPoint(Some(&s), at);
    true
}

/// One window's menu target, kept alive between presentations because the
/// row the user picked arrives after the call that showed the menu has
/// returned.
pub struct MacMenu {
    target: Retained<MenuTarget>,
}

impl MacMenu {
    pub fn new() -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        Some(Self {
            target: MenuTarget::new(mtm),
        })
    }

    /// Schedules `items` to be shown over `window` at `at` (logical
    /// viewport px). Returns false when the window has no AppKit view,
    /// which is a host this build cannot show a menu for.
    pub fn present(&self, window: &Window, at: Vec2, items: &[MenuItem]) -> bool {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Ok(handle) = window.window_handle() else {
            return false;
        };
        let RawWindowHandle::AppKit(h) = handle.as_raw() else {
            return false;
        };
        // SAFETY: winit hands out the content view of a live window, and
        // this runs on the main thread (the event loop's).
        let view: Retained<NSView> =
            unsafe { Retained::retain(h.ns_view.as_ptr().cast::<NSView>()) }.expect("live view");
        *self.target.ivars().pending.borrow_mut() = Some(Pending {
            view,
            at,
            items: items.to_vec(),
        });
        self.target.ivars().chosen.set(-1);
        // Zero delay is not "now": it is the next turn of the run loop,
        // which is the whole point (see the module docs).
        let sel: Sel = sel!(kuiMenuShow);
        unsafe {
            let _: () = msg_send![&*self.target, performSelector: sel, withObject: std::ptr::null::<AnyObject>(), afterDelay: 0.0f64];
        }
        true
    }

    /// The row the last menu reported, taken. `None` while a menu is up,
    /// and `None` for a menu the user dismissed.
    pub fn take_chosen(&self) -> Option<usize> {
        let chosen = self.target.ivars().chosen.replace(-1);
        (chosen >= 0).then_some(chosen as usize)
    }

    /// Whether a menu is waiting to be shown or is on screen. False means
    /// the presentation is over and [`Self::take_chosen`] is the answer —
    /// including the answer "nothing", for a menu the user dismissed.
    pub fn busy(&self) -> bool {
        self.target.ivars().up.get() || self.target.ivars().pending.borrow().is_some()
    }
}
