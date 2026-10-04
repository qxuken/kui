//! Files dragged in from the Finder, with *where* they are.
//!
//! winit 0.30 registers the window delegate as the
//! `NSDraggingDestination` and answers three of its selectors —
//! `draggingEntered:` (queues `HoveredFile` per path, answers Copy),
//! `performDragOperation:` (queues `DroppedFile` per path) and
//! `draggingExited:` — with no position on any of them, and never
//! implements `draggingUpdated:`, so between entering and releasing the
//! window hears nothing at all. During an OS drag the pointer is the drag
//! session's and no `CursorMoved` arrives either. A drop *zone* is a
//! place, so the runner asks AppKit itself.
//!
//! **How.** The same mechanism as `macos_text_input`: the selectors are
//! Apple's, so the runner overrides them on winit's delegate class at
//! runtime, once per process — `draggingEntered:`, `draggingExited:` and
//! `performDragOperation:` replaced (`method_setImplementation`),
//! `draggingUpdated:` and `draggingEnded:` added (`class_addMethod`).
//! Each reads `draggingLocation` — window coordinates, converted through
//! the content view, which winit flips, so the result is the logical
//! top-left frame the core uses — and on enter and perform the
//! pasteboard's file names; queues one message per call by delegate; and
//! wakes the loop, which drains them into `InputEvent::DragFiles` /
//! `DropFiles` / `DragCancel` for the pane whose delegate it is.
//!
//! **Replaced, not wrapped.** winit's implementations queue exactly the
//! events these supersede, with no position; had they run, the runner
//! would hear every file twice and have to know which copy to drop. On
//! macOS the runner ignores winit's three file events outright.
//!
//! **The answer.** What `draggingUpdated:` returns *is* the cursor — the
//! green plus over a place that takes the files, the not-allowed circle
//! over one that does not — and a `None` on the last update before the
//! release means the drop never happens and the icon slides home. The
//! delegate runs inside AppKit's callback, where the `App` is winit's to
//! borrow, so it answers from a stamp the runner writes after every
//! dispatch (`Core::drop_target().is_some()`): one update late, and
//! AppKit sends updates periodically while the drag is inside the window
//! whether or not the pointer moved, so a stale answer never outlives a
//! frame or two. Enter answers Copy before any stamp exists — an
//! optimistic first answer corrected on the first update, rather than a
//! not-allowed cursor over a zone for a frame.
//!
//! `draggingEnded:` fires after every session, released or not, drop or
//! no drop; it is a cancel when the session delivered no drop and no
//! exit — the release off every zone that AppKit refused — so the last
//! zone the files were over un-lights.
//!
//! The dependence is on Apple's `NSDraggingDestination` selectors and on
//! winit registering its window delegate as the destination, both
//! checked at install; a winit that moved the destination elsewhere is
//! skipped, and the runner falls back to winit's positionless events.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use kui_core::Vec2;
use objc2::encode::Encode;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Method, Sel};
use objc2::{ffi, msg_send, sel};
// The type winit registered the window for (`registerForDraggedTypes`):
// deprecated in favour of per-item file URLs, and the one the pasteboard
// answers for a registration made with it, so the one to read.
#[allow(deprecated)]
use objc2_app_kit::NSFilenamesPboardType;
use objc2_app_kit::{NSPasteboard, NSView};
use objc2_foundation::{NSArray, NSPoint, NSString};
use winit::window::Window;

use crate::Waker;

/// What one delegate callback left for the runner.
#[derive(Clone, Debug, PartialEq)]
pub enum DragMsg {
    /// Entered or moved: the files are over the window at this point.
    Over(Vec<String>, Vec2),
    /// Released at this point.
    Drop(Vec<String>, Vec2),
    /// Left the window, or the session ended without a drop.
    Cancel,
}

/// `NSDragOperationNone` / `NSDragOperationCopy`, as `draggingEntered:`
/// and `draggingUpdated:` return them.
const OP_NONE: usize = 0;
const OP_COPY: usize = 1;

/// One window's drag session, by its delegate.
#[derive(Clone, Copy, Debug, Default)]
struct Session {
    /// What the runner last stamped: a zone is under the files.
    accepts: bool,
    /// The session delivered a drop, or an exit, so `draggingEnded:` is
    /// not a cancel.
    settled: bool,
}

#[derive(Default)]
struct State {
    /// Per delegate (its pointer): the stamp and the session.
    sessions: HashMap<usize, Session>,
    /// Messages waiting for the runner, by delegate, oldest first.
    queue: Vec<(usize, DragMsg)>,
    waker: Option<Waker>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

static INSTALLED: OnceLock<bool> = OnceLock::new();

fn view_of(window: &Window) -> Option<Retained<NSView>> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let RawWindowHandle::AppKit(h) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: a live winit content view, on the event loop's thread.
    unsafe { Retained::retain(h.ns_view.as_ptr().cast::<NSView>()) }
}

/// The window delegate's identity, as the overrides see it.
pub fn delegate_ptr(window: &Window) -> Option<usize> {
    let view = view_of(window)?;
    let ns_window = view.window()?;
    let delegate = ns_window.delegate()?;
    Some(Retained::as_ptr(&delegate) as *const AnyObject as usize)
}

