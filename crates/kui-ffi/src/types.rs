//! Opaque handles and the repr(C) mirrors of the core's spec structs —
//! what `include/kui.h` declares, field for field. See the ABI notes in
//! `abi` for which side may write which struct.

use super::*;

// ---------------------------------------------------------------------------
// Opaque + repr(C) types

/// Opaque: a `Core` plus the pending event queue. The core is either owned
/// (standalone contexts from `kui_ctx_new`) or borrowed from the windowed
/// runner for the duration of a view callback — behind a pointer either way,
/// so every entry point works identically on both.
pub struct KuiCtx {
    pub(crate) core: *mut Core,
    /// Keep-alive for standalone contexts; never read directly.
    pub(crate) _owned: Option<Box<Core>>,
    pub(crate) events: Vec<UiEvent>,
    /// Payload most recently handed out by kui_poll_event; freed on the next
    /// poll (or context free) so C never manages event payload lifetime.
    pub(crate) last_payload: Option<Box<KuiValue>>,
    /// Text most recently handed out by kui_edit_text; freed on the next call.
    pub(crate) last_edit_text: Option<String>,
    /// Warnings most recently handed out by kui_take_warnings; their strings
    /// stay valid until the next call.
    pub(crate) last_warnings: Vec<kui_core::Warning>,
    /// Access tree most recently handed out by kui_access_tree; its strings
    /// stay valid until the next call.
    pub(crate) last_access: kui_core::AccessTree,
    /// Announcements most recently handed out by kui_take_announcements;
    /// their strings stay valid until the next call.
    pub(crate) last_announcements: Vec<kui_core::Announcement>,
    /// One entry per node the `kui_open*` family has open, holding its key
    /// and `KuiSpec.tooltip` hint. `kui_close` pops it and floats the hint
    /// as the node's last child while it is hovered — which is what the
    /// other bindings' `tooltip` prop does at the same point.
    pub(crate) open_tooltips: Vec<Option<(kui_core::Key, String)>>,
    /// Window commands taken from the core and not yet handed out one at a
    /// time by `kui_take_window_command`.
    pub(crate) window_commands: VecDeque<WindowCommand>,
    /// The name most recently handed out by kui_ctx_window_name; valid
    /// until the next call.
    pub(crate) last_window_name: Option<Rc<str>>,
    /// Which slot this context is a C extension's fill of, and the params
    /// the host passed it (ADR 0014): what `kui_slot_name` / `kui_slot_params`
    /// answer. `None` on every other context - a standalone one, a C host's
    /// view callback - where they answer false and NULL.
    pub(crate) slot_name: Option<String>,
    pub(crate) slot_namespace: Option<String>,
    pub(crate) slot_params: Option<KuiValue>,
}

impl KuiCtx {
    pub(crate) fn core(&mut self) -> &mut Core {
        unsafe { &mut *self.core }
    }

    /// A context that borrows someone else's frame instead of owning a
    /// `Core`: what `kui_run`'s view callback and a C extension's both get.
    /// Only the builder entry points are meaningful on one - the queues
    /// below stay empty, because the runner owns event delivery.
    ///
    /// The borrow is not in the type (`KuiCtx` is what C holds, and it has
    /// no lifetime), so the caller keeps it: use the context inside the
    /// scope this `&mut Core` came from and let it go at the end of it.
    pub(crate) fn borrowing(core: &mut Core) -> Self {
        Self {
            core,
            _owned: None,
            events: Vec::new(),
            last_payload: None,
            last_edit_text: None,
            last_warnings: Vec::new(),
            last_access: Default::default(),
            last_announcements: Vec::new(),
            open_tooltips: Vec::new(),
            window_commands: VecDeque::new(),
            last_window_name: None,
            slot_name: None,
            slot_namespace: None,
            slot_params: None,
        }
    }

    /// Records a just-opened node's hover hint for `kui_close`.
    pub(crate) fn push_tooltip(&mut self, key: kui_core::Key, hint: KuiStr) {
        self.open_tooltips
            .push(opt_str(hint).map(|h| (key, h.into_owned())));
    }
}

