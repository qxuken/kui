//! Frame building: begin, open, close, text, images, the measurement
//! queries, and finish plus the draw data a renderer reads. Flat and
//! non-panicking, like the `Core` builder it delegates to.

use super::*;

// ---------------------------------------------------------------------------
// Spec helpers

/// Fills a spec's `float_*` fields from a preset name — the same four names
/// (`"parent"`, `"viewport"`, `"below"`, `"above"`) the JSX and Lua `float`
/// props take, resolved by the same core function. Returns false and leaves
/// the spec alone for an unknown name.
///
/// The fields stay writable afterwards, so a preset is a starting point:
/// take `"below"` and set `float_dy` to change only the gap.
#[unsafe(no_mangle)]
pub extern "C" fn kui_spec_float_preset(spec: *mut KuiSpec, name: KuiStr) -> bool {
    guard(false, || {
        let (Some(s), Some(cfg)) = (unsafe { spec.as_mut() }, FloatConfig::preset(&kstr(name)))
        else {
            return false;
        };
        s.float_mode = match cfg.anchor {
            kui_core::FloatAnchor::Parent => KUI_FLOAT_PARENT,
            kui_core::FloatAnchor::Viewport => KUI_FLOAT_VIEWPORT,
        };
        s.float_anchor_x = align_code(cfg.anchor_point.0);
        s.float_anchor_y = align_code(cfg.anchor_point.1);
        s.float_self_x = align_code(cfg.self_point.0);
        s.float_self_y = align_code(cfg.self_point.1);
        s.float_dx = cfg.offset.x;
        s.float_dy = cfg.offset.y;
        s.float_fit = cfg.fit as u32;
        true
    })
}

// ---------------------------------------------------------------------------
// Frame building

#[unsafe(no_mangle)]
pub extern "C" fn kui_frame_begin(ptr: *mut KuiCtx, w: f32, h: f32, scale: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            // A frame that ended with nodes unclosed must not leak its
            // hints into the next one.
            c.open_tooltips.clear();
            c.core()
                .begin_frame(Size::new(w, h), if scale > 0.0 { scale } else { 1.0 });
        }
    });
}