/// Whose loop to wake when a drag reaches a window.
pub fn set_waker(waker: Waker) {
    STATE.with(|s| s.borrow_mut().waker = Some(waker));
}

/// Installs the overrides on `window`'s delegate class (once per process)
/// and starts answering for the window. Returns false when there is no
/// AppKit window, or its delegate defines none of the selectors — a
/// winit whose delegate is not the dragging destination.
pub fn attach(window: &Window) -> bool {
    let Some(view) = view_of(window) else {
        return false;
    };
    let Some(ns_window) = view.window() else {
        return false;
    };
    let Some(delegate) = ns_window.delegate() else {
        return false;
    };
    let ptr = Retained::as_ptr(&delegate) as *const AnyObject as usize;
    // SAFETY: `class` on any object.
    let cls: &AnyClass = unsafe { msg_send![&*delegate, class] };
    let installed = *INSTALLED.get_or_init(|| install(cls));
    if installed {
        STATE.with(|s| s.borrow_mut().sessions.insert(ptr, Session::default()));
    }
    installed
}

/// Whether the overrides are in: then winit's own file events are the
/// duplicates and the runner ignores them.
pub fn installed() -> bool {
    INSTALLED.get().copied().unwrap_or(false)
}

/// Stops answering for `window`: its session and any message not yet
/// drained go with it.
pub fn detach(window: &Window) {
    let Some(ptr) = delegate_ptr(window) else {
        return;
    };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.sessions.remove(&ptr);
        s.queue.retain(|(d, _)| *d != ptr);
    });
}

/// Stamps what `window` answers the OS: whether a zone is under the
/// files, from the core the runner just dispatched to.
pub fn stamp(window: &Window, accepts: bool) {
    let Some(ptr) = delegate_ptr(window) else {
        return;
    };
    STATE.with(|s| {
        if let Some(session) = s.borrow_mut().sessions.get_mut(&ptr) {
            session.accepts = accepts;
        }
    });
}

/// The messages the delegates left since the last drain, oldest first.
pub fn take_messages() -> Vec<(usize, DragMsg)> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().queue))
}

type EnteredFn = unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject) -> usize;
type UpdatedFn = unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject) -> usize;
type ExitedFn = unsafe extern "C-unwind" fn(&AnyObject, Sel, Option<&AnyObject>);
type PerformFn = unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject) -> Bool;
type EndedFn = unsafe extern "C-unwind" fn(&AnyObject, Sel, Option<&AnyObject>);

/// A typed override as the runtime takes it.
fn imp<F: Copy>(f: F) -> Imp {
    const { assert!(std::mem::size_of::<F>() == std::mem::size_of::<Imp>()) };
    // SAFETY: a function pointer of any signature; the runtime calls it
    // with the selector's, which is the type it was cast from.
    unsafe { std::mem::transmute_copy(&f) }
}

/// The class in `cls`'s chain that defines `sel`, and the method.
fn defining_class(cls: &AnyClass, sel: Sel) -> Option<(&AnyClass, &Method)> {
    let mut at = Some(cls);
    while let Some(c) = at {
        if let Some(m) = c
            .instance_methods()
            .iter()
            .find(|m| m.name() == sel)
            .copied()
        {
            return Some((c, m));
        }
        at = c.superclass();
    }
    None
}

fn install(cls: &AnyClass) -> bool {
    let Some((owner, entered)) = defining_class(cls, sel!(draggingEntered:)) else {
        return false;
    };
    let Some((_, exited)) = defining_class(cls, sel!(draggingExited:)) else {
        return false;
    };
    let Some((_, perform)) = defining_class(cls, sel!(performDragOperation:)) else {
        return false;
    };
    // Something already answers the two winit does not define: another
    // override, or a winit that grew them. Leave the class alone.
    if defining_class(cls, sel!(draggingUpdated:)).is_some()
        || defining_class(cls, sel!(draggingEnded:)).is_some()
    {
        return false;
    }
    let owner = owner as *const AnyClass as *mut AnyClass;
    let types_op = format!("{}@:@\0", usize::ENCODING);
    let types_void = "v@:@\0";
    // SAFETY: each type string matches its override's signature; the
    // class is alive for the process.
    let added = unsafe {
        ffi::class_addMethod(
            owner,
            sel!(draggingUpdated:),
            imp(updated_override as UpdatedFn),
            types_op.as_ptr().cast(),
        )
        .as_bool()
            && ffi::class_addMethod(
                owner,
                sel!(draggingEnded:),
                imp(ended_override as EndedFn),
                types_void.as_ptr().cast(),
            )
            .as_bool()
    };
    if !added {
        return false;
    }
    let imps: [(&Method, Imp); 3] = [
        (entered, imp(entered_override as EnteredFn)),
        (exited, imp(exited_override as ExitedFn)),
        (perform, imp(perform_override as PerformFn)),
    ];
    for (m, imp) in imps {
        // SAFETY: each override has the selector's C signature and answers
        // what the protocol asks; the originals are not called (see the
        // module doc).
        unsafe { m.set_implementation(imp) };
    }
    true
}

