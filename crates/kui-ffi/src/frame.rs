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
            // Never crosses: a node-anchored float is the core's own
            // (a devtools tab's content).
            kui_core::FloatAnchor::Node(_) => KUI_FLOAT_PARENT,
        };
        s.float_anchor_x = align_code(cfg.anchor_point.0);
        s.float_anchor_y = align_code(cfg.anchor_point.1);
        s.float_self_x = align_code(cfg.self_point.0);
        s.float_self_y = align_code(cfg.self_point.1);
        s.float_dx = cfg.offset.x;
        s.float_dy = cfg.offset.y;
        s.float_fit = cfg.fit as u32;
        s.float_clip = cfg.clip as u32;
        true
    })
}

// ---------------------------------------------------------------------------
// Frame building

/// Starts a frame: `w` by `h` logical pixels at device scale `scale`
/// (a value of 0 or less reads as 1).
///
/// Everything declared since the last frame is forgotten; build the whole
/// tree again, then [`kui_frame_finish`]. Call [`kui_set_time`] first if
/// anything transitions.
#[unsafe(no_mangle)]
pub extern "C" fn kui_frame_begin(ptr: *mut KuiCtx, w: f32, h: f32, scale: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core()
                .begin_frame(Size::new(w, h), if scale > 0.0 { scale } else { 1.0 });
        }
    });
}

/// Configures the frame's root node from `spec`: its direction, padding,
/// gap, alignment and background. Everything else declared this frame is
/// its child. Call it first after [`kui_frame_begin`]; a NULL `spec` does
/// nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_root(ptr: *mut KuiCtx, spec: *const KuiSpec) {
    guard((), || {
        if let (Some(c), Some(s)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) {
            c.core().configure_root(spec_of(s, NONE, NONE, NONE, NONE));
        }
    });
}

/// Opens a box node built from `spec`; everything until the matching
/// [`kui_close`] is its child. Returns the node's key, or 0 for a bad
/// context or a NULL `spec`.
///
/// `on_click` (NULL for none) is consumed: a press and release on the node
/// emits it as the payload of a `click` event. The key is derived from the
/// node's position in the tree, so a node that must keep state across
/// frames while siblings come and go takes [`kui_open_keyed`].
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

/// [`kui_open`] with a stable identity: the key is derived from `label`
/// under the parent, so hover, focus, scroll offsets, edit buffers and
/// transitions follow the node while its siblings change. `on_click`
/// (NULL for none) is consumed. Returns the key, or 0 on failure.
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

/// Declares how many indexed rows the open node's virtual list has, built
/// or not (`rowCount`): what Select All inside a `selectable` virtual
/// list spans, since the rows the frame built are all the core can see.
/// Call it inside the list's container, after its `kui_open_*`. Does
/// nothing outside any node.
#[unsafe(no_mangle)]
pub extern "C" fn kui_row_count(ptr: *mut KuiCtx, rows: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().row_count(rows);
        }
    });
}

/// `kui_open` under a data index rather than a name: the key auto-keying
/// would have given the `i`th child, given to this node wherever it sits.
/// A virtualising list opens each row with its own row number, so a row
/// keeps its hover, focus, edit buffer and tweens as the built range slides
/// over it — and agrees with a list that builds every row.
#[unsafe(no_mangle)]
pub extern "C" fn kui_open_indexed(
    ptr: *mut KuiCtx,
    index: u64,
    spec: *const KuiSpec,
    on_click: *mut KuiValue,
) -> u64 {
    guard(0, || {
        let (Some(c), Some(s)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) else {
            return 0;
        };
        let key = c
            .core()
            .open_indexed(index, spec_of(s, on_click, NONE, NONE, NONE));
        c.push_tooltip(key, s.tooltip);
        key.0
    })
}

/// An image node. Fit sizing takes the image's pixel size as logical px;
/// Fit height against a resolved width keeps the aspect. radius rounds it.
#[unsafe(no_mangle)]
pub extern "C" fn kui_image(ptr: *mut KuiCtx, id: u64, spec: *const KuiSpec) {
    kui_image_with(ptr, id, 0, 0, spec);
}

