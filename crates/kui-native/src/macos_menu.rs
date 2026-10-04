//! The macOS context menu: an `NSMenu`, shown where the platform shows
//! one, with the platform's own wording, keyboard and Services.
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
//! ([`MacMenuBar`]). It is
//! the same `NSMenu`, one level up and with the opposite lifetime: not
//! popped up and torn down around one press, but *set* on `NSApp` and left
//! there until the declaration changes. It also needs none of the deferral
//! above — `setMainMenu:` returns immediately, and AppKit runs the bar's
//! own tracking loop from its own turn of the run loop, never from inside
//! a winit callback. What it does need is a wake: an item chosen from the
//! bar (⌘S with no other event behind it, above all) has to reach a loop
//! that may be asleep, so the target holds a [`Waker`] and rings it.
//!
//! **And the bar that is up when the app declares none** ([`MacMenuBar`]
//! again): winit's
//! application menu kept as it is, with an Edit menu and a Window menu
//! beside it. The Window menu is what macOS's window shortcuts fire
//! through — Fill, Center, the tiling arrows and full screen are rows
//! AppKit adds to `NSApp.windowsMenu` and nowhere else, so a process
//! without one has no fn+ctrl+F — and the Edit menu's rows replay the
//! chord they spell rather than binding a role, so an app that hears ⌘C
//! itself still does.
//!
//! What this is still not: a submenu of the app's own, or a Services entry.

use std::cell::{Cell, RefCell};

use kui_core::{MenuBar, MenuItem, MenuRole, Vec2};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSAttributedStringNSStringDrawing, NSColor, NSEventModifierFlags, NSFont,
    NSFontAttributeName, NSForegroundColorAttributeName, NSMenu, NSMenuItem,
    NSMutableParagraphStyle, NSParagraphStyleAttributeName, NSTextAlignment, NSTextTab, NSView,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSAttributedString, NSDictionary, NSMutableAttributedString,
    NSObject, NSObjectProtocol, NSPoint, NSRange, NSString, ns_string,
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
    let hints = HintTab::of(items);
    for (i, item) in items.iter().enumerate() {
        if item.role == MenuRole::Separator {
            menu.addItem(&NSMenuItem::separatorItem(mtm));
            continue;
        }
        let target = unsafe { &*(target as *const MenuTarget as *const AnyObject) };
        menu.addItem(&menu_row(mtm, item, target, sel!(kuiMenuPick:), i, hints));
    }
    menu
}

/// The accelerator a row draws without binding it: one AppKit cannot
/// take as a key equivalent — a spelling kui does not parse (`"gd"`, an
/// app's own multi-key hint) or one with no modifier, which would fire
/// on every press of the key (see [`menu_row`]). Such a hint is
/// drawn exactly as written, bound by nobody.
fn hint(item: &MenuItem) -> Option<&str> {
    let text = item.accel_text().filter(|t| !t.is_empty())?;
    let bound = kui_core::Accel::parse(text)
        .filter(|a| a.mods.any())
        .and_then(|a| a.key_equivalent())
        .is_some_and(|k| !k.is_empty());
    (!bound).then_some(text)
}

/// Where a menu's hints end: one right-aligned tab stop for every row
/// that has one, past the widest title, so the hints line up in a column
/// of their own the way key equivalents do. AppKit has no
/// key-equivalent column for a string it does not bind, so the hint is
/// drawn in the row's attributed title, after a tab.
#[derive(Clone, Copy)]
struct HintTab(f64);

impl HintTab {
    /// The gap between the widest title and the widest hint, in points:
    /// about what AppKit leaves before a key equivalent.
    const GAP: f64 = 24.0;