/// `on_click` (nullable) is consumed.
#[unsafe(no_mangle)]
pub extern "C" fn kui_root(ptr: *mut KuiCtx, spec: *const KuiSpec) {
    guard((), || {
        if let (Some(c), Some(s)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) {
            c.core().configure_root(spec_of(s, NONE, NONE, NONE, NONE));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_open(ptr: *mut KuiCtx, spec: *const KuiSpec, on_click: *mut KuiValue) -> u64 {
    guard(0, || {
        let (Some(c), Some(s)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) else {
            return 0;
        };
        let key = c.core().open(spec_of(s, on_click, NONE, NONE, NONE));
        c.push_tooltip(key, s.tooltip);
        key.0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_open_keyed(
    ptr: *mut KuiCtx,
    label: KuiStr,
    spec: *const KuiSpec,
    on_click: *mut KuiValue,
) -> u64 {
    guard(0, || {
        let (Some(c), Some(s)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) else {
            return 0;
        };
        let key = c
            .core()
            .open_keyed(&kstr(label), spec_of(s, on_click, NONE, NONE, NONE));
        c.push_tooltip(key, s.tooltip);
        key.0
    })
}

/// An image node. Fit sizing takes the image's pixel size as logical px;
/// Fit height against a resolved width keeps the aspect. radius rounds it.
#[unsafe(no_mangle)]
pub extern "C" fn kui_image(ptr: *mut KuiCtx, id: u64, spec: *const KuiSpec) {
    guard((), || {
        if let (Some(c), Some(s)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) {
            let spec = spec_of(s, NONE, NONE, NONE, NONE);
            c.core().image_node(kui_core::ImageId::from_ffi(id), spec);
        }
    });
}

/// Like `kui_open_keyed`, but the node is draggable: press-drag emits
/// `{kind="drag", phase, x, y, dx, dy, tag}` events. `on_drag` (the tag,
/// nullable) and `on_click` (nullable) are consumed. A drag past the click
/// slop suppresses the click.
#[unsafe(no_mangle)]
pub extern "C" fn kui_open_draggable(
    ptr: *mut KuiCtx,
    label: KuiStr,
    spec: *const KuiSpec,
    on_click: *mut KuiValue,
    on_drag: *mut KuiValue,
) -> u64 {
    guard(0, || {
        let (Some(c), Some(s)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) else {
            return 0;
        };
        let spec = spec_of(s, on_click, NONE, NONE, NONE)
            .on_drag(take_msg(on_drag).unwrap_or(Value::Null));
        let key = c.core().open_keyed(&kstr(label), spec);
        c.push_tooltip(key, s.tooltip);
        key.0
    })
}

/// The general container: every message prop at once. NULL = absent (so a
/// NULL `on_drag` here does NOT make the node draggable, unlike
/// `kui_open_draggable`). A non-NULL `on_key` makes the node a key sink;
/// give it focus with `kui_set_key_focus` and presses arrive as
/// `{kind="key", code, ctrl, alt, shift, super, text, repeat, tag}`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_open_with(
    ptr: *mut KuiCtx,
    label: KuiStr,
    spec: *const KuiSpec,
    on_click: *mut KuiValue,
    on_drag: *mut KuiValue,
    on_key: *mut KuiValue,
    on_hover: *mut KuiValue,
) -> u64 {
    guard(0, || {
        let (Some(c), Some(s)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) else {
            for p in [on_click, on_drag, on_key, on_hover] {
                drop(take_msg(p));
            }
            return 0;
        };
        let key = c.core().open_keyed(
            &kstr(label),
            spec_of(s, on_click, on_drag, on_key, on_hover),
        );
        c.push_tooltip(key, s.tooltip);
        key.0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_close(ptr: *mut KuiCtx) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            // The tooltip prop's third effect, at the point the Lua and
            // Node lowerings apply it: the node's last child, while hovered.
            if let Some((key, hint)) = c.open_tooltips.pop().flatten()
                && c.core().is_hovered(key)
            {
                kui_core::widgets::tooltip(&mut kui_core::Ui::wrap(c.core()), &hint);
            }
            c.core().close();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_text(ptr: *mut KuiCtx, text: KuiStr, style: *const KuiTextStyle) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let style = unsafe { style.as_ref() }
                .map(text_style_of)
                .unwrap_or_default();
            c.core().text_node(&kstr(text), style);
        }
    });
}

/// Runs `f` over the core `Span`s a `KuiSpan` array describes; None for a
/// NULL or empty array.
fn with_spans<R>(
    spans: *const KuiSpan,
    span_count: usize,
    f: impl FnOnce(&[Span<'_>]) -> R,
) -> Option<R> {
    if spans.is_null() || span_count == 0 {
        return None;
    }
    let raw = unsafe { std::slice::from_raw_parts(spans, span_count) };
    let texts: Vec<std::borrow::Cow<'_, str>> = raw.iter().map(|s| kstr(s.text)).collect();
    let spans: Vec<Span<'_>> = raw
        .iter()
        .zip(texts.iter())
        .map(|(s, t)| {
            let mut span = Span::new(t.as_ref());
            if s.flags & 1 != 0 {
                span = span.bold();
            }
            if s.flags & 2 != 0 {
                span = span.italic();
            }
            if s.color != 0 {
                span = span.color(Color::hex(s.color));
            }
            span
        })
        .collect();
    Some(f(&spans))
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_rich_text(
    ptr: *mut KuiCtx,
    spans: *const KuiSpan,
    span_count: usize,
    base: *const KuiTextStyle,
) {
    guard((), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return;
        };
        let base = unsafe { base.as_ref() }
            .map(text_style_of)
            .unwrap_or_default();
        with_spans(spans, span_count, |spans| {
            c.core().rich_text_node(spans, base)
        });
    });
}

fn metrics_of(m: kui_core::TextMetrics) -> KuiTextMetrics {
    KuiTextMetrics {
        width: m.width,
        height: m.height,
        lines: m.lines,
        ..Default::default()
    }
}

/// Measures `text` in `style` the way layout would, without adding a node:
/// unwrapped with `max_w <= 0`, else wrapped to `max_w` logical px. Logical
/// px at the scale of the current or last frame (1 before any frame).
/// `wrap` / `max_lines` / `ellipsis` in the style apply. Returns false only
/// for a bad context or NULL `out`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_measure_text(
    ptr: *mut KuiCtx,
    text: KuiStr,
    style: *const KuiTextStyle,
    max_w: f32,
    out: *mut KuiTextMetrics,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let style = unsafe { style.as_ref() }
            .map(text_style_of)
            .unwrap_or_default();
        let max_w = (max_w > 0.0).then_some(max_w);
        write_out(
            out,
            metrics_of(c.core().measure_text(&kstr(text), &style, max_w)),
        )
    })
}

/// `kui_measure_text` for a rich-text paragraph.
#[unsafe(no_mangle)]
pub extern "C" fn kui_measure_rich_text(
    ptr: *mut KuiCtx,
    spans: *const KuiSpan,
    span_count: usize,
    base: *const KuiTextStyle,
    max_w: f32,
    out: *mut KuiTextMetrics,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let base = unsafe { base.as_ref() }
            .map(text_style_of)
            .unwrap_or_default();
        let max_w = (max_w > 0.0).then_some(max_w);
        let m = with_spans(spans, span_count, |spans| {
            c.core().measure_rich_text(spans, &base, max_w)
        });
        write_out(out, m.map(metrics_of).unwrap_or_default())
    })
}

/// The key a child labeled `label` of the currently open container would get.
#[unsafe(no_mangle)]
pub extern "C" fn kui_child_key(ptr: *mut KuiCtx, label: KuiStr) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        c.core().child_key(&kstr(label)).0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_is_hovered(ptr: *mut KuiCtx, key: u64) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().is_hovered(Key(key)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_is_pressed(ptr: *mut KuiCtx, key: u64) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().is_pressed(Key(key)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_frame_finish(ptr: *mut KuiCtx) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().finish_frame();
            // Raised by the frame itself, not by input: the resize a
            // changed viewport produced at kui_frame_begin, and hover
            // enter/leave from this frame changing what sits under a
            // still cursor.
            let pending = c.core().take_pending_events();
            c.events.extend(pending);
        }
    });
}

