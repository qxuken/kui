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

/// The stock select: a field showing the choice in force that, clicked,
/// opens the core's own menu
/// of `count` options read from `items` — the same rows `kui_open_menu`
/// takes — under it, the `current`th checked (`-1` for none). The host
/// holds no open state: the choice arrives as the `{kind:"menu", role,
/// item}` event a menu row posts, on the key this returns, and drawing
/// the field again with the new `current` is the whole loop; a host that
/// shows menus itself sees the menu in `kui_menu` as any other.
///
/// Returns the node key, or 0 for no label, no items, or a row with a
/// role this build does not know. A `current` past the end, or on a
/// separator, raises a `select-current-ignored` warning and reads as
/// none.
#[unsafe(no_mangle)]
pub extern "C" fn kui_select(
    ptr: *mut KuiCtx,
    label: KuiStr,
    items: *const KuiMenuItem,
    count: usize,
    current: i64,
) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let label = kstr(label);
        if label.is_empty() || items.is_null() || count == 0 {
            return 0;
        }
        let rows = unsafe { std::slice::from_raw_parts(items, count) };
        let mut parsed = Vec::with_capacity(count);
        for row in rows {
            let Some(item) = row.to_core() else {
                return 0;
            };
            parsed.push(item);
        }
        let current = usize::try_from(current).ok();
        kui_core::widgets::select_items(&mut kui_core::Ui::wrap(c.core()), &label, &parsed, current)
            .0
    })
}

/// The stock button, labelled and keyed by `label`: a press by the
/// pointer, Space, Enter or assistive technology emits `payload` as a
/// `click` event's payload. Consumes `payload` (NULL for none).
#[unsafe(no_mangle)]
pub extern "C" fn kui_button(ptr: *mut KuiCtx, label: KuiStr, payload: *mut KuiValue) {
    kui_button_with(ptr, label, std::ptr::null(), payload);
}

/// [`kui_button`] with the rows the stock button admits read off `spec` —
/// `label`, `description`, `tooltip`, `disabled` — and every other field
/// ignored: the look is `widgets::button_spec`'s, and a zeroed `KuiSpec`
/// is the schema default rather than "unset", so there is nothing to
/// merge. A NULL `spec` is [`kui_button`]. Consumes payload.
#[unsafe(no_mangle)]
pub extern "C" fn kui_button_with(
    ptr: *mut KuiCtx,
    text: KuiStr,
    spec: *const KuiSpec,
    payload: *mut KuiValue,
) {
    guard((), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            if !payload.is_null() {
                drop(unsafe { Box::from_raw(payload) });
            }
            return;
        };
        let text = kstr(text);
        let value = if payload.is_null() {
            Value::Null
        } else {
            unsafe { Box::from_raw(payload) }.0
        };
        // The same data as kui_core::widgets::button: hover/pressed colors
        // are declared on the spec and resolved by the core. The tooltip
        // is applied before the description, as `spec_of` orders them, so
        // the explicit field wins over the shorthand.
        let (theme, metrics) = {
            let core = c.core();
            (*core.theme(), *core.metrics())
        };
        let mut node = kui_core::widgets::button_spec(&theme, &metrics).on_click(value);
        let mut hint = None;
        if let Some(s) = unsafe { spec.as_ref() } {
            if let Some(h) = opt_str(s.tooltip) {
                node = node.apply_tooltip(&h);
                hint = Some(h);
            }
            if let Some(l) = opt_str(s.label) {
                node = node.label(l.as_ref());
            }
            if let Some(d) = opt_str(s.description) {
                node = node.description(d.as_ref());
            }
            node = node.disabled(s.disabled != 0);
            if s.accent != 0 {
                node = node.accent();
            }
        }
        kui_core::widgets::button_with(
            &mut kui_core::Ui::wrap(c.core()),
            &text,
            &text,
            node,
            hint.as_deref(),
        );
    });
}

/// The rows a stock toggle reads off a `KuiSpec`, over
/// `widgets::toggle_spec` (`schema::TOGGLE_ROWS_JSX`): the button's access
/// rows and its state. Every other field is ignored, as `kui_button_with`
/// ignores them. The tooltip goes first, so an explicit description wins.
fn toggle(
    ptr: *mut KuiCtx,
    kind: kui_core::widgets::Toggle,
    text: KuiStr,
    spec: *const KuiSpec,
    payload: *mut KuiValue,
) -> u64 {
    guard(0, || {
        let value = if payload.is_null() {
            Value::Null
        } else {
            unsafe { Box::from_raw(payload) }.0
        };
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let text = kstr(text);
        let metrics = *c.core().metrics();
        let mut node = kui_core::widgets::toggle_spec(&metrics).on_click(value);
        let mut hint = None;
        if let Some(s) = unsafe { spec.as_ref() } {
            if let Some(h) = opt_str(s.tooltip) {
                node = node.apply_tooltip(&h);
                hint = Some(h);
            }
            if let Some(l) = opt_str(s.label) {
                node = node.label(l.as_ref());
            }
            if let Some(d) = opt_str(s.description) {
                node = node.description(d.as_ref());
            }
            node = node
                .disabled(s.disabled != 0)
                .checked(s.checked != 0)
                .mixed(s.mixed != 0);
        }
        kui_core::widgets::toggle_with(
            &mut kui_core::Ui::wrap(c.core()),
            kind,
            &text,
            &text,
            node,
            hint.as_deref(),
        )
        .0
    })
}

