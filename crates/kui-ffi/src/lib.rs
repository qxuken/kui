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

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use kui_core::{
    Align, Color, Core, Edges, EditKey, EditOptions, FloatConfig, InputEvent, Key, Mods, NodeSpec,
    Rect, Size, Sizing, Span, TextStyle, UiEvent, Value, Vec2, WindowButton, WindowCommand,
    WindowEnv,
};

// ---------------------------------------------------------------------------
// Opaque + repr(C) types

/// Opaque: a `Core` plus the pending event queue. The core is either owned
/// (standalone contexts from `kui_ctx_new`) or borrowed from the windowed
/// runner for the duration of a view callback — behind a pointer either way,
/// so every entry point works identically on both.
pub struct KuiCtx {
    core: *mut Core,
    /// Keep-alive for standalone contexts; never read directly.
    _owned: Option<Box<Core>>,
    events: Vec<UiEvent>,
    /// Payload most recently handed out by kui_poll_event; freed on the next
    /// poll (or context free) so C never manages event payload lifetime.
    last_payload: Option<Box<KuiValue>>,
    /// Text most recently handed out by kui_edit_text; freed on the next call.
    last_edit_text: Option<String>,
}

impl KuiCtx {
    fn core(&mut self) -> &mut Core {
        unsafe { &mut *self.core }
    }
}