/// [`kui_image`] with its two options: `sampling` is `KUI_SAMPLING_LINEAR`
/// (0, the default) or `KUI_SAMPLING_NEAREST`; `fit` is `KUI_FIT_FILL`
/// (0, the default), `KUI_FIT_CONTAIN` or `KUI_FIT_COVER`. An index past
/// the table reads as the default.
#[unsafe(no_mangle)]
pub extern "C" fn kui_image_with(
    ptr: *mut KuiCtx,
    id: u64,
    sampling: u32,
    fit: u32,
    spec: *const KuiSpec,
) {
    guard((), || {
        if let (Some(c), Some(s)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) {
            let spec = spec_of(s, NONE, NONE, NONE, NONE);
            let opts = kui_core::ImageOpts {
                sampling: kui_core::Sampling::ALL
                    .get(sampling as usize)
                    .copied()
                    .unwrap_or_default(),
                fit: kui_core::ImageFit::ALL
                    .get(fit as usize)
                    .copied()
                    .unwrap_or_default(),
            };
            c.core()
                .image_node_with(kui_core::ImageId::from_ffi(id), opts, spec);
        }
    });
}

/// The spec of a leaf placed like a stroke, with its three payloads
/// consumed whether or not there is a `spec` to read: a NULL one is a leaf
/// with no fill, which a stroked path still is, and its handlers hold.
fn leaf_spec(
    spec: *const KuiSpec,
    on_click: *mut KuiValue,
    on_drag: *mut KuiValue,
    on_hover: *mut KuiValue,
) -> kui_core::NodeSpec {
    match unsafe { spec.as_ref() } {
        Some(s) => spec_of(s, on_click, on_drag, NONE, on_hover),
        None => {
            let mut spec = kui_core::NodeSpec::column();
            if let Some(v) = take_msg(on_click) {
                spec = spec.on_click(v);
            }
            if let Some(v) = take_msg(on_drag) {
                spec = spec.on_drag(v);
            }
            if let Some(v) = take_msg(on_hover) {
                spec = spec.on_hover(v);
            }
            spec
        }
    }
}

/// A filled polygon through `count` points at `xy` (x0, y0, x1, y1, ...),
/// at most eight (more are dropped with a `polygon-points-truncated`
/// warning, fewer than three draw nothing), filled with `spec`'s `bg`.
/// Placed like a stroke: a float sized to its own bounding box, in the
/// parent's box space. `label` keys the node (empty for a key from the
/// tree position). The three payloads are consumed as [`kui_open_with`]
/// consumes them; a fill with one is hit by its outline. A NULL `spec` is
/// a polygon with no fill, so nothing is drawn.
#[unsafe(no_mangle)]
pub extern "C" fn kui_polygon(
    ptr: *mut KuiCtx,
    label: KuiStr,
    xy: *const f32,
    count: usize,
    spec: *const KuiSpec,
    on_click: *mut KuiValue,
    on_drag: *mut KuiValue,
    on_hover: *mut KuiValue,
) {
    guard((), || {
        // First, so the payloads are consumed on every way out.
        let spec = leaf_spec(spec, on_click, on_drag, on_hover);
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return;
        };
        if xy.is_null() || count < 3 {
            return;
        }
        let floats = unsafe { std::slice::from_raw_parts(xy, count * 2) };
        let points: Vec<kui_core::Vec2> = floats
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| kui_core::Vec2::new(p[0], p[1]))
            .collect();
        match opt_str(label) {
            Some(label) => c.core().polygon_node_keyed(&label, &points, spec),
            None => c.core().polygon_node(&points, spec),
        }
    });
}

