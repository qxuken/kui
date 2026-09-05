//! The shared widgets (`kui_core::widgets`) as entry points, body
//! callbacks re-entering through the same context pointer.

use super::*;

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