/// Opaque dynamic value (event payloads).
pub struct KuiValue(pub(crate) Value);

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
/// `KuiSpec.min_w` / `min_h` as the node's own fit size (`KUI_MIN_FIT`).
pub const KUI_MIN_FIT: f32 = -1.0;
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
/// `KuiSpec.float_mode`: in flow, or which rect the float attaches to. The
/// non-zero values are `kui_core::FLOAT_PRESETS` indices plus one, so zero
/// can still mean "no float".
pub const KUI_FLOAT_NONE: u32 = 0;
pub const KUI_FLOAT_PARENT: u32 = 1;
pub const KUI_FLOAT_VIEWPORT: u32 = 2;

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
    /// Clamps applied after sizing resolves; 0 for max means unconstrained,
    /// and a negative min (`KUI_MIN_FIT`) is the node's own fit size.
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
    /// `KUI_CLIP` | `KUI_SCROLL_X` | `KUI_SCROLL_Y`; the same bits the
    /// binary protocol carries, applied by `NodeSpec::overflow_bits`.
    pub overflow: u32,
    /// `KUI_FLOAT_NONE` (in flow), `KUI_FLOAT_PARENT` or `KUI_FLOAT_VIEWPORT`.
    /// `kui_spec_float_preset` fills this and the fields below from one of
    /// the named presets.
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
    /// commands (kui_take_window_command), never events.
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
    /// Non-zero: where focus lands when the enclosing `modal` scope is
    /// entered - the first node in the modal's Tab ring declaring it,
    /// instead of the ring's first, so a destructive confirm opens on its
    /// Cancel. Read on entry only; nothing declaring it (or only nodes
    /// the ring skips) keeps the ring's first node.
    pub initial_focus: u32,
    /// Exit: with `set` non-zero and a `transition_ms`, the frame after the
    /// view stops declaring this node its subtree is copied out of the last
    /// frame that had it and replayed — frozen where layout left it, on top
    /// of everything and inert — while the named slots ease from where they
    /// were to these values (see `KuiEnter`, which an exit reuses: an exit
    /// is an entrance read the other way).
    pub exit: KuiEnter,
    /// KUI_LIVE_* (0 = KUI_LIVE_OFF, the default): when the text inside
    /// this node changes, a screen reader reads the change without being
    /// asked. A node that declares it is semantic, so a plain box marked
    /// live is not elided from the access tree. For a one-off with no node
    /// behind it, `kui_announce` is the other half (see
    /// `docs/adr/0008-live-regions-and-announcements.md`).
    pub live: u32,
    /// Non-zero, with a non-NULL `on_key` on `kui_open_with`: the sink hears
    /// releases too, as the same `{kind="key"}` payload with `phase="up"`
    /// (`text` null, `repeat` false) — for a held-key interaction (WASD,
    /// press-and-hold, a key that arms a mode while it is down). A key only
    /// comes up where it went down, and focus leaving while a key is held
    /// delivers the `up` first. Zero: presses only, which is what a keymap
    /// wants — one that heard both halves would run every binding twice.
    pub key_up: u32,
    /// What a slider role's position reads as, empty for none (ARIA's
    /// `aria-valuetext`). Without one a reader has only `value_now` and the
    /// range and says a percentage — 25 in [5..60] is "36 percent" — so a
    /// value whose unit carries the meaning says it here: "25 minutes". It
    /// replaces the number in the reading rather than joining it, and a
    /// nudge announces the new text. Copied while the node opens. It
    /// arrives back as `KuiAccessNode.value` (KUI_ACCESS_HAS_VALUE), the
    /// one string slot a node has.
    pub value_text: KuiStr,
    /// The accessible description (empty = none): the extra sentence a
    /// reader says after the name, for what the name cannot say on its own
    /// — what a button will do, why a control is disabled, what format a
    /// field wants. `tooltip` is the shorthand that also draws the string
    /// and hover-tracks the node; this is the description alone, for a hint
    /// that is spoken and never drawn. Both write the one slot and this one
    /// is applied second, so it wins over a `tooltip` on the same node. It
    /// reads only on a node that reaches the access tree — a role, a label,
    /// a control — since a plain box is elided and takes its description
    /// with it. Borrowed while the node opens.
    pub description: KuiStr,
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
    /// The node's one string value (KUI_ACCESS_HAS_VALUE): an editor's
    /// text, or a slider's `value_text` — the platform has one slot, and a
    /// slider that named its reading reads as that instead of its number.
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
    /// KUI_ORIENTATION_* (0 = unset: this node is not a composite
    /// container). How the container arranges its items, from its own
    /// `dir` (`docs/adr/0007-composite-keyboard-patterns.md`).
    pub orientation: u32,
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
/// The node declared `live` (see `KuiSpec.live`), and which politeness.
/// Two bits rather than a `live` field, because `KuiAccessNode` is an
/// [out-array] struct that a host allocates: appending to it would be an
/// ABI break, and `flags` has room (see
/// `docs/adr/0006-c-abi-versioning.md`).
pub const KUI_ACCESS_LIVE_POLITE: u32 = 1 << 18;
pub const KUI_ACCESS_LIVE_ASSERTIVE: u32 = 1 << 19;

