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
    Align, Color, Core, Edges, EditKey, EditOptions, Enter, FloatConfig, InputEvent, Key, Keyframe,
    Mods, MouseButton, NodeSpec, Rect, Size, Sizing, Span, TextStyle, UiEvent, Value, Vec2,
    WindowButton, WindowCommand, WindowEnv,
};

// ---------------------------------------------------------------------------
// ABI version, and who is allowed to write which struct
//
// `include/kui.h` is hand-mirrored from the `repr(C)` structs below, and
// `mod abi_parity` settles the two at build time — but only for a host
// compiled against the header it links against. Nothing settles an old
// *binary* against a new library, and the two directions are not equally
// forgiving:
//
// - **[in]** — the host allocates and fills it, the library reads it:
//   `KuiSpec`, `KuiSizing`, `KuiKeyframe`, `KuiEnter`, `KuiTextStyle`,
//   `KuiSpan`, `KuiPlay`, `KuiAudio`. Appending a field is compatible: a
//   host that predates it passes the shorter struct, the library reads no
//   further than the host wrote, and the zeroed tail is the documented
//   default. `KuiSpec` grew `tooltip` exactly this way.
// - **[out]** — the host allocates it, the library writes it: `KuiEvent`,
//   `KuiDrawData`, `KuiTextMetrics`, `KuiScrollGeometry`. Appending a field
//   here is memory corruption at a host that has not recompiled — it
//   reserved the shorter struct and the library writes the longer one — so
//   each of these leads with `size`, which the host sets to its own
//   `sizeof`. [`write_out`] writes no further than that, which turns the
//   append back into a compatible change.
// - **[out-array]** — the host allocates an array, the library fills up to
//   `cap` of its elements: `KuiAccessNode`, `KuiAccessRun`, `KuiWarning`,
//   `KuiAudioCommand`. A `size` field cannot save these. The library
//   strides by its own `size_of`, so element 1 lands past the host's
//   element 1 whatever element 0 says, and the damage is done before any
//   in-band handshake could be read. Growing one of these means adding an
//   explicit stride parameter — a source break every host sees — and
//   bumping `KUI_ABI_VERSION`.
// - **[lib]** — the library allocates it and the host reads it: `KuiQuad`,
//   through `KuiDrawData.quads`. The same stride problem, mirrored: the
//   host walks the array with its own `sizeof`. Read-only, so it misreads
//   rather than corrupting, but it misreads every quad after the first.
//   Growing it bumps the version.
//
// `KuiStr` is the exception: it crosses in both directions (`kui_edit_text`
// and `kui_value_as_str` write one) and its layout is frozen at (ptr, len).
// `KuiEvent` is also handed to `kui_run`'s `on_event` as a library-owned
// `*const KuiEvent`; a single struct behind a pointer is safe to append to,
// since the host reads only the prefix it knows, so it is the [out] use
// above that constrains the type.

/// The ABI this build implements, returned by `kui_abi_version`.
/// `KUI_ABI_VERSION` in `include/kui.h` is the one a host compiled against,
/// and `mod abi_parity` asserts the two agree.
///
/// **Bump it when the layout of anything the library writes or allocates
/// changes** — an [out], [out-array] or [lib] struct in the note above, in
/// any way, appends included. **Do not bump it** for a field appended to an
/// [in] struct, which old hosts survive by construction, nor for a new
/// function: a host that does not call one is unaffected, and one that does
/// fails to *link*, which is loud.
pub const KUI_ABI_VERSION: u32 = 1;

/// The ABI version this library implements, for a host to compare against
/// the `KUI_ABI_VERSION` of the header it compiled against, before its
/// first other call.
///
/// This is the one mismatch a C host cannot otherwise detect: the header
/// and the library are settled at build time by `mod abi_parity`, but a
/// host loads whatever `libkui_ffi` the system hands it, and the failure
/// that follows (a newer library writing a longer `KuiEvent` into an older
/// host's shorter one) is silent memory corruption, not a crash.
#[unsafe(no_mangle)]
pub extern "C" fn kui_abi_version() -> u32 {
    KUI_ABI_VERSION
}

/// Bytes through the end of field `$f` (of type `$t`) in `$ty`: what a
/// caller must have reserved to hold the fields up to and including it.
///
/// Used to state each [out] struct's ABI-1 layout without writing a number
/// down — `usize` and pointer widths differ per target — and without
/// tracking future growth, which is the point: appending a field never
/// moves the last ABI-1 field, so the floor stays put.
macro_rules! abi_through {
    ($ty:ty, $f:ident, $t:ty) => {
        (std::mem::offset_of!($ty, $f) + std::mem::size_of::<$t>()) as u32
    };
}

/// A struct the library writes into memory the **caller** reserved.
///
/// Each leads with `size`, set by the caller to the `sizeof` of its own
/// copy, so the library can write no further than the caller's reservation
/// and a later appended field costs an un-recompiled host nothing. This is
/// deliberately not the rule for [in] structs: the library only reads
/// those, so a short one is already safe, and a `size` field would be a tax
/// on every `KuiSpec` literal in every builder call.
///
/// # Safety
///
/// The implementor must be `repr(C)` with `size: u32` as its first field,
/// so that reading the first four bytes behind a `*mut Self` reads the
/// caller's reservation and nothing else.
unsafe trait OutParam: Sized {
    /// This struct's layout in ABI 1, where the handshake starts, measured
    /// through its last ABI-1 field. A caller reserving less than this
    /// predates the handshake entirely, so the call refuses rather than
    /// guessing what the bytes mean.
    const ABI_V1_SIZE: u32;

    /// Where the library records how many bytes it filled.
    fn size_mut(&mut self) -> &mut u32;
}

/// Whether `out` is a reservation this library can write into: non-NULL,
/// and at least the ABI-1 layout.
///
/// Separate from [`write_out`] so a call can refuse *before* it moves any
/// state — `kui_poll_event` must not pop an event it then cannot deliver.
fn out_accepts<T: OutParam>(out: *mut T) -> bool {
    if out.is_null() {
        return false;
    }
    // `size` leads the struct (the trait's safety contract), so this reads
    // the caller's reservation without assuming the rest of it is there.
    // The caller must have set it; an uninitialized `size` is the one thing
    // this cannot catch, which is why the header leads with the
    // KUI_*_INIT initializers rather than describing the field.
    let reserved = unsafe { out.cast::<u32>().read() };
    reserved >= T::ABI_V1_SIZE
}

