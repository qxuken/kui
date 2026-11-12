//! The text the platform types into a window from outside the keyboard:
//! the Emoji & Symbols palette and Dictation.
//!
//! Both talk to the focused view through `NSTextInputClient`, which
//! winit's view implements — for a composition. What it drops is the
//! rest of the protocol:
//!
//! - `insertText:replacementRange:` is committed only when the view has
//!   marked text (`view.rs`, `insert_text`; winit master gates on a
//!   composition session instead, the same effect). The palette's insert
//!   is not a composition: it arrives with no marked text and outside
//!   any `keyDown:`, and winit ignores it. So did every emoji picked
//!   from the menu, until this.
//! - Dictation starts only against a view that answers `isEditable`
//!   with YES **and** `selectedRange` with a real range — measured on a
//!   bare `NSTextInputClient` view: with either missing the Start
//!   Dictation row does nothing, with both it flips to Cancel Dictation
//!   and the microphone comes up. winit's view answers neither.
//!
//! **How.** The selectors are Apple's protocol selectors, not winit's
//! names, so the runner overrides them on winit's view class at runtime
//! (`method_setImplementation` / `class_addMethod`), once per process,
//! and calls winit's implementation first wherever it had one:
//!
//! - `keyDown:` — wrapped to count depth, because ordinary typing *also*
//!   reaches `insertText:` (winit's `keyDown:` calls
//!   `interpretKeyEvents:`, and turns that into `KeyboardInput` itself).
//!   An insert inside a key press is the keyboard's; one outside it is
//!   the platform's.
//! - `insertText:replacementRange:` — winit's runs; then, if the view
//!   had no marked text and no key press is on the stack, the string is
//!   queued for the runner, which dispatches it as `InputEvent::Commit`
//!   to the window's key target — the channel a composition's commit
//!   already takes, so a sink hears a `text` event and a
//!   stock editor types it. Never twice: winit commits exactly when it
//!   had marked text, and this commits exactly when it had not.
//! - `isEditable` (added) and `selectedRange` (replaced) — answered from
//!   facts the runner stamps per window after each frame: editable when
//!   the core has an IME anchor (a focused stock editor or a sink with a
//!   caret), the range from the focused stock editor's caret and
//!   selection as UTF-16 offsets into its text, `{0, 0}` for a sink.
//!   With nothing focused, winit's own answers (`{NSNotFound, 0}`).
//! - `firstRectForCharacterRange:actualRange:` and
//!   `preferredTextAccessoryPlacement` — a view without a caret has no
//!   rectangle and hides the system cursor accessory, including the
//!   input-source indicator. Winit's cached rect can outlive its caret
//!   even after the runner turns composition off.
//!
//! **What stays winit's.** A replacement range is ignored — winit's own
//! TODO — so a dictation *revision* of words already committed appends
//! instead of replacing. Writing Tools additionally wants the selected
//! text back (`attributedSubstringForProposedRange:`) and a real range on
//! the insert that applies its result, which is the surrounding-text half
//! of the protocol no window on kui has asked for; AutoFill wants an
//! `NSTextField`. Both rows stay trimmed from the Edit menu
//! (`macos_menu::trim_edit`).
//!
//! **The first thing in the runner that reaches into winit rather than
//! around it.** The dependence is on Apple's selectors and on winit's
//! view being the `NSTextInputClient`, both of which every winit version
//! since IME support has had; a version that stopped defining a selector
//! here is skipped at install, not crashed.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use kui_core::{Core, Key};
use objc2::encode::Encode;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Method, Sel};
use objc2::{ffi, msg_send, sel};
use objc2_app_kit::{NSTextCursorAccessoryPlacement, NSView};
use objc2_foundation::{NSAttributedString, NSNotFound, NSObject, NSRange, NSRect, NSString};
use winit::window::Window;

use crate::Waker;

/// The stock editor's key, version, caret byte and selection bytes: what
/// a UTF-16 count was made from.
type CountedFrom = (Key, u64, usize, Option<(usize, usize)>);

