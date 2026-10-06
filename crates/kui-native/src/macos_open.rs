//! The documents macOS asks the app to open: the Finder's Open With, a
//! file dropped on the Dock icon, `open -a App file`, a double-click on a
//! document whose type the app's `Info.plist` declares
//! (`CFBundleDocumentTypes`). Backlog F124.
//!
//! AppKit delivers every one of those as the `kAEOpenDocuments` Apple
//! event, which it hands to the application delegate as
//! `application:openURLs:` (or, on a delegate that has only the older
//! ones, `application:openFiles:` / `application:openFile:`). At launch
//! the event comes inside `finishLaunching` — after
//! `applicationWillFinishLaunching:`, before
//! `applicationDidFinishLaunching:`, so before winit's `resumed` and
//! before any window exists — and the paths are *not* in the process's
//! arguments; while the app runs it comes whenever the user opens one.
//! winit 0.30's delegate (`WinitApplicationDelegate`, which it makes
//! `NSApp`'s delegate when the event loop is built) answers only
//! `applicationDidFinishLaunching:` and `applicationWillTerminate:`, so
//! the event went nowhere: no kui app heard a document the Finder handed
//! it, and AppKit could say the app "cannot open files" of the type.
//!
//! **How.** The same mechanism as `macos_drop` and `macos_text_input`:
//! the selector is Apple's, so the runner adds it to the delegate's class
//! at runtime (`class_addMethod`), once per process — the event loop is
//! one per process and parked between runners (`PARKED_LOOP`), and so is
//! its delegate. The override keeps each URL that `isFileURL` as its
//! `path`, queues the list, and wakes the loop; the shell drains the
//! queue in `about_to_wait` and dispatches `InputEvent::Open` to the main
//! window's core, which the app hears as `{kind="open", paths}` on the
//! root. A list that arrives before the main window has opened — the
//! launch-time one, always — waits in the queue for the first turn after
//! it has. A URL that is not a file (a scheme the app registers) is left
//! out; the event's payload has room for a `urls` beside `paths` when an
//! app asks for one.
//!
//! **Added, not replaced.** There is no winit implementation to wrap. A
//! delegate class that already answers one of the three selectors — a
//! winit that grew one, or an app that installed its own before the
//! runner's — is left alone, so an app that does its own swizzling keeps
//! it.
//!
//! **Which thread.** AppKit sends the delegate its messages on the main
//! thread, which is the event loop's on macOS, so the queue is a
//! thread-local, as `macos_drop`'s is.
//!
//! The dependence is on Apple's `NSApplicationDelegate` selector and on
//! winit setting `NSApp`'s delegate when the loop is built, before the
//! shell attaches; a loop with no delegate by then is skipped, and the
//! app hears no documents, as before.

use std::cell::RefCell;
use std::sync::OnceLock;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
use objc2::{class, ffi, msg_send, sel};
use objc2_foundation::NSString;

use crate::Waker;

#[derive(Default)]
struct State {
    /// What the delegate was handed since the last drain, one list per
    /// call, oldest first.
    queue: Vec<Vec<String>>,
    waker: Option<Waker>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

static INSTALLED: OnceLock<bool> = OnceLock::new();

/// Whose loop to wake when the OS hands over documents. Each shell that
/// attaches to the loop sets its own.
pub fn set_waker(waker: Waker) {
    STATE.with(|s| s.borrow_mut().waker = Some(waker));
}

/// Adds `application:openURLs:` to `NSApp`'s delegate class, once per
/// process. False when there is no delegate yet, or its class already
/// answers documents itself.
pub fn install() -> bool {
    *INSTALLED.get_or_init(install_once)
}

/// The lists the OS handed over since the last drain, oldest first.
pub fn take() -> Vec<Vec<String>> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().queue))
}

/// Puts a list taken back at the end of the queue, for a later turn.
pub fn requeue(paths: Vec<String>) {
    STATE.with(|s| s.borrow_mut().queue.push(paths));
}

/// Whether a list is waiting.
pub fn pending() -> bool {
    STATE.with(|s| !s.borrow().queue.is_empty())
}

type OpenUrlsFn = unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, &AnyObject);

/// A typed override as the runtime takes it.
fn imp<F: Copy>(f: F) -> Imp {
    const { assert!(std::mem::size_of::<F>() == std::mem::size_of::<Imp>()) };
    // SAFETY: a function pointer of any signature; the runtime calls it
    // with the selector's, which is the type it was cast from.
    unsafe { std::mem::transmute_copy(&f) }
}

fn install_once() -> bool {
    // SAFETY: `sharedApplication` on the main thread, where the event loop
    // that made it runs; `delegate` on any NSApplication.
    let app: Retained<AnyObject> = unsafe { msg_send![class!(NSApplication), sharedApplication] };
    let delegate: Option<Retained<AnyObject>> = unsafe { msg_send![&*app, delegate] };
    let Some(delegate) = delegate else {
        return false;
    };
    // SAFETY: `class` on any object.
    let cls: &AnyClass = unsafe { msg_send![&*delegate, class] };
    // Something already answers documents: an app's own override, or a
    // winit that grew one. `instancesRespondToSelector:` walks the chain.
    let answers = |sel: Sel| -> bool {
        // SAFETY: a class method every class has.
        let yes: Bool = unsafe { msg_send![cls, instancesRespondToSelector: sel] };
        yes.as_bool()
    };
    if answers(sel!(application:openURLs:))
        || answers(sel!(application:openFiles:))
        || answers(sel!(application:openFile:))
    {
        return false;
    }
    let owner = cls as *const AnyClass as *mut AnyClass;
    // SAFETY: the type string matches the override's signature — void,
    // self, _cmd, the NSApplication, the NSArray<NSURL *> — and the class
    // is alive for the process.
    unsafe {
        ffi::class_addMethod(
            owner,
            sel!(application:openURLs:),
            imp(open_urls_override as OpenUrlsFn),
            c"v@:@@".as_ptr(),
        )
        .as_bool()
    }
}

/// The file paths among `urls`, an `NSArray<NSURL *>`, in its order.
fn paths_of(urls: &AnyObject) -> Vec<String> {
    // SAFETY: `count` and `objectAtIndex:` on the array AppKit hands the
    // selector; each element an NSURL, asked `isFileURL` and `path`.
    let count: usize = unsafe { msg_send![urls, count] };
    let mut paths = Vec::with_capacity(count);
    for i in 0..count {
        let url: Retained<AnyObject> = unsafe { msg_send![urls, objectAtIndex: i] };
        let file: Bool = unsafe { msg_send![&*url, isFileURL] };
        if !file.as_bool() {
            continue;
        }
        let path: Option<Retained<NSString>> = unsafe { msg_send![&*url, path] };
        if let Some(path) = path {
            paths.push(path.to_string());
        }
    }
    paths
}

unsafe extern "C-unwind" fn open_urls_override(
    _this: &AnyObject,
    _sel: Sel,
    _app: &AnyObject,
    urls: &AnyObject,
) {
    let paths = paths_of(urls);
    if paths.is_empty() {
        return;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.queue.push(paths);
        if let Some(w) = &s.waker {
            w.wake();
        }
    });
}