/// The stock checkbox: a box drawn from `spec`'s `checked` / `mixed`,
/// labelled `text` and keyed by it; a press posts `payload`
/// (consumed). Reads `checked`, `mixed`, `label`, `description`,
/// `tooltip` and `disabled` off `spec` (NULL = none of them). Its key.
#[unsafe(no_mangle)]
pub extern "C" fn kui_checkbox(
    ptr: *mut KuiCtx,
    text: KuiStr,
    spec: *const KuiSpec,
    payload: *mut KuiValue,
) -> u64 {
    toggle(
        ptr,
        kui_core::widgets::Toggle::Checkbox,
        text,
        spec,
        payload,
    )
}

/// The stock radio; see [`kui_checkbox`]. Declare radios between
/// [`kui_radio_group_open`] and [`kui_close`].
#[unsafe(no_mangle)]
pub extern "C" fn kui_radio(
    ptr: *mut KuiCtx,
    text: KuiStr,
    spec: *const KuiSpec,
    payload: *mut KuiValue,
) -> u64 {
    toggle(ptr, kui_core::widgets::Toggle::Radio, text, spec, payload)
}

/// The stock switch; see [`kui_checkbox`].
#[unsafe(no_mangle)]
pub extern "C" fn kui_switch(
    ptr: *mut KuiCtx,
    text: KuiStr,
    spec: *const KuiSpec,
    payload: *mut KuiValue,
) -> u64 {
    toggle(ptr, kui_core::widgets::Toggle::Switch, text, spec, payload)
}

/// Opens a radio group named `label`: `spec`'s box rows (NULL for a
/// column), the group's role and name, and the stock gap where `spec` has
/// none. Declare its radios, then [`kui_close`]. Returns its key.
#[unsafe(no_mangle)]
pub extern "C" fn kui_radio_group_open(
    ptr: *mut KuiCtx,
    label: KuiStr,
    spec: *const KuiSpec,
) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let label = kstr(label);
        let base = unsafe { spec.as_ref() }
            .map_or_else(NodeSpec::column, |s| spec_of(s, NONE, NONE, NONE, NONE));
        let metrics = *c.core().metrics();
        let node = kui_core::widgets::radio_group_open_spec(&metrics, &label, base);
        c.core().open_keyed(&label, node).0
    })
}

/// The stock slider, named and keyed by `label`. Reads the
/// value fields (`value_now` / `value_min` / `value_max` / `value_step` by
/// their `KUI_VALUE_*` bits, `value_text`), `on_change`, `width` /
/// `min_w` / `max_w` where set, `label` (a name other than the key),
/// `description`, `tooltip` and `disabled` off `spec`; every other field
/// is its look's. With `on_change` the core proposes values as
/// `{kind:"change", value, phase, tag}`. Its key.
#[unsafe(no_mangle)]
pub extern "C" fn kui_slider(ptr: *mut KuiCtx, label: KuiStr, spec: *const KuiSpec) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let label = kstr(label);
        let metrics = *c.core().metrics();
        let mut node = kui_core::widgets::slider_spec(&metrics);
        let mut hint = None;
        if let Some(s) = unsafe { spec.as_ref() } {
            if let Some(h) = opt_str(s.tooltip) {
                node = node.apply_tooltip(&h);
                hint = Some(h);
            }
            if let Some(l) = opt_str(s.label) {
                node = node.label(l.as_ref());
            }
            if let Some(d) = opt_str(s.description) {
                node = node.description(d.as_ref());
            }
            if s.width.tag != 0 {
                node = node.width(sizing_of(s.width));
            }
            if s.min_w != 0.0 {
                node = node.min_width(min_of(s.min_w));
            }
            if s.max_w > 0.0 {
                node = node.max_width(s.max_w);
            }
            if s.value_set & KUI_VALUE_NOW != 0 {
                node = node.value_now(s.value_now);
            }
            if s.value_set & KUI_VALUE_MIN != 0 {
                node = node.value_min(s.value_min);
            }
            if s.value_set & KUI_VALUE_MAX != 0 {
                node = node.value_max(s.value_max);
            }
            if s.value_set & KUI_VALUE_STEP != 0 {
                node = node.value_step(s.value_step);
            }
            if let Some(t) = opt_str(s.value_text) {
                node = node.value_text(t.into_owned());
            }
            if let Some(tag) = unsafe { s.on_change.as_ref() } {
                node = node.on_change(tag.0.clone());
            }
            node = node.disabled(s.disabled != 0);
        }
        kui_core::widgets::slider_with(
            &mut kui_core::Ui::wrap(c.core()),
            &label,
            node,
            hint.as_deref(),
        )
        .0
    })
}

/// An editable text node keyed by `label`, seeded with `initial` the
/// first time it is seen; the core keeps its buffer, caret and undo
/// history across frames. `flags` are `KUI_EDIT_MULTILINE`,
/// `KUI_EDIT_AUTOFOCUS` and `KUI_EDIT_WRAP`; `spec` (required) is the
/// box around it. Read the text with [`kui_edit_text`]; `changed` and
/// `submit` events carry the key. Returns the key, or 0 on failure.
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
            multiline: flags & KUI_EDIT_MULTILINE != 0,
            autofocus: flags & KUI_EDIT_AUTOFOCUS != 0,
            wrap: flags & KUI_EDIT_WRAP != 0,
            ..Default::default()
        };
        let spec = spec_of(sp, NONE, NONE, NONE, NONE);
        c.core()
            .text_edit(&kstr(label), &kstr(initial), &opts, spec)
            .0
    })
}