/// What one view answers the platform, stamped by the runner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Facts {
    editable: bool,
    /// UTF-16 offset of the caret, and the selection's length past it.
    caret: usize,
    selection: usize,
    /// What the UTF-16 numbers were computed from — the stock editor's
    /// key, version, caret byte and selection bytes — so a frame that
    /// moved nothing recounts nothing (the count walks the text up to
    /// the caret).
    from: Option<CountedFrom>,
}

#[derive(Default)]
struct State {
    /// How many `keyDown:` calls are on the stack (0 or 1 in practice).
    key_depth: u32,
    /// Per view (its pointer), what it answers.
    facts: HashMap<usize, Facts>,
    /// Inserts the platform made outside a key press, by view, oldest
    /// first, waiting for the runner.
    commits: Vec<(usize, String)>,
    waker: Option<Waker>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// winit's implementations, called first by the overrides. Set once,
/// when the first window installs; the fn pointers are immutable after.
struct Originals {
    key_down: Imp,
    insert_text: Imp,
    selected_range: Imp,
    first_rect: Imp,
}

static ORIGINALS: OnceLock<Option<Originals>> = OnceLock::new();

fn view_of(window: &Window) -> Option<Retained<NSView>> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let RawWindowHandle::AppKit(h) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: a live winit content view, on the event loop's thread.
    unsafe { Retained::retain(h.ns_view.as_ptr().cast::<NSView>()) }
}

/// The view's identity, as the overrides see it.
pub fn view_ptr(window: &Window) -> Option<usize> {
    view_of(window).map(|v| Retained::as_ptr(&v) as usize)
}

/// Whose loop to wake when the platform inserts text. Set once, when
/// the runner has a proxy to give.
pub fn set_waker(waker: Waker) {
    STATE.with(|s| s.borrow_mut().waker = Some(waker));
}

/// Installs the overrides on `window`'s view class (once per process)
/// and starts answering for the view. Returns false when there is no
/// AppKit view, or the class defines none of the selectors this needs —
/// a winit whose view is not the `NSTextInputClient`.
pub fn attach(window: &Window) -> bool {
    let Some(view) = view_of(window) else {
        return false;
    };
    let cls = view.class();
    let installed = ORIGINALS.get_or_init(|| install(cls)).is_some();
    if installed {
        STATE.with(|s| {
            s.borrow_mut()
                .facts
                .insert(Retained::as_ptr(&view) as usize, Facts::default())
        });
    }
    installed
}

/// Stops answering for `window`'s view: its facts and any insert not yet
/// drained go with it.
pub fn detach(window: &Window) {
    let Some(ptr) = view_ptr(window) else {
        return;
    };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.facts.remove(&ptr);
        s.commits.retain(|(v, _)| *v != ptr);
    });
}

/// Stamps what `window`'s view answers, from the frame `core` just laid
/// out. Cheap when nothing moved: a compare against what the last count
/// was computed from.
pub fn stamp(window: &Window, core: &Core) {
    let Some(view) = view_of(window) else {
        return;
    };
    let ptr = Retained::as_ptr(&view) as usize;
    let lost_anchor = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(facts) = s.facts.get_mut(&ptr) else {
            return false;
        };
        if core.ime_rect().is_none() {
            let lost_anchor = facts.editable;
            *facts = Facts::default();
            return lost_anchor;
        }
        facts.editable = true;
        let Some(key) = core.edit.focused() else {
            // A sink with a caret: give dictation a valid range.
            facts.caret = 0;
            facts.selection = 0;
            facts.from = None;
            return false;
        };
        let Some((caret, selection)) = core.edit.caret_and_selection(key) else {
            return false;
        };
        let from = Some((key, core.edit.version(key), caret, selection));
        if facts.from == from {
            return false;
        }
        facts.from = from;
        // A secure editor's offsets are in its bullets, one UTF-16 unit
        // each, and its text is not copied out to count them (backlog
        // F177: kui zeroes what it holds of a password).
        let text = if core.edit.is_secure(key) {
            None
        } else {
            let Some(text) = core.edit.text(key) else {
                return false;
            };
            Some(text)
        };
        let utf16 = |byte: usize| match &text {
            Some(text) => text.get(..byte).map_or(0, |t| t.encode_utf16().count()),
            None => byte / kui_core::SECURE_BULLET.len_utf8(),
        };
        match selection {
            Some((a, z)) => {
                facts.caret = utf16(a);
                facts.selection = utf16(z) - facts.caret;
            }
            None => {
                facts.caret = utf16(caret);
                facts.selection = 0;
            }
        }
        false
    });
    // AppKit can cache the coordinates even with composition disabled.
    // Invalidate after releasing the facts borrow, since it can query us.
    if lost_anchor && let Some(context) = view.inputContext() {
        context.invalidateCharacterCoordinates();
    }
}

