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

/// A box the registered WGSL `id` paints; see `Core::fragment_node`. It
/// has no intrinsic size, so `spec` must give it one. `params` may be null
/// when `count` is 0; more than sixteen are dropped with a warning.
#[unsafe(no_mangle)]
pub extern "C" fn kui_fragment(
    ptr: *mut KuiCtx,
    id: u64,
    params: *const f32,
    count: usize,
    spec: *const KuiSpec,
) {
    guard((), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return;
        };
        let p = fragment_params(params, count);
        let spec = fragment_spec(spec);
        c.core()
            .fragment_node(kui_core::FragmentId::from_ffi(id), p, spec);
    });
}

/// `kui_fragment` as a parent: its children paint over it. Balance with
/// `kui_close`. An empty `label` is the unkeyed form.
#[unsafe(no_mangle)]
pub extern "C" fn kui_fragment_open(
    ptr: *mut KuiCtx,
    label: KuiStr,
    id: u64,
    params: *const f32,
    count: usize,
    spec: *const KuiSpec,
) {
    guard((), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return;
        };
        let p = fragment_params(params, count);
        let spec = fragment_spec(spec);
        let id = kui_core::FragmentId::from_ffi(id);
        match opt_str(label) {
            Some(label) => c.core().open_fragment_keyed(&label, id, p, spec),
            None => c.core().open_fragment(id, p, spec),
        };
        // `kui_close` pops one tooltip slot per open node.
        c.open_tooltips.push(None);
    });
}

/// The `params` slice behind a possibly-null pointer.
fn fragment_params<'a>(params: *const f32, count: usize) -> &'a [f32] {
    if params.is_null() || count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(params, count) }
    }
}

/// A fragment's spec: the whole box vocabulary, or a bare column when the
/// host passed NULL.
fn fragment_spec(spec: *const KuiSpec) -> kui_core::NodeSpec {
    match unsafe { spec.as_ref() } {
        Some(s) => spec_of(s, NONE, NONE, NONE, NONE),
        None => kui_core::NodeSpec::column(),
    }
}

/// A round-capped stroke from (x0, y0) to (x1, y1); see `Core::line_node`.
/// `spec` may be NULL. `width <= 0` is 1; `color` 0 is the default
/// foreground, like a text style's.
#[unsafe(no_mangle)]
pub extern "C" fn kui_line(
    ptr: *mut KuiCtx,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    width: f32,
    color: u32,
    spec: *const KuiSpec,
) {
    let xy = [x0, y0, x1, y1];
    let none = KuiStr {
        ptr: std::ptr::null(),
        len: 0,
    };
    kui_polyline(ptr, none, xy.as_ptr(), 2, width, color, false, spec);
}