/// Writes `value` into `out`, clipped to what the caller reserved, and
/// reports how many bytes that was in `out`'s own `size`.
///
/// Returns false — writing nothing — when [`out_accepts`] refuses. Growth
/// is append-only by the note above, so "the fields that fit" is exactly
/// "the first `n` bytes", and the `size` written back is stable under
/// repetition: a poll loop reusing one struct clamps to the same `n` every
/// time round.
fn write_out<T: OutParam>(out: *mut T, mut value: T) -> bool {
    if !out_accepts(out) {
        return false;
    }
    let reserved = unsafe { out.cast::<u32>().read() } as usize;
    let n = reserved.min(std::mem::size_of::<T>());
    *value.size_mut() = n as u32;
    unsafe {
        std::ptr::copy_nonoverlapping((&raw const value).cast::<u8>(), out.cast::<u8>(), n);
    }
    true
}

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
    /// Warnings most recently handed out by kui_take_warnings; their strings
    /// stay valid until the next call.
    last_warnings: Vec<kui_core::Warning>,
    /// Access tree most recently handed out by kui_access_tree; its strings
    /// stay valid until the next call.
    last_access: kui_core::AccessTree,
    /// One entry per node the `kui_open*` family has open, holding its key
    /// and `KuiSpec.tooltip` hint. `kui_close` pops it and floats the hint
    /// as the node's last child while it is hovered — which is what the
    /// other bindings' `tooltip` prop does at the same point.
    open_tooltips: Vec<Option<(kui_core::Key, String)>>,
}

impl KuiCtx {
    fn core(&mut self) -> &mut Core {
        unsafe { &mut *self.core }
    }