/// Opaque dynamic value (event payloads).
pub struct KuiValue(Value);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiStr {
    pub ptr: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiSizing {
    /// 0 = fit, 1 = grow(value), 2 = fixed(value px), 3 = percent(value 0..1)
    pub tag: u32,
    pub value: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiSpec {
    pub width: KuiSizing,
    pub height: KuiSizing,
    /// Clamps applied after sizing resolves; 0 for max means unconstrained.
    pub min_w: f32,
    pub max_w: f32,
    pub min_h: f32,
    pub max_h: f32,
    /// 0 = column, 1 = row
    pub dir: u32,
    pub pad_l: f32,
    pub pad_r: f32,
    pub pad_t: f32,
    pub pad_b: f32,
    pub gap: f32,
    /// 0 = start, 1 = center, 2 = end
    pub main_align: u32,
    pub cross_align: u32,
    /// 0xRRGGBBAA; 0 = transparent
    pub bg: u32,
    pub border_color: u32,
    pub border_w: f32,
    pub radius: f32,
    /// bit 0 = clip, bit 1 = scroll_x, bit 2 = scroll_y
    pub overflow: u32,
    /// 0 = in flow, 1 = float anchored to parent, 2 = float anchored to viewport
    pub float_mode: u32,
    /// Attach points as align values (0 start, 1 center, 2 end).
    pub float_anchor_x: u32,
    pub float_anchor_y: u32,
    pub float_self_x: u32,
    pub float_self_y: u32,
    pub float_dx: f32,
    pub float_dy: f32,
    /// Non-zero: flip across the anchor / clamp to stay in the viewport.
    pub float_fit: u32,
    /// Non-zero: hover-track this node (kui_is_hovered) without a payload.
    pub hoverable: u32,
    /// Window-chrome role: 0 = none, 1 = drag, 2 = close button,
    /// 3 = minimize button, 4 = maximize button. Chrome nodes emit window
    /// commands (kui_take_window_commands), never events.
    pub window_role: u32,
    /// Positive: ease sizing/colors/radius changes over this many ms (the
    /// node needs a stable key, i.e. kui_open_keyed). Needs kui_set_time.
    pub transition_ms: f32,
    /// KUI_EASE_* curve for `transition_ms`.
    pub easing: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiTextStyle {
    pub size: f32,
    /// <= 0 picks the default (size * 1.35).
    pub line_height: f32,
    /// 0xRRGGBBAA; 0 = default foreground
    pub color: u32,
    /// KUI_FONT_SANS (0, default) / KUI_FONT_SERIF / KUI_FONT_MONO.
    pub family: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiSpan {
    pub text: KuiStr,
    /// 0xRRGGBBAA; 0 = inherit the paragraph color
    pub color: u32,
    /// bit 0 = bold, bit 1 = italic
    pub flags: u32,
}

#[repr(C)]
pub struct KuiEvent {
    pub origin: u16,
    pub key: u64,
    /// Borrowed until the next `kui_poll_event`/`kui_ctx_free`; NULL if none.
    pub payload: *const KuiValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiQuad {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
    pub border_color: [f32; 4],
    pub radius: f32,
    pub border_w: f32,
    /// 0 = solid, 1 = mask glyph, 2 = color glyph
    pub kind: u32,
    pub uv: [u32; 4],
    /// Clip rect (physical px): x, y, w, h. Pixels outside are transparent.
    pub clip: [f32; 4],
}

#[repr(C)]
pub struct KuiDrawData {
    pub quads: *const KuiQuad,
    pub quad_count: usize,
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub scale: f32,
    /// RGBA, atlas_size * atlas_size * 4 bytes.
    pub atlas_pixels: *const u8,
    pub atlas_size: u32,
    /// Re-upload the atlas texture when either of these changes/sets.
    pub atlas_dirty: bool,
    pub atlas_epoch: u64,
}

// ---------------------------------------------------------------------------
// Conversion helpers

fn kstr<'a>(s: KuiStr) -> std::borrow::Cow<'a, str> {
    if s.ptr.is_null() || s.len == 0 {
        return "".into();
    }
    let bytes = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
    String::from_utf8_lossy(bytes)
}

fn color_of(hex: u32) -> Color {
    if hex == 0 {
        Color::TRANSPARENT
    } else {
        Color::hex(hex)
    }
}

fn sizing_of(s: KuiSizing) -> Sizing {
    match s.tag {
        1 => Sizing::Grow(s.value),
        2 => Sizing::Fixed(s.value),
        3 => Sizing::Percent(s.value),
        _ => Sizing::Fit,
    }
}

fn align_of(a: u32) -> Align {
    match a {
        1 => Align::Center,
        2 => Align::End,
        _ => Align::Start,
    }
}

/// Null-able, consumed message payloads (`KuiValue*` owned by the caller
/// until passed here).
const NONE: *mut KuiValue = std::ptr::null_mut();

fn take_msg(p: *mut KuiValue) -> Option<Value> {
    // Consumes the value.
    (!p.is_null()).then(|| unsafe { Box::from_raw(p) }.0)
}

fn spec_of(
    s: &KuiSpec,
    on_click: *mut KuiValue,
    on_drag: *mut KuiValue,
    on_key: *mut KuiValue,
) -> NodeSpec {
    let mut spec = if s.dir == 1 {
        NodeSpec::row()
    } else {
        NodeSpec::column()
    };
    spec = spec
        .width(sizing_of(s.width))
        .height(sizing_of(s.height))
        .min_width(s.min_w.max(0.0))
        .max_width(if s.max_w > 0.0 {
            s.max_w
        } else {
            f32::INFINITY
        })
        .min_height(s.min_h.max(0.0))
        .max_height(if s.max_h > 0.0 {
            s.max_h
        } else {
            f32::INFINITY
        })
        .padding(Edges {
            l: s.pad_l,
            r: s.pad_r,
            t: s.pad_t,
            b: s.pad_b,
        })
        .gap(s.gap)
        .main_align(align_of(s.main_align))
        .cross_align(align_of(s.cross_align))
        .bg(color_of(s.bg))
        .radius(s.radius);
    if s.border_w > 0.0 {
        spec = spec.border(s.border_w, color_of(s.border_color));
    }
    if s.overflow & 1 != 0 {
        spec = spec.clip();
    }
    if s.overflow & 2 != 0 {
        spec = spec.scroll_x();
    }
    if s.overflow & 4 != 0 {
        spec = spec.scroll_y();
    }
    if s.float_mode != 0 {
        let mut cfg = if s.float_mode == 2 {
            FloatConfig::viewport()
        } else {
            FloatConfig::parent()
        }
        .at(align_of(s.float_anchor_x), align_of(s.float_anchor_y))
        .self_at(align_of(s.float_self_x), align_of(s.float_self_y))
        .offset(s.float_dx, s.float_dy);
        if s.float_fit != 0 {
            cfg = cfg.fit();
        }
        spec = spec.float(cfg);
    }
    if s.hoverable != 0 {
        spec = spec.hoverable();
    }
    match s.window_role {
        1 => spec = spec.window_drag(),
        2 => spec = spec.window_button(WindowButton::Close),
        3 => spec = spec.window_button(WindowButton::Minimize),
        4 => spec = spec.window_button(WindowButton::Maximize),
        _ => {}
    }
    if s.transition_ms > 0.0 {
        spec = spec.transition(s.transition_ms);
    }
    if s.easing != 0 {
        spec = spec.easing(kui_core::schema::easing_idx(s.easing as usize));
    }
    if let Some(v) = take_msg(on_click) {
        spec = spec.on_click(v);
    }
    if let Some(v) = take_msg(on_drag) {
        spec = spec.on_drag(v);
    }
    if let Some(v) = take_msg(on_key) {
        spec = spec.on_key(v);
    }
    spec
}

fn text_style_of(s: &KuiTextStyle) -> TextStyle {
    let mut style = TextStyle::new(if s.size > 0.0 { s.size } else { 16.0 });
    if s.line_height > 0.0 {
        style = style.line_height(s.line_height);
    }
    if s.color != 0 {
        style = style.color(Color::hex(s.color));
    }
    style.family(match s.family {
        1 => kui_core::FontFamily::Serif,
        2 => kui_core::FontFamily::Mono,
        _ => kui_core::FontFamily::Sans,
    })
}

fn guard<T>(default: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(default)
}

unsafe fn ctx<'a>(ptr: *mut KuiCtx) -> Option<&'a mut KuiCtx> {
    unsafe { ptr.as_mut() }
}

// ---------------------------------------------------------------------------
// Context lifecycle + input

#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_new() -> *mut KuiCtx {
    guard(std::ptr::null_mut(), || {
        let mut owned = Box::new(Core::new());
        let core: *mut Core = &mut *owned;
        Box::into_raw(Box::new(KuiCtx {
            core,
            _owned: Some(owned),
            events: Vec::new(),
            last_payload: None,
            last_edit_text: None,
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_free(ptr: *mut KuiCtx) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(ptr) });
    }
}

fn push_input(ptr: *mut KuiCtx, ev: InputEvent) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let evs = c.core().handle_input(ev);
            c.events.extend(evs);
        }
    })
}

/// Cursor position in logical coordinates.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_cursor(ptr: *mut KuiCtx, x: f32, y: f32) {
    push_input(ptr, InputEvent::CursorMoved(Vec2::new(x, y)));
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_input_cursor_left(ptr: *mut KuiCtx) {
    push_input(ptr, InputEvent::CursorLeft);
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_input_mouse(ptr: *mut KuiCtx, down: bool, clicks: u32) {
    push_input(
        ptr,
        if down {
            InputEvent::MouseDown(clicks.clamp(1, u8::MAX as u32) as u8)
        } else {
            InputEvent::MouseUp
        },
    );
}

/// Wheel/trackpad delta in logical px (positive y = scroll up).
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_scroll(ptr: *mut KuiCtx, dx: f32, dy: f32) {
    push_input(ptr, InputEvent::Scroll(Vec2::new(dx, dy)));
}

/// Committed text input (typing, paste); routed to the focused editor.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_text(ptr: *mut KuiCtx, text: KuiStr) {
    guard((), || {
        let text = kstr(text).into_owned();
        push_input(ptr, InputEvent::Text(text));
    });
}

fn edit_key_of(key: u32) -> Option<EditKey> {
    Some(match key {
        0 => EditKey::Left,
        1 => EditKey::Right,
        2 => EditKey::Up,
        3 => EditKey::Down,
        4 => EditKey::Home,
        5 => EditKey::End,
        6 => EditKey::PageUp,
        7 => EditKey::PageDown,
        8 => EditKey::Backspace,
        9 => EditKey::Delete,
        10 => EditKey::Enter,
        11 => EditKey::Tab,
        12 => EditKey::SelectAll,
        13 => EditKey::Escape,
        14 => EditKey::Undo,
        15 => EditKey::Redo,
        _ => return None,
    })
}

/// Physical modifier state changed (KUI_KMOD_* bits). The host polls a
/// `{kind="modifiers", shift, ctrl, alt, super}` event when it differs from
/// the last report.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_modifiers(ptr: *mut KuiCtx, mods: u32) {
    push_input(
        ptr,
        InputEvent::Modifiers(kui_core::KeyMods {
            shift: mods & 1 != 0,
            ctrl: mods & 2 != 0,
            alt: mods & 4 != 0,
            super_key: mods & 8 != 0,
        }),
    );
}

/// Editing key with modifier bits (1 = shift, 2 = word/alt, 4 = doc/primary).
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_key(ptr: *mut KuiCtx, key: u32, mods: u32) {
    if let Some(k) = edit_key_of(key) {
        let mods = Mods {
            shift: mods & 1 != 0,
            word: mods & 2 != 0,
            doc: mods & 4 != 0,
        };
        push_input(ptr, InputEvent::Key(k, mods));
    }
}

/// Pops the next pending UI event. The payload pointer stays valid until the
/// next poll call on the same context (or context free).
#[unsafe(no_mangle)]
pub extern "C" fn kui_poll_event(ptr: *mut KuiCtx, out: *mut KuiEvent) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        // Drop the previously handed-out payload.
        c.last_payload = None;
        if c.events.is_empty() || out.is_null() {
            return false;
        }
        let ev = c.events.remove(0);
        let payload = Box::new(KuiValue(ev.payload));
        let payload_ptr: *const KuiValue = &*payload;
        c.last_payload = Some(payload);
        unsafe {
            *out = KuiEvent {
                origin: ev.origin.0,
                key: ev.key.0,
                payload: payload_ptr,
            };
        }
        true
    })
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