/// A stroke through `count` points at `xy` (x0, y0, x1, y1, ...): a
/// polyline, or with `curve` a smooth curve through them, flattened in the
/// core. `label` keys the node (empty = a key from the tree position), for
/// a stroke that transitions or exits. See `Core::line_node`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_polyline(
    ptr: *mut KuiCtx,
    label: KuiStr,
    xy: *const f32,
    count: usize,
    width: f32,
    color: u32,
    curve: bool,
    spec: *const KuiSpec,
) {
    guard((), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return;
        };
        if xy.is_null() || count < 2 {
            return;
        }
        let floats = unsafe { std::slice::from_raw_parts(xy, count * 2) };
        let points: Vec<kui_core::Vec2> = floats
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| kui_core::Vec2::new(p[0], p[1]))
            .collect();
        let spec = match unsafe { spec.as_ref() } {
            Some(s) => spec_of(s, NONE, NONE, NONE, NONE),
            None => kui_core::NodeSpec::column(),
        };
        let color = if color == 0 {
            kui_core::TextStyle::default().color
        } else {
            color_of(color)
        };
        let mut stroke = kui_core::Stroke::new(if width > 0.0 { width } else { 1.0 }, color);
        stroke.curve = curve;
        match opt_str(label) {
            Some(label) => c.core().line_node_keyed(&label, &points, stroke, spec),
            None => c.core().line_node(&points, stroke, spec),
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
/// `{kind="key", phase="down", code, ctrl, alt, shift, super, text, repeat,
/// tag}` — releases too, with `phase="up"`, when the spec sets `key_up`.
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
            if s.flags & 4 != 0 {
                span = span.underline();
            }
            if s.flags & 8 != 0 {
                span = span.strikethrough();
            }
            if s.color != 0 {
                span = span.color(Color::hex(s.color));
            }
            if s.bg != 0 {
                span = span.bg(Color::hex(s.bg));
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

/// Where a point (logical viewport px, as a click or drag event carries
/// it) lands in the text the node `key` drew: a byte offset into that text
/// — across the node's text runs in order, the way the access tree reads a
/// `line` — and the visual line. False for a key that drew no text, a bad
/// context or a NULL `out`. Answered from the frame that finished.
#[unsafe(no_mangle)]
pub extern "C" fn kui_text_hit(
    ptr: *mut KuiCtx,
    key: u64,
    x: f32,
    y: f32,
    out: *mut KuiTextHit,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(h) = c.core().text_hit(Key(key), Vec2::new(x, y)) else {
            return false;
        };
        write_out(
            out,
            KuiTextHit {
                line: h.line,
                byte: h.byte as u64,
                ..Default::default()
            },
        )
    })
}

/// The caret rect for byte offset `byte` in the text the node `key` drew:
/// logical viewport px, zero wide, one line tall. A byte past the text is
/// the end. False for a key that drew no text, a bad context or a NULL
/// `out`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_caret_rect(
    ptr: *mut KuiCtx,
    key: u64,
    byte: usize,
    out: *mut KuiCaretRect,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(r) = c.core().caret_rect(Key(key), byte) else {
            return false;
        };
        write_out(
            out,
            KuiCaretRect {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
                ..Default::default()
            },
        )
    })
}

/// Where the OS candidate window goes while a composition is under way:
/// the focused editor's caret, or a custom editor's `line` carrying
/// `caret`. False when nothing with a caret is focused. A host driving its
/// own window reads it after each frame and hands it to the platform.
#[unsafe(no_mangle)]
pub extern "C" fn kui_ime_rect(ptr: *mut KuiCtx, out: *mut KuiCaretRect) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(r) = c.core().ime_rect() else {
            return false;
        };
        write_out(
            out,
            KuiCaretRect {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
                ..Default::default()
            },
        )
    })
}

