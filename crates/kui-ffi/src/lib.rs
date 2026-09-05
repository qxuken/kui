//! C API for kui. The IR is plain data, so this layer is translation, not
//! architecture: repr(C) mirrors of the spec structs, opaque handles for
//! `Core` and `Value`, and flat builder calls delegating to `Core`'s
//! non-panicking frame API. See include/kui.h for the C-side contract.
//!
//! Conventions:
//! - Strings cross as (ptr, len), UTF-8; invalid bytes are replaced.
//! - `KuiValue*` created by `kui_value_*` constructors is owned by the caller
//!   until passed to a function documented as consuming it.
//! - Every entry point catches panics and turns them into no-ops/false.

// Safe extern fns taking raw pointers is the point of this layer: every
// entry point null-checks and catches panics instead of being `unsafe`.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

// The other direction: a C shared library as a guest inside a host that
// already owns the frame. See the module docs for where its `kui_*` symbols
// come from, which is the only interesting part.
mod ext;
pub use ext::CExtension;

#[macro_use]
mod abi;
mod convert;
mod focus;
mod frame;
mod input;
mod resources;
mod types;
mod windows;

pub use abi::*;
pub use focus::*;
pub use frame::*;
pub use input::*;
pub use resources::*;
pub use types::*;
pub use windows::*;

use convert::*;

use std::collections::VecDeque;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use kui_core::{
    Align, Color, Core, Edges, EditKey, EditOptions, Enter, FloatConfig, InputEvent, Key, Keyframe,
    Mods, MouseButton, NodeSpec, Rect, Size, Sizing, Span, TextStyle, UiEvent, Value, Vec2,
    WindowButton, WindowCommand, WindowConfig, WindowId, WindowKind,
};

// ---------------------------------------------------------------------------
// Context lifecycle + input

#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_new() -> *mut KuiCtx {
    guard(std::ptr::null_mut(), || {
        let mut owned = Box::new(Core::new());
        // A host driving its own frames opts into diagnostics explicitly,
        // like everything else it drains; kui_run follows the runner's
        // debug-build default.
        owned.set_diagnostics(false);
        let core: *mut Core = &mut *owned;
        Box::into_raw(Box::new(KuiCtx {
            core,
            _owned: Some(owned),
            events: Vec::new(),
            last_payload: None,
            last_edit_text: None,
            last_warnings: Vec::new(),
            last_access: Default::default(),
            open_tooltips: Vec::new(),
            window_commands: VecDeque::new(),
            last_window_name: None,
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_free(ptr: *mut KuiCtx) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(ptr) });
    }
}

// ---------------------------------------------------------------------------
// Host environment

/// Host facts for views to read (`refresh_hz <= 0` = unknown). Survives
/// across frames; set on change or every frame, either works.
#[unsafe(no_mangle)]
pub extern "C" fn kui_env_set(ptr: *mut KuiCtx, refresh_hz: f32, focused: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().env.refresh_hz = (refresh_hz > 0.0).then_some(refresh_hz);
            c.core().env.focused = focused;
        }
    });
}

/// The frame clock for transitions (monotonic seconds, any origin). Set it
/// before each kui_frame_begin; a host that never does sees transitions
/// snap to their targets.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_time(ptr: *mut KuiCtx, now_secs: f64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_time(now_secs);
        }
    });
}

/// True when the last frame left a transition mid-flight: draw another
/// frame without waiting for input.
#[unsafe(no_mangle)]
pub extern "C" fn kui_animating(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().animating())
    })
}

/// Rasterize outline glyphs as LCD subpixel coverage (`KUI_QUAD_GLYPH_SUBPIXEL`,
/// atlas rgb = per-channel coverage) instead of alpha masks. Only for
/// renderers that blend per channel; flipping it re-rasterizes every glyph.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_subpixel_text(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_subpixel_text(on);
        }
    });
}

/// Scrolls whatever contains `key` so the node shows — what Tab does to
/// the control it lands on, asked for by name. The request resolves at the
/// next `kui_frame_finish`, against the frame it lays out (the one being
/// built when called from a view callback, the one after it otherwise — a
/// frame is requested, so one comes), so a row a view is about to declare
/// for the first time reveals fine. A key that frame does not declare, or
/// one with nothing scrollable above it, is a no-op and is not kept for a
/// later frame; the last reveal before a frame wins.
#[unsafe(no_mangle)]
pub extern "C" fn kui_reveal(ptr: *mut KuiCtx, key: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().reveal(Key(key));
        }
    });
}

/// Sets a scroll container's retained offset, the way the wheel would
/// (positive = content moved up / left). Takes effect on the next frame,
/// whose layout clamps it to that frame's overflow: 0,0 is "jump to the
/// top" and a huge value is "jump to the end" without knowing the content
/// height. An offset written for a key that never scrolls is harmless.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_scroll(ptr: *mut KuiCtx, key: u64, x: f32, y: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_scroll(Key(key), kui_core::Vec2::new(x, y));
        }
    });
}

/// Reads that offset back, as the last layout clamped it — the number to
/// persist and hand to `kui_set_scroll` later. Writes 0,0 for a node that
/// never scrolled; either out pointer may be NULL.
#[unsafe(no_mangle)]
pub extern "C" fn kui_scroll_offset(ptr: *mut KuiCtx, key: u64, x: *mut f32, y: *mut f32) {
    guard((), || {
        let off =
            unsafe { ctx(ptr) }.map_or(kui_core::Vec2::ZERO, |c| c.core().scroll_offset(Key(key)));
        unsafe {
            if let Some(x) = x.as_mut() {
                *x = off.x;
            }
            if let Some(y) = y.as_mut() {
                *y = off.y;
            }
        }
    });
}

/// Everything the last layout resolved for the container `key`. Returns
/// false — leaving `out` untouched — for a bad context, a NULL `out`, or a
/// key no layout has ever resolved as a scroll container.
///
/// This is what makes a long list affordable: the core builds every child a
/// view declares, so ten thousand rows cost ten thousand rows, but a view
/// that knows `h` and `offset_y` can declare the rows that fit plus two
/// spacers holding the space of the rest, and pay for a screenful. Read
/// during a build it describes the previous frame, so a resize slices one
/// frame late — build a row or two extra at each end.
#[unsafe(no_mangle)]
pub extern "C" fn kui_scroll_geometry(
    ptr: *mut KuiCtx,
    key: u64,
    out: *mut KuiScrollGeometry,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(g) = c.core().scroll_geometry(Key(key)) else {
            return false;
        };
        write_out(
            out,
            KuiScrollGeometry {
                x: g.rect.x,
                y: g.rect.y,
                w: g.rect.w,
                h: g.rect.h,
                content_w: g.content.w,
                content_h: g.content.h,
                offset_x: g.offset.x,
                offset_y: g.offset.y,
                max_offset_x: g.max_offset.x,
                max_offset_y: g.max_offset.y,
                ..Default::default()
            },
        )
    })
}

/// Drains the warnings the core raised since the last call (silent
/// misconfigurations it noticed while finishing frames; each once) into
/// `out`, up to `cap`; returns the count. The strings stay valid until the
/// next call on this context. Standalone contexts start with the checks
/// off (`kui_set_diagnostics` turns them on); kui_run prints them to
/// stderr itself in debug builds.
#[unsafe(no_mangle)]
pub extern "C" fn kui_take_warnings(ptr: *mut KuiCtx, out: *mut KuiWarning, cap: usize) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if out.is_null() || cap == 0 {
            return 0;
        }
        c.last_warnings = c.core().take_warnings();
        let n = c.last_warnings.len().min(cap);
        for (i, w) in c.last_warnings.iter().take(n).enumerate() {
            let s = |s: &str| KuiStr {
                ptr: s.as_ptr(),
                len: s.len(),
            };
            unsafe {
                out.add(i).write(KuiWarning {
                    code: s(w.code),
                    key: w.key.0,
                    message: s(&w.message),
                })
            };
        }
        n
    })
}

/// The access tree of the last finished frame (what assistive technology
/// sees; see docs/adr/0001-accessibility-as-data.md): fills `out` with up
/// to `cap` nodes in tree order (the root first) and returns the total
/// count, so a short buffer can be resized and the call repeated. Strings
/// stay valid until the next call on this context. A host that never
/// asks pays nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_access_tree(ptr: *mut KuiCtx, out: *mut KuiAccessNode, cap: usize) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        c.last_access = c.core().access_tree().clone();
        let total = c.last_access.nodes.len();
        if out.is_null() || cap == 0 {
            return total;
        }
        let s = |s: &Option<String>| match s {
            Some(s) => KuiStr {
                ptr: s.as_ptr(),
                len: s.len(),
            },
            None => KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
        };
        for (i, n) in c.last_access.nodes.iter().take(cap).enumerate() {
            let mut flags = 0;
            if n.value.is_some() {
                flags |= KUI_ACCESS_HAS_VALUE;
            }
            if n.selection.is_some() {
                flags |= KUI_ACCESS_HAS_SELECTION;
            }
            if n.focused {
                flags |= KUI_ACCESS_FOCUSED;
            }
            if let Some(checked) = n.checked {
                flags |= KUI_ACCESS_CHECKED_SET;
                if checked {
                    flags |= KUI_ACCESS_CHECKED;
                }
            }
            if n.number.is_some() {
                flags |= KUI_ACCESS_HAS_NUMBER;
            }
            if n.min.is_some() {
                flags |= KUI_ACCESS_HAS_MIN;
            }
            if n.max.is_some() {
                flags |= KUI_ACCESS_HAS_MAX;
            }
            if n.scroll.is_some() {
                flags |= KUI_ACCESS_HAS_SCROLL;
            }
            if n.anchor.is_some() && n.focus.is_some() {
                flags |= KUI_ACCESS_HAS_TEXT_SELECTION;
            }
            if n.disabled {
                flags |= KUI_ACCESS_DISABLED;
            }
            if let Some(selected) = n.selected {
                flags |= KUI_ACCESS_SELECTED_SET;
                if selected {
                    flags |= KUI_ACCESS_SELECTED;
                }
            }
            if let Some(expanded) = n.expanded {
                flags |= KUI_ACCESS_EXPANDED_SET;
                if expanded {
                    flags |= KUI_ACCESS_EXPANDED;
                }
            }
            if n.pos_in_set.is_some() {
                flags |= KUI_ACCESS_HAS_POS_IN_SET;
            }
            if n.set_size.is_some() {
                flags |= KUI_ACCESS_HAS_SET_SIZE;
            }
            if n.modal {
                flags |= KUI_ACCESS_MODAL;
            }
            let scroll = n.scroll.unwrap_or_default();
            let (sel_start, sel_end) = n.selection.unwrap_or((0, 0));
            unsafe {
                out.add(i).write(KuiAccessNode {
                    key: n.key.0,
                    parent: n.parent.map_or(0, |k| k.0),
                    origin: n.origin.0 as u32,
                    role: role_code(n.role),
                    flags,
                    actions: n.actions,
                    name: s(&n.name),
                    description: s(&n.description),
                    value: s(&n.value),
                    x: n.rect.x,
                    y: n.rect.y,
                    w: n.rect.w,
                    h: n.rect.h,
                    caret: n.caret.unwrap_or(0) as u32,
                    selection_start: sel_start as u32,
                    selection_end: sel_end as u32,
                    value_now: n.number.unwrap_or(0.0),
                    value_min: n.min.unwrap_or(0.0),
                    value_max: n.max.unwrap_or(0.0),
                    scroll_x: scroll.x,
                    scroll_y: scroll.y,
                    scroll_max_x: scroll.max_x,
                    scroll_max_y: scroll.max_y,
                    anchor_run: n.anchor.map_or(0, |p| p.run.0),
                    anchor_char: n.anchor.map_or(0, |p| p.character as u32),
                    focus_run: n.focus.map_or(0, |p| p.run.0),
                    focus_char: n.focus.map_or(0, |p| p.character as u32),
                    run_count: n.runs.len() as u32,
                    pos_in_set: n.pos_in_set.unwrap_or(0) as u32,
                    set_size: n.set_size.unwrap_or(0) as u32,
                    orientation: orientation_code(n.orientation),
                })
            };
        }
        total
    })
}

