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

#[unsafe(no_mangle)]
pub extern "C" fn kui_is_focused(ptr: *mut KuiCtx, key: u64) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().is_focused(Key(key)))
    })
}
