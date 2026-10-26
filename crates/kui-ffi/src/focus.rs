//! Keyboard focus: declaring it, moving it, and reading where it is.

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

/// Moves keyboard focus to `key` now (0 blurs). For focus that follows a
/// declaration rather than a moment, [`kui_set_key_focus`].
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

/// The reverse of [`kui_key_of`]: the label `key` was opened under, in the
/// frame being built so far and then the last finished one, borrowed until
/// the next `kui_label_of` on this context or [`kui_ctx_free`]. False for
/// an auto-keyed node or a key no frame declared. What a host holding a
/// key from an event, `kui_focused` or a log turns back into the name the
/// view gave it; the label is the local one, so two rows of a list under
/// different parents answer the same.
#[unsafe(no_mangle)]
pub extern "C" fn kui_label_of(ptr: *mut KuiCtx, key: u64, out: *mut KuiStr) -> bool {
    guard(false, || {
        let (Some(c), Some(out)) = (unsafe { ctx(ptr) }, unsafe { out.as_mut() }) else {
            return false;
        };
        let Some(label) = c.core().label_of(Key(key)) else {
            return false;
        };
        c.label_of = label.to_owned();
        *out = KuiStr {
            ptr: c.label_of.as_ptr(),
            len: c.label_of.len(),
        };
        true
    })
}

/// The key of the first node in the last finished frame whose accessible
/// name is `name` (backlog F137) — its `label` row, else its own text, else
/// a control's derived name — so a test presses "the button named Like" as
/// a reader would. Not `kui_key_of`'s key label. 0 when no node has the
/// name; two raise `ambiguous-name` (`kui_take_warnings`).
#[unsafe(no_mangle)]
pub extern "C" fn kui_key_named(ptr: *mut KuiCtx, name: KuiStr) -> u64 {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| c.core().key_named(&kstr(name)).map_or(0, |k| k.0))
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

/// Whether the window's "Tab moves focus" toggle is on (backlog C99): a
/// multiline editor whose Tab would type walks the focus ring instead.
/// Ctrl+M flips it in such an editor (Ctrl+Shift+M on macOS); what a
/// status label reads.
#[unsafe(no_mangle)]
pub extern "C" fn kui_tab_moves_focus(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().tab_moves_focus())
    })
}

/// Sets the appearance the caller's `dark` / `light` conditions read in
/// place of the palette's (backlog C113): a `KUI_APPEARANCE_*`, `UNKNOWN`
/// (0) giving the palette's back. Per origin: between frames the host's,
/// inside a C extension's fill that extension's. Kept until set again.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_rules_appearance(ptr: *mut KuiCtx, appearance: u32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let a = kui_core::Appearance::ALL
                .get(appearance as usize)
                .copied()
                .unwrap_or_default();
            c.core().set_rules_appearance(a);
        }
    })
}

/// The caret's blink phase: `true` draws it. A custom
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

/// Enters the focus region `key` names (a node declared with
/// `focus_region`; 0 is the main ring) at the end of the frame being
/// built: focus lands on what that ring last held, else its
/// `initial_focus`, else its first stop.
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

/// Whether the node `key` holds keyboard focus.
#[unsafe(no_mangle)]
pub extern "C" fn kui_is_focused(ptr: *mut KuiCtx, key: u64) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().is_focused(Key(key)))
    })
}