/// Drops the composition `window`'s view has open, if any — what the
/// runner calls before turning the window's IME off, so the input
/// method's own candidate window closes with it rather than waiting on a
/// view that no longer passes it keys. winit's `set_ime_allowed(false)`
/// clears its marked text but leaves the input context alone.
pub fn discard_marked_text(window: &Window) {
    let Some(view) = view_of(window) else {
        return;
    };
    if let Some(context) = view.inputContext() {
        context.discardMarkedText();
    }
}

/// The inserts the platform made since the last drain, by view pointer,
/// oldest first.
pub fn take_commits() -> Vec<(usize, String)> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().commits))
}

type KeyDownFn = unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject);
type InsertTextFn = unsafe extern "C-unwind" fn(&AnyObject, Sel, &NSObject, NSRange);
type SelectedRangeFn = unsafe extern "C-unwind" fn(&AnyObject, Sel) -> NSRange;
type FirstRectFn = unsafe extern "C-unwind" fn(&AnyObject, Sel, NSRange, *mut NSRange) -> NSRect;
type IsEditableFn = unsafe extern "C-unwind" fn(&AnyObject, Sel) -> Bool;
type AccessoryPlacementFn =
    unsafe extern "C-unwind" fn(&AnyObject, Sel) -> NSTextCursorAccessoryPlacement;

/// A typed override as the runtime takes it.
fn imp<F: Copy>(f: F) -> Imp {
    const { assert!(std::mem::size_of::<F>() == std::mem::size_of::<Imp>()) };
    // SAFETY: a function pointer of any signature; the runtime calls it
    // with the selector's, which is the type it was cast from.
    unsafe { std::mem::transmute_copy(&f) }
}

/// The class in `cls`'s chain that defines `sel` — the nearest one, so
/// what runs today is what is replaced — and its implementation.
/// `instance_method` alone would answer with an inherited method, and
/// replacing *that* would reach every `NSView` in the process; the
/// view's own class is not winit's either, since AccessKit subclasses
/// it at runtime (`AccessKitSubclassOfWinitView`) and defines none of
/// these selectors itself.
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