/// Window chrome facts for views to read (widgets::titlebar adapts to
/// them). `controls_w/h > 0` describe the keep-out rect of controls the OS
/// draws over the content (macOS traffic lights), anchored top-left.
#[unsafe(no_mangle)]
pub extern "C" fn kui_env_set_window(
    ptr: *mut KuiCtx,
    custom_chrome: bool,
    maximized: bool,
    fullscreen: bool,
    controls_w: f32,
    controls_h: f32,
) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().env.window = WindowEnv {
                custom_chrome,
                maximized,
                fullscreen,
                native_controls: (controls_w > 0.0 && controls_h > 0.0)
                    .then(|| Rect::new(0.0, 0.0, controls_w, controls_h)),
            };
        }
    });
}

/// Drains window intents produced by chrome nodes into `out` (each entry:
/// 1 = start drag, 2 = close, 3 = minimize, 4 = toggle maximize); returns
/// how many were written. Call after each input until it returns 0, and
/// apply them to the real window.
#[unsafe(no_mangle)]
pub extern "C" fn kui_take_window_commands(ptr: *mut KuiCtx, out: *mut u32, cap: usize) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if out.is_null() || cap == 0 {
            return 0;
        }
        let cmds = c.core().take_window_commands();
        let n = cmds.len().min(cap);
        for (i, cmd) in cmds.into_iter().take(n).enumerate() {
            let code = match cmd {
                WindowCommand::StartDrag => 1,
                WindowCommand::Close => 2,
                WindowCommand::Minimize => 3,
                WindowCommand::ToggleMaximize => 4,
            };
            unsafe { out.add(i).write(code) };
        }
        n
    })
}