/// A path — any outline — as `count` floats at `ops` in the flat op form
/// ([`kui_path_parse`] makes it from SVG path data): filled with `spec`'s
/// `bg` by `fill_rule` (`KUI_FILL_NONZERO` or `KUI_FILL_EVENODD`) and,
/// when `width` is positive, stroked `width` wide in `color` (0 for the
/// theme's foreground) over the fill; turned by `rotate` turns about
/// `pivot` (two floats in the path's coordinates, NULL for the centre of
/// its box) by the quad that draws it, so a path that only turns is
/// rasterized once — 0 and NULL for no turn. Placed like a stroke: a
/// float sized to its own bounding box, in the parent's box space. `label` keys the
/// node (empty for a key from the tree position). The three payloads are
/// consumed as [`kui_open_with`] consumes them; a path with one is hit
/// by its outline under the fill rule. A NULL `spec` is a path with no
/// fill, its payloads kept; ops that are not the flat form raise
/// `path-malformed` under the node's key and draw nothing.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn kui_path(
    ptr: *mut KuiCtx,
    label: KuiStr,
    ops: *const f32,
    count: usize,
    fill_rule: u32,
    width: f32,
    color: u32,
    rotate: f32,
    pivot: *const f32,
    spec: *const KuiSpec,
    on_click: *mut KuiValue,
    on_drag: *mut KuiValue,
    on_hover: *mut KuiValue,
) {
    guard((), || {
        // First, so the payloads are consumed on every way out.
        let spec = leaf_spec(spec, on_click, on_drag, on_hover);
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return;
        };
        if ops.is_null() || count == 0 {
            return;
        }
        let floats = unsafe { std::slice::from_raw_parts(ops, count) };
        let rule = kui_core::FillRule::from_index(fill_rule as usize);
        let stroke = (width > 0.0).then(|| {
            let color = if color == 0 {
                c.core().theme().fg
            } else {
                color_of(color)
            };
            kui_core::Stroke::new(width, color)
        });
        let turn = unsafe { turn_of(rotate, pivot) };
        match opt_str(label) {
            Some(label) => c
                .core()
                .path_flat_node_keyed(&label, floats, rule, stroke, turn, spec),
            None => c.core().path_flat_node(floats, rule, stroke, turn, spec),
        }
    });
}

/// A path's turn as C spells it: `rotate` in turns and `pivot`, two
/// floats in the path's own coordinates or NULL for the centre of its
/// box. No turn at all — the tight box, the binned mask — is `rotate` 0
/// with a NULL `pivot`, so a path that turns through 0 names its pivot.
///
/// # Safety
/// `pivot` is NULL or points at two floats.
unsafe fn turn_of(rotate: f32, pivot: *const f32) -> Option<kui_core::Turn> {
    let pivot = (!pivot.is_null()).then(|| {
        let p = unsafe { std::slice::from_raw_parts(pivot, 2) };
        kui_core::Vec2::new(p[0], p[1])
    });
    (rotate != 0.0 || pivot.is_some()).then_some(kui_core::Turn {
        turns: rotate,
        pivot,
    })
}

/// [`kui_path`] from SVG path data instead of the flat form: `d` goes
/// through the one parser every binding uses, and data that does not parse
/// raises `path-malformed` under the node's key and draws nothing — the
/// same as `<path d>` in JSX and `path { d = }` in Lua.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn kui_path_d(
    ptr: *mut KuiCtx,
    label: KuiStr,
    d: KuiStr,
    fill_rule: u32,
    width: f32,
    color: u32,
    rotate: f32,
    pivot: *const f32,
    spec: *const KuiSpec,
    on_click: *mut KuiValue,
    on_drag: *mut KuiValue,
    on_hover: *mut KuiValue,
) {
    guard((), || {
        // First, so the payloads are consumed on every way out.
        let spec = leaf_spec(spec, on_click, on_drag, on_hover);
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return;
        };
        let d = kstr(d);
        let turn = unsafe { turn_of(rotate, pivot) };
        let rule = kui_core::FillRule::from_index(fill_rule as usize);
        let stroke = (width > 0.0).then(|| {
            let color = if color == 0 {
                c.core().theme().fg
            } else {
                color_of(color)
            };
            kui_core::Stroke::new(width, color)
        });
        match opt_str(label) {
            Some(label) => c
                .core()
                .path_d_node_keyed(&label, &d, rule, stroke, turn, spec),
            None => c.core().path_d_node(&d, rule, stroke, turn, spec),
        }
    });
}