    /// `None` when no row of `items` has a hint, so a menu without any
    /// is built exactly as before.
    fn of(items: &[MenuItem]) -> Option<Self> {
        let font = NSFont::menuFontOfSize(0.0);
        let (mut title, mut hinted) = (0f64, None::<f64>);
        for item in items.iter().filter(|i| i.role != MenuRole::Separator) {
            title = title.max(text_width(item.text(), &font));
            if let Some(h) = hint(item) {
                hinted = Some(hinted.unwrap_or(0.0).max(text_width(h, &font)));
            }
        }
        hinted.map(|h| Self(title + Self::GAP + h))
    }
}

/// The width `text` takes in `font`, in points.
fn text_width(text: &str, font: &NSFont) -> f64 {
    let s = NSMutableAttributedString::from_nsstring(&NSString::from_str(text));
    let all = NSRange::new(0, s.length());
    // SAFETY: the font attribute takes an `NSFont`.
    unsafe { s.addAttribute_value_range(NSFontAttributeName, font, all) };
    s.size().width
}

/// `title`, a tab, and `hint` in the secondary label colour, right-aligned
/// at `tab`: a row's title with its hint drawn where a key equivalent
/// would be. A disabled row's whole title is the disabled colour, since
/// AppKit dims a plain title for a disabled row but draws an attributed
/// one as given.
fn hinted_title(
    title: &str,
    hint: &str,
    tab: HintTab,
    enabled: bool,
) -> Retained<NSAttributedString> {
    let s =
        NSMutableAttributedString::from_nsstring(&NSString::from_str(&format!("{title}\t{hint}")));
    let all = NSRange::new(0, s.length());
    let title_len = title.encode_utf16().count();
    let hint_range = NSRange::new(title_len + 1, s.length() - title_len - 1);
    let style = NSMutableParagraphStyle::new();
    // SAFETY: an empty options dictionary is a valid one.
    let stop = unsafe {
        NSTextTab::initWithTextAlignment_location_options(
            NSTextTab::alloc(),
            NSTextAlignment::Right,
            tab.0,
            &NSDictionary::new(),
        )
    };
    style.setTabStops(Some(&NSArray::from_retained_slice(&[stop])));
    // SAFETY: each attribute is given the class its key takes.
    unsafe {
        s.addAttribute_value_range(NSFontAttributeName, &NSFont::menuFontOfSize(0.0), all);
        s.addAttribute_value_range(NSParagraphStyleAttributeName, &style, all);
        if enabled {
            s.addAttribute_value_range(
                NSForegroundColorAttributeName,
                &NSColor::secondaryLabelColor(),
                hint_range,
            );
        } else {
            s.addAttribute_value_range(
                NSForegroundColorAttributeName,
                &NSColor::disabledControlTextColor(),
                all,
            );
        }
    }
    Retained::into_super(s)
}

/// One `NSMenuItem` for a row, the same for the context menu and the bar
///: the row was one `MenuItem` on the way in, so it is one
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
/// — including into an `edit` — and swallow the key. They, and the
/// spellings kui does not parse, are drawn in the title instead, at the
/// menu's [`HintTab`] (F119: they were dropped, and the row said nothing
/// of its keys).
fn menu_row(
    mtm: MainThreadMarker,
    item: &MenuItem,
    target: &AnyObject,
    action: Sel,
    tag: usize,
    hints: Option<HintTab>,
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
        if let (Some(tab), Some(h)) = (hints, hint(item)) {
            row.setAttributedTitle(Some(&hinted_title(item.text(), h, tab, item.enabled)));
        }
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
/// otherwise has no way to offer.
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

/// A row the responder chain answers: no target, so the key window (or
/// the application) is what performs `action`, and AppKit's own
/// validation greys it where nothing does.
fn responder_row(
    mtm: MainThreadMarker,
    title: &str,
    action: Sel,
    key: &str,
    mods: Option<NSEventModifierFlags>,
) -> Retained<NSMenuItem> {
    let row = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(action),
            &NSString::from_str(key),
        )
    };
    if let Some(mods) = mods {
        row.setKeyEquivalentModifierMask(mods);
    }
    row
}