/// Declares this frame's window title (cleared each kui_frame_begin).
#[unsafe(no_mangle)]
pub extern "C" fn kui_window_title(ptr: *mut KuiCtx, title: KuiStr) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let title = kstr(title).into_owned();
            c.core().set_window_title(&title);
        }
    });
}

/// The title declared this frame, if any — for hosts driving their own
/// window: diff and apply after kui_frame_finish. The view is valid until
/// the next kui_frame_begin.
#[unsafe(no_mangle)]
pub extern "C" fn kui_window_title_get(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
    guard(false, || {
        let (Some(c), Some(out)) = (unsafe { ctx(ptr) }, unsafe { out.as_mut() }) else {
            return false;
        };
        let Some(t) = c.core().window_title() else {
            return false;
        };
        *out = KuiStr {
            ptr: t.as_ptr(),
            len: t.len(),
        };
        true
    })
}

// ---------------------------------------------------------------------------
// Frame building

#[unsafe(no_mangle)]
pub extern "C" fn kui_frame_begin(ptr: *mut KuiCtx, w: f32, h: f32, scale: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
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
            c.core().configure_root(spec_of(s, NONE, NONE, NONE));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_open(ptr: *mut KuiCtx, spec: *const KuiSpec, on_click: *mut KuiValue) -> u64 {
    guard(0, || {
        let (Some(c), Some(s)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) else {
            return 0;
        };
        c.core().open(spec_of(s, on_click, NONE, NONE)).0
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
        c.core()
            .open_keyed(&kstr(label), spec_of(s, on_click, NONE, NONE))
            .0
    })
}

/// In-progress IME composition, shown at the focused editor's caret.
/// Empty text clears it; the commit arrives via `kui_input_text`.
/// `cursor_start`/`cursor_end` are byte offsets into `text`, or
/// `UINT32_MAX` for none.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_preedit(
    ptr: *mut KuiCtx,
    text: KuiStr,
    cursor_start: u32,
    cursor_end: u32,
) {
    guard((), || {
        let text = kstr(text).into_owned();
        let cursor =
            (cursor_start != u32::MAX).then_some((cursor_start as usize, cursor_end as usize));
        push_input(ptr, InputEvent::Preedit(text, cursor));
    });
}

