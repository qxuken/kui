//! Keyboard focus: declaring it, moving it, and reading where it is
//! (`docs/adr/0002-keyboard-focus-as-data.md`).

use super::*;

/// Declares `key` focused this frame (0 blurs at once). Edge-triggered: the
/// node takes focus on the first frame it is declared, and a declaration
/// repeated every frame does not clobber a Tab press or a click. Any
/// focusable node (an editor, an `on_key` sink, a control, a `focusable`
/// box). To move focus at any time, `kui_focus`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_key_focus(ptr: *mut KuiCtx, key: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_key_focus((key != 0).then_some(Key(key)));
        }
    });
}

/// Moves keyboard focus to `key` now (0 blurs); see
/// docs/adr/0002-keyboard-focus-as-data.md.
#[unsafe(no_mangle)]
pub extern "C" fn kui_focus(ptr: *mut KuiCtx, key: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_focus((key != 0).then_some(Key(key)));
        }
    });
}

/// The key of the node opened under `label` (`kui_open` with a label, the
/// widgets' `label` argument) in the last finished frame — or, from inside
/// a view callback, in the frame so far and then the last one. 0 when no
/// node declared it. The door for a host that never saw an event from the
/// node: keys hash the path from the root, through the auto-keyed
/// ancestors a host cannot spell, so `kui_focus(ctx, kui_key_of(ctx,
/// KUI_STR("note")))` is how "focus the editor I just declared" is said.
/// Two nodes on one label under different parents resolve to the first in
/// tree order and raise `ambiguous-key` (`kui_take_warnings`).
#[unsafe(no_mangle)]
pub extern "C" fn kui_key_of(ptr: *mut KuiCtx, label: KuiStr) -> u64 {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| c.core().key_of(&kstr(label)).map_or(0, |k| k.0))
    })
}

/// What Tab (`forward`) / Shift-Tab does: focus the next / previous
/// focusable node in tree order, wrapping. A key sink that binds Tab
/// itself calls this to hand the keyboard on.
#[unsafe(no_mangle)]
pub extern "C" fn kui_focus_next(ptr: *mut KuiCtx, forward: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().focus_next(forward);
        }
    });
}

/// The node holding keyboard focus; 0 for none.
#[unsafe(no_mangle)]
pub extern "C" fn kui_focused(ptr: *mut KuiCtx) -> u64 {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| c.core().focus().map_or(0, |k| k.0))
    })
}

/// Whether focus got where it is by keyboard or assistive technology
/// rather than a click — when it shows (the core's ring, or `focus_bg`).
#[unsafe(no_mangle)]
pub extern "C" fn kui_focus_visible(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().focus_visible())
    })
}

/// The caret's blink phase — `true` draws it (backlog C35). A custom
/// editor reads it in its view and skips its caret node on the off phase,
/// keeping the `caret` row on its `KUI_ROLE_LINE` either way. Under
/// `kui_run` the runner's clock sets it; a host driving its own window
/// sets it with `kui_set_caret_visible` on a clock of its own, armed while
/// `kui_has_caret` and re-armed solid when `kui_caret_stamp` changes.
#[unsafe(no_mangle)]
pub extern "C" fn kui_caret_visible(ptr: *mut KuiCtx) -> bool {
    guard(true, || {
        unsafe { ctx(ptr) }.is_none_or(|c| c.core().caret_visible())
    })
}

/// Sets the blink phase; see `kui_caret_visible`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_caret_visible(ptr: *mut KuiCtx, visible: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_caret_visible(visible);
        }
    });
}

/// Whether there is a caret to blink: a focused editor's, or the `caret`
/// a line under the focused sink declares. What a host's blink clock is
/// armed on.
#[unsafe(no_mangle)]
pub extern "C" fn kui_has_caret(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().has_caret())
    })
}

/// Changes whenever the caret moved or focus changed — compare across
/// frames to re-arm the blink with the caret solid.
#[unsafe(no_mangle)]
pub extern "C" fn kui_caret_stamp(ptr: *mut KuiCtx) -> u64 {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| c.core().caret_stamp())
    })
}

/// Enters the focus region `key` names (0: the main ring) at the end of
/// the frame being built; see `Core::focus_region` and
/// `docs/adr/0022-focus-regions.md`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_focus_region(ptr: *mut KuiCtx, key: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().focus_region((key != 0).then_some(Key(key)));
        }
    });
}

/// The focus region in effect; 0 for the main ring.
#[unsafe(no_mangle)]
pub extern "C" fn kui_region(ptr: *mut KuiCtx) -> u64 {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| c.core().region().map_or(0, |k| k.0))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_is_focused(ptr: *mut KuiCtx, key: u64) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().is_focused(Key(key)))
    })
}