/// A terminal's screen as one node (backlog C20): `rows × cols` cells from
/// `cells` (fewer draw as blank), shaped once per character and placed at
/// `col × cell_w` ever after. `style` sizes the cells (`size`, `family` /
/// `font`, `line_height`); `spec` is the node's own (an `on_key` makes it
/// the sink, an `on_click` / `on_drag` carry `cell: {row, col}`), the
/// three payloads taken the way `kui_open_with` takes them; `label`
/// keys the node (empty = auto). `cursor_shape` is `KUI_CELL_CURSOR_*` or
/// 0 for none, at (`cursor_row`, `cursor_col`) in `cursor_color`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn kui_cells(
    ptr: *mut KuiCtx,
    label: KuiStr,
    rows: u32,
    cols: u32,
    cells: *const KuiCell,
    count: usize,
    style: *const KuiTextStyle,
    spec: *const KuiSpec,
    on_click: *mut KuiValue,
    on_drag: *mut KuiValue,
    on_key: *mut KuiValue,
    cursor_row: u32,
    cursor_col: u32,
    cursor_shape: u32,
    cursor_color: u32,
) {
    guard((), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            for p in [on_click, on_drag, on_key] {
                drop(take_msg(p));
            }
            return;
        };
        if rows == 0 || cols == 0 {
            for p in [on_click, on_drag, on_key] {
                drop(take_msg(p));
            }
            return;
        }
        let raw: &[KuiCell] = if cells.is_null() || count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(cells, count) }
        };
        let cells: Vec<kui_core::Cell> = raw
            .iter()
            .map(|k| kui_core::Cell {
                ch: char::from_u32(k.ch).unwrap_or(' '),
                fg: k.fg,
                bg: k.bg,
                flags: k.flags as u8,
            })
            .collect();
        let style = unsafe { style.as_ref() }
            .map(text_style_of)
            .unwrap_or_default();
        let spec = match unsafe { spec.as_ref() } {
            Some(s) => spec_of(s, on_click, on_drag, on_key, NONE),
            None => {
                for p in [on_click, on_drag, on_key] {
                    drop(take_msg(p));
                }
                kui_core::NodeSpec::default()
            }
        };
        let cursor =
            kui_core::CellCursor::from_index(cursor_shape.wrapping_sub(1) as usize).map(|shape| {
                (
                    cursor_row as usize,
                    cursor_col as usize,
                    shape,
                    color_of(cursor_color),
                )
            });
        let grid = kui_core::CellGrid {
            rows: rows as usize,
            cols: cols as usize,
            cells: &cells,
            style,
            cursor,
        };
        let label = kstr(label);
        if label.is_empty() {
            c.core().cells(&grid, spec);
        } else {
            c.core().cells_keyed(&label, &grid, spec);
        }
    });
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
            // `Ui::finish`, which is the Rust runner's: the extensions get
            // the last word before layout — every `ns/root` the host did
            // not declare, and the `unknown-slot` warning — and then the
            // frame finishes. The core is behind a raw pointer on the
            // context so the `Ui` can borrow it and the list at once.
            let core: &mut Core = unsafe { &mut *c.core };
            kui_core::Ui::with_filler(core, &mut c.extensions).finish();
            // Raised by the frame itself, not by input: the resize a
            // changed viewport produced at kui_frame_begin, and hover
            // enter/leave from this frame changing what sits under a
            // still cursor.
            let pending = c.core().take_pending_events();
            c.absorb(pending);
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
        // The fragment draws are transcribed rather than cast: the core's
        // `FragmentDraw` holds a `FragmentId`, which is a slotmap key, and
        // `KuiFragmentDraw` holds the `u64` a host can pass back. Kept on
        // the context so the pointer outlives this call.
        let fragments: Vec<KuiFragmentDraw> = {
            let (dl, _) = c.core().output();
            dl.fragments
                .iter()
                .map(|f| KuiFragmentDraw {
                    fragment: f.id.to_ffi(),
                    params: f.params,
                })
                .collect()
        };
        c.fragment_draws = fragments;
        let fragment_draws = c.fragment_draws.as_ptr();
        let fragment_count = c.fragment_draws.len();
        let (dl, atlas) = c.core().output();
        let data = KuiDrawData {
            quads: dl.quads.as_ptr().cast(),
            quad_count: dl.quads.len(),
            clips: dl.clips.as_ptr().cast(),
            clip_count: dl.clips.len(),
            viewport_w: dl.viewport.w,
            viewport_h: dl.viewport.h,
            scale: dl.scale,
            atlas_pixels: atlas.pixels.as_ptr(),
            atlas_size: atlas.size,
            atlas_dirty: atlas.dirty,
            atlas_epoch: atlas.epoch,
            fragments: fragment_draws,
            fragment_count,
            time: dl.time,
            ..Default::default()
        };
        atlas.dirty = false;
        write_out(out, data)
    })
}

const _: () = {
    // KuiQuad and KuiClip must mirror kui_core::Quad and kui_core::Clip
    // field-for-field for the casts above.
    assert!(std::mem::size_of::<KuiQuad>() == std::mem::size_of::<kui_core::Quad>());
    assert!(std::mem::size_of::<KuiClip>() == std::mem::size_of::<kui_core::Clip>());
};