/// KUI_ORIENTATION_* is the position in `Orientation::ALL` plus one
/// (0 = unset: the node is not a composite container).
pub const KUI_ORIENTATION_HORIZONTAL: u32 = 1;
pub const KUI_ORIENTATION_VERTICAL: u32 = 2;

pub(crate) fn orientation_code(o: Option<kui_core::Orientation>) -> u32 {
    o.and_then(|o| kui_core::Orientation::ALL.iter().position(|x| *x == o))
        .map_or(0, |i| i as u32 + 1)
}

/// KUI_EXPANDED_* is the position in `schema::EXPANDED` plus one (0 = unset:
/// the node does not expand).
pub const KUI_EXPANDED_COLLAPSED: u32 = 1;
pub const KUI_EXPANDED_EXPANDED: u32 = 2;

/// KUI_LIVE_* is the position in `schema::LIVE` itself, not the position
/// plus one: unlike a disclosure, a live region's zero *is* a value —
/// "not a live region" is what an unset field already means, so there is
/// no unset state to reserve zero for.
pub const KUI_LIVE_OFF: u32 = 0;
pub const KUI_LIVE_POLITE: u32 = 1;
pub const KUI_LIVE_ASSERTIVE: u32 = 2;

/// One queued announcement (`kui_take_announcements`): something to say
/// once, with no node behind it. `live` is KUI_LIVE_POLITE or
/// KUI_LIVE_ASSERTIVE — never KUI_LIVE_OFF, which `kui_announce` drops.
/// `text` borrows the context's buffer and stays valid until the next
/// `kui_take_announcements` on the same context (see
/// `docs/adr/0008-live-regions-and-announcements.md`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiAnnouncement {
    pub text: KuiStr,
    pub live: u32,
}

pub const KUI_VALUE_NOW: u32 = 1 << 0;
pub const KUI_VALUE_MIN: u32 = 1 << 1;
pub const KUI_VALUE_MAX: u32 = 1 << 2;

/// KUI_ROLE_* is the position in `Role::ALL` plus one (0 = unset).
pub(crate) fn role_code(role: kui_core::Role) -> u32 {
    kui_core::Role::ALL
        .iter()
        .position(|r| *r == role)
        .map_or(0, |i| i as u32 + 1)
}

pub(crate) fn role_of_code(code: u32) -> Option<kui_core::Role> {
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
    /// Removal releases the playback instead of stopping it: it plays to
    /// its end. Appended after `paused`, which a host that predates it
    /// simply does not write — the zeroed tail is the old behaviour, so
    /// this is the compatible append `abi.rs` describes for an [in] struct.
    pub finish: u32,
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
    /// OpenType features for the shaper, in the spelling every binding
    /// shares: `tag=value` pairs separated by spaces or commas, a bare
    /// tag meaning 1 and `-tag` 0 (`"liga=0 calt=0"`, `"tnum"`). Empty
    /// (a zeroed `KuiStr`) is the font's defaults. Appended in the
    /// compatible way: a host predating it passes the shorter struct.
    pub features: KuiStr,
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

/// One polled event ([out]). `size` leads it so that `window` — ABI 4's
/// append, and anything after it — reaches a host that has not recompiled
/// as a shorter write rather than as a longer one.
#[repr(C)]
pub struct KuiEvent {
    /// Set to `sizeof(KuiEvent)` before the call (`KUI_EVENT_INIT` does);
    /// comes back as the number of bytes the library filled.
    pub size: u32,
    pub origin: u16,
    pub key: u64,
    /// Borrowed until the next `kui_poll_event`/`kui_ctx_free`; NULL if none.
    pub payload: *const KuiValue,
    /// Which window the event came from; 0 (`KUI_WINDOW_MAIN`) until ADR
    /// 0004's step 3 opens a second one.
    ///
    /// **Appended in ABI 4**, and the first field ever appended to an [out]
    /// struct. It sits after every ABI-1 field on purpose: `ABI_V1_SIZE`
    /// is measured through `payload`, so a host that reserved the old
    /// layout still passes [`out_accepts`] and still gets every byte it
    /// knows about — [`write_out`] simply stops before this one.
    pub window: u32,
}

impl Default for KuiEvent {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            origin: 0,
            key: 0,
            payload: std::ptr::null(),
            window: 0,
        }
    }
}