/// Parses SVG path data (`M L H V C S Q T A Z`, absolute or relative) into
/// the flat op form [`kui_path`] takes — a `KUI_PATH_*` code then its
/// operands, every coordinate absolute — through the one parser every
/// binding uses. Returns how many floats the form needs; they are written
/// to `out` when `cap` holds them all, and not at all otherwise, so a
/// host may call once with `cap` 0 to size a buffer. Returns 0 for data
/// that does not parse.
#[unsafe(no_mangle)]
pub extern "C" fn kui_path_parse(d: KuiStr, out: *mut f32, cap: usize) -> usize {
    guard(0, || {
        let Some(d) = opt_str(d) else {
            return 0;
        };
        let Ok(path) = kui_core::Path::parse(&d) else {
            return 0;
        };
        let floats = path.to_floats();
        if !out.is_null() && cap >= floats.len() {
            let dst = unsafe { std::slice::from_raw_parts_mut(out, floats.len()) };
            dst.copy_from_slice(&floats);
        }
        floats.len()
    })
}

/// A box painted by the WGSL fragment function registered as `id`
/// ([`kui_fragment_add`]). It has no intrinsic size, so `spec` must give
/// it one. `params` are `count` floats the function reads, NULL when
/// `count` is 0; more than sixteen are dropped with a warning.
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
    });
}

/// [`kui_fragment`] reading `image` through the shader's `kui_sample`:
/// an image handle from [`kui_image_add`], or 0 for none, which is
/// `kui_fragment`. `label` keys the node (empty for a key from the tree
/// position); a leaf.
#[unsafe(no_mangle)]
pub extern "C" fn kui_fragment_with(
    ptr: *mut KuiCtx,
    label: KuiStr,
    id: u64,
    image: u64,
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
        let frag = fragment_ref(id, image);
        match opt_str(label) {
            Some(label) => c.core().fragment_node_keyed(&label, frag, p, spec),
            None => c.core().fragment_node(frag, p, spec),
        };
    });
}

/// `kui_fragment_with` as a parent: its children paint over it. Balance
/// with `kui_close`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_fragment_open_with(
    ptr: *mut KuiCtx,
    label: KuiStr,
    id: u64,
    image: u64,
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
        let frag = fragment_ref(id, image);
        match opt_str(label) {
            Some(label) => c.core().open_fragment_keyed(&label, frag, p, spec),
            None => c.core().open_fragment(frag, p, spec),
        };
    });
}

/// The function and its image, 0 for none.
fn fragment_ref(id: u64, image: u64) -> kui_core::FragmentRef {
    kui_core::FragmentRef {
        id: kui_core::FragmentId::from_ffi(id),
        image: (image != 0).then(|| kui_core::ImageId::from_ffi(image)),
    }
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

/// A round-capped stroke from (x0, y0) to (x1, y1) in the parent's box
/// space, as a float sized to its bounding box. `spec` may be NULL.
/// `width <= 0` is 1; `color` 0 is the theme's foreground, like a text
/// style's.
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
    kui_polyline(
        ptr,
        none,
        xy.as_ptr(),
        2,
        width,
        color,
        false,
        spec,
        NONE,
        NONE,
        NONE,
    );
}