/// Keeps, of the rows AppKit appended to an Edit menu, the two the
/// runner made work, and removes the two it cannot.
///
/// AppKit recognises a bar's Edit menu by its head's title in
/// `setMainMenu:` of a root it has not seen, and appends a separator,
/// Writing Tools ▸, AutoFill ▸, Start Dictation and Emoji & Symbols
/// (checked by hand on macOS 26.6). None of the four did
/// anything in a winit window; Emoji & Symbols and Start Dictation do
/// now, through the view answers `mod macos_text_input` installs, and
/// stay. Writing Tools opens its panel and every tool in it does
/// nothing, because the view cannot hand back the selected text; AutoFill
/// greys its own rows, wanting an `NSTextField`. A row that opens a panel
/// that cannot act is worse than no row, so those two go, with their
/// separator when nothing is left after it. Removing them holds: AppKit
/// appends in the scan and not on open, and does not scan a root twice.
fn trim_edit(menu: &NSMenu, own: isize) {
    let kept = |item: &NSMenuItem| {
        item.action()
            .is_some_and(|a| a == sel!(orderFrontCharacterPalette:) || a == sel!(startDictation:))
    };
    let mut i = own;
    while i < menu.numberOfItems() {
        let item = menu.itemAtIndex(i).expect("counted");
        if item.isSeparatorItem() || kept(&item) {
            i += 1;
        } else {
            menu.removeItemAtIndex(i);
        }
    }
    // The separator AppKit put before its rows, alone: nothing after it.
    if menu.numberOfItems() == own + 1
        && menu.itemAtIndex(own).is_some_and(|it| it.isSeparatorItem())
    {
        menu.removeItemAtIndex(own);
    }
}

/// Enter Full Screen, in every Window menu kui registers.
///
/// AppKit adds this row itself, but only to a Window menu registered
/// before `finishLaunching` — which none of kui's are, since the bar is
/// applied from the run loop — so kui adds it. The row is the platform's
/// own: `toggleFullScreen:` on the key window, and AppKit's validation
/// retitles it *Exit Full Screen* while the window is in one. Bound to
/// fn+F, the chord macOS 15 moved it to, with a hidden twin on ⌃⌘F, the
/// chord before that; a hidden item's key equivalent still fires, which
/// is how AppKit spells its own alternates.
fn full_screen_rows(mtm: MainThreadMarker, menu: &NSMenu) {
    menu.addItem(&responder_row(
        mtm,
        "Enter Full Screen",
        sel!(toggleFullScreen:),
        "f",
        Some(NSEventModifierFlags::Function),
    ));
    let twin = responder_row(
        mtm,
        "Enter Full Screen",
        sel!(toggleFullScreen:),
        "f",
        Some(NSEventModifierFlags::Control | NSEventModifierFlags::Command),
    );
    twin.setHidden(true);
    menu.addItem(&twin);
}