/// Registers a w×h RGBA image (pixels copied); returns its handle, 0 on
/// failure. Draw it with `kui_image`; free it with `kui_image_remove`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_image_add(ptr: *mut KuiCtx, w: u32, h: u32, rgba: *const u8) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if rgba.is_null() || w == 0 || h == 0 {
            return 0;
        }
        let data = unsafe { std::slice::from_raw_parts(rgba, (w * h * 4) as usize) }.to_vec();
        c.core().resources.add_image(w, h, data).to_ffi()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_image_remove(ptr: *mut KuiCtx, id: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().remove_image(kui_core::ImageId::from_ffi(id));
        }
    });
}

/// An image node. Fit sizing takes the image's pixel size as logical px;
/// Fit height against a resolved width keeps the aspect. radius rounds it.
#[unsafe(no_mangle)]
pub extern "C" fn kui_image(ptr: *mut KuiCtx, id: u64, spec: *const KuiSpec) {
    guard((), || {
        if let (Some(c), Some(s)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) {
            let spec = spec_of(s, NONE, NONE, NONE);
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
        let spec =
            spec_of(s, on_click, NONE, NONE).on_drag(take_msg(on_drag).unwrap_or(Value::Null));
        c.core().open_keyed(&kstr(label), spec).0
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
) -> u64 {
    guard(0, || {
        let (Some(c), Some(s)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) else {
            return 0;
        };
        c.core()
            .open_keyed(&kstr(label), spec_of(s, on_click, on_drag, on_key))
            .0
    })
}

/// Routes the keyboard at a key-sink node (one opened with `on_key`) for
/// this frame; 0 clears. Declare it every frame you want it, like the
/// window title. A focused editor still wins.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_key_focus(ptr: *mut KuiCtx, key: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_key_focus((key != 0).then_some(Key(key)));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_close(ptr: *mut KuiCtx) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
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
        if spans.is_null() || span_count == 0 {
            return;
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
        let base = unsafe { base.as_ref() }
            .map(text_style_of)
            .unwrap_or_default();
        c.core().rich_text_node(&spans, base);
    });
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
        let key = c.core().child_key(&label);
        let bg = if c.core().is_pressed(key) {
            Color::rgb8(0x2f, 0x54, 0xc4)
        } else if c.core().is_hovered(key) {
            Color::rgb8(0x47, 0x6c, 0xe0)
        } else {
            Color::rgb8(0x3b, 0x5b, 0xd4)
        };
        c.core().open_keyed(
            &label,
            NodeSpec::row()
                .pad_xy(14.0, 8.0)
                .bg(bg)
                .radius(6.0)
                .center()
                .on_click(value),
        );
        c.core()
            .text_node(&label, TextStyle::new(15.0).color(Color::WHITE));
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
        let spec = spec_of(sp, NONE, NONE, NONE);
        c.core()
            .text_edit(&kstr(label), &kstr(initial), &opts, spec)
            .0
    })
}

/// Current text of an editor. The returned view is valid until the next
/// kui_edit_text call (or context free).
#[unsafe(no_mangle)]
pub extern "C" fn kui_edit_text(ptr: *mut KuiCtx, key: u64, out: *mut KuiStr) -> bool {
    guard(false, || {
        let (Some(c), Some(out)) = (unsafe { ctx(ptr) }, unsafe { out.as_mut() }) else {
            return false;
        };
        let Some(text) = c.core().edit_text(Key(key)) else {
            return false;
        };
        c.last_edit_text = Some(text);
        let s = c.last_edit_text.as_ref().unwrap();
        *out = KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        };
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_edit_set_text(ptr: *mut KuiCtx, key: u64, text: KuiStr) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let text = kstr(text).into_owned();
            c.core().set_edit_text(Key(key), &text);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_is_focused(ptr: *mut KuiCtx, key: u64) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().is_focused(Key(key)))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_frame_finish(ptr: *mut KuiCtx) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().finish_frame();
        }
    });
}

