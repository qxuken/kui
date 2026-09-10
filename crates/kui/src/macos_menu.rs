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
//! **The application menu bar is the other half of this file**
//! ([`MacMenuBar`], `docs/adr/0018-a-menu-bar-the-app-declares.md`). It is
//! the same `NSMenu`, one level up and with the opposite lifetime: not
//! popped up and torn down around one press, but *set* on `NSApp` and left
//! there until the declaration changes. It also needs none of the deferral
//! above — `setMainMenu:` returns immediately, and AppKit runs the bar's
//! own tracking loop from its own turn of the run loop, never from inside
//! a winit callback. What it does need is a wake: an item chosen from the
//! bar (⌘S with no other event behind it, above all) has to reach a loop
//! that may be asleep, so the target holds a [`Waker`] and rings it.
//!
//! What this is still not: a submenu, or a Services entry.

use std::cell::{Cell, RefCell};

use kui_core::{MenuBar, MenuItem, MenuRole, Vec2};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem, NSView};
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

/// Builds the `NSMenu` for `items`: one [`menu_row`] per row, so a
/// context menu's row reads exactly as the bar's does — the role's wording
/// where the row declared none, its accelerator or the role's, its check
/// state. Display only for the accelerator, since the app's own key
/// handling is what actually runs a row; a popup menu's key equivalents
/// fire only while it is up.
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
        let target = unsafe { &*(target as *const MenuTarget as *const AnyObject) };
        menu.addItem(&menu_row(mtm, item, target, sel!(kuiMenuPick:), i));
    }
    menu
}

/// One `NSMenuItem` for a row, the same for the context menu and the bar
/// (ADR 0018): the row was one `MenuItem` on the way in, so it is one
/// reading on the way out, wherever the platform shows it. `tag` is what
/// `action` hears back.
///
/// The accelerator the item declares, parsed back into the parts AppKit
/// wants. `accel_text` is the row's own or, for a standard role, the one
/// this platform reads it by (`⌘C`) — so a role needs no special case,
/// and a row that declares an empty accelerator on purpose binds nothing,
/// which is what declaring it empty means.
///
/// Modifier-less ones are drawn and not bound. AppKit matches a key
/// equivalent in `performKeyEquivalent:`, ahead of the responder chain,
/// so a bare `"space"` would fire the item on every space the user typed
/// — including into an `edit` — and swallow the key.
fn menu_row(
    mtm: MainThreadMarker,
    item: &MenuItem,
    target: &AnyObject,
    action: Sel,
    tag: usize,
) -> Retained<NSMenuItem> {
    let accel = item
        .accel_text()
        .and_then(kui_core::Accel::parse)
        .filter(|a| a.mods.any());
    let key = accel
        .as_ref()
        .and_then(|a| a.key_equivalent())
        .unwrap_or_default();
    let title = NSString::from_str(item.text());
    let equiv = NSString::from_str(&key);
    let row = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &title,
            Some(action),
            &equiv,
        )
    };
    unsafe {
        row.setTarget(Some(target));
        row.setTag(tag as isize);
        row.setEnabled(item.enabled);
        // `NSControlStateValueOn` / `Off`, which are 1 and 0.
        row.setState(objc2_app_kit::NSControlStateValue::from(if item.checked {
            1isize
        } else {
            0
        }));
        if let Some(a) = accel.filter(|_| !key.is_empty()) {
            let mut flags = NSEventModifierFlags::empty();
            if a.mods.super_key {
                flags |= NSEventModifierFlags::Command;
            }
            if a.mods.shift {
                flags |= NSEventModifierFlags::Shift;
            }
            if a.mods.alt {
                flags |= NSEventModifierFlags::Option;
            }
            if a.mods.ctrl {
                flags |= NSEventModifierFlags::Control;
            }
            row.setKeyEquivalentModifierMask(flags);
        }
    }
    row
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

// -- The application menu bar -----------------------------------------------

/// The ivars of [`BarTarget`]: what the last pick was, and who to wake
/// about it.
struct BarIvars {
    /// The flat index of the chosen item in the map the last `apply` built,
    /// or `-1` for none. Read and cleared by the runner's drain.
    chosen: Cell<isize>,
    /// The loop to wake when one arrives. A ⌘-shortcut chosen from the
    /// menu bar is the whole of the event as far as winit is concerned —
    /// AppKit consumed the key — so without this the app would sit still
    /// until the user moved the mouse.
    waker: RefCell<Option<crate::Waker>>,
}