/// Keeps `window` out of the Window menu's list of windows:
/// a popup is a menu surface with no title, and the list is for windows
/// the user would switch to.
pub fn exclude_from_windows_menu(window: &Window) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else {
        return;
    };
    // SAFETY: a live winit content view, on the event loop's thread.
    let view = unsafe { Retained::retain(h.ns_view.as_ptr().cast::<NSView>()) };
    if let Some(ns_window) = view.and_then(|v| v.window()) {
        ns_window.setExcludedFromWindowsMenu(true);
    }
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
    /// or [`CHORD_TAG`] plus a row of [`EDIT_ROWS`] for the standard Edit
    /// menu, or `-1` for none. Read and cleared by the runner's drain.
    chosen: Cell<isize>,
    /// The loop to wake when one arrives. A ⌘-shortcut chosen from the
    /// menu bar is the whole of the event as far as winit is concerned —
    /// AppKit consumed the key — so without this the app would sit still
    /// until the user moved the mouse.
    waker: RefCell<Option<crate::Waker>>,
    /// Which rows of the standard Edit menu apply, as [`EditState::bits`]:
    /// stamped by the runner after each event batch and read by
    /// `validateMenuItem:` when the menu opens or a key equivalent is
    /// matched — so the answer is a cell read, and AppKit's callback never
    /// touches the core (the re-entrance this module exists to avoid).
    edit_state: Cell<u8>,
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

        /// AppKit asks whether a row applies, for the menus that
        /// autoenable: the standard Edit menu, whose chord
        /// rows apply when the runner's last stamp said so, and a declared
        /// `Window` menu, whose own rows keep the state the declaration
        /// set — answering it back is what lets that menu autoenable at
        /// all, so that the rows AppKit and kui add beside them are
        /// validated (retitled, greyed) the way the standard bar's are.
        #[unsafe(method(validateMenuItem:))]
        fn validate(&self, item: &NSMenuItem) -> objc2::runtime::Bool {
            let tag = item.tag();
            if tag < CHORD_TAG {
                return objc2::runtime::Bool::new(item.isEnabled());
            }
            let state = EditState::from_bits(self.ivars().edit_state.get());
            let on = EDIT_ROWS
                .get((tag - CHORD_TAG) as usize)
                .copied()
                .flatten()
                .is_some_and(|(_, letter, shift)| state.applies(letter, shift));
            objc2::runtime::Bool::new(on)
        }
    }

    unsafe impl NSObjectProtocol for BarTarget {}
);

impl BarTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(BarIvars {
            chosen: Cell::new(-1),
            waker: RefCell::new(None),
            edit_state: Cell::new(0),
        });
        unsafe { msg_send![super(this), init] }
    }
}

/// The process's menu bar: `NSApp.mainMenu`, built from whatever the
/// frontmost window's frame declared.
///
/// One per process and not one per window, because that is what the
/// platform has: the runner applies the declaration
/// of the window that holds the keyboard, and re-applies when either the
/// declaration or that window changes.
pub struct MacMenuBar {
    target: Retained<BarTarget>,
    /// `(menu, item)` per flat tag, so a pick is one index lookup.
    map: RefCell<Vec<(usize, usize)>>,
    /// The standard bar, built the first time it is wanted and
    /// kept: applying it again after a declaration is one `setMainMenu:`,
    /// and its Window menu stays the one `NSApp.windowsMenu` names.
    standard: RefCell<Option<Standard>>,
}

/// The standard bar and the one menu of it the platform must be told
/// about by name.
struct Standard {
    root: Retained<NSMenu>,
    window: Retained<NSMenu>,
    /// The Edit menu and how many of its rows are kui's, for
    /// [`trim_edit`] after every `setMainMenu:`.
    edit: (Retained<NSMenu>, isize),
}

/// Where the standard Edit menu's tags start. The declared bar's tags are
/// indices into `map`, which never reaches this; a tag at or past it is a
/// row of [`EDIT_ROWS`].
const CHORD_TAG: isize = 1 << 20;

/// A row of the standard Edit menu: the chord it spells.
/// Chosen from the menu, it is *replayed* as that chord
/// rather than performed as a role, so the sink that would have heard
/// ⌘C from the keyboard hears it from the menu too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditChord {
    pub letter: char,
    pub shift: bool,
}

/// The standard Edit menu, top to bottom: title, letter, Shift. `None` is
/// a separator. The wording and the order are macOS's own.
const EDIT_ROWS: [Option<(&str, char, bool)>; 7] = [
    Some(("Undo", 'z', false)),
    Some(("Redo", 'z', true)),
    None,
    Some(("Cut", 'x', false)),
    Some(("Copy", 'c', false)),
    Some(("Paste", 'v', false)),
    Some(("Select All", 'a', false)),
];