// SAFETY: `repr(C)` with `size: u32` first.
unsafe impl OutParam for KuiEvent {
    /// Through `payload`: the last field ABI 1 shipped, and so still the
    /// floor now that `window` follows it.
    const ABI_V1_SIZE: u32 = abi_through!(KuiEvent, payload, *const KuiValue);
    fn size_mut(&mut self) -> &mut u32 {
        &mut self.size
    }
}

/// What a declared window is ([in], `kui_window_declare`), and what an
/// `Open` command carries back out inside [`KuiWindowCommand`]. Read
/// literally, so start from `KUI_WINDOW_CONFIG_INIT` (a normal, activating
/// 640x480 window) or pass NULL for exactly that.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct KuiWindowConfig {
    /// `KUI_WINDOW_KIND_*`; 0 is a normal window.
    pub kind: u32,
    /// Initial inner size, logical px. Zero means the default.
    pub width: f32,
    pub height: f32,
    /// Whether opening it takes OS focus. Start a popup from
    /// `KUI_WINDOW_POPUP_INIT`, which clears it: a popup that takes focus
    /// blurs the field that opened it.
    pub activates: u32,
    /// `KUI_WINDOW_KIND_POPUP` only: the rect the popup is placed against,
    /// in the **declaring window's** logical coordinates — the `x`/`y`/`w`/
    /// `h` an `onLayout` event already reports for the field or button the
    /// menu belongs to. The host resolves it to screen coordinates against
    /// that window's own position. Ignored by a normal window, and read on
    /// the opening edge with the rest of the config.
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub anchor_w: f32,
    pub anchor_h: f32,
}

/// `KUI_WINDOW_KIND_NORMAL`: a regular top-level window.
pub const KUI_WINDOW_KIND_NORMAL: u32 = 0;
/// `KUI_WINDOW_KIND_POPUP`: a borderless, taskbar-less menu surface owned
/// by the window that declared it, placed against `anchor_*` in screen
/// coordinates and closed when its owner closes (ADR 0004 decision 9).
pub const KUI_WINDOW_KIND_POPUP: u32 = 1;

pub(crate) fn window_config_of(c: Option<&KuiWindowConfig>) -> WindowConfig {
    let Some(c) = c else {
        return WindowConfig::default();
    };
    let size = if c.width > 0.0 && c.height > 0.0 {
        Size::new(c.width, c.height)
    } else {
        WindowConfig::DEFAULT_SIZE
    };
    WindowConfig {
        // A kind this build does not have reads as a normal window, so a
        // host built against a later header degrades to a window rather
        // than to nothing. `kui_window_declare` raises `unknown-window-kind`
        // on the way past, so the degradation is reported and not silent.
        kind: match c.kind {
            KUI_WINDOW_KIND_POPUP => WindowKind::Popup,
            _ => WindowKind::Normal,
        },
        size,
        activates: c.activates != 0,
        anchor: Rect::new(c.anchor_x, c.anchor_y, c.anchor_w, c.anchor_h),
    }
}

fn window_config_to_c(c: WindowConfig) -> KuiWindowConfig {
    KuiWindowConfig {
        kind: match c.kind {
            WindowKind::Normal => KUI_WINDOW_KIND_NORMAL,
            WindowKind::Popup => KUI_WINDOW_KIND_POPUP,
        },
        width: c.size.w,
        height: c.size.h,
        activates: c.activates as u32,
        anchor_x: c.anchor.x,
        anchor_y: c.anchor.y,
        anchor_w: c.anchor.w,
        anchor_h: c.anchor.h,
    }
}