/// A stroke through `count` points at `xy` (x0, y0, x1, y1, ...): a
/// polyline, or with `curve` a smooth curve through them. `label` keys the
/// node (empty for a key from the tree position), for a stroke that
/// transitions or exits. The three payloads are consumed as
/// [`kui_open_with`] consumes them; a stroke with one is hit by its shape,
/// not its bounding box.
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
    on_click: *mut KuiValue,
    on_drag: *mut KuiValue,
    on_hover: *mut KuiValue,
) {
    guard((), || {
        // First, so the payloads are consumed on every way out.
        let spec = leaf_spec(spec, on_click, on_drag, on_hover);
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
        // A stroke with no colour of its own is the theme's foreground,
        // the way a text run with none is.
        let color = if color == 0 {
            c.core().theme().fg
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

/// [`kui_open_keyed`] for a draggable node: a press-drag emits
/// `{kind="drag", phase, x, y, dx, dy, tag}` events with `on_drag` as the
/// tag. `on_drag` and `on_click` (either NULL) are consumed. A drag past
/// the click slop suppresses the click.
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

/// The general keyed container: every event tag at once, each consumed,
/// NULL meaning absent (so a NULL `on_drag` here does not make the node
/// draggable, unlike [`kui_open_draggable`]). A non-NULL `on_key` makes
/// the node a key sink: give it focus with [`kui_set_key_focus`] and
/// presses arrive as `{kind="key", phase="down", code, ctrl, alt, shift,
/// super, text, repeat, tag}`, releases too (`phase="up"`) when the spec
/// sets `key_up`. `on_hover` tags pointer enter and leave. Returns the
/// key, or 0 on failure.
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

/// Closes the node the last `kui_open*` opened. Every open must be
/// closed before [`kui_frame_finish`].
#[unsafe(no_mangle)]
pub extern "C" fn kui_close(ptr: *mut KuiCtx) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            // The tooltip prop's third effect — the hint floating below the
            // node while hovered — is the core's, on `close` (`Core::hint`).
            c.core().close();
        }
    });
}

/// A paragraph of plain text in one style (NULL for the default style).
/// A text has no box of its own; wrap it in a [`kui_open`] for padding, a
/// background or a click.
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
            if s.flags & KUI_SPAN_BOLD != 0 {
                span = span.bold();
            }
            if s.flags & KUI_SPAN_ITALIC != 0 {
                span = span.italic();
            }
            if s.flags & KUI_SPAN_UNDERLINE != 0 {
                span = span.underline();
            }
            if s.flags & KUI_SPAN_STRIKETHROUGH != 0 {
                span = span.strikethrough();
            }
            if s.underline_color != 0 {
                span = span.underline_color(Color::hex(s.underline_color));
            }
            if s.underline_style != 0 {
                span =
                    span.underline_style(kui_core::UnderlineStyle::from_index(s.underline_style));
            }
            if s.color != 0 {
                span = span.color(Color::hex(s.color));
            }
            if s.bg != 0 {
                span = span.bg(Color::hex(s.bg));
            }
            if s.bg_radius > 0.0 {
                span = span.bg_radius(s.bg_radius);
            }
            span
        })
        .collect();
    Some(f(&spans))
}

/// A paragraph of `span_count` styled runs from `spans`, set in `base`
/// (NULL for the default style) where a span says nothing. A NULL or
/// empty array draws nothing.
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

/// A terminal's screen as one node: `rows` by `cols` cells from `cells`
/// (fewer draw as blank), shaped once per character and placed on a fixed
/// grid. `style` sizes the cells (`size`, `family` / `font`,
/// `line_height`); `spec` is the node's own (an `on_key` makes it the key
/// sink, an `on_click` / `on_drag` carry `cell: {row, col}`), the three
/// payloads consumed as [`kui_open_with`] consumes them; `label` keys the
/// node (empty for auto). `cursor_shape` is `KUI_CELL_CURSOR_*` or 0 for
/// none, drawn at (`cursor_row`, `cursor_col`) in `cursor_color`.
/// `origin_line` is the absolute line row 0 is, so a selection keeps its
/// ends across a scroll; 0 says nothing.
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
    // `origin_line`: the absolute line row 0 is; 0 says nothing.
    origin_line: u64,
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
                ul: k.ul,
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
            origin_line,
        };
        let label = kstr(label);
        if label.is_empty() {
            c.core().cells(&grid, spec);
        } else {
            c.core().cells_keyed(&label, &grid, spec);
        }
    });
}