fn install(cls: &AnyClass) -> Option<Originals> {
    let (_, key_down) = defining_class(cls, sel!(keyDown:))?;
    let (owner, insert_text) = defining_class(cls, sel!(insertText:replacementRange:))?;
    let (_, selected_range) = defining_class(cls, sel!(selectedRange))?;
    let (_, first_rect) = defining_class(cls, sel!(firstRectForCharacterRange:actualRange:))?;
    let originals = Originals {
        key_down: key_down.implementation(),
        insert_text: insert_text.implementation(),
        selected_range: selected_range.implementation(),
        first_rect: first_rect.implementation(),
    };
    // Added to the class whose NSTextInputClient this is: winit does not
    // define isEditable and dictation asks for it.
    let types = format!("{}@:\0", Bool::ENCODING);
    // SAFETY: the type string matches the override's signature; the class
    // is alive for the process.
    let owner = owner as *const AnyClass as *mut AnyClass;
    let added = unsafe {
        ffi::class_addMethod(
            owner,
            sel!(isEditable),
            imp(is_editable_override as IsEditableFn),
            types.as_ptr().cast(),
        )
    };
    if !added.as_bool() {
        return None;
    }
    let types = format!("{}@:\0", NSTextCursorAccessoryPlacement::ENCODING);
    // SAFETY: the optional NSTextInputClient selector returns NSInteger.
    unsafe {
        ffi::class_addMethod(
            owner,
            sel!(preferredTextAccessoryPlacement),
            imp(accessory_placement_override as AccessoryPlacementFn),
            types.as_ptr().cast(),
        );
    }
    // Replaced, each on the class that defines it. An `Imp` is any
    // function pointer as far as the runtime is concerned; the
    // signatures are checked where each is called.
    let imps: [(&Method, Imp); 4] = [
        (key_down, imp(key_down_override as KeyDownFn)),
        (insert_text, imp(insert_text_override as InsertTextFn)),
        (
            selected_range,
            imp(selected_range_override as SelectedRangeFn),
        ),
        (first_rect, imp(first_rect_override as FirstRectFn)),
    ];
    for (m, imp) in imps {
        // SAFETY: each override has the selector's C signature, calls the
        // implementation it replaced, and handles what it handled.
        unsafe { m.set_implementation(imp) };
    }
    Some(originals)
}

fn originals() -> &'static Originals {
    ORIGINALS
        .get()
        .and_then(Option::as_ref)
        .expect("an override runs only once installed")
}

fn facts_of(this: &AnyObject) -> Option<Facts> {
    let ptr = this as *const AnyObject as usize;
    STATE.with(|s| s.borrow().facts.get(&ptr).copied())
}

/// Counts a key press on the stack for as long as winit's `keyDown:`
/// runs — released on unwind too, so a panic in winit's handler does not
/// leave every later insert looking like typing.
struct KeyGuard;

impl KeyGuard {
    fn enter() -> Self {
        STATE.with(|s| s.borrow_mut().key_depth += 1);
        Self
    }
}

impl Drop for KeyGuard {
    fn drop(&mut self) {
        STATE.with(|s| s.borrow_mut().key_depth -= 1);
    }
}

unsafe extern "C-unwind" fn key_down_override(this: &AnyObject, sel: Sel, event: &AnyObject) {
    let _inside = KeyGuard::enter();
    // SAFETY: winit's `keyDown:`, with the arguments it takes.
    let orig: KeyDownFn = unsafe { std::mem::transmute(originals().key_down) };
    unsafe { orig(this, sel, event) }
}

unsafe extern "C-unwind" fn insert_text_override(
    this: &AnyObject,
    sel: Sel,
    string: &NSObject,
    range: NSRange,
) {
    // Read before winit runs: a commit clears the marked text.
    let had_marked: bool = unsafe { msg_send![this, hasMarkedText] };
    let inside_key = STATE.with(|s| s.borrow().key_depth > 0);
    // SAFETY: winit's `insertText:replacementRange:`, with its arguments.
    let orig: InsertTextFn = unsafe { std::mem::transmute(originals().insert_text) };
    unsafe { orig(this, sel, string, range) };
    if had_marked || inside_key {
        return;
    }
    // Guaranteed an `NSString` or an `NSAttributedString`, per the
    // protocol; winit reads it the same way.
    let text = if let Some(s) = string.downcast_ref::<NSAttributedString>() {
        s.string().to_string()
    } else if let Some(s) = string.downcast_ref::<NSString>() {
        s.to_string()
    } else {
        return;
    };
    // The same exclusion winit applies: a control character here is a
    // command, not text.
    if text.is_empty() || text.chars().next().is_some_and(char::is_control) {
        return;
    }
    let ptr = this as *const AnyObject as usize;
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if !s.facts.contains_key(&ptr) {
            return;
        }
        s.commits.push((ptr, text));
        if let Some(w) = &s.waker {
            w.wake();
        }
    });
}