/// Draw data for the finished frame. Pointers are valid until the next
/// `kui_frame_begin` on this context. `KuiQuad` is layout-compatible with the
/// core quad (asserted below), so this is a cast, not a copy.
///
/// Returns false — writing nothing, and leaving `atlas_dirty` set so the
/// next call still reports it — for a bad context or an `out` whose `size`
/// this library cannot honour. It returned `void` before the size
/// handshake; a host that ignores the result still compiles.
#[unsafe(no_mangle)]
pub extern "C" fn kui_draw_data(ptr: *mut KuiCtx, out: *mut KuiDrawData) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        if !out_accepts(out) {
            return false;
        }
        let (dl, atlas) = c.core().output();
        let data = KuiDrawData {
            quads: dl.quads.as_ptr().cast(),
            quad_count: dl.quads.len(),
            viewport_w: dl.viewport.w,
            viewport_h: dl.viewport.h,
            scale: dl.scale,
            atlas_pixels: atlas.pixels.as_ptr(),
            atlas_size: atlas.size,
            atlas_dirty: atlas.dirty,
            atlas_epoch: atlas.epoch,
            ..Default::default()
        };
        atlas.dirty = false;
        write_out(out, data)
    })
}

const _: () = {
    // KuiQuad must mirror kui_core::Quad field-for-field for the cast above.
    assert!(std::mem::size_of::<KuiQuad>() == std::mem::size_of::<kui_core::Quad>());
};