/// Which rows of the standard Edit menu apply right now:
/// what the runner reads off the front window's core once per event batch
/// and stamps on the bar, so the menu greys Copy with nothing to copy the
/// way a Mac menu does. Each row is lit when the chord it spells would do
/// something — or when a key sink would hear the chord, since a sink may
/// do anything with it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditState {
    pub undo: bool,
    pub redo: bool,
    pub cut: bool,
    pub copy: bool,
    pub paste: bool,
    pub select_all: bool,
}

impl EditState {
    /// Bit 0 undo, 1 redo, 2 cut, 3 copy, 4 paste, 5 select all.
    fn bits(self) -> u8 {
        (self.undo as u8)
            | (self.redo as u8) << 1
            | (self.cut as u8) << 2
            | (self.copy as u8) << 3
            | (self.paste as u8) << 4
            | (self.select_all as u8) << 5
    }

    fn from_bits(b: u8) -> Self {
        Self {
            undo: b & 1 != 0,
            redo: b & 2 != 0,
            cut: b & 4 != 0,
            copy: b & 8 != 0,
            paste: b & 16 != 0,
            select_all: b & 32 != 0,
        }
    }

    /// Whether the row spelling `letter` (with Shift) applies.
    fn applies(self, letter: char, shift: bool) -> bool {
        match (letter, shift) {
            ('z', false) => self.undo,
            ('z', true) => self.redo,
            ('x', _) => self.cut,
            ('c', _) => self.copy,
            ('v', _) => self.paste,
            ('a', _) => self.select_all,
            _ => true,
        }
    }
}

/// What the user chose from the bar, as the runner reads it back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarPick {
    /// `(menu, item)` of the declared bar.
    Item(usize, usize),
    /// A row of the standard Edit menu.
    Chord(EditChord),
}

impl MacMenuBar {
    pub fn new() -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        Some(Self {
            target: BarTarget::new(mtm),
            map: RefCell::new(Vec::new()),
            standard: RefCell::new(None),
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
    /// An empty declaration puts the standard bar up: the one
    /// a process with no declaration has, not no bar at all.
    ///
    /// A declared menu titled `Window` is registered as the platform's
    /// Window menu, which is what makes AppKit add its tiling rows to it
    /// (and their shortcuts work), and gains the one row AppKit adds only
    /// at launch, Enter Full Screen; any other declaration has none of
    /// this, which is what declaring the bar exactly means.
    pub fn apply(&self, bar: &MenuBar) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        // Built before anything else is applied, so that the application
        // menu it keeps is winit's and not a declaration's first menu.
        self.standard(mtm, &app);
        if bar.is_empty() {
            self.map.borrow_mut().clear();
            let standard = self.standard.borrow();
            let standard = standard.as_ref().expect("built above");
            app.setWindowsMenu(Some(&standard.window));
            app.setMainMenu(Some(&standard.root));
            trim_edit(&standard.edit.0, standard.edit.1);
            return;
        }
        let mut map = Vec::new();
        let mut windows_menu = None;
        let mut edit_menus = Vec::new();
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
            let hints = HintTab::of(&menu.items);
            for (ii, item) in menu.items.iter().enumerate() {
                if item.role == MenuRole::Separator {
                    sub.addItem(&NSMenuItem::separatorItem(mtm));
                    continue;
                }
                let tag = map.len();
                map.push((mi, ii));
                sub.addItem(&self.row(mtm, item, tag, hints));
            }
            if menu.label == "Edit" {
                edit_menus.push((sub.clone(), sub.numberOfItems()));
            }
            if menu.label == "Window" {
                full_screen_rows(mtm, &sub);
                // Validated, unlike the other declared menus: the rows
                // AppKit adds once this is registered, and the full-screen
                // row above, are the platform's, and only validation
                // retitles Enter Full Screen to Exit Full Screen or greys
                // Remove Window from Set. The declared rows keep their
                // own state through `validateMenuItem:`.
                sub.setAutoenablesItems(true);
                windows_menu = Some(sub.clone());
            }
            head.setSubmenu(Some(&sub));
            root.addItem(&head);
        }
        *self.map.borrow_mut() = map;
        app.setWindowsMenu(windows_menu.as_deref());
        app.setMainMenu(Some(&root));
        for (menu, own) in &edit_menus {
            trim_edit(menu, *own);
        }
    }