unsafe extern "C-unwind" fn selected_range_override(this: &AnyObject, sel: Sel) -> NSRange {
    match facts_of(this) {
        Some(f) if f.editable => NSRange::new(f.caret, f.selection),
        _ => {
            // SAFETY: winit's `selectedRange`.
            let orig: SelectedRangeFn = unsafe { std::mem::transmute(originals().selected_range) };
            unsafe { orig(this, sel) }
        }
    }
}

unsafe extern "C-unwind" fn first_rect_override(
    this: &AnyObject,
    sel: Sel,
    range: NSRange,
    actual_range: *mut NSRange,
) -> NSRect {
    if facts_of(this).is_some_and(|f| !f.editable) {
        if !actual_range.is_null() {
            // SAFETY: AppKit supplies a writable out pointer or null.
            unsafe { actual_range.write(NSRange::new(NSNotFound as usize, 0)) };
        }
        return NSRect::ZERO;
    }
    // SAFETY: winit's firstRectForCharacterRange:actualRange: implementation.
    let orig: FirstRectFn = unsafe { std::mem::transmute(originals().first_rect) };
    unsafe { orig(this, sel, range, actual_range) }
}

unsafe extern "C-unwind" fn accessory_placement_override(
    this: &AnyObject,
    _sel: Sel,
) -> NSTextCursorAccessoryPlacement {
    if facts_of(this).is_some_and(|f| !f.editable) {
        NSTextCursorAccessoryPlacement::Invisible
    } else {
        NSTextCursorAccessoryPlacement::Unspecified
    }
}

/// YES for a view the runner has not registered yet — one being created.
/// AppKit will not *create* an `NSTextInputContext` for a view that
/// answers `isEditable` NO (an existing context is handed back regardless,
/// which is why the first window never showed this), and winit's view
/// reads `inputContext` in its initialiser and panics on nil. The first
/// window's view was born before the class was patched; a second one, on
/// a loop taken back from an earlier runner, is born patched — and got
/// no context. Registered views answer from the stamp.
unsafe extern "C-unwind" fn is_editable_override(this: &AnyObject, _sel: Sel) -> Bool {
    Bool::new(facts_of(this).is_none_or(|f| f.editable))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appkit_has_no_anchor_or_accessory_when_the_caret_disappears() {
        let view = NSObject::new();
        let ptr = Retained::as_ptr(&view).cast::<AnyObject>();
        // SAFETY: NSObject is an Objective-C object, retained by view.
        let this = unsafe { &*ptr };
        let id = ptr as usize;
        let accessory =
            || unsafe { accessory_placement_override(this, sel!(preferredTextAccessoryPlacement)) };
        let expected = [
            (true, NSTextCursorAccessoryPlacement::Unspecified),
            (false, NSTextCursorAccessoryPlacement::Invisible),
            (true, NSTextCursorAccessoryPlacement::Unspecified),
        ];
        for (editable, placement) in expected {
            STATE.with(|s| {
                s.borrow_mut().facts.insert(
                    id,
                    Facts {
                        editable,
                        ..Facts::default()
                    },
                );
            });
            assert_eq!(accessory(), placement);
            if !editable {
                let mut actual = NSRange::new(0, 1);
                // SAFETY: there is no caret, so the override answers directly.
                let rect = unsafe {
                    first_rect_override(
                        this,
                        sel!(firstRectForCharacterRange:actualRange:),
                        NSRange::new(0, 0),
                        &mut actual,
                    )
                };
                assert_eq!(rect, NSRect::ZERO);
                assert_eq!(actual, NSRange::new(NSNotFound as usize, 0));
            }
        }
        STATE.with(|s| s.borrow_mut().facts.remove(&id));
        assert_eq!(accessory(), NSTextCursorAccessoryPlacement::Unspecified);
    }
}