/// Draw data for the finished frame. Pointers are valid until the next
/// `kui_frame_begin` on this context. `KuiQuad` is layout-compatible with the
/// core quad (asserted below), so this is a cast, not a copy.
#[unsafe(no_mangle)]
pub extern "C" fn kui_draw_data(ptr: *mut KuiCtx, out: *mut KuiDrawData) {
    guard((), || {
        let (Some(c), Some(out)) = (unsafe { ctx(ptr) }, unsafe { out.as_mut() }) else {
            return;
        };
        let (dl, atlas) = c.core().output();
        *out = KuiDrawData {
            quads: dl.quads.as_ptr().cast(),
            quad_count: dl.quads.len(),
            viewport_w: dl.viewport.w,
            viewport_h: dl.viewport.h,
            scale: dl.scale,
            atlas_pixels: atlas.pixels.as_ptr(),
            atlas_size: atlas.size,
            atlas_dirty: atlas.dirty,
            atlas_epoch: atlas.epoch,
        };
        atlas.dirty = false;
    });
}

const _: () = {
    // KuiQuad must mirror kui_core::Quad field-for-field for the cast above.
    assert!(std::mem::size_of::<KuiQuad>() == std::mem::size_of::<kui_core::Quad>());
};

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
        let mut shim = KuiCtx {
            core: ui.core() as *mut Core,
            _owned: None,
            events: Vec::new(),
            last_payload: None,
            last_edit_text: None,
        };
        (self.view)(self.user, &mut shim);
    }

    fn on_event(&mut self, ev: kui::UiEvent) {
        let Some(cb) = self.on_event else { return };
        let payload = KuiValue(ev.payload);
        let out = KuiEvent {
            origin: ev.origin.0,
            key: ev.key.0,
            payload: &payload,
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

    #[test]
    fn zeroed_structs_are_the_schema_defaults() {
        let out = PropsOut::new();
        assert_eq!(spec_of(&zeroed_spec(), NONE, NONE, NONE), out.spec);
        assert_eq!(text_style_of(&zeroed_style()), out.style);
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
            let sample = match def.kind {
                Kind::F32 => Parsed::F32(F),
                Kind::Color => Parsed::Color(Color::hex(C)),
                Kind::Flag => Parsed::Flag,
                Kind::Enum(_) => Parsed::Enum(1),
                Kind::Sizing => Parsed::Sizing(Sizing::Percent(0.5)),
                Kind::Msg => Parsed::Msg(Value::Int(7)),
            };
            let mut expected = PropsOut::new();
            apply(def, sample, &mut expected).unwrap();

            let mut s = zeroed_spec();
            let mut t = zeroed_style();
            let (mut click, mut drag, mut key) = (NONE, NONE, NONE);
            let pct = KuiSizing { tag: 3, value: 0.5 };
            match def.name {
                "width" => s.width = pct,
                "height" => s.height = pct,
                "minWidth" => s.min_w = F,
                "maxWidth" => s.max_w = F,
                "minHeight" => s.min_h = F,
                "maxHeight" => s.max_h = F,
                "gap" => s.gap = F,
                "radius" => s.radius = F,
                "mainAlign" => s.main_align = 1,
                "crossAlign" => s.cross_align = 1,
                "center" => (s.main_align, s.cross_align) = (1, 1),
                "bg" => s.bg = C,
                "hoverable" => s.hoverable = 1,
                "window" => s.window_role = 2, // KUI_WINDOW_* = schema index + 1
                "transition" => s.transition_ms = F,
                "easing" => s.easing = 1,
                "onClick" => click = msg(Value::Int(7)),
                "onDrag" => drag = msg(Value::Int(7)),
                "onKey" => key = msg(Value::Int(7)),
                "lineHeight" => t.line_height = F,
                "color" => t.color = C,
                "family" => t.family = 1,
                other => panic!(
                    "schema prop {other:?} has no C counterpart: add a KuiSpec/KuiTextStyle \
                     field (append-only — the struct is ABI), mirror it in include/kui.h, \
                     apply it in spec_of/text_style_of, and map it here"
                ),
            }
            match def.target() {
                Target::Spec => assert_eq!(
                    spec_of(&s, click, drag, key),
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
            .main_align(Align::Center)
            .cross_align(Align::End)
            .bg(Color::hex(0x14161eff))
            .radius(6.0)
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
            .on_click(Value::str("c"))
            .on_drag(Value::str("d"))
            .on_key(Value::str("k"));
        let got = spec_of(&s, msg("c".into()), msg("d".into()), msg("k".into()));
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

        let mut draw: KuiDrawData = unsafe { std::mem::zeroed() };
        kui_draw_data(ctx, &mut draw);
        assert!(draw.quad_count > 20, "got {} quads", draw.quad_count);
        kui_ctx_free(ctx);
    }
}