    /// Records a just-opened node's hover hint for `kui_close`.
    fn push_tooltip(&mut self, key: kui_core::Key, hint: KuiStr) {
        self.open_tooltips
            .push(opt_str(hint).map(|h| (key, h.into_owned())));
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

/// Which of a `KuiKeyframe`'s fields are set (its `set` bits).
pub const KUI_KF_AT: u32 = 1 << 0;
pub const KUI_KF_WIDTH: u32 = 1 << 1;
pub const KUI_KF_HEIGHT: u32 = 1 << 2;
pub const KUI_KF_BG: u32 = 1 << 3;
pub const KUI_KF_RADIUS: u32 = 1 << 4;
pub const KUI_KF_OPACITY: u32 = 1 << 5;

/// One keyframe stop (`KuiSpec.keyframes`): a zeroed stop sets nothing.
/// `set` says which fields count, so 0 stays a legal value for each.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiKeyframe {
    pub set: u32,
    /// 0..1 (KUI_KF_AT); unset stops spread evenly, a lone one sits at 1.
    pub at: f32,
    pub width: KuiSizing,
    pub height: KuiSizing,
    /// 0xRRGGBBAA
    pub bg: u32,
    pub radius: f32,
    /// Group opacity 0..1 (KUI_KF_OPACITY).
    pub opacity: f32,
}

/// Which of a `KuiEnter`'s fields are set (its `set` bits); 0 = no entrance.
pub const KUI_ENTER_OFFSET: u32 = 1 << 0;
pub const KUI_ENTER_WIDTH: u32 = 1 << 1;
pub const KUI_ENTER_HEIGHT: u32 = 1 << 2;
pub const KUI_ENTER_BG: u32 = 1 << 3;
pub const KUI_ENTER_RADIUS: u32 = 1 << 4;
pub const KUI_ENTER_OPACITY: u32 = 1 << 5;

/// Where a node starts the first frame it is seen (`KuiSpec.enter`): the
/// slots `set` names ease in from these values over `transition_ms`
/// instead of snapping. A zeroed struct is no entrance.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiEnter {
    pub set: u32,
    /// Logical px the node slides in from (KUI_ENTER_OFFSET).
    pub dx: f32,
    pub dy: f32,
    pub width: KuiSizing,
    pub height: KuiSizing,
    /// 0xRRGGBBAA
    pub bg: u32,
    pub radius: f32,
    /// Group opacity 0..1 (KUI_ENTER_OPACITY); 0 fades the subtree in.
    pub opacity: f32,
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
    /// Non-zero (with `transition_ms`): also ease the node's position, so
    /// reordered siblings slide into place.
    pub slide: u32,
    /// 0xRRGGBBAA background while hovered (or while any node in the same
    /// hover group is); 0 = none. Implies hover tracking; eases with
    /// `transition_ms`.
    pub hover_bg: u32,
    /// 0xRRGGBBAA background while pressed; 0 = none. Implies hover tracking.
    pub pressed_bg: u32,
    /// Hover group name (empty = none): members show hover_bg / pressed_bg
    /// together. Hashed by the core; the string is not retained.
    pub hover_group: KuiStr,
    /// Non-zero: `radius_tl..radius_bl` are the four corner radii and
    /// `radius` is ignored (zero = uniform `radius` on every corner).
    pub per_corner: u32,
    pub radius_tl: f32,
    pub radius_tr: f32,
    pub radius_br: f32,
    pub radius_bl: f32,
    /// KUI_REPEAT_* direction for `keyframes` (CSS animation-direction).
    pub repeat: u32,
    /// Holds the keyframe cycle back by this many ms (CSS animation-delay).
    pub delay_ms: f32,
    /// CSS-style stops (`keyframes_len` of them; NULL/0 = none): the slots
    /// they name cycle over `transition_ms`, forever, without the view
    /// redrawing. Read while the node opens; not retained.
    pub keyframes: *const KuiKeyframe,
    pub keyframes_len: usize,
    /// Entrance: with `set` non-zero, the named slots ease in from these
    /// values on the node's first sight (see `KuiEnter`).
    pub enter: KuiEnter,
    /// Registered sounds (`kui_sound_add`) played when the node is clicked /
    /// the pointer enters it; 0 = none. Either makes the node hover-tracked.
    pub click_sound: u64,
    pub hover_sound: u64,
    /// Layout tag (NULL = none): the node's laid-out rect arrives as
    /// `{kind="layout", x, y, w, h, parent, tag}` on its first frame and
    /// whenever it changes. Borrowed — cloned while the node opens, so the
    /// caller keeps ownership and frees it as usual; `kui_value_null()`
    /// asks for the events without a tag.
    pub on_layout: *const KuiValue,
    /// KUI_ROLE_* (0 = unset: the core derives one). What the node is to
    /// assistive technology; KUI_ROLE_NONE hides it and its subtree.
    pub role: u32,
    /// Accessible name (empty = none). Copied while the node opens.
    pub label: KuiStr,
    /// Non-zero: a checkbox / radio / switch role is on.
    pub checked: u32,
    /// KUI_VALUE_NOW / MIN / MAX bits saying which of the three below are
    /// set (a slider role's position and range).
    pub value_set: u32,
    pub value_now: f32,
    pub value_min: f32,
    pub value_max: f32,
    /// On a KUI_ROLE_LINE of a custom editor: the caret's byte offset into
    /// the line's text, and the selection's other end (KUI_VALUE_CARET /
    /// KUI_VALUE_ANCHOR in `value_set` say which are present).
    pub caret: u32,
    pub selection_anchor: u32,
    /// Non-zero: reachable by Tab (and focused by a click) without a click
    /// payload or a control role (docs/adr/0002-keyboard-focus-as-data.md).
    pub focusable: u32,
    /// Non-zero: inert — no click, drag or key sink, no hover / pressed /
    /// focus background, skipped by Tab, reported disabled to assistive
    /// technology; hover tracking stays so a tooltip can say why.
    pub disabled: u32,
    /// 0xRRGGBBAA background while the node holds keyboard-visible focus
    /// (Tab or assistive technology put it there); 0 = the core's default
    /// ring.
    pub focus_bg: u32,
    /// Hover hint (empty = none), the `tooltip` prop of the other
    /// bindings: makes the node hover-tracked, becomes its accessible
    /// description, and floats `kui_core::widgets::tooltip` below it as
    /// its last child while the pointer is over it. `kui_close` draws it,
    /// so it only applies to the `kui_open*` family.
    pub tooltip: KuiStr,
    /// Modal surface (NULL = none): while this node is declared the Tab
    /// ring is its subtree, everything outside it is inert to the pointer,
    /// the wheel and assistive technology, and Escape or a press outside
    /// emits `{kind="dismiss", reason, tag}` on it — the app stops opening
    /// the node. The last one declared wins (a confirm inside a dialog);
    /// a modal that must cover the app is a float. Borrowed — cloned
    /// while the node opens, so the caller keeps ownership;
    /// `kui_value_null()` asks for the behaviour without a tag.
    pub modal: *const KuiValue,
    /// Context menu (NULL = none): a secondary-button press over this node
    /// emits `{kind="contextmenu", x, y, tag}` on it, at the logical point
    /// to open the menu at. The press moves no focus, places no caret and
    /// produces no click. Borrowed — cloned while the node opens, so the
    /// caller keeps ownership; `kui_value_null()` asks for the behaviour
    /// without a tag.
    pub on_context_menu: *const KuiValue,
    /// KUI_CURSOR_* (0 = unset: the core derives one from what the node
    /// does). Overrides the pointer shape while the pointer is over this
    /// node; a node with only a cursor is hover-tracked so it can be found.
    pub cursor: u32,
    /// Non-zero: this node is the current one of its set — the shown tab,
    /// the picked row, the link for the page you are on. A KUI_ROLE_TAB
    /// reports the state either way; a row or a link reports it only when
    /// this is set.
    pub selected: u32,
    /// KUI_EXPANDED_* (0 = unset: the node does not expand and says
    /// nothing about it). What a twisty, an accordion header or a menu
    /// button reads as.
    pub expanded: u32,
    /// Non-zero: `opacity` is the node's group opacity (0 without this bit
    /// means "not set", so a fully transparent subtree stays expressible).
    pub opacity_set: u32,
    /// Group opacity 0..1 with `opacity_set`: fades this node and its whole
    /// subtree. A per-quad alpha multiply, not an offscreen composite, so
    /// overlapping pieces of one subtree show their seams through the fade;
    /// layout, hit-testing and the access tree are untouched. Eases with
    /// `transition_ms`.
    pub opacity: f32,
    /// 0xRRGGBBAA drop-shadow color; 0 = no shadow, and nothing else here
    /// draws without it. The shadow is the node's rounded rect moved by
    /// `shadow_x`/`shadow_y`, grown by `shadow_spread` and blurred over
    /// `shadow_blur`, painted behind the node. Outer shadows only, and the
    /// shape is not knocked out of the middle.
    pub shadow_color: u32,
    /// Blur radius (logical px): the edge ramps over this distance and
    /// reaches this far past the shape. 0 = a hard edge.
    pub shadow_blur: f32,
    /// Offset (logical px); positive `shadow_y` casts downward.
    pub shadow_x: f32,
    pub shadow_y: f32,
    /// Grows (negative: shrinks) the shape before blurring (logical px).
    pub shadow_spread: f32,
    /// Non-zero: children that don't fit the main axis start a new line
    /// instead of overflowing or shrinking. Rows only — a column, or a row
    /// with KUI_OVERFLOW_SCROLL_X, lays out as if this were 0 and raises a
    /// `wrap-ignored` warning.
    pub wrap_children: u32,
    /// Space between wrap lines, across the main axis (`gap` stays the
    /// space between children along it).
    pub cross_gap: f32,
}

/// One laid-out run of an editor's text (`kui_access_runs`): what a
/// screen reader reads by character and word. `text` ends with `"\n"`
/// (counted as a zero-width character) when the line continues into
/// another. Character positions are relative to `x`. Arrays and strings
/// are borrowed until the next `kui_access_tree` / `kui_access_runs` on
/// the context.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiAccessRun {
    /// The run's own id (a `KuiAccessNode.anchor_run` / `focus_run`, and
    /// what `kui_input_access_text` takes).
    pub key: u64,
    /// The line it belongs to (a buffer line, or a KUI_ROLE_LINE ordinal
    /// for a custom editor) and its byte range in that line's text.
    pub line: u32,
    pub start: u32,
    pub end: u32,
    pub text: KuiStr,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub char_count: u32,
    pub char_lengths: *const u8,
    pub char_positions: *const f32,
    pub char_widths: *const f32,
    pub word_start_count: u32,
    pub word_starts: *const u8,
    pub rtl: u32,
}

pub const KUI_VALUE_CARET: u32 = 1 << 3;
pub const KUI_VALUE_ANCHOR: u32 = 1 << 4;
pub const KUI_ACCESS_HAS_TEXT_SELECTION: u32 = 1 << 9;