/// `KUI_DISMISS_OUTSIDE`, `KUI_DISMISS_ESCAPE`: why a window was asked to
/// go away (`kui_window_dismissed`).
pub const KUI_DISMISS_OUTSIDE: u32 = 0;
pub const KUI_DISMISS_ESCAPE: u32 = 1;

/// `KUI_CMD_START_DRAG`, `KUI_CMD_CLOSE`, `KUI_CMD_MINIMIZE`,
/// `KUI_CMD_TOGGLE_MAXIMIZE`: the verbs chrome nodes issue.
pub const KUI_CMD_START_DRAG: u32 = 1;
pub const KUI_CMD_CLOSE: u32 = 2;
pub const KUI_CMD_MINIMIZE: u32 = 3;
pub const KUI_CMD_TOGGLE_MAXIMIZE: u32 = 4;
/// `KUI_CMD_OPEN`: the declared set gained a window; `config` says what.
pub const KUI_CMD_OPEN: u32 = 5;
/// `KUI_CMD_SET_SIZE`: the app asked for a size (`kui_set_window_size`);
/// `width`/`height` carry it. `KUI_CMD_FOCUS`: it asked for focus.
pub const KUI_CMD_SET_SIZE: u32 = 6;
pub const KUI_CMD_FOCUS: u32 = 7;

/// One window command ([out], `kui_take_window_command`): what a chrome
/// node asked for, or what the declared window set's diff decided. Plain
/// data by ADR 0004 decision 5 — an `Open` carries no title (the window's
/// first frame declares one through `kui_window_title`) — so nothing
/// borrowed enters a host's drain loop. `size` leads it like every [out]
/// struct, so a field appended later reaches an older host as a shorter
/// write.
#[repr(C)]
pub struct KuiWindowCommand {
    /// Set to `sizeof(KuiWindowCommand)` before the call
    /// (`KUI_WINDOW_COMMAND_INIT` does); comes back as the bytes filled.
    pub size: u32,
    /// `KUI_CMD_*`.
    pub kind: u32,
    /// Which window: the one the chrome node was drawn in, or for
    /// `KUI_CMD_OPEN` the id the core assigned the new window — what its
    /// events will carry in `KuiEvent.window`.
    pub window: u32,
    /// `KUI_CMD_OPEN` only: which frontend's declaration won (0 = the host,
    /// 1+ = an extension), so a host can refuse an extension's window.
    pub origin: u16,
    /// `KUI_CMD_OPEN` only: the config from the declaration that opened it.
    pub config: KuiWindowConfig,
    /// `KUI_CMD_SET_SIZE` only: the size asked for, logical px. Appended in
    /// ABI 6, so a host that reserved through `config` never sees these —
    /// and never needs to, since only its own `kui_set_window_size` call
    /// can produce the verb that fills them. A `SetSize` is a bare size and
    /// not a `KuiWindowConfig`, the way it is in the core: a config is read
    /// on the opening edge only, and this moves a window that already
    /// exists.
    pub width: f32,
    pub height: f32,
    /// `KUI_CMD_OPEN` only: the window whose frame declared this one. For a
    /// `KUI_WINDOW_KIND_POPUP` it is the owner — the surface `config`'s
    /// `anchor_*` is measured against, the one to parent it to, and the one
    /// whose closing closes it. (The core closes it either way: an owner's
    /// declarations leave the declared set with it, so the same drain
    /// carries the popup's `KUI_CMD_CLOSE`.)
    pub owner: u32,
}

impl Default for KuiWindowCommand {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            kind: 0,
            window: 0,
            origin: 0,
            config: KuiWindowConfig::default(),
            width: 0.0,
            height: 0.0,
            owner: 0,
        }
    }
}