/// A request from assistive technology on node `key`: one KUI_ACCESS_*
/// action bit (the node must advertise it in `KuiAccessNode.actions`),
/// with `value` the new text for KUI_ACCESS_SET_VALUE (empty otherwise).
/// Resolved the way the pointer or keyboard equivalent would be: a click
/// emits the node's payload, focus lands on an editor, a slider nudge
/// arrives as a `{kind="access", action, tag}` event.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_access(ptr: *mut KuiCtx, key: u64, action: u32, value: KuiStr) {
    guard((), || {
        let Some(action) = kui_core::AccessAction::ALL
            .iter()
            .copied()
            .find(|a| a.bit() == action)
        else {
            return;
        };
        let value = (!value.ptr.is_null() && value.len > 0).then(|| kstr(value).into_owned());
        push_input(
            ptr,
            InputEvent::Access(kui_core::AccessRequest {
                key: Key(key),
                action,
                value,
                anchor: None,
                focus: None,
            }),
        );
    });
}

/// The laid-out text of editor node `key` as runs (see `KuiAccessRun`):
/// fills `out` with up to `cap` of them and returns the total. Runs are
/// what `KuiAccessNode.anchor_run` / `focus_run` and
/// `kui_input_access_text` refer to. Borrowed until the next
/// `kui_access_tree` / `kui_access_runs` on the context; a node with no
/// text (or no such node) has none.
#[unsafe(no_mangle)]
pub extern "C" fn kui_access_runs(
    ptr: *mut KuiCtx,
    key: u64,
    out: *mut KuiAccessRun,
    cap: usize,
) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        c.last_access = c.core().access_tree().clone();
        let Some(node) = c.last_access.get(Key(key)) else {
            return 0;
        };
        let total = node.runs.len();
        if out.is_null() || cap == 0 {
            return total;
        }
        for (i, r) in node.runs.iter().take(cap).enumerate() {
            unsafe {
                out.add(i).write(KuiAccessRun {
                    key: r.key.0,
                    line: r.line as u32,
                    start: r.start as u32,
                    end: r.end as u32,
                    text: KuiStr {
                        ptr: r.text.as_ptr(),
                        len: r.text.len(),
                    },
                    x: r.rect.x,
                    y: r.rect.y,
                    w: r.rect.w,
                    h: r.rect.h,
                    char_count: r.char_lengths.len() as u32,
                    char_lengths: r.char_lengths.as_ptr(),
                    char_positions: r.char_positions.as_ptr(),
                    char_widths: r.char_widths.as_ptr(),
                    word_start_count: r.word_starts.len() as u32,
                    word_starts: r.word_starts.as_ptr(),
                    rtl: r.rtl as u32,
                })
            };
        }
        total
    })
}

/// A text request from assistive technology on editor node `key`:
/// KUI_ACCESS_SET_TEXT_SELECTION with the selection as run positions
/// (`anchor_*` the end that stays, `focus_*` the caret), or
/// KUI_ACCESS_REPLACE_SELECTED_TEXT / KUI_ACCESS_SET_VALUE with `value`.
/// A built-in editor applies it (a `changed` event follows an edit); a
/// custom editor gets it as a `{kind="access", action, anchor, focus,
/// text, tag}` event to apply itself.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_access_text(
    ptr: *mut KuiCtx,
    key: u64,
    action: u32,
    anchor_run: u64,
    anchor_char: u32,
    focus_run: u64,
    focus_char: u32,
    value: KuiStr,
) {
    guard((), || {
        let Some(action) = kui_core::AccessAction::ALL
            .iter()
            .copied()
            .find(|a| a.bit() == action)
        else {
            return;
        };
        let mut req = kui_core::AccessRequest::new(Key(key), action);
        if !value.ptr.is_null() && value.len > 0 {
            req.value = Some(kstr(value).into_owned());
        }
        if action == kui_core::AccessAction::SetTextSelection {
            req.anchor = Some(kui_core::TextPos {
                run: Key(anchor_run),
                character: anchor_char as usize,
            });
            req.focus = Some(kui_core::TextPos {
                run: Key(focus_run),
                character: focus_char as usize,
            });
        }
        push_input(ptr, InputEvent::Access(req));
    });
}

/// Turns the diagnostic checks behind kui_take_warnings on or off (off by
/// default for a standalone context: a development build opts in).
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_diagnostics(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_diagnostics(on);
        }
    });
}

// ---------------------------------------------------------------------------
// Widgets: the same `kui_core::widgets` the Rust, Lua and Node frontends
// use. Container widgets take a body callback that re-enters through the
// same context pointer — as with kui_run's view callback, the wrapping `Ui`
// is not touched while C runs.

fn with_ui(ptr: *mut KuiCtx, f: impl FnOnce(&mut kui_core::Ui<'_>)) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            f(&mut kui_core::Ui::wrap(c.core()));
        }
    });
}

/// Adaptive titlebar: drag strip + standard title + window buttons, all
/// driven by the env facts (`kui_env_set_window`).
#[unsafe(no_mangle)]
pub extern "C" fn kui_titlebar(ptr: *mut KuiCtx, title: KuiStr) {
    with_ui(ptr, |ui| kui_core::widgets::titlebar(ui, &kstr(title)));
}

/// Titlebar hosting custom content (tabs, search): `body` builds it between
/// the OS-controls inset and the window buttons.
#[unsafe(no_mangle)]
pub extern "C" fn kui_titlebar_with(ptr: *mut KuiCtx, body: ViewFn, user: *mut c_void) {
    with_ui(ptr, |ui| {
        kui_core::widgets::titlebar_with(ui, |_| body(user, ptr))
    });
}

/// The minimize/maximize/close cluster; draws nothing when the OS provides
/// controls, so it is always safe to call.
#[unsafe(no_mangle)]
pub extern "C" fn kui_window_buttons(ptr: *mut KuiCtx) {
    with_ui(ptr, kui_core::widgets::window_buttons);
}

/// A hint floated below the enclosing node. Gate it on `kui_is_hovered` of
/// a hoverable parent.
#[unsafe(no_mangle)]
pub extern "C" fn kui_tooltip(ptr: *mut KuiCtx, text: KuiStr) {
    with_ui(ptr, |ui| kui_core::widgets::tooltip(ui, &kstr(text)));
}

/// Tooltip chrome around content built by `body`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_tooltip_with(ptr: *mut KuiCtx, body: ViewFn, user: *mut c_void) {
    with_ui(ptr, |ui| {
        kui_core::widgets::tooltip_with(ui, |_| body(user, ptr))
    });
}

/// Per-phase frame-latency bars vs the display budget. Reads the runner's
/// frame stats — renders empty chrome in a standalone (headless) context.
#[unsafe(no_mangle)]
pub extern "C" fn kui_latency_graph(ptr: *mut KuiCtx) {
    with_ui(ptr, kui_core::widgets::latency_graph);
}

/// The graph in a translucent panel floating in a viewport corner picked by
/// KUI_START/CENTER/END attach values.
#[unsafe(no_mangle)]
pub extern "C" fn kui_latency_hud(ptr: *mut KuiCtx, x: u32, y: u32) {
    with_ui(ptr, |ui| {
        kui_core::widgets::latency_hud_at(ui, align_of(x), align_of(y))
    });
}

/// Single-line input with chrome (background, focus ring). Returns the node
/// key; read it with `kui_edit_text`, "changed"/"submit" events carry it.
#[unsafe(no_mangle)]
pub extern "C" fn kui_text_input(ptr: *mut KuiCtx, label: KuiStr, initial: KuiStr) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        kui_core::widgets::text_input(
            &mut kui_core::Ui::wrap(c.core()),
            &kstr(label),
            &kstr(initial),
        )
        .0
    })
}

/// Convenience button matching `kui_core::widgets::button`. Consumes payload.
#[unsafe(no_mangle)]
pub extern "C" fn kui_button(ptr: *mut KuiCtx, label: KuiStr, payload: *mut KuiValue) {
    guard((), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            if !payload.is_null() {
                drop(unsafe { Box::from_raw(payload) });
            }
            return;
        };
        let label = kstr(label);
        let value = if payload.is_null() {
            Value::Null
        } else {
            unsafe { Box::from_raw(payload) }.0
        };
        // The same data as kui_core::widgets::button: hover/pressed colors
        // are declared on the spec and resolved by the core.
        c.core()
            .open_keyed(&label, kui_core::widgets::button_spec().on_click(value));
        c.core().text_node(
            &label,
            TextStyle::new(kui_core::widgets::BUTTON_TEXT).color(Color::WHITE),
        );
        c.core().close();
    });
}

/// Editable text node; flags: 1 = multiline, 2 = autofocus. Returns its key.
#[unsafe(no_mangle)]
pub extern "C" fn kui_text_edit(
    ptr: *mut KuiCtx,
    label: KuiStr,
    initial: KuiStr,
    style: *const KuiTextStyle,
    flags: u32,
    spec: *const KuiSpec,
) -> u64 {
    guard(0, || {
        let (Some(c), Some(sp)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) else {
            return 0;
        };
        let opts = EditOptions {
            style: unsafe { style.as_ref() }
                .map(text_style_of)
                .unwrap_or_default(),
            multiline: flags & 1 != 0,
            autofocus: flags & 2 != 0,
            ..Default::default()
        };
        let spec = spec_of(sp, NONE, NONE, NONE, NONE);
        c.core()
            .text_edit(&kstr(label), &kstr(initial), &opts, spec)
            .0
    })
}