/// One node of the access tree (`kui_access_tree`): what assistive
/// technology sees. `role` is KUI_ROLE_*, `flags` KUI_ACCESS_HAS_* /
/// FOCUSED / CHECKED bits saying which optional fields hold, `actions`
/// the KUI_ACCESS_* bits the node accepts through `kui_input_access`.
/// Strings are borrowed until the next `kui_access_tree` on the context.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiAccessNode {
    pub key: u64,
    /// The nearest semantic ancestor; 0 for the root.
    pub parent: u64,
    pub origin: u32,
    pub role: u32,
    pub flags: u32,
    pub actions: u32,
    pub name: KuiStr,
    pub description: KuiStr,
    /// An editor's text (KUI_ACCESS_HAS_VALUE).
    pub value: KuiStr,
    /// Logical px, viewport coordinates.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Byte offsets into `value` (KUI_ACCESS_HAS_VALUE / HAS_SELECTION).
    pub caret: u32,
    pub selection_start: u32,
    pub selection_end: u32,
    /// A slider's position and range (KUI_ACCESS_HAS_NUMBER / MIN / MAX).
    pub value_now: f32,
    pub value_min: f32,
    pub value_max: f32,
    /// A scroll view's offsets and range (KUI_ACCESS_HAS_SCROLL).
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub scroll_max_x: f32,
    pub scroll_max_y: f32,
    /// An editor's caret (`focus_*`) and the selection's other end
    /// (`anchor_*`) as run positions (KUI_ACCESS_HAS_TEXT_SELECTION):
    /// a run key from `kui_access_runs` and a character index into it.
    pub anchor_run: u64,
    pub anchor_char: u32,
    pub focus_run: u64,
    pub focus_char: u32,
    /// How many runs `kui_access_runs` returns for this node.
    pub run_count: u32,
    /// "3 of 7" (KUI_ACCESS_HAS_POS_IN_SET / HAS_SET_SIZE): the item's
    /// zero-based ordinal among its list's or tab list's items, and the
    /// count on that container.
    pub pos_in_set: u32,
    pub set_size: u32,
}

pub const KUI_ACCESS_HAS_VALUE: u32 = 1 << 0;
pub const KUI_ACCESS_HAS_SELECTION: u32 = 1 << 1;
pub const KUI_ACCESS_FOCUSED: u32 = 1 << 2;
pub const KUI_ACCESS_CHECKED_SET: u32 = 1 << 3;
pub const KUI_ACCESS_CHECKED: u32 = 1 << 4;
pub const KUI_ACCESS_HAS_NUMBER: u32 = 1 << 5;
pub const KUI_ACCESS_HAS_MIN: u32 = 1 << 6;
pub const KUI_ACCESS_HAS_MAX: u32 = 1 << 7;
pub const KUI_ACCESS_HAS_SCROLL: u32 = 1 << 8;
/// The node is `disabled`: inert, not a Tab stop (bit 9 is
/// KUI_ACCESS_HAS_TEXT_SELECTION).
pub const KUI_ACCESS_DISABLED: u32 = 1 << 10;
/// The node is the frame's `modal` surface (`aria-modal`): focus and input
/// are confined to it (`docs/adr/0003-modal-surfaces.md`).
pub const KUI_ACCESS_MODAL: u32 = 1 << 11;
/// The node has a selected state at all, and what it is: every
/// KUI_ROLE_TAB, and a row or link the view marked (see `KuiSpec.selected`).
pub const KUI_ACCESS_SELECTED_SET: u32 = 1 << 12;
pub const KUI_ACCESS_SELECTED: u32 = 1 << 13;
/// The node expands, and whether it is open (see `KuiSpec.expanded`).
pub const KUI_ACCESS_EXPANDED_SET: u32 = 1 << 14;
pub const KUI_ACCESS_EXPANDED: u32 = 1 << 15;
/// `pos_in_set` holds (on an item), `set_size` holds (on its container).
pub const KUI_ACCESS_HAS_POS_IN_SET: u32 = 1 << 16;
pub const KUI_ACCESS_HAS_SET_SIZE: u32 = 1 << 17;

/// KUI_EXPANDED_* is the position in `schema::EXPANDED` plus one (0 = unset:
/// the node does not expand).
pub const KUI_EXPANDED_COLLAPSED: u32 = 1;
pub const KUI_EXPANDED_EXPANDED: u32 = 2;

pub const KUI_VALUE_NOW: u32 = 1 << 0;
pub const KUI_VALUE_MIN: u32 = 1 << 1;
pub const KUI_VALUE_MAX: u32 = 1 << 2;

/// KUI_ROLE_* is the position in `Role::ALL` plus one (0 = unset).
fn role_code(role: kui_core::Role) -> u32 {
    kui_core::Role::ALL
        .iter()
        .position(|r| *r == role)
        .map_or(0, |i| i as u32 + 1)
}

fn role_of_code(code: u32) -> Option<kui_core::Role> {
    (code > 0)
        .then(|| kui_core::Role::ALL.get(code as usize - 1).copied())
        .flatten()
}

/// What a piece of text measures (`kui_measure_text`), logical px at the
/// scale of the current or last frame.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiTextMetrics {
    /// [out] reservation; see `KUI_TEXT_METRICS_INIT`.
    pub size: u32,
    pub width: f32,
    pub height: f32,
    /// Lines after wrapping (capped by `max_lines`).
    pub lines: u32,
}

// Hand-written rather than derived: a derived `Default` would zero `size`,
// and a zero `size` is the one value the handshake refuses.
impl Default for KuiTextMetrics {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            width: 0.0,
            height: 0.0,
            lines: 0,
        }
    }
}

// SAFETY: `repr(C)` with `size: u32` first.
unsafe impl OutParam for KuiTextMetrics {
    const ABI_V1_SIZE: u32 = abi_through!(KuiTextMetrics, lines, u32);
    fn size_mut(&mut self) -> &mut u32 {
        &mut self.size
    }
}

/// One diagnostic (`kui_take_warnings`): a silent misconfiguration the
/// core noticed. `code` is stable (`grow-weight-ignored`,
/// `transition-auto-key`, `duplicate-key`); the strings are borrowed until
/// the next `kui_take_warnings` on the same context.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiWarning {
    pub code: KuiStr,
    pub key: u64,
    pub message: KuiStr,
}

/// Options for `kui_play`. NULL means defaults; a given struct is read
/// literally (so `volume` must be set — `KUI_PLAY_INIT` in kui.h does).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiPlay {
    /// Linear amplitude, 0..1.
    pub volume: f32,
    pub looped: u32,
    pub fade_in_ms: f32,
}