/// [`kui_measure_text`] for a rich-text paragraph.
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

/// Whether the pointer is over the node `key`, as of the last input. Only
/// a node that is hover-tracked (a click or hover tag, `hoverable`,
/// `hover_bg`, a tooltip or a cursor) is ever hovered.
#[unsafe(no_mangle)]
pub extern "C" fn kui_is_hovered(ptr: *mut KuiCtx, key: u64) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().is_hovered(Key(key)))
    })
}

/// Whether files dragged in from the OS are over `key`, for
/// drop-dependent layout; the colour alone is `KuiSpec.drop_bg`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_is_drop_target(ptr: *mut KuiCtx, key: u64) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().is_drop_target(Key(key)))
    })
}

/// The drop zone the dragged files are over, or 0: what a driver answers
/// the OS with after every `kui_input_drag_files` (a copy cursor over a
/// zone, not-allowed elsewhere, and a release off every zone refused).
#[unsafe(no_mangle)]
pub extern "C" fn kui_drop_target(ptr: *mut KuiCtx) -> u64 {
    guard(0, || {
        unsafe { ctx(ptr) }
            .and_then(|c| c.core().drop_target())
            .map_or(0, |k| k.0)
    })
}

/// Whether a primary press that started on the node `key` is still held.
#[unsafe(no_mangle)]
pub extern "C" fn kui_is_pressed(ptr: *mut KuiCtx, key: u64) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().is_pressed(Key(key)))
    })
}

/// Finishes the frame: lets loaded extensions fill what the view did not,
/// lays the tree out and paints it. After it, [`kui_draw_data`] has the
/// frame and [`kui_poll_event`] has anything the frame itself produced (a
/// `resize`, a hover change under a still pointer, a `layout`).
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
/// The finished frame's draw list, into `out` (start from
/// `KUI_DRAW_DATA_INIT`). The pointers are valid until the next
/// [`kui_frame_begin`] on this context; the quads are the core's own
/// array, not a copy.
///
/// Reading it clears the atlas's dirty flag, so upload the atlas when
/// `atlas_dirty` is set or `atlas_epoch` changed. Returns false, writing
/// nothing and leaving the flag set, for a bad context or a `size` this
/// library cannot honour.
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
                .map(|f| {
                    let (image_source, image_texture) = match f.image {
                        kui_core::FragmentImage::None => (KUI_FRAGMENT_IMAGE_NONE, 0),
                        kui_core::FragmentImage::Atlas(_) => (KUI_FRAGMENT_IMAGE_ATLAS, 0),
                        kui_core::FragmentImage::Texture { index, .. } => {
                            (KUI_FRAGMENT_IMAGE_TEXTURE, index)
                        }
                    };
                    KuiFragmentDraw {
                        fragment: f.id.to_ffi(),
                        params: f.params,
                        image_source,
                        image_texture,
                        image_uv: f.image.uv(),
                    }
                })
                .collect()
        };
        c.fragment_draws = fragments;
        let fragment_draws = c.fragment_draws.as_ptr();
        let fragment_count = c.fragment_draws.len();
        // The texture draws likewise: `TextureDraw` holds an `ImageId`,
        // and the pixels' revision and size ride from the parallel list.
        let textures: Vec<KuiTextureDraw> = {
            let (dl, _) = c.core().output();
            dl.textures
                .iter()
                .zip(&dl.texture_pixels)
                .map(|(t, px)| KuiTextureDraw {
                    image: t.id.to_ffi(),
                    rev: px.rev,
                    width: px.width,
                    height: px.height,
                    uv: t.uv,
                })
                .collect()
        };
        c.texture_draws = textures;
        let texture_draws = c.texture_draws.as_ptr();
        let texture_count = c.texture_draws.len();
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
            textures: texture_draws,
            texture_count,
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
