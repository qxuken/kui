//! Reading and moving the window's selection
//! (`docs/adr/0017-selection-as-a-scope.md`).
//!
//! A press-drag inside a `selectable` node needs nothing from a host —
//! the core owns the gesture. These are for the host that wants to *act*
//! on the result: read it, replace it, drop it. The copy itself is
//! `kui_request_copy`, which is separate because it can end in a question
//! rather than an answer.

use super::*;

/// The window's selected text — a `selectable` scope's, a `cells` grid's,
/// or the focused editor's, whichever the window holds. False when
/// nothing is selected. `out` is borrowed until the next selection call
/// on this context.
#[unsafe(no_mangle)]
pub extern "C" fn kui_selection_text(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(text) = c.core().copy_selection() else {
            return false;
        };
        c.selection_text = text;
        write_str(out, &c.selection_text)
    })
}

/// The same selection with the formatting the text declared — bold,
/// italic, a span's own colour — for a host offering a second clipboard
/// flavour. False when there is no text selection or nothing to carry;
/// never a replacement for `kui_selection_text`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_selection_html(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(html) = c.core().selection_html() else {
            return false;
        };
        c.selection_html = html;
        write_str(out, &c.selection_html)
    })
}

/// Selects everything in the scope `key` declared — every run of a
/// `selectable` container, or the whole screen of a `cells` grid. False
/// for a node that is not a scope, or drew nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_select_all_in(ptr: *mut KuiCtx, key: u64) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        key != 0 && c.core().select_all_in(Key(key))
    })
}

/// Drops the window's selection, whichever kind it is; true when there
/// was one.
#[unsafe(no_mangle)]
pub extern "C" fn kui_clear_selection(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().clear_selection())
    })
}

/// Hands a borrowed view of `s` back through an out pointer, when the
/// caller supplied one.
fn write_str(out: *mut KuiStr, s: &str) -> bool {
    if !out.is_null() {
        unsafe {
            out.write(KuiStr {
                ptr: s.as_ptr(),
                len: s.len(),
            })
        };
    }
    true
}