/// What a `kui_audio` node declares; read literally (`KUI_AUDIO_INIT`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiAudio {
    /// A registered sound (`kui_sound_add`).
    pub src: u64,
    /// Linear amplitude, 0..1.
    pub volume: f32,
    pub looped: u32,
    pub paused: u32,
}

/// One audio command for a host that drives its own device
/// (`kui_take_audio_commands`); `kind` is `KUI_AUDIO_*` and says which of
/// the other fields mean anything.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct KuiAudioCommand {
    pub kind: u32,
    pub playback: u64,
    pub sound: u64,
    /// Linear amplitude (play, set_volume, master_volume).
    pub volume: f32,
    /// Fade / tween duration in ms (fade-in for play).
    pub ms: f32,
    pub looped: u32,
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
    /// A registered font handle (kui_font_add / kui_font_add_system);
    /// non-zero overrides `family`.
    pub font: u64,
    /// KUI_WRAP_WORD (0, default) / KUI_WRAP_GLYPH / KUI_WRAP_NONE.
    pub wrap: u32,
    /// Lay out at most this many lines; 0 = unlimited.
    pub max_lines: u32,
    /// Non-zero: end the last line with an ellipsis when the text is cut
    /// off (a single line unless `max_lines` says otherwise).
    pub ellipsis: u32,
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

/// One polled event ([out]). `size` leads it so that ADR 0004's appended
/// `window` — and anything after it — reaches a host that has not
/// recompiled as a shorter write rather than as a longer one.
#[repr(C)]
pub struct KuiEvent {
    /// Set to `sizeof(KuiEvent)` before the call (`KUI_EVENT_INIT` does);
    /// comes back as the number of bytes the library filled.
    pub size: u32,
    pub origin: u16,
    pub key: u64,
    /// Borrowed until the next `kui_poll_event`/`kui_ctx_free`; NULL if none.
    pub payload: *const KuiValue,
}

impl Default for KuiEvent {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            origin: 0,
            key: 0,
            payload: std::ptr::null(),
        }
    }
}

// SAFETY: `repr(C)` with `size: u32` first.
unsafe impl OutParam for KuiEvent {
    const ABI_V1_SIZE: u32 = abi_through!(KuiEvent, payload, *const KuiValue);
    fn size_mut(&mut self) -> &mut u32 {
        &mut self.size
    }
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
    /// Corner radii (physical px), clockwise from the top-left.
    pub radius: [f32; 4],
    pub border_w: f32,
    /// KUI_QUAD_SHADOW only: blur radius (physical px), which is also how
    /// far `x`/`y`/`w`/`h` is inflated past the shape being blurred.
    pub blur: f32,
    /// KUI_QUAD_*
    pub kind: u32,
    pub uv: [u32; 4],
    /// Clip rect (physical px): x, y, w, h. Pixels outside are transparent.
    pub clip: [f32; 4],
}

#[repr(C)]
pub struct KuiDrawData {
    /// [out] reservation; see `KUI_DRAW_DATA_INIT`.
    pub size: u32,
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

impl Default for KuiDrawData {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            quads: std::ptr::null(),
            quad_count: 0,
            viewport_w: 0.0,
            viewport_h: 0.0,
            scale: 1.0,
            atlas_pixels: std::ptr::null(),
            atlas_size: 0,
            atlas_dirty: false,
            atlas_epoch: 0,
        }
    }
}