// ---------------------------------------------------------------------------
// Values

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_null() -> *mut KuiValue {
    Box::into_raw(Box::new(KuiValue(Value::Null)))
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_bool(v: bool) -> *mut KuiValue {
    Box::into_raw(Box::new(KuiValue(Value::Bool(v))))
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_int(v: i64) -> *mut KuiValue {
    Box::into_raw(Box::new(KuiValue(Value::Int(v))))
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_float(v: f64) -> *mut KuiValue {
    Box::into_raw(Box::new(KuiValue(Value::Float(v))))
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_str(s: KuiStr) -> *mut KuiValue {
    guard(std::ptr::null_mut(), || {
        Box::into_raw(Box::new(KuiValue(Value::Str(kstr(s).into_owned()))))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_map() -> *mut KuiValue {
    Box::into_raw(Box::new(KuiValue(Value::Map(Vec::new()))))
}

/// Sets `key` on a map value. Consumes `val`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_map_set(map: *mut KuiValue, key: KuiStr, val: *mut KuiValue) {
    guard((), || {
        if val.is_null() {
            return;
        }
        let val = unsafe { Box::from_raw(val) };
        let Some(map) = (unsafe { map.as_mut() }) else {
            return;
        };
        if let Value::Map(entries) = &mut map.0 {
            let key = kstr(key).into_owned();
            if let Some(e) = entries.iter_mut().find(|(k, _)| *k == key) {
                e.1 = val.0;
            } else {
                entries.push((key, val.0));
            }
        }
    });
}

/// Borrowed lookup on a map value; NULL if absent. Valid as long as the map.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_get(v: *const KuiValue, key: KuiStr) -> *const KuiValue {
    guard(std::ptr::null(), || {
        let Some(v) = (unsafe { v.as_ref() }) else {
            return std::ptr::null();
        };
        match v.0.get(&kstr(key)) {
            // Value and KuiValue are layout-identical (single field).
            Some(inner) => (inner as *const Value).cast(),
            None => std::ptr::null(),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_as_int(v: *const KuiValue, out: *mut i64) -> bool {
    guard(false, || {
        let (Some(v), Some(out)) = (unsafe { v.as_ref() }, unsafe { out.as_mut() }) else {
            return false;
        };
        match v.0.as_int() {
            Some(i) => {
                *out = i;
                true
            }
            None => false,
        }
    })
}

/// Borrowed string view; valid as long as the value.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_as_str(v: *const KuiValue, out: *mut KuiStr) -> bool {
    guard(false, || {
        let (Some(v), Some(out)) = (unsafe { v.as_ref() }, unsafe { out.as_mut() }) else {
            return false;
        };
        match v.0.as_str() {
            Some(s) => {
                *out = KuiStr {
                    ptr: s.as_ptr(),
                    len: s.len(),
                };
                true
            }
            None => false,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_free(v: *mut KuiValue) {
    if !v.is_null() {
        drop(unsafe { Box::from_raw(v) });
    }
}

// ---------------------------------------------------------------------------
// Windowed runner (winit + wgpu) via C callbacks

type ViewFn = extern "C" fn(user: *mut c_void, ctx: *mut KuiCtx);
type EventFn = extern "C" fn(user: *mut c_void, ev: *const KuiEvent);

struct CApp {
    user: *mut c_void,
    view: ViewFn,
    on_event: Option<EventFn>,
}

impl kui::App for CApp {
    fn view(&mut self, ui: &mut kui::Ui<'_>) {
        // Hand the callback a context that borrows the runner's Core for the
        // duration of view(); builder entry points only touch `core`.
        let mut shim = KuiCtx::borrowing(ui.core());
        (self.view)(self.user, &mut shim);
    }

    fn on_event(&mut self, ev: kui::UiEvent) {
        let Some(cb) = self.on_event else { return };
        let payload = KuiValue(ev.payload);
        // Library-allocated, so the full struct: `size` says how much of it
        // is meaningful, which is all of it.
        let out = KuiEvent {
            origin: ev.origin.0,
            key: ev.key.0,
            payload: &payload,
            window: ev.window.0,
            ..Default::default()
        };
        cb(self.user, &out);
    }
}

/// Runs a windowed app driven by C callbacks. Blocks until the window closes.
/// Returns false if the event loop could not start.
#[unsafe(no_mangle)]
pub extern "C" fn kui_run(
    title: KuiStr,
    view: ViewFn,
    on_event: EventFn,
    user: *mut c_void,
) -> bool {
    guard(false, || {
        let title = kstr(title).into_owned();
        let app = CApp {
            user,
            view,
            on_event: Some(on_event),
        };
        kui::run(&title, app, vec![]).is_ok()
    })
}

// ---------------------------------------------------------------------------
// Parity with the shared prop schema. `KuiSpec` has to be a static repr(C)
// layout, so it cannot read `kui_core::schema::PROPS` at runtime the way Lua
// and Node do — instead these tests pin it to the table: a schema row with
// no C counterpart fails `every_schema_prop_has_a_c_counterpart`.

#[cfg(test)]
mod schema_parity {
    use super::*;
    use kui_core::schema::{Kind, PROPS, Parsed, PropsOut, Target, apply};

    fn zeroed_spec() -> KuiSpec {
        // Zero-initialized is the documented C default.
        unsafe { std::mem::zeroed() }
    }

    fn zeroed_style() -> KuiTextStyle {
        unsafe { std::mem::zeroed() }
    }

    fn msg(v: Value) -> *mut KuiValue {
        Box::into_raw(Box::new(KuiValue(v)))
    }

    /// C reaches the same four presets by name as JSX and Lua, and the
    /// fields it fills round-trip back through `spec_of` to the config the
    /// core would have built. A preset is a starting point, not a mode: the
    /// fields stay writable afterwards.
    #[test]
    fn a_named_float_preset_round_trips_through_the_struct() {
        for name in kui_core::FLOAT_PRESETS {
            let mut spec = zeroed_spec();
            assert!(kui_spec_float_preset(
                &mut spec,
                KuiStr {
                    ptr: name.as_ptr(),
                    len: name.len(),
                }
            ));
            assert_eq!(
                spec_of(&spec, NONE, NONE, NONE, NONE).layout.float,
                FloatConfig::preset(name),
                "{name}: the C fields do not rebuild the preset"
            );
        }

        // An unknown name changes nothing, so a typo leaves a node in flow
        // rather than floating it somewhere arbitrary.
        let mut spec = zeroed_spec();
        assert!(!kui_spec_float_preset(
            &mut spec,
            KuiStr {
                ptr: "beneath".as_ptr(),
                len: 7,
            }
        ));
        assert_eq!(spec.float_mode, KUI_FLOAT_NONE);
        assert_eq!(spec_of(&spec, NONE, NONE, NONE, NONE).layout.float, None);
    }

    #[test]
    fn zeroed_structs_are_the_schema_defaults() {
        let out = PropsOut::new();
        assert_eq!(spec_of(&zeroed_spec(), NONE, NONE, NONE, NONE), out.spec);
        assert_eq!(text_style_of(&zeroed_style()), out.style);
    }

    /// Every role reaches a spec from C, including the ones only a custom
    /// editor declares. The row above pins the header's *names* to these
    /// codes; this pins the codes to the roles, so the two ends of
    /// `KuiSpec.role` cannot agree on a number that means something else
    /// by the time it is a `Spec`.
    #[test]
    fn every_role_round_trips_through_the_c_code() {
        for role in kui_core::Role::ALL {
            let mut spec = zeroed_spec();
            spec.role = role_code(role);
            assert_eq!(
                spec_of(&spec, NONE, NONE, NONE, NONE).role,
                Some(role),
                "{}: KUI_ROLE_{} does not arrive as itself",
                role.name(),
                role_code(role),
            );
        }
        // Zero is "unset" and stays the way a plain box is declared, and a
        // code past the end is ignored rather than folded onto a real role.
        assert_eq!(role_of_code(0), None);
        assert_eq!(role_of_code(kui_core::Role::ALL.len() as u32 + 1), None);
    }

    /// Every `PROPS` row, applied with a sample value through the schema,
    /// must be reproducible by setting a `KuiSpec`/`KuiTextStyle` field (or
    /// passing a message pointer). The `match` is the C-side mapping; a new
    /// row without an arm panics with instructions.
    #[test]
    fn every_schema_prop_has_a_c_counterpart() {
        const F: f32 = 37.0;
        const C: u32 = 0x11223344;
        for def in PROPS {
            // `opacity` is a 0..=1 slot whose default is the top of the
            // range, so the shared sample would clamp back to it.
            let f = if def.name == "opacity" { 0.5 } else { F };
            let sample = match def.kind {
                Kind::F32 => Parsed::F32(f),
                Kind::Color => Parsed::Color(Color::hex(C)),
                Kind::Flag => Parsed::Flag,
                Kind::Enum(_) => Parsed::Enum(1),
                Kind::Sizing => Parsed::Sizing(Sizing::Percent(0.5)),
                Kind::Msg | Kind::Tag => Parsed::Msg(Value::Int(7)),
                Kind::Str => Parsed::Str("name".into()),
                Kind::Resource => Parsed::Resource(7),
                Kind::Keyframes => Parsed::Keyframes(vec![Keyframe::default().at(0.5).radius(F)]),
                Kind::Enter => Parsed::Enter(Enter::from(-F, 0.0).radius(F)),
            };
            let mut expected = PropsOut::new();
            apply(def, sample, &mut expected).unwrap();
            let layout_tag = KuiValue(Value::Int(7));

            let stops = [KuiKeyframe {
                set: KUI_KF_AT | KUI_KF_RADIUS,
                at: 0.5,
                width: KuiSizing { tag: 0, value: 0.0 },
                height: KuiSizing { tag: 0, value: 0.0 },
                bg: 0,
                radius: F,
                opacity: 0.0,
            }];
            let mut s = zeroed_spec();
            let mut t = zeroed_style();
            let (mut click, mut drag, mut key, mut hover) = (NONE, NONE, NONE, NONE);
            let pct = KuiSizing { tag: 3, value: 0.5 };
            let name = KuiStr {
                ptr: "name".as_ptr(),
                len: 4,
            };
            match def.name {
                "width" => s.width = pct,
                "height" => s.height = pct,
                "minWidth" => s.min_w = F,
                "maxWidth" => s.max_w = F,
                "minHeight" => s.min_h = F,
                "maxHeight" => s.max_h = F,
                "gap" => s.gap = F,
                "crossGap" => s.cross_gap = F,
                "wrapChildren" => s.wrap_children = 1,
                "radius" => s.radius = F,
                "radiusTL" => (s.per_corner, s.radius_tl) = (1, F),
                "radiusTR" => (s.per_corner, s.radius_tr) = (1, F),
                "radiusBR" => (s.per_corner, s.radius_br) = (1, F),
                "radiusBL" => (s.per_corner, s.radius_bl) = (1, F),
                // A 0..=1 slot whose default is the top of the range, so
                // the shared sample would clamp back to it.
                "opacity" => (s.opacity_set, s.opacity) = (1, 0.5),
                "shadowColor" => s.shadow_color = C,
                "shadowBlur" => s.shadow_blur = F,
                "shadowX" => s.shadow_x = F,
                "shadowY" => s.shadow_y = F,
                "shadowSpread" => s.shadow_spread = F,
                "mainAlign" => s.main_align = 1,
                "crossAlign" => s.cross_align = 1,
                "center" => (s.main_align, s.cross_align) = (1, 1),
                "bg" => s.bg = C,
                "hoverable" => s.hoverable = 1,
                "window" => s.window_role = 2, // KUI_WINDOW_* = schema index + 1
                "transition" => s.transition_ms = F,
                "easing" => s.easing = 1,
                "slide" => s.slide = 1,
                "keyframes" => (s.keyframes, s.keyframes_len) = (stops.as_ptr(), 1),
                "enter" => {
                    s.enter = KuiEnter {
                        set: KUI_ENTER_OFFSET | KUI_ENTER_RADIUS,
                        dx: -F,
                        dy: 0.0,
                        width: KuiSizing { tag: 0, value: 0.0 },
                        height: KuiSizing { tag: 0, value: 0.0 },
                        bg: 0,
                        radius: F,
                        opacity: 0.0,
                    }
                }
                "exit" => {
                    s.exit = KuiEnter {
                        set: KUI_ENTER_OFFSET | KUI_ENTER_RADIUS,
                        dx: -F,
                        dy: 0.0,
                        width: KuiSizing { tag: 0, value: 0.0 },
                        height: KuiSizing { tag: 0, value: 0.0 },
                        bg: 0,
                        radius: F,
                        opacity: 0.0,
                    }
                }
                "repeat" => s.repeat = 1,
                "delay" => s.delay_ms = F,
                "onClick" => click = msg(Value::Int(7)),
                "onDrag" => drag = msg(Value::Int(7)),
                "onKey" => key = msg(Value::Int(7)),
                "onHover" => hover = msg(Value::Int(7)),
                "onLayout" => s.on_layout = &layout_tag,
                "modal" => s.modal = &layout_tag,
                "onContextMenu" => s.on_context_menu = &layout_tag,
                "cursor" => s.cursor = 2, // KUI_CURSOR_* = schema index + 1
                "hoverBg" => s.hover_bg = C,
                "pressedBg" => s.pressed_bg = C,
                "hoverGroup" => s.hover_group = name,
                "focusable" => s.focusable = 1,
                "initialFocus" => s.initial_focus = 1,
                "disabled" => s.disabled = 1,
                "focusBg" => s.focus_bg = C,
                "clickSound" => s.click_sound = 7,
                "hoverSound" => s.hover_sound = 7,
                "role" => s.role = 2, // KUI_ROLE_* = Role::ALL index + 1; ROLES[1] = button
                "label" => s.label = name,
                "checked" => s.checked = 1,
                "selected" => s.selected = 1,
                "expanded" => s.expanded = KUI_EXPANDED_EXPANDED,
                "valueNow" => (s.value_set, s.value_now) = (KUI_VALUE_NOW, F),
                "valueMin" => (s.value_set, s.value_min) = (KUI_VALUE_MIN, F),
                "valueMax" => (s.value_set, s.value_max) = (KUI_VALUE_MAX, F),
                "caret" => (s.value_set, s.caret) = (KUI_VALUE_CARET, F as u32),
                "selectionAnchor" => {
                    (s.value_set, s.selection_anchor) = (KUI_VALUE_ANCHOR, F as u32)
                }
                "lineHeight" => t.line_height = F,
                "color" => t.color = C,
                "family" => t.family = 1,
                "font" => t.font = 7,
                "wrap" => t.wrap = 1,
                "maxLines" => t.max_lines = F as u32,
                "ellipsis" => t.ellipsis = 1,
                other => panic!(
                    "schema prop {other:?} has no C counterpart: add a KuiSpec/KuiTextStyle \
                     field (append-only — the struct is ABI), mirror it in include/kui.h, \
                     apply it in spec_of/text_style_of, and map it here"
                ),
            }
            match def.target() {
                Target::Spec => assert_eq!(
                    spec_of(&s, click, drag, key, hover),
                    expected.spec,
                    "{}: C mapping disagrees with the schema",
                    def.name
                ),
                Target::Style => assert_eq!(
                    text_style_of(&t),
                    expected.style,
                    "{}: C mapping disagrees with the schema",
                    def.name
                ),
            }
        }
    }

    /// The hand-written composites (dir, pad, border, overflow, float) and
    /// the whole struct at once against the Rust builder.
    #[test]
    fn fully_populated_spec_matches_the_rust_builder() {
        let stops = [
            KuiKeyframe {
                set: KUI_KF_WIDTH | KUI_KF_BG,
                at: 0.0,
                width: KuiSizing { tag: 1, value: 0.0 },
                height: KuiSizing { tag: 0, value: 0.0 },
                bg: 0x11_22_33_ff,
                radius: 0.0,
                opacity: 0.0,
            },
            KuiKeyframe {
                set: KUI_KF_AT | KUI_KF_WIDTH | KUI_KF_HEIGHT | KUI_KF_RADIUS,
                at: 0.75,
                width: KuiSizing { tag: 1, value: 1.0 },
                height: KuiSizing { tag: 3, value: 0.5 },
                bg: 0,
                radius: 9.0,
                opacity: 0.0,
            },
        ];
        let modal_tag = KuiValue(Value::str("m"));
        let menu_tag = KuiValue(Value::str("cm"));
        let s = KuiSpec {
            width: KuiSizing { tag: 1, value: 2.0 },
            height: KuiSizing {
                tag: 2,
                value: 120.0,
            },
            min_w: 10.0,
            max_w: 500.0,
            min_h: 5.0,
            max_h: 300.0,
            dir: 1,
            pad_l: 1.0,
            pad_r: 2.0,
            pad_t: 3.0,
            pad_b: 4.0,
            gap: 8.0,
            main_align: 1,
            cross_align: 2,
            bg: 0x14161eff,
            border_color: 0x2a2d3aff,
            border_w: 1.0,
            radius: 6.0,
            overflow: 1 | 2 | 4,
            float_mode: 2,
            float_anchor_x: 2,
            float_anchor_y: 2,
            float_self_x: 2,
            float_self_y: 2,
            float_dx: -8.0,
            float_dy: -8.0,
            float_fit: 1,
            hoverable: 1,
            window_role: 1,
            transition_ms: 150.0,
            easing: 3,
            slide: 1,
            hover_bg: 0x47_6c_e0_ff,
            pressed_bg: 0x2f_54_c4_ff,
            hover_group: KuiStr {
                ptr: "grp".as_ptr(),
                len: 3,
            },
            per_corner: 1,
            radius_tl: 1.0,
            radius_tr: 2.0,
            radius_br: 3.0,
            radius_bl: 4.0,
            repeat: 2,
            delay_ms: 50.0,
            keyframes: stops.as_ptr(),
            keyframes_len: stops.len(),
            enter: unsafe { std::mem::zeroed() },
            click_sound: 0,
            hover_sound: 0,
            on_layout: std::ptr::null(),
            role: 2,
            label: KuiStr {
                ptr: "lbl".as_ptr(),
                len: 3,
            },
            checked: 1,
            value_set: KUI_VALUE_NOW | KUI_VALUE_MIN | KUI_VALUE_MAX,
            value_now: 3.0,
            value_min: 0.0,
            value_max: 10.0,
            caret: 0,
            selection_anchor: 0,
            focusable: 1,
            disabled: 1,
            focus_bg: 0x11_22_33_ff,
            tooltip: KuiStr {
                ptr: "hint".as_ptr(),
                len: 4,
            },
            modal: &modal_tag,
            on_context_menu: &menu_tag,
            cursor: 7, // KUI_CURSOR_EW_RESIZE
            selected: 1,
            expanded: KUI_EXPANDED_EXPANDED,
            opacity_set: 1,
            opacity: 0.4,
            shadow_color: 0x00_00_00_66,
            shadow_blur: 12.0,
            shadow_x: 0.0,
            shadow_y: 4.0,
            shadow_spread: -2.0,
            wrap_children: 1,
            cross_gap: 6.0,
            initial_focus: 1,
            exit: unsafe { std::mem::zeroed() },
        };
        let expected = NodeSpec::row()
            .width(Sizing::Grow(2.0))
            .height(Sizing::Fixed(120.0))
            .min_width(10.0)
            .max_width(500.0)
            .min_height(5.0)
            .max_height(300.0)
            .padding(Edges {
                l: 1.0,
                r: 2.0,
                t: 3.0,
                b: 4.0,
            })
            .gap(8.0)
            .wrap()
            .cross_gap(6.0)
            .main_align(Align::Center)
            .cross_align(Align::End)
            .bg(Color::hex(0x14161eff))
            .radii(1.0, 2.0, 3.0, 4.0)
            .border(1.0, Color::hex(0x2a2d3aff))
            .clip()
            .scroll_x()
            .scroll_y()
            .float(
                FloatConfig::viewport()
                    .at(Align::End, Align::End)
                    .self_at(Align::End, Align::End)
                    .offset(-8.0, -8.0)
                    .fit(),
            )
            .hoverable()
            .window_drag()
            .transition(150.0)
            .easing(kui_core::Easing::EaseInOut)
            .slide()
            .hover_bg(Color::hex(0x47_6c_e0_ff))
            .pressed_bg(Color::hex(0x2f_54_c4_ff))
            .hover_group("grp")
            .focusable()
            .initial_focus()
            .disabled(true)
            .focus_bg(Color::hex(0x11_22_33_ff))
            .description("hint")
            .role(kui_core::Role::Button)
            .label("lbl")
            .checked(true)
            .selected(true)
            .expanded(true)
            .value_now(3.0)
            .value_min(0.0)
            .value_max(10.0)
            .opacity(0.4)
            .shadow(kui_core::Shadow {
                color: Color::hex(0x00_00_00_66),
                dx: 0.0,
                dy: 4.0,
                blur: 12.0,
                spread: -2.0,
            })
            .repeat(kui_core::Repeat::Alternate)
            .delay(50.0)
            .keyframes(vec![
                Keyframe::default()
                    .width(Sizing::Grow(0.0))
                    .bg(Color::hex(0x11_22_33_ff)),
                Keyframe::default()
                    .at(0.75)
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Percent(0.5))
                    .radius(9.0),
            ])
            .on_click(Value::str("c"))
            .on_drag(Value::str("d"))
            .on_key(Value::str("k"))
            .on_hover(Value::str("h"))
            .modal(Value::str("m"))
            .on_context_menu(Value::str("cm"))
            .cursor(kui_core::CursorShape::EwResize);
        let got = spec_of(
            &s,
            msg("c".into()),
            msg("d".into()),
            msg("k".into()),
            msg("h".into()),
        );
        assert_eq!(got, expected);
    }
}

#[cfg(test)]
mod widgets_headless {
    use super::*;

    fn ks(s: &str) -> KuiStr {
        KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    extern "C" fn tab(_user: *mut c_void, ctx: *mut KuiCtx) {
        let style: KuiTextStyle = unsafe { std::mem::zeroed() };
        kui_text(ctx, ks("tab"), &style);
    }

    /// Every widget entry point builds through a standalone context, the
    /// body callbacks re-enter through the same pointer, and the editor
    /// created by kui_text_input reads back.
    #[test]
    fn widgets_build_and_draw() {
        let ctx = kui_ctx_new();
        assert!(!ctx.is_null());
        kui_frame_begin(ctx, 800.0, 600.0, 1.0);
        let root: KuiSpec = unsafe { std::mem::zeroed() };
        kui_root(ctx, &root);
        kui_titlebar_with(ctx, tab, std::ptr::null_mut());
        kui_titlebar(ctx, ks("plain"));
        kui_window_buttons(ctx);
        kui_latency_graph(ctx);
        kui_latency_hud(ctx, 2, 2);
        let mut badge: KuiSpec = unsafe { std::mem::zeroed() };
        badge.hoverable = 1;
        kui_open(ctx, &badge, NONE);
        kui_tooltip(ctx, ks("hint"));
        kui_tooltip_with(ctx, tab, std::ptr::null_mut());
        kui_close(ctx);
        let key = kui_text_input(ctx, ks("name"), ks("init"));
        assert_ne!(key, 0);
        kui_frame_finish(ctx);

        let mut text = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_edit_text(ctx, key, &mut text));
        assert_eq!(&*kstr(text), "init");

        let mut draw = KuiDrawData::default();
        kui_draw_data(ctx, &mut draw);
        assert!(draw.quad_count > 20, "got {} quads", draw.quad_count);
        kui_ctx_free(ctx);
    }
}

#[cfg(test)]
mod window_commands_headless {
    use super::*;

    /// `MAIN`, which the header spells for C.
    const MAIN: u32 = WindowId::MAIN.0;

    fn drain(ctx: *mut KuiCtx) -> Vec<(u32, u32, f32, f32)> {
        let mut out = Vec::new();
        let mut cmd = KuiWindowCommand::default();
        while kui_take_window_command(ctx, &raw mut cmd) {
            out.push((cmd.kind, cmd.window, cmd.width, cmd.height));
        }
        out
    }

    /// A size request carries its size through the drain, and a focus
    /// request the window it names; both leave in the order they were
    /// queued, behind whatever the chrome produced, and once.
    #[test]
    fn size_and_focus_requests_drain_with_their_payload() {
        let ctx = kui_ctx_new();
        kui_set_window_size(ctx, MAIN, 640.0, 480.0);
        kui_focus_window(ctx, MAIN);
        assert_eq!(
            drain(ctx),
            vec![
                (KUI_CMD_SET_SIZE, MAIN, 640.0, 480.0),
                (KUI_CMD_FOCUS, MAIN, 0.0, 0.0),
            ]
        );
        assert!(drain(ctx).is_empty(), "drained once");
        kui_ctx_free(ctx);
    }

    /// The ABI-6 append is the compatible kind: a host that reserved only
    /// through `config` (every ABI-5 build) still drains, still reads the
    /// verb and the window, and simply never sees the size — which it
    /// cannot need, since only its own `kui_set_window_size` produces the
    /// verb that fills it.
    #[test]
    fn an_abi_5_host_drains_without_seeing_the_appended_size() {
        let ctx = kui_ctx_new();
        kui_set_window_size(ctx, MAIN, 640.0, 480.0);
        let mut cmd = KuiWindowCommand {
            size: abi_through!(KuiWindowCommand, config, KuiWindowConfig),
            width: 12.5,
            ..Default::default()
        };
        assert!(kui_take_window_command(ctx, &raw mut cmd));
        assert_eq!((cmd.kind, cmd.window), (KUI_CMD_SET_SIZE, MAIN));
        assert_eq!(
            cmd.size,
            KuiWindowCommand::ABI_V1_SIZE,
            "the prefix it asked for"
        );
        assert_eq!(cmd.width, 12.5, "and nothing written past it");
        kui_ctx_free(ctx);
    }
}

#[cfg(test)]
mod audio_headless {
    use super::*;

    /// A click on a `click_sound` node and an audio node both surface as
    /// commands through the C drain; an ended tagged playback polls out as
    /// a `sound` event.
    #[test]
    fn sounds_flow_through_the_c_api() {
        let ctx = kui_ctx_new();
        let wav = b"RIFF....WAVE";
        let sound = kui_sound_add(ctx, wav.as_ptr(), wav.len());
        assert_ne!(sound, 0);

        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing {
            tag: 2,
            value: 40.0,
        };
        spec.height = KuiSizing {
            tag: 2,
            value: 20.0,
        };
        spec.click_sound = sound;
        let audio = KuiAudio {
            src: sound,
            volume: 0.5,
            looped: 1,
            paused: 0,
        };
        let frame = |ctx: *mut KuiCtx| {
            kui_frame_begin(ctx, 200.0, 100.0, 1.0);
            kui_open(ctx, &spec, NONE);
            kui_close(ctx);
            kui_audio(ctx, KUI_EMPTY, &audio, kui_value_str(KUI_STR_TEST));
            kui_frame_finish(ctx);
        };
        frame(ctx);
        let mut out = [KuiAudioCommand::default(); 8];
        let n = kui_take_audio_commands(ctx, out.as_mut_ptr(), out.len());
        assert_eq!(n, 1, "the audio node started once");
        assert_eq!((out[0].kind, out[0].sound, out[0].looped), (1, sound, 1));
        assert_eq!(out[0].volume, 0.5);
        let music = out[0].playback;

        kui_input_cursor(ctx, 5.0, 5.0);
        kui_input_mouse(ctx, true, 1);
        kui_input_mouse(ctx, false, 1);
        let n = kui_take_audio_commands(ctx, out.as_mut_ptr(), out.len());
        assert_eq!(n, 1, "the click played its sound");
        assert_eq!((out[0].kind, out[0].sound), (1, sound));

        // Re-declaring is silent; the driver reporting the music ended
        // surfaces the tag as an event.
        frame(ctx);
        assert_eq!(kui_take_audio_commands(ctx, out.as_mut_ptr(), out.len()), 0);
        kui_audio_ended(ctx, music);
        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &mut ev));
        let payload = unsafe { &*ev.payload };
        assert_eq!(payload.0.get("kind").and_then(Value::as_str), Some("sound"));
        assert_eq!(payload.0.get("tag").and_then(Value::as_str), Some("music"));
        assert_eq!(
            payload.0.get("playback").and_then(Value::as_int),
            Some(music as i64)
        );
        kui_ctx_free(ctx);
    }

    const KUI_EMPTY: KuiStr = KuiStr {
        ptr: std::ptr::null(),
        len: 0,
    };
    const KUI_STR_TEST: KuiStr = KuiStr {
        ptr: "music".as_ptr(),
        len: 5,
    };
}

#[cfg(test)]
mod queries_headless {
    use super::*;

    fn ks(s: &str) -> KuiStr {
        KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    /// Raw keys cross as strings: a C host drives an `on_key` sink with
    /// `kui_input_key_down` / `_up`, gets both halves back as one
    /// `{kind="key"}` payload apart by `phase`, and lets go of what is
    /// held when its window loses the keyboard.
    #[test]
    fn raw_keys_and_their_releases_cross_the_boundary() {
        let ctx = kui_ctx_new();
        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing {
            tag: 2,
            value: 100.0,
        };
        spec.height = KuiSizing {
            tag: 2,
            value: 50.0,
        };
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        let sink = kui_open_with(
            ctx,
            ks("sink"),
            &spec,
            NONE,
            NONE,
            kui_value_str(ks("keys")),
            NONE,
        );
        kui_close(ctx);
        kui_set_key_focus(ctx, sink);
        kui_frame_finish(ctx);

        let null = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        // A NULL text means "whatever this key inserts".
        kui_input_key_down(ctx, ks("w"), 0, null, false);
        kui_input_key_down(ctx, ks("w"), 0, null, true);
        kui_input_key_up(ctx, ks("w"), 0);
        // Held when the window loses the keyboard: the release is made up.
        kui_input_key_down(ctx, ks("f5"), KMOD_CTRL, null, false);
        kui_release_held_keys(ctx);
        // An unknown name is ignored rather than delivered as "unknown".
        kui_input_key_down(ctx, ks("nonsense"), 0, null, false);

        let mut ev = KuiEvent::default();
        let mut seen = Vec::new();
        while kui_poll_event(ctx, &mut ev) {
            let get = |k: &str| {
                let v = kui_value_get(ev.payload, ks(k));
                let mut out = KuiStr {
                    ptr: std::ptr::null(),
                    len: 0,
                };
                kui_value_as_str(v, &mut out).then(|| kstr(out).into_owned())
            };
            assert_eq!(ev.key, sink);
            assert_eq!(get("kind").as_deref(), Some("key"));
            assert_eq!(get("tag").as_deref(), Some("keys"));
            seen.push((
                get("phase").unwrap_or_default(),
                get("code").unwrap_or_default(),
                get("text"),
            ));
        }
        assert_eq!(
            seen,
            [
                ("down".into(), "w".into(), Some("w".into())),
                ("down".into(), "w".into(), Some("w".into())),
                ("up".into(), "w".into(), None),
                ("down".into(), "f5".into(), None),
                ("up".into(), "f5".into(), None),
            ]
        );
        kui_ctx_free(ctx);
    }

    const KMOD_CTRL: u32 = 1 << 1;

    /// An editor's runs cross as rows with borrowed arrays, and a text
    /// request addresses them: select "world" by run positions, type
    /// over it, read the text back.
    #[test]
    fn editor_runs_and_text_requests_cross_the_boundary() {
        let ctx = kui_ctx_new();
        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing {
            tag: 2,
            value: 300.0,
        };
        spec.label = ks("Doc");
        kui_frame_begin(ctx, 400.0, 200.0, 1.0);
        let key = kui_text_edit(
            ctx,
            ks("doc"),
            ks("hello world"),
            std::ptr::null(),
            2, // KUI_EDIT_AUTOFOCUS
            &spec,
        );
        kui_frame_finish(ctx);

        let mut nodes = [unsafe { std::mem::zeroed::<KuiAccessNode>() }; 4];
        assert_eq!(kui_access_tree(ctx, nodes.as_mut_ptr(), nodes.len()), 2);
        let ed = nodes[1];
        assert_eq!(ed.key, key);
        assert_eq!(ed.role, role_code(kui_core::Role::TextInput));
        assert_eq!(ed.run_count, 1);
        assert_ne!(ed.flags & KUI_ACCESS_HAS_TEXT_SELECTION, 0);
        assert_ne!(ed.flags & KUI_ACCESS_FOCUSED, 0);
        assert_eq!((ed.focus_char, ed.anchor_char), (0, 0));

        let mut runs = [unsafe { std::mem::zeroed::<KuiAccessRun>() }; 4];
        assert_eq!(kui_access_runs(ctx, key, runs.as_mut_ptr(), runs.len()), 1);
        let r = runs[0];
        assert_eq!(r.key, ed.focus_run);
        assert_eq!(kstr(r.text).as_ref(), "hello world");
        assert_eq!((r.line, r.start, r.end), (0, 0, 11));
        assert_eq!(r.char_count, 11);
        let starts =
            unsafe { std::slice::from_raw_parts(r.word_starts, r.word_start_count as usize) };
        assert_eq!(starts, [0, 6]);
        let positions =
            unsafe { std::slice::from_raw_parts(r.char_positions, r.char_count as usize) };
        assert_eq!(positions[0], 0.0);
        assert!(positions[6] > positions[0]);
        assert_eq!(kui_access_runs(ctx, 12345, runs.as_mut_ptr(), 4), 0);

        kui_input_access_text(
            ctx,
            key,
            kui_core::AccessAction::SetTextSelection.bit(),
            r.key,
            6,
            r.key,
            11,
            ks(""),
        );
        kui_input_access_text(
            ctx,
            key,
            kui_core::AccessAction::ReplaceSelectedText.bit(),
            0,
            0,
            0,
            0,
            ks("there"),
        );
        let mut text = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_edit_text(ctx, key, &mut text));
        assert_eq!(kstr(text).as_ref(), "hello there");
        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &mut ev));
        assert_eq!(
            unsafe { &*ev.payload }
                .0
                .get("kind")
                .and_then(Value::as_str),
            Some("changed")
        );
        kui_ctx_free(ctx);
    }

    /// The access tree crosses as rows (plain boxes elided), and an
    /// assistive request comes back in as input: a click on a labelled
    /// button emits its payload, a slider nudge arrives as an `access`
    /// event.
    #[test]
    fn access_tree_and_requests_cross_the_boundary() {
        let ctx = kui_ctx_new();
        let mut button = unsafe { std::mem::zeroed::<KuiSpec>() };
        button.width = KuiSizing {
            tag: 2,
            value: 40.0,
        };
        button.height = KuiSizing {
            tag: 2,
            value: 20.0,
        };
        button.label = ks("Save");
        let mut slider = unsafe { std::mem::zeroed::<KuiSpec>() };
        slider.width = KuiSizing {
            tag: 2,
            value: 100.0,
        };
        slider.height = KuiSizing {
            tag: 2,
            value: 10.0,
        };
        slider.role = role_code(kui_core::Role::Slider);
        slider.label = ks("Volume");
        slider.value_set = KUI_VALUE_NOW | KUI_VALUE_MAX;
        slider.value_now = 3.0;
        slider.value_max = 10.0;
        let plain = unsafe { std::mem::zeroed::<KuiSpec>() };
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        kui_open_keyed(ctx, ks("save"), &button, kui_value_str(ks("save")));
        kui_close(ctx);
        kui_open_keyed(ctx, ks("plain"), &plain, NONE);
        kui_close(ctx);
        kui_open_keyed(ctx, ks("vol"), &slider, NONE);
        kui_close(ctx);
        kui_frame_finish(ctx);

        assert_eq!(
            kui_access_tree(ctx, std::ptr::null_mut(), 0),
            3,
            "window, button, slider: the plain box is elided"
        );
        let mut out = [unsafe { std::mem::zeroed::<KuiAccessNode>() }; 8];
        assert_eq!(kui_access_tree(ctx, out.as_mut_ptr(), out.len()), 3);
        assert_eq!(out[0].role, role_code(kui_core::Role::Window));
        assert_eq!(out[0].parent, 0);
        assert_eq!(out[1].role, role_code(kui_core::Role::Button));
        assert_eq!(kstr(out[1].name).as_ref(), "Save");
        assert_eq!(out[1].parent, out[0].key);
        assert_ne!(out[1].actions & kui_core::AccessAction::Click.bit(), 0);
        assert_eq!(
            (out[1].x, out[1].y, out[1].w, out[1].h),
            (0.0, 0.0, 40.0, 20.0)
        );
        assert_eq!(out[2].role, role_code(kui_core::Role::Slider));
        assert_eq!(kstr(out[2].name).as_ref(), "Volume");
        assert_eq!(
            out[2].flags & (KUI_ACCESS_HAS_NUMBER | KUI_ACCESS_HAS_MIN | KUI_ACCESS_HAS_MAX),
            KUI_ACCESS_HAS_NUMBER | KUI_ACCESS_HAS_MAX
        );
        assert_eq!((out[2].value_now, out[2].value_max), (3.0, 10.0));
        // A short buffer still reports the total.
        assert_eq!(kui_access_tree(ctx, out.as_mut_ptr(), 1), 3);

        kui_input_access(ctx, out[1].key, kui_core::AccessAction::Click.bit(), ks(""));
        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &mut ev));
        assert_eq!(ev.key, out[1].key);
        assert_eq!(unsafe { &*ev.payload }.0.as_str(), Some("save"));
        kui_input_access(
            ctx,
            out[2].key,
            kui_core::AccessAction::Increment.bit(),
            ks(""),
        );
        assert!(kui_poll_event(ctx, &mut ev));
        let payload = unsafe { &*ev.payload };
        assert_eq!(
            payload.0.get("kind").and_then(Value::as_str),
            Some("access")
        );
        assert_eq!(
            payload.0.get("action").and_then(Value::as_str),
            Some("increment")
        );
        assert!(!kui_poll_event(ctx, &mut ev));
        // An unknown action bit is ignored, not a crash.
        kui_input_access(ctx, out[1].key, 1 << 30, ks(""));
        assert!(!kui_poll_event(ctx, &mut ev));
        kui_ctx_free(ctx);
    }

    /// Measurement, layout events and warnings all reach C: the measured
    /// width of a label is what layout gives its node, a `layout` event
    /// polls out with the node's rect, and a lone weighted grow child
    /// warns once.
    #[test]
    fn measure_layout_and_warnings_flow_through_the_c_api() {
        let ctx = kui_ctx_new();
        // Standalone contexts start quiet; a host opts in.
        kui_set_diagnostics(ctx, true);
        let style: KuiTextStyle = unsafe { std::mem::zeroed() };
        let mut m = KuiTextMetrics::default();
        assert!(kui_measure_text(ctx, ks("hello"), &style, 0.0, &mut m));
        assert!(m.width > 0.0 && m.height > 0.0 && m.lines == 1);
        let mut wrapped = KuiTextMetrics::default();
        assert!(kui_measure_text(
            ctx,
            ks("hello world again"),
            &style,
            m.width,
            &mut wrapped
        ));
        assert!(
            wrapped.lines > 1,
            "wraps at the width of one word: {}",
            wrapped.lines
        );

        let tag = KuiValue(Value::str("panel"));
        let mut spec: KuiSpec = unsafe { std::mem::zeroed() };
        spec.width = KuiSizing { tag: 1, value: 2.0 };
        spec.height = KuiSizing {
            tag: 2,
            value: 20.0,
        };
        spec.on_layout = &tag;
        kui_frame_begin(ctx, 300.0, 100.0, 1.0);
        let mut root: KuiSpec = unsafe { std::mem::zeroed() };
        root.dir = 1;
        root.width = KuiSizing { tag: 1, value: 1.0 };
        kui_root(ctx, &root);
        let key = kui_open_keyed(ctx, ks("panel"), &spec, NONE);
        kui_close(ctx);
        kui_frame_finish(ctx);

        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &mut ev));
        assert_eq!(ev.key, key);
        let payload = unsafe { &*ev.payload };
        assert_eq!(
            payload.0.get("kind").and_then(Value::as_str),
            Some("layout")
        );
        assert_eq!(payload.0.get("w").and_then(Value::as_float), Some(300.0));
        assert_eq!(payload.0.get("tag").and_then(Value::as_str), Some("panel"));

        let mut out = [KuiWarning {
            code: KUI_EMPTY,
            key: 0,
            message: KUI_EMPTY,
        }; 4];
        let n = kui_take_warnings(ctx, out.as_mut_ptr(), out.len());
        assert_eq!(n, 1, "the lone grow-2 child warns");
        assert_eq!(&*kstr(out[0].code), "grow-weight-ignored");
        assert_eq!(out[0].key, key);
        assert!(kstr(out[0].message).contains("only grow child"));
        kui_ctx_free(ctx);
    }

    const KUI_EMPTY: KuiStr = KuiStr {
        ptr: std::ptr::null(),
        len: 0,
    };
}

// ---------------------------------------------------------------------------
// ABI parity with include/kui.h. The header is hand-written (it carries prose
// the generator would lose), so nothing in Rust makes it match the structs
// above: a field added to `KuiSpec` but missing from - or misordered in - the
// header silently shifts every field after it at runtime. This module writes
// `target/kui-abi-assert.c`, a translation unit of `_Static_assert`s pinning
// each field's offset, size and C type to what Rust actually lays out, and
// each member of the enums the API reads as list indices to its position in
// the list; examples/c/build.sh compiles it against the header, in CI too.

/// The two halves of ADR 0004's named gap: a version a host can compare,
/// and a size on every struct the library writes into the host's memory.
#[cfg(test)]
mod abi_handshake {
    use super::*;

    fn ks(s: &str) -> KuiStr {
        KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    /// A finished frame holding one clickable scroll container, and the
    /// click: enough for every out-param below to have something real to
    /// refuse to write.
    fn ctx_with_a_scroller_and_a_pending_event() -> *mut KuiCtx {
        let ctx = kui_ctx_new();
        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing {
            tag: 2,
            value: 100.0,
        };
        spec.height = KuiSizing {
            tag: 2,
            value: 50.0,
        };
        spec.overflow = 1 | 4; // KUI_CLIP | KUI_SCROLL_Y
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        kui_open_keyed(ctx, ks("scroller"), &spec, kui_value_str(ks("hit")));
        kui_close(ctx);
        kui_frame_finish(ctx);
        kui_input_cursor(ctx, 10.0, 10.0);
        kui_input_mouse(ctx, true, 1);
        kui_input_mouse(ctx, false, 1);
        ctx
    }

    fn window_cmds(ctx: *mut KuiCtx) -> Vec<KuiWindowCommand> {
        let mut out = Vec::new();
        let mut cmd = KuiWindowCommand::default();
        while kui_take_window_command(ctx, &raw mut cmd) {
            out.push(KuiWindowCommand {
                size: cmd.size,
                kind: cmd.kind,
                window: cmd.window,
                origin: cmd.origin,
                config: cmd.config,
                width: cmd.width,
                height: cmd.height,
            });
        }
        out
    }

    /// `(kind, phase)` of every `window` event queued.
    fn window_events(ctx: *mut KuiCtx) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut ev = KuiEvent::default();
        while kui_poll_event(ctx, &raw mut ev) {
            let payload = unsafe { &(*ev.payload).0 };
            let kind = payload.get("kind").and_then(Value::as_str).unwrap_or("-");
            if kind != "window" {
                continue;
            }
            let phase = payload.get("phase").and_then(Value::as_str).unwrap_or("-");
            let name = payload.get("name").and_then(Value::as_str).unwrap_or("-");
            out.push((phase.to_string(), name.to_string()));
        }
        out
    }

    fn frame_declaring(ctx: *mut KuiCtx, declare: &[(f32, f32)]) {
        kui_frame_begin(ctx, 320.0, 240.0, 1.0);
        for (w, h) in declare {
            let cfg = KuiWindowConfig {
                kind: KUI_WINDOW_KIND_NORMAL,
                width: *w,
                height: *h,
                activates: 1,
            };
            kui_window_declare(ctx, ks("palette"), &cfg);
        }
        kui_frame_finish(ctx);
    }

    /// ADR 0004's step 3 through the C surface: a declaration opens a
    /// window (once, with the first config, warning about the second),
    /// an OS close reported back keeps it closed while declared, and the
    /// declaration lapsing and starting again opens it anew.
    #[test]
    fn a_declared_window_opens_closes_and_warns_through_the_c_api() {
        let ctx = kui_ctx_new();
        kui_set_diagnostics(ctx, true);
        assert_eq!(kui_ctx_window(ctx), 0);
        let mut name = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_ctx_window_name(ctx, &raw mut name));
        assert_eq!(&*kstr(name), "main");

        frame_declaring(ctx, &[(400.0, 300.0), (500.0, 500.0)]);
        let cmds = window_cmds(ctx);
        assert_eq!(cmds.len(), 1);
        assert_eq!(
            (cmds[0].kind, cmds[0].window, cmds[0].origin),
            (KUI_CMD_OPEN, 1, 0)
        );
        assert_eq!(
            (
                cmds[0].config.width,
                cmds[0].config.height,
                cmds[0].config.activates
            ),
            (400.0, 300.0, 1),
            "the first declaration's config, not the second's"
        );
        assert_eq!(cmds[0].size, std::mem::size_of::<KuiWindowCommand>() as u32);
        assert_eq!(
            window_events(ctx),
            vec![("opened".into(), "palette".into())]
        );

        // The user closes it.
        kui_window_closed(ctx, 1);
        assert_eq!(
            window_events(ctx),
            vec![("closed".into(), "palette".into())]
        );
        // Still declared: nothing reopens, and the diagnostics say why.
        frame_declaring(ctx, &[(400.0, 300.0)]);
        assert!(window_cmds(ctx).is_empty());
        let mut warnings = [KuiWarning {
            code: ks(""),
            key: 0,
            message: ks(""),
        }; 8];
        let n = kui_take_warnings(ctx, warnings.as_mut_ptr(), 8);
        let codes: Vec<String> = warnings[..n]
            .iter()
            .map(|w| kstr(w.code).into_owned())
            .collect();
        assert_eq!(
            codes,
            vec![
                "duplicate-window-config".to_string(),
                "window-declared-while-closed".to_string()
            ]
        );

        // The declaration lapses, then starts again: a new window, new id.
        frame_declaring(ctx, &[]);
        assert!(window_cmds(ctx).is_empty());
        frame_declaring(ctx, &[(400.0, 300.0)]);
        let cmds = window_cmds(ctx);
        assert_eq!(cmds.len(), 1);
        assert_eq!((cmds[0].kind, cmds[0].window), (KUI_CMD_OPEN, 2));
        // And stops: the diff closes it.
        frame_declaring(ctx, &[]);
        let cmds = window_cmds(ctx);
        assert_eq!(cmds.len(), 1);
        assert_eq!((cmds[0].kind, cmds[0].window), (KUI_CMD_CLOSE, 2));
        assert_eq!(
            window_events(ctx),
            vec![
                ("opened".into(), "palette".into()),
                ("closed".into(), "palette".into())
            ]
        );
        kui_ctx_free(ctx);
    }

    /// The size handshake on the new [out] struct, the way `kui_poll_event`
    /// has it: a reservation below the layout is refused before anything
    /// is popped, so the command is still there for a proper call.
    #[test]
    fn a_short_window_command_reservation_is_refused_and_keeps_the_command() {
        let ctx = kui_ctx_new();
        frame_declaring(ctx, &[(400.0, 300.0)]);
        let mut short = KuiWindowCommand {
            size: 4,
            kind: 0xdead,
            ..Default::default()
        };
        assert!(!kui_take_window_command(ctx, &raw mut short));
        assert_eq!(short.kind, 0xdead, "nothing was written");
        let cmds = window_cmds(ctx);
        assert_eq!(cmds.len(), 1, "the refused command is still queued");
        assert_eq!(cmds[0].kind, KUI_CMD_OPEN);
        kui_ctx_free(ctx);
    }

    /// A context standing in for a declared window: the id
    /// `kui_env_set_window` gives it is what its events carry and what
    /// its name resolves through.
    #[test]
    fn env_set_window_names_the_context_and_stamps_its_events() {
        let ctx = kui_ctx_new();
        frame_declaring(ctx, &[(400.0, 300.0)]);
        window_cmds(ctx);
        window_events(ctx);
        kui_env_set_window(ctx, 1, false, false, false, 0.0, 0.0);
        assert_eq!(kui_ctx_window(ctx), 1);
        let mut name = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_ctx_window_name(ctx, &raw mut name));
        assert_eq!(&*kstr(name), "palette");
        // A frame at another viewport raises a resize, stamped with the id.
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        kui_window_declare(ctx, ks("palette"), std::ptr::null());
        kui_frame_finish(ctx);
        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &raw mut ev));
        assert_eq!(ev.window, 1);
        kui_ctx_free(ctx);
    }

    #[test]
    fn the_exported_version_is_the_one_the_header_states() {
        // The C side of this is a _Static_assert in mod abi_parity; this is
        // the half that survives the header being absent.
        assert_eq!(kui_abi_version(), KUI_ABI_VERSION);
    }

    /// The whole point of the size field, on a struct that has already
    /// grown: a caller that reserved only the first two fields gets them,
    /// and the third — the appended one — is left exactly as it was.
    ///
    /// This was written before any real [out] struct had grown, so that
    /// the truncating path would not be first exercised by the change that
    /// depends on it. `KuiEvent` has since grown `window`, and
    /// [`an_abi_3_host_polls_events_without_seeing_the_appended_window`]
    /// runs the same path through the public API — this one stays as the
    /// unit-level statement of the rule, on a struct with nothing else
    /// going on.
    #[test]
    fn a_short_reservation_is_filled_only_as_far_as_it_goes() {
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct Grown {
            size: u32,
            was_always_here: u32,
            appended_later: u32,
        }
        // SAFETY: repr(C) with `size: u32` first.
        unsafe impl OutParam for Grown {
            const ABI_V1_SIZE: u32 = abi_through!(Grown, was_always_here, u32);
            fn size_mut(&mut self) -> &mut u32 {
                &mut self.size
            }
        }

        // A host built before `appended_later` existed: it reserved the
        // whole struct it knew, which is the ABI-1 layout.
        let mut old = Grown {
            size: Grown::ABI_V1_SIZE,
            was_always_here: 0,
            appended_later: 0xdeadbeef,
        };
        assert!(write_out(
            &raw mut old,
            Grown {
                size: 0,
                was_always_here: 7,
                appended_later: 9,
            },
        ));
        assert_eq!(old.was_always_here, 7);
        assert_eq!(
            old.appended_later, 0xdeadbeef,
            "the library wrote past what the caller reserved"
        );
        assert_eq!(old.size, Grown::ABI_V1_SIZE, "size reports what was filled");

        // A host built after: it gets everything, and `size` says so.
        let mut new = Grown {
            size: std::mem::size_of::<Grown>() as u32,
            was_always_here: 0,
            appended_later: 0,
        };
        assert!(write_out(
            &raw mut new,
            Grown {
                size: 0,
                was_always_here: 7,
                appended_later: 9,
            },
        ));
        assert_eq!((new.was_always_here, new.appended_later), (7, 9));
        assert_eq!(new.size, std::mem::size_of::<Grown>() as u32);

        // And `size` coming back as the filled count is idempotent, so a
        // loop reusing one struct clamps to the same prefix every time.
        assert!(write_out(
            &raw mut old,
            Grown {
                size: 0,
                was_always_here: 11,
                appended_later: 0,
            },
        ));
        assert_eq!((old.size, old.was_always_here), (Grown::ABI_V1_SIZE, 11));
        assert_eq!(old.appended_later, 0xdeadbeef);
    }

    /// The same rule on the real struct, through the real entry point:
    /// `KuiEvent.window` is ABI 4's append, and a host that predates it —
    /// one whose `KuiEvent` ends after `payload`, which is the ABI-1 floor
    /// `out_accepts` measures against — keeps polling events and simply
    /// never sees the new field.
    ///
    /// The old host is spelled as a reservation rather than as a second
    /// struct because that is all the library ever sees of it: four bytes
    /// of `size`, and a promise about what lies behind them.
    #[test]
    fn an_abi_3_host_polls_events_without_seeing_the_appended_window() {
        let ctx = ctx_with_a_scroller_and_a_pending_event();
        let abi3 = KuiEvent::ABI_V1_SIZE;
        assert!(
            (abi3 as usize) < std::mem::size_of::<KuiEvent>(),
            "`window` must sit past the ABI-1 layout, or this proves nothing"
        );

        let mut ev = KuiEvent {
            size: abi3,
            window: 0xdead,
            ..Default::default()
        };
        assert!(kui_poll_event(ctx, &raw mut ev), "the event still arrives");
        assert_ne!(ev.key, 0, "and the fields it knows are filled");
        assert!(!ev.payload.is_null());
        assert_eq!(ev.size, abi3, "`size` reports the prefix that was filled");
        assert_eq!(
            ev.window, 0xdead,
            "the library wrote past what an ABI 3 host reserved"
        );

        kui_ctx_free(ctx);
    }

    /// And a host built against this ABI gets the field, which is 0 until
    /// ADR 0004's step 3 opens a second window.
    #[test]
    fn a_current_host_sees_window_and_it_is_the_main_one() {
        let ctx = ctx_with_a_scroller_and_a_pending_event();
        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &raw mut ev));
        assert_eq!(ev.window, 0);
        assert_eq!(ev.size, std::mem::size_of::<KuiEvent>() as u32);
        kui_ctx_free(ctx);
    }

    /// A reservation smaller than ABI 1 is refused rather than guessed at —
    /// and refused *before* the queue moves, so the event is still there
    /// for a caller that asks properly.
    #[test]
    fn an_unreadable_reservation_refuses_without_dropping_the_event() {
        let ctx = ctx_with_a_scroller_and_a_pending_event();
        // 4 bytes: what an un-set `size`, or one from a host predating the
        // field, looks like to this library.
        let mut stale = KuiEvent {
            size: 4,
            ..Default::default()
        };
        assert!(!kui_poll_event(ctx, &raw mut stale));
        assert_eq!(stale.key, 0, "nothing was written");

        let mut ev = KuiEvent::default();
        assert!(
            kui_poll_event(ctx, &raw mut ev),
            "the event was not dropped"
        );
        assert!(!ev.payload.is_null());
        kui_ctx_free(ctx);
    }

    /// The same refusal on the other three, so the rule is the family's and
    /// not `kui_poll_event`'s.
    #[test]
    fn every_out_param_refuses_a_reservation_it_cannot_honour() {
        let ctx = ctx_with_a_scroller_and_a_pending_event();

        // A key with real geometry behind it, so the refusal below is the
        // reservation being rejected and not the lookup missing.
        let scroller = kui_child_key(ctx, ks("scroller"));
        let mut geom = KuiScrollGeometry::default();
        assert!(kui_scroll_geometry(ctx, scroller, &raw mut geom));
        assert!(geom.h > 0.0);
        let mut short = KuiScrollGeometry {
            size: 2,
            ..Default::default()
        };
        assert!(!kui_scroll_geometry(ctx, scroller, &raw mut short));
        assert_eq!(short.h, 0.0, "nothing was written");

        let mut m = KuiTextMetrics {
            size: 0,
            ..Default::default()
        };
        let style: KuiTextStyle = unsafe { std::mem::zeroed() };
        assert!(!kui_measure_text(ctx, ks("hello"), &style, 0.0, &raw mut m));
        assert_eq!(m.width, 0.0);

        let mut draw = KuiDrawData {
            size: 1,
            ..Default::default()
        };
        assert!(!kui_draw_data(ctx, &raw mut draw));
        assert!(draw.quads.is_null());
        // Refused, so the atlas is still owed to whoever asks next.
        let mut good = KuiDrawData::default();
        assert!(kui_draw_data(ctx, &raw mut good));
        assert_eq!(good.size, std::mem::size_of::<KuiDrawData>() as u32);

        // NULL is the older half of the same rule and still holds.
        assert!(!kui_poll_event(ctx, std::ptr::null_mut()));
        assert!(!kui_draw_data(ctx, std::ptr::null_mut()));
        kui_ctx_free(ctx);
    }
}