/// The dragged files' names from the session's pasteboard, as winit reads
/// them. Empty when the pasteboard holds none.
#[allow(deprecated)]
fn paths_of(sender: &AnyObject) -> Vec<String> {
    // SAFETY: `sender` is the `NSDraggingInfo` the protocol hands every
    // destination method.
    let pb: Option<Retained<NSPasteboard>> = unsafe { msg_send![sender, draggingPasteboard] };
    let Some(pb) = pb else {
        return Vec::new();
    };
    let Some(list) = pb.propertyListForType(unsafe { NSFilenamesPboardType }) else {
        return Vec::new();
    };
    // An array of strings for this type, per the pasteboard's contract;
    // winit casts unchecked, this checks each.
    let Ok(names) = list.downcast::<NSArray>() else {
        return Vec::new();
    };
    names
        .iter()
        .filter_map(|s| s.downcast_ref::<NSString>().map(|s| s.to_string()))
        .collect()
}

/// Where the dragged image's cursor is, in the logical top-left frame of
/// the window's content view.
fn location_of(sender: &AnyObject) -> Vec2 {
    // SAFETY: the protocol's `draggingLocation`, an NSPoint in the
    // destination window's base coordinates.
    let p: NSPoint = unsafe { msg_send![sender, draggingLocation] };
    // The destination window (the session's, not the delegate's — a
    // delegate has no `window`), and its content view: winit's, flipped,
    // so converting from the window puts the origin at the top-left.
    let window: Option<Retained<AnyObject>> =
        unsafe { msg_send![sender, draggingDestinationWindow] };
    let Some(window) = window else {
        return Vec2::new(p.x as f32, p.y as f32);
    };
    let view: Option<Retained<NSView>> = unsafe { msg_send![&*window, contentView] };
    let Some(view) = view else {
        return Vec2::new(p.x as f32, p.y as f32);
    };
    let local = view.convertPoint_fromView(p, None);
    Vec2::new(local.x as f32, local.y as f32)
}

fn push(this: &AnyObject, msg: DragMsg) {
    let ptr = this as *const AnyObject as usize;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if !s.sessions.contains_key(&ptr) {
            return;
        }
        s.queue.push((ptr, msg));
        if let Some(w) = &s.waker {
            w.wake();
        }
    });
}

fn session_of(this: &AnyObject) -> Option<Session> {
    let ptr = this as *const AnyObject as usize;
    STATE.with(|s| s.borrow().sessions.get(&ptr).copied())
}

fn update_session(this: &AnyObject, f: impl FnOnce(&mut Session)) {
    let ptr = this as *const AnyObject as usize;
    STATE.with(|s| {
        if let Some(session) = s.borrow_mut().sessions.get_mut(&ptr) {
            f(session);
        }
    });
}

unsafe extern "C-unwind" fn entered_override(
    this: &AnyObject,
    _sel: Sel,
    sender: &AnyObject,
) -> usize {
    if session_of(this).is_none() {
        return OP_NONE;
    }
    // Optimistic until the runner has looked (module doc).
    update_session(this, |s| {
        s.accepts = true;
        s.settled = false;
    });
    let paths = paths_of(sender);
    if paths.is_empty() {
        return OP_NONE;
    }
    push(this, DragMsg::Over(paths, location_of(sender)));
    OP_COPY
}

unsafe extern "C-unwind" fn updated_override(
    this: &AnyObject,
    _sel: Sel,
    sender: &AnyObject,
) -> usize {
    let Some(session) = session_of(this) else {
        return OP_NONE;
    };
    let paths = paths_of(sender);
    if paths.is_empty() {
        return OP_NONE;
    }
    push(this, DragMsg::Over(paths, location_of(sender)));
    if session.accepts { OP_COPY } else { OP_NONE }
}

unsafe extern "C-unwind" fn exited_override(
    this: &AnyObject,
    _sel: Sel,
    _sender: Option<&AnyObject>,
) {
    update_session(this, |s| s.settled = true);
    push(this, DragMsg::Cancel);
}

unsafe extern "C-unwind" fn perform_override(
    this: &AnyObject,
    _sel: Sel,
    sender: &AnyObject,
) -> Bool {
    let Some(session) = session_of(this) else {
        return Bool::NO;
    };
    update_session(this, |s| s.settled = true);
    let paths = paths_of(sender);
    if paths.is_empty() {
        return Bool::NO;
    }
    push(this, DragMsg::Drop(paths, location_of(sender)));
    // A release the stamp refuses is one AppKit slides home; the core
    // emits nothing for a drop off every zone either way.
    Bool::new(session.accepts)
}

unsafe extern "C-unwind" fn ended_override(
    this: &AnyObject,
    _sel: Sel,
    _sender: Option<&AnyObject>,
) {
    let Some(session) = session_of(this) else {
        return;
    };
    if !session.settled {
        update_session(this, |s| s.settled = true);
        push(this, DragMsg::Cancel);
    }
}