define_class!(
    // SAFETY:
    // - `NSObject` has no subclassing requirements.
    // - `BarTarget` does not implement `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "KuiMenuBarTarget"]
    #[ivars = BarIvars]
    struct BarTarget;

    impl BarTarget {
        /// Every row's action. `tag` is the item's index in the flat map
        /// `apply` built, so the answer needs no lookup and no menu walk.
        #[unsafe(method(kuiBarPick:))]
        fn pick(&self, sender: &NSMenuItem) {
            self.ivars().chosen.set(sender.tag());
            if let Some(waker) = self.ivars().waker.borrow().as_ref() {
                waker.wake();
            }
        }
    }

    unsafe impl NSObjectProtocol for BarTarget {}
);

impl BarTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(BarIvars {
            chosen: Cell::new(-1),
            waker: RefCell::new(None),
        });
        unsafe { msg_send![super(this), init] }
    }
}

/// The process's menu bar: `NSApp.mainMenu`, built from whatever the
/// frontmost window's frame declared.
///
/// One per process and not one per window, because that is what the
/// platform has (ADR 0018, decision 8): the runner applies the declaration
/// of the window that holds the keyboard, and re-applies when either the
/// declaration or that window changes.
pub struct MacMenuBar {
    target: Retained<BarTarget>,
    /// `(menu, item)` per flat tag, so a pick is one index lookup.
    map: RefCell<Vec<(usize, usize)>>,
}

impl MacMenuBar {
    pub fn new() -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        Some(Self {
            target: BarTarget::new(mtm),
            map: RefCell::new(Vec::new()),
        })
    }

    /// Whose loop to wake when the user chooses from the bar. Set once,
    /// when the runner has a proxy to give.
    pub fn set_waker(&self, waker: crate::Waker) {
        *self.target.ivars().waker.borrow_mut() = Some(waker);
    }

    /// Hands `bar` to AppKit, replacing whatever was there.
    ///
    /// The first menu is the application menu: macOS draws that one's title
    /// from the process itself whatever the declaration says, which is why
    /// an app's own menu belongs first and is documented as doing so.
    /// An empty declaration takes the bar away.
    pub fn apply(&self, bar: &MenuBar) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        if bar.is_empty() {
            app.setMainMenu(None);
            self.map.borrow_mut().clear();
            return;
        }
        let mut map = Vec::new();
        let root = NSMenu::new(mtm);
        // Every enable state is the declaration's; without this AppKit
        // greys out every row whose target does not answer
        // `validateMenuItem:`, which is all of them.
        root.setAutoenablesItems(false);
        for (mi, menu) in bar.menus.iter().enumerate() {
            let title = NSString::from_str(&menu.label);
            // A top-level entry is a titled item whose submenu holds the
            // rows; the item itself does nothing when clicked.
            let head = NSMenuItem::new(mtm);
            head.setTitle(&title);
            head.setEnabled(menu.enabled);
            let sub = NSMenu::initWithTitle(NSMenu::alloc(mtm), &title);
            sub.setAutoenablesItems(false);
            for (ii, item) in menu.items.iter().enumerate() {
                if item.role == MenuRole::Separator {
                    sub.addItem(&NSMenuItem::separatorItem(mtm));
                    continue;
                }
                let tag = map.len();
                map.push((mi, ii));
                sub.addItem(&self.row(mtm, item, tag));
            }
            head.setSubmenu(Some(&sub));
            root.addItem(&head);
        }
        *self.map.borrow_mut() = map;
        app.setMainMenu(Some(&root));
    }

    /// One row: its wording, its shortcut where kui can parse one, and the
    /// tag that says which item it is. `declare_menu_bar` has already
    /// normalized the accelerator into the platform's spelling, so this
    /// is the same string the drawn bar would have shown — one
    /// declaration, two readings of it, and never two different
    /// shortcuts.
    fn row(&self, mtm: MainThreadMarker, item: &MenuItem, tag: usize) -> Retained<NSMenuItem> {
        let target = unsafe { &*(&*self.target as *const BarTarget as *const AnyObject) };
        menu_row(mtm, item, target, sel!(kuiBarPick:), tag)
    }

    /// The `(menu, item)` the last pick chose, taken. `None` when nothing
    /// has been chosen since the last drain.
    pub fn take_chosen(&self) -> Option<(usize, usize)> {
        let tag = self.target.ivars().chosen.replace(-1);
        (tag >= 0)
            .then(|| self.map.borrow().get(tag as usize).copied())
            .flatten()
    }
}