#[cfg(test)]
mod abi_parity {
    use super::*;
    use std::fmt::Write as _;

    /// One repr(C) struct's ABI, restated as a table and emitted as C.
    ///
    /// The table is pinned to the Rust definition at compile time: the
    /// destructuring names every field, so adding one to the struct stops
    /// this module compiling until it is listed here (and mirrored in
    /// include/kui.h), and each binding is checked against the Rust type the
    /// row declares, so `radius: f32 => "float"` cannot outlive a change to
    /// `radius: u32`. Offsets, sizes and alignments come from Rust itself.
    macro_rules! abi_struct {
        ($out:expr, $ty:ident { $($f:ident : $rt:ty => $c:literal),* $(,)? }) => {{
            // Rebuilding the struct field by field is the pin: a field
            // added to $ty and not to the table below is a missing field in
            // this initializer (E0063, by name), and a field whose Rust type
            // changed no longer matches the `$rt` the row declares.
            #[allow(dead_code)]
            fn pinned(v: $ty) -> $ty {
                $(let $f: $rt = v.$f;)*
                $ty { $($f),* }
            }
            writeln!(
                $out,
                "KUI_STRUCT({}, {}, {});",
                stringify!($ty),
                std::mem::size_of::<$ty>(),
                std::mem::align_of::<$ty>(),
            )
            .unwrap();
            $(writeln!(
                $out,
                "KUI_FIELD({}, {}, {}, {}, {});",
                stringify!($ty),
                stringify!($f),
                $c,
                std::mem::offset_of!($ty, $f),
                std::mem::size_of::<$rt>(),
            )
            .unwrap();)*
            writeln!($out).unwrap();
        }};
    }