// SAFETY: `repr(C)` with `size: u32` first.
unsafe impl OutParam for KuiWindowCommand {
    /// Through `config`: the whole struct as it first shipped, in ABI 5.
    /// ABI 6's `width`/`height` and ABI 7's `owner` sit past it, so the
    /// floor never moved for those — but ABI 7 also grew `KuiWindowConfig`
    /// itself, by the four anchor floats a popup is placed against, and a
    /// field appended *inside* an embedded struct moves everything after
    /// it. So the floor is 16 bytes higher than the whole ABI-6 struct was,
    /// an ABI-6 host's reservation is refused rather than short-written
    /// (`out_accepts`), and `kui_abi_version()` is what catches that before
    /// it looks like an empty queue. The size handshake bounds the damage;
    /// only the version check prevents it.
    const ABI_V1_SIZE: u32 = abi_through!(KuiWindowCommand, config, KuiWindowConfig);
    fn size_mut(&mut self) -> &mut u32 {
        &mut self.size
    }
}

pub(crate) fn window_command_to_c(cmd: WindowCommand) -> KuiWindowCommand {
    let mut size = Size::new(0.0, 0.0);
    let mut owner = 0;
    let (kind, origin, config) = match cmd {
        WindowCommand::StartDrag(_) => (KUI_CMD_START_DRAG, 0, KuiWindowConfig::default()),
        WindowCommand::Close(_) => (KUI_CMD_CLOSE, 0, KuiWindowConfig::default()),
        WindowCommand::Minimize(_) => (KUI_CMD_MINIMIZE, 0, KuiWindowConfig::default()),
        WindowCommand::ToggleMaximize(_) => {
            (KUI_CMD_TOGGLE_MAXIMIZE, 0, KuiWindowConfig::default())
        }
        WindowCommand::Open {
            owner: o,
            origin,
            config,
            ..
        } => {
            owner = o.0;
            (KUI_CMD_OPEN, origin.0, window_config_to_c(config))
        }
        WindowCommand::SetSize { size: s, .. } => {
            size = s;
            (KUI_CMD_SET_SIZE, 0, KuiWindowConfig::default())
        }
        WindowCommand::Focus(_) => (KUI_CMD_FOCUS, 0, KuiWindowConfig::default()),
    };
    KuiWindowCommand {
        kind,
        window: cmd.window().0,
        origin,
        config,
        width: size.w,
        height: size.h,
        owner,
        ..Default::default()
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
    /// Atlas texels: x, y, w, h. `KUI_QUAD_SEGMENT`: the endpoints as
    /// float bits (see `kui_core::Quad::segment_ends`).
    pub uv: [u32; 4],
    /// Clip rect (physical px): x, y, w, h. Pixels outside are transparent.
    pub clip: [f32; 4],
    /// Corner radii of the clip (physical px), clockwise from the
    /// top-left: pixels outside the rounded clip are transparent too. All
    /// zero — every quad of a frame with no rounded clipper — is the plain
    /// rect clip.
    pub clip_radius: [f32; 4],
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

/// [out] Where a point landed in the text a keyed node drew
/// (`kui_text_hit`): a byte offset into that text, across the node's text
/// runs in order, and the visual (wrapped) line it is on.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiTextHit {
    /// [out] reservation; see `KUI_TEXT_HIT_INIT`.
    pub size: u32,
    pub line: u32,
    pub byte: u64,
}

impl Default for KuiTextHit {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            line: 0,
            byte: 0,
        }
    }
}

// SAFETY: `repr(C)` with `size: u32` first.
unsafe impl OutParam for KuiTextHit {
    const ABI_V1_SIZE: u32 = abi_through!(KuiTextHit, byte, u64);
    fn size_mut(&mut self) -> &mut u32 {
        &mut self.size
    }
}

/// [out] A caret rect (`kui_caret_rect`): logical px in viewport
/// coordinates, zero wide, one line tall.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiCaretRect {
    /// [out] reservation; see `KUI_CARET_RECT_INIT`.
    pub size: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Default for KuiCaretRect {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        }
    }
}

// SAFETY: `repr(C)` with `size: u32` first.
unsafe impl OutParam for KuiCaretRect {
    const ABI_V1_SIZE: u32 = abi_through!(KuiCaretRect, h, f32);
    fn size_mut(&mut self) -> &mut u32 {
        &mut self.size
    }
}

// SAFETY: `repr(C)` with `size: u32` first.
unsafe impl OutParam for KuiScrollGeometry {
    const ABI_V1_SIZE: u32 = abi_through!(KuiScrollGeometry, max_offset_y, f32);
    fn size_mut(&mut self) -> &mut u32 {
        &mut self.size
    }
}