// SAFETY: `repr(C)` with `size: u32` first.
unsafe impl OutParam for KuiDrawData {
    const ABI_V1_SIZE: u32 = abi_through!(KuiDrawData, atlas_epoch, u64);
    fn size_mut(&mut self) -> &mut u32 {
        &mut self.size
    }
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

fn enter_of(e: &KuiEnter) -> Enter {
    let mut en = Enter::default();
    if e.set & KUI_ENTER_OFFSET != 0 {
        en = en.offset(e.dx, e.dy);
    }
    if e.set & KUI_ENTER_WIDTH != 0 {
        en = en.width(sizing_of(e.width));
    }
    if e.set & KUI_ENTER_HEIGHT != 0 {
        en = en.height(sizing_of(e.height));
    }
    if e.set & KUI_ENTER_BG != 0 {
        en = en.bg(color_of(e.bg));
    }
    if e.set & KUI_ENTER_RADIUS != 0 {
        en = en.radius(e.radius);
    }
    if e.set & KUI_ENTER_OPACITY != 0 {
        en = en.opacity(e.opacity);
    }
    en
}

fn keyframe_of(k: &KuiKeyframe) -> Keyframe {
    let mut kf = Keyframe::default();
    if k.set & KUI_KF_AT != 0 {
        kf = kf.at(k.at);
    }
    if k.set & KUI_KF_WIDTH != 0 {
        kf = kf.width(sizing_of(k.width));
    }
    if k.set & KUI_KF_HEIGHT != 0 {
        kf = kf.height(sizing_of(k.height));
    }
    if k.set & KUI_KF_BG != 0 {
        kf = kf.bg(color_of(k.bg));
    }
    if k.set & KUI_KF_RADIUS != 0 {
        kf = kf.radius(k.radius);
    }
    if k.set & KUI_KF_OPACITY != 0 {
        kf = kf.opacity(k.opacity);
    }
    kf
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

/// A `KuiStr` as a borrowed `&str`, or None when it is empty/NULL.
fn opt_str<'a>(s: KuiStr) -> Option<std::borrow::Cow<'a, str>> {
    (!s.ptr.is_null() && s.len > 0).then(|| kstr(s))
}

fn spec_of(
    s: &KuiSpec,
    on_click: *mut KuiValue,
    on_drag: *mut KuiValue,
    on_key: *mut KuiValue,
    on_hover: *mut KuiValue,
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
        .cross_gap(s.cross_gap)
        .main_align(align_of(s.main_align))
        .cross_align(align_of(s.cross_align))
        .bg(color_of(s.bg))
        .radius(s.radius);
    if s.per_corner != 0 {
        spec = spec.radii(s.radius_tl, s.radius_tr, s.radius_br, s.radius_bl);
    }
    if s.border_w > 0.0 {
        spec = spec.border(s.border_w, color_of(s.border_color));
    }
    if s.opacity_set != 0 {
        spec = spec.opacity(s.opacity);
    }
    if s.wrap_children != 0 {
        spec = spec.wrap();
    }
    // Unconditional: the shadow draws only where its color is visible, and
    // that check belongs at emission, not here — a zeroed struct is the
    // default shadow either way.
    spec = spec.shadow(kui_core::Shadow {
        color: color_of(s.shadow_color),
        dx: s.shadow_x,
        dy: s.shadow_y,
        blur: s.shadow_blur,
        spread: s.shadow_spread,
    });
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
    if s.slide != 0 {
        spec = spec.slide();
    }
    if s.hover_bg != 0 {
        spec = spec.hover_bg(color_of(s.hover_bg));
    }
    if s.pressed_bg != 0 {
        spec = spec.pressed_bg(color_of(s.pressed_bg));
    }
    if !s.hover_group.ptr.is_null() && s.hover_group.len > 0 {
        spec = spec.hover_group(&kstr(s.hover_group));
    }
    if s.repeat != 0 {
        spec = spec.repeat(kui_core::schema::repeat_idx(s.repeat as usize));
    }
    if s.delay_ms != 0.0 {
        spec = spec.delay(s.delay_ms);
    }
    if !s.keyframes.is_null() && s.keyframes_len > 0 {
        let stops = unsafe { std::slice::from_raw_parts(s.keyframes, s.keyframes_len) };
        spec = spec.keyframes(stops.iter().map(keyframe_of).collect());
    }
    if s.enter.set != 0 {
        spec = spec.enter(enter_of(&s.enter));
    }
    if s.click_sound != 0 {
        spec = spec.click_sound(kui_core::SoundId::from_ffi(s.click_sound));
    }
    if s.hover_sound != 0 {
        spec = spec.hover_sound(kui_core::SoundId::from_ffi(s.hover_sound));
    }
    if let Some(tag) = unsafe { s.on_layout.as_ref() } {
        // Borrowed, unlike the message arguments: the spec is const.
        spec = spec.on_layout(tag.0.clone());
    }
    if let Some(tag) = unsafe { s.modal.as_ref() } {
        spec = spec.modal(tag.0.clone());
    }
    if let Some(tag) = unsafe { s.on_context_menu.as_ref() } {
        spec = spec.on_context_menu(tag.0.clone());
    }
    if s.cursor != 0 {
        // KUI_CURSOR_* = schema index + 1, so zero can mean "derive".
        spec = spec.cursor(kui_core::schema::cursor_idx(s.cursor as usize - 1));
    }
    if let Some(role) = role_of_code(s.role) {
        spec = spec.role(role);
    }
    if !s.label.ptr.is_null() && s.label.len > 0 {
        spec = spec.label(kstr(s.label).as_ref());
    }
    if s.checked != 0 {
        spec = spec.checked(true);
    }
    if s.selected != 0 {
        spec = spec.selected(true);
    }
    if s.expanded != 0 {
        // KUI_EXPANDED_* = schema index + 1, so zero can mean "unset".
        spec = spec.expanded(s.expanded == KUI_EXPANDED_EXPANDED);
    }
    if s.value_set & KUI_VALUE_NOW != 0 {
        spec = spec.value_now(s.value_now);
    }
    if s.value_set & KUI_VALUE_MIN != 0 {
        spec = spec.value_min(s.value_min);
    }
    if s.value_set & KUI_VALUE_MAX != 0 {
        spec = spec.value_max(s.value_max);
    }
    if s.value_set & KUI_VALUE_CARET != 0 {
        spec = spec.caret(s.caret);
    }
    if s.value_set & KUI_VALUE_ANCHOR != 0 {
        spec = spec.selection_anchor(s.selection_anchor);
    }
    if s.focusable != 0 {
        spec = spec.focusable();
    }
    if s.disabled != 0 {
        spec = spec.disabled(true);
    }
    if s.focus_bg != 0 {
        spec = spec.focus_bg(color_of(s.focus_bg));
    }
    if let Some(hint) = opt_str(s.tooltip) {
        // The same two things the Lua and Node parsers do with the prop;
        // `kui_close` adds the third (the float, while hovered).
        spec = spec.hoverable().description(hint);
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
    if let Some(v) = take_msg(on_hover) {
        spec = spec.on_hover(v);
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
    style = style.family(match s.family {
        1 => kui_core::FontFamily::Serif,
        2 => kui_core::FontFamily::Mono,
        _ => kui_core::FontFamily::Sans,
    });
    if s.font != 0 {
        style = style.font(kui_core::FontId::from_ffi(s.font));
    }
    style = style.wrap(match s.wrap {
        1 => kui_core::TextWrap::Glyph,
        2 => kui_core::TextWrap::None,
        _ => kui_core::TextWrap::Word,
    });
    if s.max_lines > 0 {
        style = style.max_lines(s.max_lines);
    }
    if s.ellipsis != 0 {
        style = style.ellipsis();
    }
    style
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

/// A primary-button press or release; `kui_input_mouse_button` carries the
/// others. Kept as it was: it is exported ABI.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_mouse(ptr: *mut KuiCtx, down: bool, clicks: u32) {
    kui_input_mouse_button(ptr, down, MouseButton::Primary.code(), clicks);
}

/// `kui_input_mouse` for a named button (`KUI_MOUSE_*`, or `3 + n` for a
/// further button `n`).
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_mouse_button(ptr: *mut KuiCtx, down: bool, button: u32, clicks: u32) {
    let button = MouseButton::from_code(button);
    push_input(
        ptr,
        if down {
            InputEvent::MouseDown {
                button,
                clicks: clicks.clamp(1, u8::MAX as u32) as u8,
            }
        } else {
            InputEvent::MouseUp { button }
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

fn key_press_of(code: KuiStr, kmods: u32, text: KuiStr) -> Option<kui_core::KeyPress> {
    let code = kui_core::KeyCode::from_name(&kstr(code))?;
    let mods = kui_core::KeyMods {
        shift: kmods & 1 != 0,
        ctrl: kmods & 2 != 0,
        alt: kmods & 4 != 0,
        super_key: kmods & 8 != 0,
    };
    // A NULL `text` means "whatever this key inserts": the plain
    // character keys insert themselves, a chord inserts nothing.
    let text = match text.ptr.is_null() {
        false => Some(kstr(text).into_owned()),
        true if mods.ctrl || mods.alt || mods.super_key => None,
        true => match code {
            kui_core::KeyCode::Char(c) => Some(c.to_string()),
            kui_core::KeyCode::Space => Some(" ".to_string()),
            _ => None,
        },
    };
    Some(kui_core::KeyPress {
        code,
        mods,
        text,
        repeat: false,
    })
}

/// A raw key press for `on_key` sinks (the editing keys go through
/// `kui_input_key`). `code` is a single character as the layout produced it
/// ("W", "$") or a name ("left", "enter", "escape", "f5", ...); `kmods` is
/// KUI_KMOD_* bits; `text` is what the press inserts, or NULL to derive it
/// from `code`; `repeat` marks an auto-repeat. The focused sink polls
/// `{kind="key", phase="down", code, ctrl, alt, shift, super, text, repeat,
/// tag}`. An unknown `code` is ignored.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_key_down(
    ptr: *mut KuiCtx,
    code: KuiStr,
    kmods: u32,
    text: KuiStr,
    repeat: bool,
) {
    guard((), || {
        if let Some(kp) = key_press_of(code, kmods, text) {
            push_input(
                ptr,
                InputEvent::KeyDown(kui_core::KeyPress { repeat, ..kp }),
            );
        }
    });
}

/// The release of a key pressed with `kui_input_key_down`, spelled the same
/// way; the sink polls `{kind="key", phase="up", ...}` with a null `text`.
/// A release whose press the sink never got resolves nothing, and moving
/// focus while a key is held delivers the "up" first, so a held-key binding
/// (WASD, press-and-hold) cannot be left stuck down.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_key_up(ptr: *mut KuiCtx, code: KuiStr, kmods: u32) {
    guard((), || {
        if let Some(kp) = key_press_of(
            code,
            kmods,
            KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
        ) {
            push_input(ptr, InputEvent::KeyUp(kp.released()));
        }
    });
}

/// Lets go of every key the focused sink is holding, as if the user had
/// released them. Hosts call it when the window loses the keyboard: the OS
/// stops delivering key events to it, so the release of anything held over
/// an app switch would never arrive. Focus moves do this by themselves.
#[unsafe(no_mangle)]
pub extern "C" fn kui_release_held_keys(ptr: *mut KuiCtx) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().release_held_keys();
        }
    });
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
        // Also whatever a call between frames left pending — the synthetic
        // key releases `kui_focus` / `kui_release_held_keys` force.
        let pending = c.core().take_pending_events();
        c.events.extend(pending);
        // Refuse before popping: a reservation this library cannot write
        // into must leave the queue where it was, not swallow an event.
        if c.events.is_empty() || !out_accepts(out) {
            return false;
        }
        let ev = c.events.remove(0);
        let payload = Box::new(KuiValue(ev.payload));
        let payload_ptr: *const KuiValue = &*payload;
        c.last_payload = Some(payload);
        write_out(
            out,
            KuiEvent {
                origin: ev.origin.0,
                key: ev.key.0,
                payload: payload_ptr,
                ..Default::default()
            },
        )
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

/// The pointer shape for where the pointer is now (KUI_CURSOR_*, never 0):
/// derived from the topmost node under it, or whatever that node's `cursor`
/// overrode it with. A query, not a queue — read it after each input and
/// each frame and apply it to the real window when it changes. Hosts
/// without a pointer simply never call.
#[unsafe(no_mangle)]
pub extern "C" fn kui_cursor_shape(ptr: *mut KuiCtx) -> u32 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let shape = c.core().cursor_shape();
        // KUI_CURSOR_* = schema index + 1, matching KuiSpec.cursor.
        kui_core::schema::CURSORS
            .iter()
            .position(|n| *n == shape.name())
            .map_or(0, |i| i as u32 + 1)
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

/// Registers a font from file bytes (TTF/OTF/TTC, copied); returns its
/// handle for `KuiTextStyle.font`, 0 when the data holds no usable face.
#[unsafe(no_mangle)]
pub extern "C" fn kui_font_add(ptr: *mut KuiCtx, data: *const u8, len: usize) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if data.is_null() || len == 0 {
            return 0;
        }
        let bytes = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
        c.core().add_font_data(bytes).map_or(0, |id| id.to_ffi())
    })
}