    /// One [out] struct's size handshake, restated as C.
    ///
    /// The `abi_struct!` row above it already pins `size`'s offset, size and
    /// type; this adds the two facts that make the handshake work and that
    /// no field-by-field check would notice: that `size` is the *first*
    /// field (so a library can read a caller's reservation before trusting
    /// anything else in the struct), and that the layout has never shrunk
    /// below what ABI 1 shipped (so `ABI_V1_SIZE` is still a floor and not
    /// a ceiling). The floor comes from Rust's own `OutParam` impl, so the
    /// two cannot drift.
    macro_rules! abi_out_struct {
        ($out:expr, $ty:ident) => {{
            writeln!(
                $out,
                "KUI_OUT_STRUCT({}, {});",
                stringify!($ty),
                <$ty as OutParam>::ABI_V1_SIZE,
            )
            .unwrap();
            writeln!($out).unwrap();
        }};
    }

    /// One C enum whose members are the indices of a list Rust owns,
    /// restated as C.
    ///
    /// The values come from Rust and the spellings from the header, and the
    /// array's length is the list's, so a member added to the list — a role
    /// appended to `Role::ALL` — stops this module compiling until the
    /// header's name for it is listed here, and a name the header does not
    /// define stops the generated C compiling. `$base` is the value index 0
    /// takes in C: 1 wherever zero has to stay free for "unset".
    macro_rules! abi_enum {
        ($out:expr, $list:expr, $base:expr => [$($c:literal),* $(,)?]) => {{
            const NAMES: [&str; $list.len()] = [$($c),*];
            for (i, name) in NAMES.iter().enumerate() {
                writeln!($out, "KUI_ENUM({}, {});", name, i + $base).unwrap();
            }
            writeln!($out).unwrap();
        }};
    }

