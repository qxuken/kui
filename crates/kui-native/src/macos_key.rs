//! A non-activating window that cannot become key, asked of AppKit rather
//! than undone after the fact.
//!
//! `with_active(false)` only decides how a window is *ordered* at
//! creation; nothing in winit stops AppKit making it key later, and a
//! press does exactly that: `-[NSWindow sendEvent:]` makes any window
//! whose `canBecomeKeyWindow` says YES key on mouse-down, and `WinitWindow`
//! answers YES for every window. So each press on a popup took the
//! keyboard off its owner, the owner's titlebar greyed, and the runner
//! asked for it back a batch later (`settle_focus`, `close_pane`) — a
//! flick of the owner's chrome on every open and every pick, 20–50 ms
//! wide, measured through the AX API on macOS 27. The same happened at
//! open, since ordering a window front makes it key too.
//!
//! The answer the runner wanted was NO, and AppKit asks: `canBecomeKeyWindow`
//! and `canBecomeMainWindow` are replaced on the class that defines them
//! (winit's window class, once per process, the way `macos_text_input`
//! patches the view's) with overrides that answer NO for a window
//! registered here and call winit's answer for every other. A window that
//! cannot become key still gets the mouse — events go to the window under
//! the cursor, and winit's view accepts the first — and the keys it wants
//! come from its owner, which stays key: the runner routes an owner's key
//! events to the popup above it, so nothing is lost.

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::OnceLock;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Method, Sel};
use objc2::sel;
use objc2_app_kit::NSView;
use winit::window::Window;

thread_local! {
    /// The `NSWindow`s that answer NO, by pointer.
    static REFUSING: RefCell<HashSet<usize>> = RefCell::new(HashSet::new());
}

/// winit's answers, called for every window not registered here.
struct Originals {
    key: Imp,
    main: Imp,
}

static ORIGINALS: OnceLock<Option<Originals>> = OnceLock::new();

fn ns_window(window: &Window) -> Option<Retained<objc2_app_kit::NSWindow>> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let RawWindowHandle::AppKit(h) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: a live winit content view, on the event loop's thread.
    let view = unsafe { Retained::retain(h.ns_view.as_ptr().cast::<NSView>()) }?;
    view.window()
}

/// Makes `window` one that cannot become key or main. Installs the
/// overrides on its class the first time; returns false when there is no
/// AppKit window, or its class defines neither answer.
pub fn refuse_key(window: &Window) -> bool {
    let Some(ns) = ns_window(window) else {
        return false;
    };
    let installed = ORIGINALS.get_or_init(|| install(ns.class())).is_some();
    if installed {
        REFUSING.with(|r| r.borrow_mut().insert(Retained::as_ptr(&ns) as usize));
    }
    installed
}

/// Lets `window`'s pointer be reused: called before the window goes.
pub fn release(window: &Window) {
    if let Some(ns) = ns_window(window) {
        REFUSING.with(|r| r.borrow_mut().remove(&(Retained::as_ptr(&ns) as usize)));
    }
}

fn defining_class(cls: &AnyClass, sel: Sel) -> Option<&Method> {
    let mut at = Some(cls);
    while let Some(c) = at {
        if let Some(m) = c
            .instance_methods()
            .iter()
            .find(|m| m.name() == sel)
            .copied()
        {
            return Some(m);
        }
        at = c.superclass();
    }
    None
}

type AnswerFn = unsafe extern "C-unwind" fn(&AnyObject, Sel) -> Bool;

fn install(cls: &AnyClass) -> Option<Originals> {
    let key = defining_class(cls, sel!(canBecomeKeyWindow))?;
    let main = defining_class(cls, sel!(canBecomeMainWindow))?;
    let originals = Originals {
        key: key.implementation(),
        main: main.implementation(),
    };
    // SAFETY: both overrides have the selector's C signature and call the
    // implementation they replaced for a window not registered here.
    unsafe {
        key.set_implementation(imp(key_override as AnswerFn));
        main.set_implementation(imp(main_override as AnswerFn));
    }
    Some(originals)
}

/// A typed override as the runtime takes it.
fn imp<F: Copy>(f: F) -> Imp {
    const { assert!(std::mem::size_of::<F>() == std::mem::size_of::<Imp>()) };
    // SAFETY: a function pointer of any signature; the runtime calls it
    // with the selector's, which is the type it was cast from.
    unsafe { std::mem::transmute_copy(&f) }
}

fn refuses(this: &AnyObject) -> bool {
    let ptr = this as *const AnyObject as usize;
    REFUSING.with(|r| r.borrow().contains(&ptr))
}

fn original(which: fn(&Originals) -> Imp) -> AnswerFn {
    let o = ORIGINALS
        .get()
        .and_then(Option::as_ref)
        .expect("an override runs only once installed");
    // SAFETY: the implementation replaced had this signature.
    unsafe { std::mem::transmute::<Imp, AnswerFn>(which(o)) }
}

unsafe extern "C-unwind" fn key_override(this: &AnyObject, sel: Sel) -> Bool {
    if refuses(this) {
        return Bool::NO;
    }
    // SAFETY: winit's implementation, on the receiver it was defined for.
    unsafe { original(|o| o.key)(this, sel) }
}

unsafe extern "C-unwind" fn main_override(this: &AnyObject, sel: Sel) -> Bool {
    if refuses(this) {
        return Bool::NO;
    }
    // SAFETY: as above.
    unsafe { original(|o| o.main)(this, sel) }
}