/// Registers an installed font by family name; 0 when none matches.
#[unsafe(no_mangle)]
pub extern "C" fn kui_font_add_system(ptr: *mut KuiCtx, name: KuiStr) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        c.core()
            .add_system_font(&kstr(name))
            .map_or(0, |id| id.to_ffi())
    })
}

/// Registers a font file by path (memory-mapped); 0 when it cannot be read
/// or holds no usable face.
#[unsafe(no_mangle)]
pub extern "C" fn kui_font_load_file(ptr: *mut KuiCtx, path: KuiStr) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let path = kstr(path).into_owned();
        c.core().load_font_file(path).map_or(0, |id| id.to_ffi())
    })
}

/// Loads every font file under a folder (recursively) so its families can
/// be picked by name with `kui_font_add_system`; returns the face count.
#[unsafe(no_mangle)]
pub extern "C" fn kui_font_load_dir(ptr: *mut KuiCtx, dir: KuiStr) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let dir = kstr(dir).into_owned();
        c.core().load_fonts_dir(dir)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_font_remove(ptr: *mut KuiCtx, id: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().remove_font(kui_core::FontId::from_ffi(id));
        }
    });
}

// -- Audio -------------------------------------------------------------------
// Sounds are resources, playback is commands the driver drains; kui_run
// plays them itself, a host with its own loop drains kui_take_audio_commands.

/// Registers a sound from its encoded file bytes (wav/ogg/mp3/flac, copied);
/// returns its handle for `KuiSpec.click_sound` / `kui_audio` / `kui_play`,
/// 0 when empty.
#[unsafe(no_mangle)]
pub extern "C" fn kui_sound_add(ptr: *mut KuiCtx, data: *const u8, len: usize) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if data.is_null() || len == 0 {
            return 0;
        }
        let bytes = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
        c.core().add_sound(bytes).to_ffi()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_sound_remove(ptr: *mut KuiCtx, id: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().remove_sound(kui_core::SoundId::from_ffi(id));
        }
    });
}