    /// The standard bar, built once: the application menu
    /// already on `NSApp` — winit's, with its Services entry still the one
    /// the app registered — an Edit menu of [`EDIT_ROWS`], and a Window
    /// menu with the three rows AppKit does not add itself.
    ///
    /// Called before the first bar of any kind is applied: after a
    /// declaration the menu on `NSApp` is that declaration's first, which
    /// is not the one to keep.
    fn standard(&self, mtm: MainThreadMarker, app: &NSApplication) {
        if self.standard.borrow().is_some() {
            return;
        }
        // A fresh root, with winit's application-menu item moved across —
        // it stays the menu its Services entry was registered on. Moved
        // rather than copied, since an `NSMenu` has one supermenu and
        // re-parenting the menu itself is an exception; and a fresh root
        // rather than winit's with two items appended, because AppKit
        // scans a bar for the menus it knows by title only in
        // `setMainMenu:` of a root it has not seen — and that scan is
        // where Edit gains Emoji & Symbols and Dictation, which W15 made
        // work (`trim_edit` keeps those two and removes the two that
        // cannot). A process built without winit's menu
        // (`EventLoopBuilderExtMacOS::with_default_menu(false)`) gets the
        // one row nothing can do without.
        let root = NSMenu::new(mtm);
        let app_head = app
            .mainMenu()
            .filter(|m| m.numberOfItems() > 0)
            .map(|m| {
                let head = m.itemAtIndex(0).expect("counted");
                m.removeItemAtIndex(0);
                head
            })
            .unwrap_or_else(|| {
                let app_menu = NSMenu::new(mtm);
                let quit = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        NSMenuItem::alloc(mtm),
                        ns_string!("Quit"),
                        Some(sel!(terminate:)),
                        ns_string!("q"),
                    )
                };
                app_menu.addItem(&quit);
                let head = NSMenuItem::new(mtm);
                head.setSubmenu(Some(&app_menu));
                head
            });
        root.addItem(&app_head);

        // Edit: every row is a chord the runner already performs, spelled
        // where a Mac user looks for it. The key equivalent is real, so
        // AppKit consumes ⌘C and hands it here — and the runner replays it
        // as the key it was, which is why nothing is lost by the detour.
        // Autoenabled, alone among kui's menus: every row's target is
        // `BarTarget`, whose `validateMenuItem:` answers from the state the
        // runner stamped (`set_edit_state`).
        let edit = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("Edit"));
        let target = unsafe { &*(&*self.target as *const BarTarget as *const AnyObject) };
        for (i, row) in EDIT_ROWS.iter().enumerate() {
            let Some((title, letter, shift)) = row else {
                edit.addItem(&NSMenuItem::separatorItem(mtm));
                continue;
            };
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(mtm),
                    &NSString::from_str(title),
                    Some(sel!(kuiBarPick:)),
                    &NSString::from_str(&letter.to_string()),
                )
            };
            let mut flags = NSEventModifierFlags::Command;
            if *shift {
                flags |= NSEventModifierFlags::Shift;
            }
            unsafe {
                item.setTarget(Some(target));
                item.setKeyEquivalentModifierMask(flags);
                item.setTag(CHORD_TAG + i as isize);
            }
            edit.addItem(&item);
        }
        let edit_own = edit.numberOfItems();
        let edit_head = NSMenuItem::new(mtm);
        edit_head.setTitle(ns_string!("Edit"));
        edit_head.setSubmenu(Some(&edit));
        root.addItem(&edit_head);

        // Window: the rows AppKit's own responders answer, with no target
        // so the key window is the one that minimizes, and autoenabled so
        // a window that cannot zoom greys the row. Registered by `apply`
        // as the Window menu, which is what makes AppKit add Fill, Center,
        // the tiling submenus and the window list to it.
        let window = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("Window"));
        window.addItem(&responder_row(
            mtm,
            "Minimize",
            sel!(performMiniaturize:),
            "m",
            None,
        ));
        window.addItem(&responder_row(mtm, "Zoom", sel!(performZoom:), "", None));
        full_screen_rows(mtm, &window);
        window.addItem(&NSMenuItem::separatorItem(mtm));
        window.addItem(&responder_row(
            mtm,
            "Bring All to Front",
            sel!(arrangeInFront:),
            "",
            None,
        ));
        let window_head = NSMenuItem::new(mtm);
        window_head.setTitle(ns_string!("Window"));
        window_head.setSubmenu(Some(&window));
        root.addItem(&window_head);

        *self.standard.borrow_mut() = Some(Standard {
            root,
            window,
            edit: (edit, edit_own),
        });
    }

    /// One row: its wording, its shortcut where kui can parse one, and the
    /// tag that says which item it is. `declare_menu_bar` has already
    /// normalized the accelerator into the platform's spelling, so this
    /// is the same string the drawn bar would have shown — one
    /// declaration, two readings of it, and never two different
    /// shortcuts.
    fn row(
        &self,
        mtm: MainThreadMarker,
        item: &MenuItem,
        tag: usize,
        hints: Option<HintTab>,
    ) -> Retained<NSMenuItem> {
        let target = unsafe { &*(&*self.target as *const BarTarget as *const AnyObject) };
        menu_row(mtm, item, target, sel!(kuiBarPick:), tag, hints)
    }

    /// Stamps which rows of the standard Edit menu apply. Called after
    /// every event batch; a stamp equal to the last is a byte compare.
    pub fn set_edit_state(&self, state: EditState) {
        self.target.ivars().edit_state.set(state.bits());
    }

    /// What the last pick chose, taken. `None` when nothing has been
    /// chosen since the last drain.
    pub fn take_chosen(&self) -> Option<BarPick> {
        let tag = self.target.ivars().chosen.replace(-1);
        if tag < 0 {
            return None;
        }
        if tag >= CHORD_TAG {
            let (_, letter, shift) = EDIT_ROWS
                .get((tag - CHORD_TAG) as usize)
                .copied()
                .flatten()?;
            return Some(BarPick::Chord(EditChord { letter, shift }));
        }
        let (menu, item) = self.map.borrow().get(tag as usize).copied()?;
        Some(BarPick::Item(menu, item))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hint_is_what_binds_nothing() {
        let row = |accel: &str| MenuItem::new("Row").accel(accel);
        // Not parsed: drawn as written.
        assert_eq!(hint(&row("gd")), Some("gd"));
        // Parsed but bare: binding it would take the key from everything.
        assert_eq!(hint(&row("space")), Some("space"));
        // A key equivalent: bound, so AppKit draws it.
        assert_eq!(hint(&row("mod+s")), None);
        assert_eq!(hint(&MenuItem::role(MenuRole::Copy)), None);
        // Declared empty on purpose: nothing at all.
        assert_eq!(hint(&row("")), None);
        assert_eq!(hint(&MenuItem::new("Row")), None);
    }

    #[test]
    fn a_hinted_title_is_the_title_a_tab_and_the_hint() {
        let t = hinted_title("Go to Definition", "gd", HintTab(200.0), true);
        assert_eq!(t.string().to_string(), "Go to Definition\tgd");
        // Disabled or not, the text is the same; only its colour moves.
        let t = hinted_title("Rename…", "grn", HintTab(200.0), false);
        assert_eq!(t.string().to_string(), "Rename…\tgrn");
    }
}