    const PRELUDE: &str = r#"/* Generated by `cargo test -p kui-ffi abi_parity` (see
 * crates/kui-ffi/src/lib.rs, mod abi_parity) - do not edit, do not commit.
 * examples/c/build.sh regenerates and compiles it.
 *
 * Every field of every public repr(C) struct in the C API is pinned to the
 * offset, size and type Rust lays it out with, so a header that has drifted
 * from crates/kui-ffi/src/lib.rs fails to compile here instead of misreading
 * every field after the drift at runtime. Nothing links: the asserts are
 * settled in the front end.
 */
#include "kui.h"

#define KUI_STRUCT(T, size, align)                                            \
    _Static_assert(sizeof(T) == (size), "sizeof(" #T ") differs from Rust");  \
    _Static_assert(_Alignof(T) == (align), "_Alignof(" #T ") differs from Rust")

/* CT is the C type the field must have: same size at the same offset is not
 * enough, `float` and `uint32_t` swap silently. Arrays are named by what
 * they decay to (`float *` for `float[4]`); their length is covered by the
 * size assert. */
#define KUI_FIELD(T, f, CT, off, size)                                        \
    _Static_assert(offsetof(T, f) == (off), #T "." #f ": offset differs from Rust");    \
    _Static_assert(sizeof(((T *)0)->f) == (size), #T "." #f ": size differs from Rust"); \
    _Static_assert(_Generic(((T *)0)->f, CT: 1, default: 0), #T "." #f ": type differs from Rust")

/* An [out] struct: one the library writes into memory the host reserved.
 * `size` has to lead it, because the library reads the host's reservation
 * out of those four bytes before it trusts any other byte of the struct;
 * and v1 is the layout ABI 1 shipped, which is a floor the struct may grow
 * past but must never fall below - a removed or narrowed field would have
 * an old host's reservation accepted and then under-filled. */
#define KUI_OUT_STRUCT(T, v1)                                                 \
    _Static_assert(offsetof(T, size) == 0, #T ".size must be the first field"); \
    _Static_assert(sizeof(T) >= (v1), #T " shrank below its ABI 1 layout")

/* One member of an enum the library reads as an index into a list Rust owns.
 * The value assert catches a member that drifted; the name itself catches
 * the commoner failure, a list that grew and a header that did not, which
 * fails here as an undeclared identifier instead of leaving C the one
 * binding that cannot say the new value. */
#define KUI_ENUM(name, value)                                                 \
    _Static_assert((name) == (value), #name " differs from Rust")
"#;

    fn asserts() -> String {
        let mut o = String::from(PRELUDE);
        o.push('\n');

        // The header's KUI_ABI_VERSION is what a host compares against
        // kui_abi_version() at startup, so a bump made in one place and not
        // the other would make that check pass on a mismatched pair.
        writeln!(
            o,
            "_Static_assert(KUI_ABI_VERSION == {}u, \"KUI_ABI_VERSION differs from Rust\");\n",
            KUI_ABI_VERSION,
        )
        .unwrap();

        // The enums whose members are positions in a list the core owns, so
        // that a list growing on the Rust side cannot leave the header - the
        // one binding that does not read the tables at runtime - without a
        // name for the new member.
        abi_enum!(o, kui_core::Role::ALL, 1 => [
            "KUI_ROLE_NONE", "KUI_ROLE_BUTTON", "KUI_ROLE_CHECKBOX",
            "KUI_ROLE_RADIO", "KUI_ROLE_SWITCH", "KUI_ROLE_SLIDER",
            "KUI_ROLE_TAB", "KUI_ROLE_TAB_LIST", "KUI_ROLE_LINK",
            "KUI_ROLE_HEADING", "KUI_ROLE_LIST", "KUI_ROLE_LIST_ITEM",
            "KUI_ROLE_IMAGE", "KUI_ROLE_DIALOG", "KUI_ROLE_GROUP",
            "KUI_ROLE_WINDOW", "KUI_ROLE_TITLE_BAR", "KUI_ROLE_STATIC_TEXT",
            "KUI_ROLE_TEXT_INPUT", "KUI_ROLE_MULTILINE_TEXT_INPUT",
            "KUI_ROLE_SCROLL_VIEW", "KUI_ROLE_LINE",
            "KUI_ROLE_RADIO_GROUP", "KUI_ROLE_MENU", "KUI_ROLE_MENU_ITEM",
        ]);
        abi_enum!(o, kui_core::schema::ORIENTATIONS, 1 => [
            "KUI_ORIENTATION_HORIZONTAL", "KUI_ORIENTATION_VERTICAL",
        ]);
        abi_enum!(o, kui_core::schema::CURSORS, 1 => [
            "KUI_CURSOR_DEFAULT", "KUI_CURSOR_TEXT", "KUI_CURSOR_POINTER",
            "KUI_CURSOR_GRAB", "KUI_CURSOR_GRABBING", "KUI_CURSOR_NOT_ALLOWED",
            "KUI_CURSOR_EW_RESIZE", "KUI_CURSOR_NS_RESIZE",
            "KUI_CURSOR_NWSE_RESIZE", "KUI_CURSOR_NESW_RESIZE",
        ]);
        abi_enum!(o, kui_core::schema::EXPANDED, 1 => [
            "KUI_EXPANDED_COLLAPSED", "KUI_EXPANDED_EXPANDED",
        ]);
        abi_enum!(o, kui_core::schema::WINDOW_ROLES, 1 => [
            "KUI_WINDOW_DRAG", "KUI_WINDOW_CLOSE", "KUI_WINDOW_MINIMIZE",
            "KUI_WINDOW_MAXIMIZE",
        ]);
        abi_enum!(o, kui_core::schema::ALIGNS, 0 => [
            "KUI_START", "KUI_CENTER", "KUI_END",
        ]);
        abi_enum!(o, kui_core::schema::FAMILIES, 0 => [
            "KUI_FONT_SANS", "KUI_FONT_SERIF", "KUI_FONT_MONO",
        ]);
        abi_enum!(o, kui_core::schema::WRAPS, 0 => [
            "KUI_WRAP_WORD", "KUI_WRAP_GLYPH", "KUI_WRAP_NONE",
        ]);
        abi_enum!(o, kui_core::schema::EASINGS, 0 => [
            "KUI_EASE_OUT", "KUI_EASE_LINEAR", "KUI_EASE_IN",
            "KUI_EASE_IN_OUT", "KUI_EASE_SPRING", "KUI_EASE_BOUNCY",
        ]);
        abi_enum!(o, kui_core::schema::REPEATS, 0 => [
            "KUI_REPEAT_NORMAL", "KUI_REPEAT_REVERSE", "KUI_REPEAT_ALTERNATE",
            "KUI_REPEAT_ALTERNATE_REVERSE",
        ]);

        abi_struct!(o, KuiScrollGeometry {
            size: u32 => "uint32_t",
            x: f32 => "float",
            y: f32 => "float",
            w: f32 => "float",
            h: f32 => "float",
            content_w: f32 => "float",
            content_h: f32 => "float",
            offset_x: f32 => "float",
            offset_y: f32 => "float",
            max_offset_x: f32 => "float",
            max_offset_y: f32 => "float",
        });
        abi_out_struct!(o, KuiScrollGeometry);

        abi_struct!(o, KuiStr {
            ptr: *const u8 => "const uint8_t *",
            len: usize => "size_t",
        });

        abi_struct!(o, KuiSizing {
            tag: u32 => "uint32_t",
            value: f32 => "float",
        });

        abi_struct!(o, KuiKeyframe {
            set: u32 => "uint32_t",
            at: f32 => "float",
            width: KuiSizing => "KuiSizing",
            height: KuiSizing => "KuiSizing",
            bg: u32 => "uint32_t",
            radius: f32 => "float",
            opacity: f32 => "float",
        });

        abi_struct!(o, KuiEnter {
            set: u32 => "uint32_t",
            dx: f32 => "float",
            dy: f32 => "float",
            width: KuiSizing => "KuiSizing",
            height: KuiSizing => "KuiSizing",
            bg: u32 => "uint32_t",
            radius: f32 => "float",
            opacity: f32 => "float",
        });

        abi_struct!(o, KuiSpec {
            width: KuiSizing => "KuiSizing",
            height: KuiSizing => "KuiSizing",
            min_w: f32 => "float",
            max_w: f32 => "float",
            min_h: f32 => "float",
            max_h: f32 => "float",
            dir: u32 => "uint32_t",
            pad_l: f32 => "float",
            pad_r: f32 => "float",
            pad_t: f32 => "float",
            pad_b: f32 => "float",
            gap: f32 => "float",
            main_align: u32 => "uint32_t",
            cross_align: u32 => "uint32_t",
            bg: u32 => "uint32_t",
            border_color: u32 => "uint32_t",
            border_w: f32 => "float",
            radius: f32 => "float",
            overflow: u32 => "uint32_t",
            float_mode: u32 => "uint32_t",
            float_anchor_x: u32 => "uint32_t",
            float_anchor_y: u32 => "uint32_t",
            float_self_x: u32 => "uint32_t",
            float_self_y: u32 => "uint32_t",
            float_dx: f32 => "float",
            float_dy: f32 => "float",
            float_fit: u32 => "uint32_t",
            hoverable: u32 => "uint32_t",
            window_role: u32 => "uint32_t",
            transition_ms: f32 => "float",
            easing: u32 => "uint32_t",
            slide: u32 => "uint32_t",
            hover_bg: u32 => "uint32_t",
            pressed_bg: u32 => "uint32_t",
            hover_group: KuiStr => "KuiStr",
            per_corner: u32 => "uint32_t",
            radius_tl: f32 => "float",
            radius_tr: f32 => "float",
            radius_br: f32 => "float",
            radius_bl: f32 => "float",
            repeat: u32 => "uint32_t",
            delay_ms: f32 => "float",
            keyframes: *const KuiKeyframe => "const KuiKeyframe *",
            keyframes_len: usize => "size_t",
            enter: KuiEnter => "KuiEnter",
            click_sound: u64 => "uint64_t",
            hover_sound: u64 => "uint64_t",
            on_layout: *const KuiValue => "const KuiValue *",
            role: u32 => "uint32_t",
            label: KuiStr => "KuiStr",
            checked: u32 => "uint32_t",
            value_set: u32 => "uint32_t",
            value_now: f32 => "float",
            value_min: f32 => "float",
            value_max: f32 => "float",
            caret: u32 => "uint32_t",
            selection_anchor: u32 => "uint32_t",
            focusable: u32 => "uint32_t",
            disabled: u32 => "uint32_t",
            focus_bg: u32 => "uint32_t",
            tooltip: KuiStr => "KuiStr",
            modal: *const KuiValue => "const KuiValue *",
            on_context_menu: *const KuiValue => "const KuiValue *",
            cursor: u32 => "uint32_t",
            selected: u32 => "uint32_t",
            expanded: u32 => "uint32_t",
            opacity_set: u32 => "uint32_t",
            opacity: f32 => "float",
            shadow_color: u32 => "uint32_t",
            shadow_blur: f32 => "float",
            shadow_x: f32 => "float",
            shadow_y: f32 => "float",
            shadow_spread: f32 => "float",
            wrap_children: u32 => "uint32_t",
            cross_gap: f32 => "float",
            initial_focus: u32 => "uint32_t",
            exit: KuiEnter => "KuiEnter",
        });

        abi_struct!(o, KuiAccessNode {
            key: u64 => "uint64_t",
            parent: u64 => "uint64_t",
            origin: u32 => "uint32_t",
            role: u32 => "uint32_t",
            flags: u32 => "uint32_t",
            actions: u32 => "uint32_t",
            name: KuiStr => "KuiStr",
            description: KuiStr => "KuiStr",
            value: KuiStr => "KuiStr",
            x: f32 => "float",
            y: f32 => "float",
            w: f32 => "float",
            h: f32 => "float",
            caret: u32 => "uint32_t",
            selection_start: u32 => "uint32_t",
            selection_end: u32 => "uint32_t",
            value_now: f32 => "float",
            value_min: f32 => "float",
            value_max: f32 => "float",
            scroll_x: f32 => "float",
            scroll_y: f32 => "float",
            scroll_max_x: f32 => "float",
            scroll_max_y: f32 => "float",
            anchor_run: u64 => "uint64_t",
            anchor_char: u32 => "uint32_t",
            focus_run: u64 => "uint64_t",
            focus_char: u32 => "uint32_t",
            run_count: u32 => "uint32_t",
            pos_in_set: u32 => "uint32_t",
            set_size: u32 => "uint32_t",
            orientation: u32 => "uint32_t",
        });

        abi_struct!(o, KuiAccessRun {
            key: u64 => "uint64_t",
            line: u32 => "uint32_t",
            start: u32 => "uint32_t",
            end: u32 => "uint32_t",
            text: KuiStr => "KuiStr",
            x: f32 => "float",
            y: f32 => "float",
            w: f32 => "float",
            h: f32 => "float",
            char_count: u32 => "uint32_t",
            char_lengths: *const u8 => "const uint8_t *",
            char_positions: *const f32 => "const float *",
            char_widths: *const f32 => "const float *",
            word_start_count: u32 => "uint32_t",
            word_starts: *const u8 => "const uint8_t *",
            rtl: u32 => "uint32_t",
        });

        abi_struct!(o, KuiTextMetrics {
            size: u32 => "uint32_t",
            width: f32 => "float",
            height: f32 => "float",
            lines: u32 => "uint32_t",
        });
        abi_out_struct!(o, KuiTextMetrics);

        abi_struct!(o, KuiWarning {
            code: KuiStr => "KuiStr",
            key: u64 => "uint64_t",
            message: KuiStr => "KuiStr",
        });

        abi_struct!(o, KuiPlay {
            volume: f32 => "float",
            looped: u32 => "uint32_t",
            fade_in_ms: f32 => "float",
        });

        abi_struct!(o, KuiAudio {
            src: u64 => "uint64_t",
            volume: f32 => "float",
            looped: u32 => "uint32_t",
            paused: u32 => "uint32_t",
        });

        abi_struct!(o, KuiAudioCommand {
            kind: u32 => "uint32_t",
            playback: u64 => "uint64_t",
            sound: u64 => "uint64_t",
            volume: f32 => "float",
            ms: f32 => "float",
            looped: u32 => "uint32_t",
        });

        abi_struct!(o, KuiTextStyle {
            size: f32 => "float",
            line_height: f32 => "float",
            color: u32 => "uint32_t",
            family: u32 => "uint32_t",
            font: u64 => "uint64_t",
            wrap: u32 => "uint32_t",
            max_lines: u32 => "uint32_t",
            ellipsis: u32 => "uint32_t",
        });

        abi_struct!(o, KuiSpan {
            text: KuiStr => "KuiStr",
            color: u32 => "uint32_t",
            flags: u32 => "uint32_t",
        });

        abi_struct!(o, KuiEvent {
            size: u32 => "uint32_t",
            origin: u16 => "uint16_t",
            key: u64 => "uint64_t",
            payload: *const KuiValue => "const KuiValue *",
            window: u32 => "uint32_t",
        });
        abi_out_struct!(o, KuiEvent);

        abi_struct!(o, KuiWindowConfig {
            kind: u32 => "uint32_t",
            width: f32 => "float",
            height: f32 => "float",
            activates: u32 => "uint32_t",
        });

        abi_struct!(o, KuiWindowCommand {
            size: u32 => "uint32_t",
            kind: u32 => "uint32_t",
            window: u32 => "uint32_t",
            origin: u16 => "uint16_t",
            config: KuiWindowConfig => "KuiWindowConfig",
            width: f32 => "float",
            height: f32 => "float",
        });
        abi_out_struct!(o, KuiWindowCommand);

        // The command verbs and the window kind are plain constants on
        // both sides; pinned here so the header cannot renumber one.
        for (name, value) in [
            ("KUI_CMD_START_DRAG", KUI_CMD_START_DRAG),
            ("KUI_CMD_CLOSE", KUI_CMD_CLOSE),
            ("KUI_CMD_MINIMIZE", KUI_CMD_MINIMIZE),
            ("KUI_CMD_SET_SIZE", KUI_CMD_SET_SIZE),
            ("KUI_CMD_FOCUS", KUI_CMD_FOCUS),
            ("KUI_CMD_TOGGLE_MAXIMIZE", KUI_CMD_TOGGLE_MAXIMIZE),
            ("KUI_CMD_OPEN", KUI_CMD_OPEN),
            ("KUI_WINDOW_KIND_NORMAL", KUI_WINDOW_KIND_NORMAL),
            ("KUI_WINDOW_MAIN", WindowId::MAIN.0),
        ] {
            writeln!(o, "KUI_ENUM({name}, {value});").unwrap();
        }
        writeln!(o).unwrap();

        abi_struct!(o, KuiQuad {
            x: f32 => "float",
            y: f32 => "float",
            w: f32 => "float",
            h: f32 => "float",
            color: [f32; 4] => "float *",
            border_color: [f32; 4] => "float *",
            radius: [f32; 4] => "float *",
            border_w: f32 => "float",
            blur: f32 => "float",
            kind: u32 => "uint32_t",
            uv: [u32; 4] => "uint32_t *",
            clip: [f32; 4] => "float *",
            clip_radius: [f32; 4] => "float *",
        });

        abi_struct!(o, KuiDrawData {
            size: u32 => "uint32_t",
            quads: *const KuiQuad => "const KuiQuad *",
            quad_count: usize => "size_t",
            viewport_w: f32 => "float",
            viewport_h: f32 => "float",
            scale: f32 => "float",
            atlas_pixels: *const u8 => "const uint8_t *",
            atlas_size: u32 => "uint32_t",
            atlas_dirty: bool => "bool",
            atlas_epoch: u64 => "uint64_t",
        });
        abi_out_struct!(o, KuiDrawData);

        o
    }

    /// Writes the asserts next to the built library, where
    /// examples/c/build.sh picks them up. The C compiler is the check; this
    /// test only produces what it checks (and fails if the tree is not
    /// writable), so the two halves stay one `./examples/c/build.sh` apart.
    #[test]
    fn writes_the_c_abi_asserts() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
        std::fs::create_dir_all(&dir).expect("create target dir");
        let path = dir.join("kui-abi-assert.c");
        let text = asserts();
        assert!(text.contains("KUI_FIELD(KuiSpec, focus_bg,"));
        assert!(text.contains("KUI_OUT_STRUCT(KuiEvent,"));
        assert!(text.contains("KUI_OUT_STRUCT(KuiWindowCommand,"));
        assert!(text.contains("KUI_ENUM(KUI_ROLE_LINE, 22);"));
        std::fs::write(&path, text).expect("write kui-abi-assert.c");
    }
}