/// Starts a playback; returns its id for kui_stop / kui_set_volume /
/// kui_pause / kui_resume. `opts` may be NULL (defaults). A non-NULL `tag`
/// (consumed) asks for a `{kind="sound", phase="ended", playback, tag}`
/// event when the playback finishes on its own.
#[unsafe(no_mangle)]
pub extern "C" fn kui_play(
    ptr: *mut KuiCtx,
    sound: u64,
    opts: *const KuiPlay,
    tag: *mut KuiValue,
) -> u64 {
    guard(0, || {
        let tag = take_msg(tag);
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let mut po = kui_core::PlayOptions::default();
        if let Some(o) = unsafe { opts.as_ref() } {
            po.volume = o.volume;
            po.looped = o.looped != 0;
            po.fade_in_ms = o.fade_in_ms;
        }
        po.tag = tag;
        c.core().play(kui_core::SoundId::from_ffi(sound), po).0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_stop(ptr: *mut KuiCtx, playback: u64, fade_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().stop(kui_core::PlaybackId(playback), fade_ms);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_set_volume(ptr: *mut KuiCtx, playback: u64, volume: f32, tween_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core()
                .set_volume(kui_core::PlaybackId(playback), volume, tween_ms);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_pause(ptr: *mut KuiCtx, playback: u64, fade_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().pause(kui_core::PlaybackId(playback), fade_ms);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_resume(ptr: *mut KuiCtx, playback: u64, fade_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().resume(kui_core::PlaybackId(playback), fade_ms);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_set_master_volume(ptr: *mut KuiCtx, volume: f32, tween_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_master_volume(volume, tween_ms);
        }
    });
}

/// An audio node: a playback retained by key while the frame declares it
/// (present = playing, gone = stopped; volume/paused apply live, a changed
/// src restarts). Empty label = a key from the tree position. `tag`
/// (nullable, consumed) rides the `ended` event. Returns the node key.
#[unsafe(no_mangle)]
pub extern "C" fn kui_audio(
    ptr: *mut KuiCtx,
    label: KuiStr,
    spec: *const KuiAudio,
    tag: *mut KuiValue,
) -> u64 {
    guard(0, || {
        let tag = take_msg(tag);
        let (Some(c), Some(a)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) else {
            return 0;
        };
        let mut s = kui_core::AudioSpec::new(kui_core::SoundId::from_ffi(a.src))
            .volume(a.volume)
            .paused(a.paused != 0);
        if a.looped != 0 {
            s = s.looped();
        }
        s.tag = tag;
        let core = c.core();
        if label.ptr.is_null() || label.len == 0 {
            core.audio_node(s).0
        } else {
            core.audio_node_keyed(&kstr(label), s).0
        }
    })
}

/// Drains queued audio commands into `out` (up to `cap`; the rest are
/// dropped, so size it generously); returns the count. Only for hosts
/// driving their own audio device — kui_run plays them itself.
#[unsafe(no_mangle)]
pub extern "C" fn kui_take_audio_commands(
    ptr: *mut KuiCtx,
    out: *mut KuiAudioCommand,
    cap: usize,
) -> usize {
    use kui_core::AudioCommand as A;
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if out.is_null() || cap == 0 {
            return 0;
        }
        let cmds = c.core().take_audio_commands();
        let n = cmds.len().min(cap);
        for (i, cmd) in cmds.into_iter().take(n).enumerate() {
            let mut o = KuiAudioCommand::default();
            match cmd {
                A::Play {
                    playback,
                    sound,
                    volume,
                    looped,
                    fade_in_ms,
                } => {
                    o.kind = 1;
                    o.playback = playback.0;
                    o.sound = sound.to_ffi();
                    o.volume = volume;
                    o.ms = fade_in_ms;
                    o.looped = looped as u32;
                }
                A::Stop { playback, fade_ms } => {
                    o.kind = 2;
                    o.playback = playback.0;
                    o.ms = fade_ms;
                }
                A::SetVolume {
                    playback,
                    volume,
                    tween_ms,
                } => {
                    o.kind = 3;
                    o.playback = playback.0;
                    o.volume = volume;
                    o.ms = tween_ms;
                }
                A::Pause { playback, fade_ms } => {
                    o.kind = 4;
                    o.playback = playback.0;
                    o.ms = fade_ms;
                }
                A::Resume { playback, fade_ms } => {
                    o.kind = 5;
                    o.playback = playback.0;
                    o.ms = fade_ms;
                }
                A::MasterVolume { volume, tween_ms } => {
                    o.kind = 6;
                    o.volume = volume;
                    o.ms = tween_ms;
                }
                A::Unload { sound } => {
                    o.kind = 7;
                    o.sound = sound.to_ffi();
                }
            }
            unsafe { out.add(i).write(o) };
        }
        n
    })
}

/// A host driving its own device reports a playback finished on its own;
/// a tagged one becomes a `sound` event for kui_poll_event.
#[unsafe(no_mangle)]
pub extern "C" fn kui_audio_ended(ptr: *mut KuiCtx, playback: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().audio_ended(kui_core::PlaybackId(playback));
            let pending = c.core().take_pending_events();
            c.events.extend(pending);
        }
    });
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

/// What the last layout resolved for a scroll container (`kui_scroll_geometry`):
/// its own box, its content size and the clamped offset, all logical px in
/// viewport coordinates.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiScrollGeometry {
    /// [out] reservation; see `KUI_SCROLL_GEOMETRY_INIT`.
    pub size: u32,
    /// The container's box, as the last layout placed and sized it.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Its laid-out content, padding included.
    pub content_w: f32,
    pub content_h: f32,
    /// Where it is scrolled to: the retained offset clamped to the travel
    /// below, so it is always a position within the content.
    pub offset_x: f32,
    pub offset_y: f32,
    /// How far the offset can travel; zero on an axis that does not scroll.
    pub max_offset_x: f32,
    pub max_offset_y: f32,
}

impl Default for KuiScrollGeometry {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            content_w: 0.0,
            content_h: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            max_offset_x: 0.0,
            max_offset_y: 0.0,
        }
    }
}

// SAFETY: `repr(C)` with `size: u32` first.
unsafe impl OutParam for KuiScrollGeometry {
    const ABI_V1_SIZE: u32 = abi_through!(KuiScrollGeometry, max_offset_y, f32);
    fn size_mut(&mut self) -> &mut u32 {
        &mut self.size
    }
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
            last_warnings: Vec::new(),
            last_access: Default::default(),
            open_tooltips: Vec::new(),
        };
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

    #[test]
    fn zeroed_structs_are_the_schema_defaults() {
        let out = PropsOut::new();
        assert_eq!(spec_of(&zeroed_spec(), NONE, NONE, NONE, NONE), out.spec);
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
// each field's offset, size and C type to what Rust actually lays out;
// examples/c/build.sh compiles it against the header, in CI too.

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
    /// `write_out` cannot be exercised this way through the public API
    /// today, because nothing has been appended to a real [out] struct yet
    /// (every `ABI_V1_SIZE` still equals its `size_of`). This stands in for
    /// the first append, so the truncating path is not first exercised by
    /// the change that depends on it.
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
        });
        abi_out_struct!(o, KuiEvent);

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
        std::fs::write(&path, text).expect("write kui-abi-assert.c");
    }
}
