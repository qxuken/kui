//! Reading and moving the window's selection.
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

/// The text selection's two ends as the drag made them — the anchor
/// where the press landed, the focus where the pointer is — each as the
/// data index of the virtualised row it is in (`-1` outside every
/// virtualised row) and the byte inside that row's own text. Directed,
/// so a Shift-press that kept the anchor reads as one. False
/// with no text selection; a grid's is `kui_cell_selection`. Any out
/// pointer may be NULL.
#[unsafe(no_mangle)]
pub extern "C" fn kui_selection_ends(
    ptr: *mut KuiCtx,
    anchor_index: *mut i64,
    anchor_byte: *mut usize,
    focus_index: *mut i64,
    focus_byte: *mut usize,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some((a, f)) = c.core().selection_ends() else {
            return false;
        };
        let put = |index: *mut i64, byte: *mut usize, e: kui_core::RangeEnd| unsafe {
            if let Some(p) = index.as_mut() {
                *p = e.row.map_or(-1, |r| r as i64);
            }
            if let Some(p) = byte.as_mut() {
                *p = e.byte;
            }
        };
        put(anchor_index, anchor_byte, a);
        put(focus_index, focus_byte, f);
        true
    })
}

/// A `cells` grid's selection, the window's when it lives in one: the
/// grid's key, the anchor and the focus as the drag made them — each an
/// absolute line (`originLine` plus the row, so a scroll does not move
/// it) and a column — and whether it is a block rather than linewise.
/// False when the window's selection is not a
/// grid's; a text selection's ends are `kui_selection_ends`. Any out
/// pointer may be NULL.
#[unsafe(no_mangle)]
pub extern "C" fn kui_cell_selection(
    ptr: *mut KuiCtx,
    node: *mut u64,
    anchor_line: *mut u64,
    anchor_col: *mut usize,
    focus_line: *mut u64,
    focus_col: *mut usize,
    block: *mut bool,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(sel) = c.core().cell_selection() else {
            return false;
        };
        let put = |line: *mut u64, col: *mut usize, e: kui_core::CellEnd| unsafe {
            if let Some(p) = line.as_mut() {
                *p = e.line;
            }
            if let Some(p) = col.as_mut() {
                *p = e.col;
            }
        };
        unsafe {
            if let Some(p) = node.as_mut() {
                *p = sel.node.0;
            }
            if let Some(p) = block.as_mut() {
                *p = sel.block;
            }
        }
        put(anchor_line, anchor_col, sel.anchor);
        put(focus_line, focus_col, sel.focus);
        true
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
