//! Opaque handles and the repr(C) mirrors of the core's spec structs —
//! what `include/kui.h` declares, field for field. See the ABI notes in
//! `abi` for which side may write which struct.

use super::*;

/// A C callback that builds into a context: `kui_run`'s view, and the body
/// of a `kui_titlebar_with` / `kui_tooltip_with`. Here rather than in
/// `run`, because the widgets take one whether or not this build has a
/// windowed runner.
pub(crate) type ViewFn = extern "C" fn(user: *mut c_void, ctx: *mut KuiCtx);
/// A C callback that receives one event: `kui_run`'s, and only its — so
/// unlike `ViewFn` it is not here in a build without the runner.
#[cfg(feature = "runner")]
pub(crate) type EventFn = extern "C" fn(user: *mut c_void, ev: *const KuiEvent);
/// A C callback that hears the window go: `kui_on_teardown`'s, with
/// `kui_run`'s `user` (backlog RG1). Runner-only, as `EventFn` is.
#[cfg(feature = "runner")]
pub(crate) type TeardownFn = extern "C" fn(user: *mut c_void);

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
    /// The WGSL module most recently handed out by `kui_fragment_source`;
    /// valid until the next call, like every other borrowed string here.
    pub(crate) fragment_source: String,
    /// This frame's fragment draws, in `KuiFragmentDraw` form, so
    /// `kui_draw_data` can hand out a pointer that outlives the call.
    pub(crate) fragment_draws: Vec<KuiFragmentDraw>,
    /// `kui_draw_data`'s transcription of the frame's texture draws.
    pub(crate) texture_draws: Vec<KuiTextureDraw>,
    /// What `kui_image_pixels` last handed out, so the pointer outlives
    /// the call.
    pub(crate) image_pixels: Option<std::sync::Arc<Vec<u8>>>,
    /// Warnings most recently handed out by kui_take_warnings; their strings
    /// stay valid until the next call.
    pub(crate) last_warnings: Vec<kui_core::Warning>,
    /// Access tree most recently handed out by kui_access_tree; its strings
    /// stay valid until the next call.
    pub(crate) last_access: kui_core::AccessTree,
    /// Announcements most recently handed out by kui_take_announcements;
    /// their strings stay valid until the next call.
    pub(crate) last_announcements: Vec<kui_core::Announcement>,
    /// Window commands taken from the core and not yet handed out one at a
    /// time by `kui_take_window_command`.
    pub(crate) window_commands: VecDeque<WindowCommand>,
    /// The same for the menu actions `kui_take_menu_action` hands out, and
    /// the text of the one most recently handed out — borrowed by the
    /// caller until the next call, like every other string here.
    pub(crate) menu_actions: VecDeque<kui_core::MenuAction>,
    pub(crate) menu_text: String,
    pub(crate) menu_html: String,
    /// The accelerator most recently handed out by `kui_menu_bar_item`,
    /// on the same terms as `menu_text` beside it.
    pub(crate) menu_accel: String,
    /// The chord most recently handed out by `kui_devtools_key`, on the
    /// same terms.
    pub(crate) devtools_key: String,
    /// The file dialog `kui_take_file_request` most recently handed out,
    /// whose strings — and the filter `kui_file_request_filter` read,
    /// its extensions joined — are borrowed until the next call (backlog
    /// C51).
    pub(crate) file_request: Option<kui_core::FileDialog>,
    pub(crate) file_filter_text: String,
    /// The tab name most recently handed out by `kui_devtools_current_tab`,
    /// on the same terms.
    pub(crate) devtools_tab: String,
    /// The selection most recently handed out by `kui_selection_text` /
    /// `kui_selection_html`; valid until the next such call, like every
    /// other borrowed string here.
    pub(crate) selection_text: String,
    pub(crate) selection_html: String,
    /// The family names most recently handed out by `kui_font_families`,
    /// held so the `KuiStr`s written into the host's array stay valid
    /// until the next call.
    pub(crate) font_families: Vec<String>,
    /// The families most recently handed out by `kui_system_fonts`, held
    /// for the same reason: their names and weights are what the
    /// `KuiSystemFont`s written into the host's array point at.
    pub(crate) system_fonts: Vec<kui_core::SystemFont>,
    /// The node list most recently handed out by `kui_nodes`; borrowed
    /// until the next call, like every other reading here.
    pub(crate) nodes: Option<Box<KuiValue>>,
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
    /// The extensions this context hosts, in origin order (ADR 0014).
    /// `kui_ctx_add_extension` fills it, `kui_slot` fills *them* in place,
    /// `kui_frame_finish` lets them take `ns/root` and warn about slots
    /// nobody declared, and an event whose origin names one is delivered to
    /// it rather than queued for the host. `kui_run_with` moves it into
    /// the window's runner. Empty on a borrowing context: an extension
    /// does not host extensions, and a host's under the runner is the
    /// runner's.
    pub(crate) extensions: kui_core::Extensions,
    /// Why the last `kui_ctx_add_extension` said false. Valid until the
    /// next call, like every other borrowed string here.
    pub(crate) last_ext_error: String,
    /// Set only for the length of a `kui_run_with` view callback: the
    /// runner's own `Ui`, erased to a pointer because `KuiCtx` is what C
    /// holds and has no lifetime to carry one.
    ///
    /// It exists because under the windowed runner the extension list is
    /// the *runner's*, not this context's — `kui_native::Launcher` owns it, and
    /// the `Ui` it built the frame with is what knows how to fill a slot.
    /// So `kui_slot` hands the declaration to that `Ui` when this is set,
    /// and fills from `extensions` when it is not. Null on every other
    /// context: a standalone one, and an extension's own.
    pub(crate) host_ui: *mut c_void,
}

impl KuiCtx {
    pub(crate) fn core(&mut self) -> &mut Core {
        unsafe { &mut *self.core }
    }

    /// Takes the core a standalone context owns, for `kui_run_with` to
    /// open the window on (backlog AR27), and leaves the context a fresh
    /// one so that it stays a context — still the caller's to free, and
    /// to use, as one that has registered nothing. `None` on a borrowing
    /// context, whose core is someone else's frame.
    #[cfg(any(feature = "runner", test))]
    pub(crate) fn take_core(&mut self) -> Option<Box<Core>> {
        let taken = self._owned.take()?;
        let mut fresh = Box::new(Core::new());
        fresh.set_diagnostics(false);
        self.core = &mut *fresh;
        self._owned = Some(fresh);
        Some(taken)
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
            fragment_source: String::new(),
            fragment_draws: Vec::new(),
            texture_draws: Vec::new(),
            image_pixels: None,
            last_warnings: Vec::new(),
            last_access: Default::default(),
            last_announcements: Vec::new(),
            window_commands: VecDeque::new(),
            menu_actions: VecDeque::new(),
            menu_text: String::new(),
            menu_accel: String::new(),
            devtools_key: String::new(),
            file_request: None,
            file_filter_text: String::new(),
            devtools_tab: String::new(),
            menu_html: String::new(),
            selection_text: String::new(),
            selection_html: String::new(),
            font_families: Vec::new(),
            system_fonts: Vec::new(),
            nodes: None,
            last_window_name: None,
            slot_name: None,
            slot_namespace: None,
            slot_params: None,
            extensions: Default::default(),
            last_ext_error: String::new(),
            host_ui: std::ptr::null_mut(),
        }
    }

    /// [`Self::borrowing`] for the windowed runner's view callback, which
    /// has a whole `Ui` rather than a bare `Core`: same context, plus the
    /// pointer `kui_slot` needs to reach the runner's extensions. Valid
    /// for the callback and not one instruction longer, like the borrow
    /// itself.
    pub(crate) fn borrowing_in(ui: &mut kui_core::Ui<'_>) -> Self {
        // `borrowing` keeps the core as a raw pointer, so the borrow of
        // `ui` it takes ends when it returns and the `Ui` can be kept too.
        let mut this = Self::borrowing(ui.core());
        this.host_ui = std::ptr::from_mut(ui).cast();
        this
    }

    /// Takes a batch of events the core just produced: the host's own are
    /// queued for `kui_poll_event`, and one whose origin names a loaded
    /// extension is delivered to it instead, its replies queued in its
    /// place (ADR 0014 decision 6). `Extensions::route` is the walk, the
    /// same one the Rust runner's `route_events` takes.
    pub(crate) fn absorb(&mut self, events: impl IntoIterator<Item = UiEvent>) {
        let out = &mut self.events;
        self.extensions.route(events, |ev| out.push(ev));
    }

    /// Records a just-opened node's hover hint: the core floats it on
    /// `kui_close` while the node is hovered (`Core::hint`).
    pub(crate) fn push_tooltip(&mut self, key: kui_core::Key, hint: KuiStr) {
        if let Some(h) = opt_str(hint) {
            self.core().hint(key, h.into_owned());
        }
    }
}

/// Opaque dynamic value (event payloads). Transparent over `Value`, which
/// is what lets `kui_value_get` / `kui_value_at` / `kui_value_entry` hand
/// out a borrowed `*const Value` as a `*const KuiValue`.
#[repr(transparent)]
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
    /// 0 = column, 1 = row, 2 = table (`KUI_TABLE`: a column whose rows'
    /// children line up in columns, ADR 0033)
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
    /// KUI_VALUE_CARET_SOLID beside KUI_VALUE_CARET says the caret is
    /// solid — a block caret in a modal editor's normal mode — so
    /// `kui_has_caret` leaves it out and no blink clock is armed on it,
    /// while it still anchors the IME and reads to assistive technology.
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
    /// KUI_CURSOR_* (0 = unset: the I-beam over text, the arrow otherwise
    /// — a clickable or draggable node included). The pointer shape while
    /// the pointer is over this node; a node with only a cursor is
    /// hover-tracked so it can be found.
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
    /// Non-zero: ask for another frame after this one, every frame this
    /// node is declared (`animate`). What a `kui_fragment` reading `time`
    /// needs. Opt-in, because it takes the loop off input-driven; one node
    /// asking is enough for the window.
    pub animate: u32,
    /// Non-zero: paint this node's background in the OS accent colour
    /// (`kui_env_set_system`), keeping `bg` where the host never said what
    /// it is. On `kui_button_with` it takes the hover and pressed shades
    /// and the label colour with it.
    pub accent: u32,
    /// Non-zero: this node is a selection scope — the text of every node
    /// inside it selects as one run, and a press-drag across them takes
    /// the lot (`docs/adr/0017-selection-as-a-scope.md`). Declared on the
    /// container, not on each label. Appended after ABI 11 without a
    /// bump, the way `accent` was — under the [in] rule as it then stood
    /// (`abi.rs`, backlog AR50).
    pub selectable: u32,
    /// Force-click tag (`on_force_click`): a press that deepens past the
    /// second stage of a Force Touch trackpad over this node emits
    /// `{kind:"forceclick", x, y, tag}` on it. Borrowed while the node
    /// opens, like every other tag. Appended after ABI 11 without a bump,
    /// under the [in] rule as it then stood.
    pub on_force_click: *const KuiValue,
    /// Non-zero: this node's subtree is a focus region — a Tab ring of its
    /// own that the ring outside never enters and that never leaves
    /// (`docs/adr/0022-focus-regions.md`). Entered on purpose:
    /// `kui_focus_region`, a press inside it, or a focus on a node in it.
    /// Nothing else about the node changes. Appended after ABI 13 without
    /// a bump, under the [in] rule as it then stood.
    pub focus_region: u32,
    /// When this node's scrollbars are drawn: `KUI_SCROLLBAR_*` (the
    /// `scrollbar` row's index plus one), 0 for the default, which is
    /// `KUI_SCROLLBAR_VISIBLE`. Appended after ABI 13 without a bump, like
    /// the three below.
    pub scrollbar: u32,
    /// The thumb's width at rest, logical px; 0 for the stock 4. Under
    /// the pointer or dragged it is 2 px wider.
    pub scrollbar_width: f32,
    /// The thumb at rest and under the pointer, `0xRRGGBBAA`; 0 (fully
    /// transparent, like every unset colour) for the theme's `scrollbar`
    /// and `scrollbar_active`.
    pub scrollbar_color: u32,
    pub scrollbar_active_color: u32,
    /// Non-zero: scroll anchoring on this scrolling node (backlog C26,
    /// CSS's `overflow-anchor`) — the first child in view keeps its place
    /// on screen when the content before it changes size. Appended after
    /// ABI 14 without a bump, under the [in] rule as it then stood.
    pub anchor: u32,
    /// Scroll tag (`on_scroll`): the wheel over this node emits
    /// `{kind:"scroll", x, y, dx, dy, lines, tag}` on it instead of
    /// scrolling anything — `lines` the whole lines a `cells` grid's delta
    /// covers, null elsewhere — and a drag-select held past a grid's edge
    /// arrives the same way once a frame
    /// (`docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md`).
    /// Borrowed while the node opens, like every other tag. Appended after
    /// ABI 15 without a bump, under the [in] rule as it then stood — the
    /// last append that rule covered.
    pub on_scroll: *const KuiValue,
    /// Drop-zone tag (`on_drop`, ADR 0031): files dragged in from the OS
    /// over this node emit `{kind:"drop", phase, paths, x, y, tag}` on it
    /// — `enter`, `move`, `leave`, `drop`. A node inside a zone is the
    /// zone's; a node that is no zone is looked past. Borrowed while the
    /// node opens, like every other tag. ABI 18 (the first append under
    /// the amended rule).
    pub on_drop: *const KuiValue,
    /// Background while dragged files are over this node, `0xRRGGBBAA`;
    /// 0 for none. Wins over `pressed_bg`, `focus_bg` and `hover_bg`;
    /// eases with `transition`. ABI 18.
    pub drop_bg: u32,
    /// Non-zero: a `KUI_FLOAT_PARENT` float takes its parent's clip, as a
    /// child does, instead of escaping every ancestor's — cut at a `clip`
    /// canvas's edge and not hit past it (`FloatConfig::clip`, backlog
    /// F90). Read with the parent anchor only; still painted as a layer
    /// over its in-flow siblings. Beside `float_fit` in meaning, at the
    /// end of the struct because that is where an append goes. ABI 19.
    pub float_clip: u32,
    /// Width over height (`aspectRatio`, backlog C14); 0 for none. It
    /// sizes the axis whose sizing is fit: a fit height from the final
    /// width, a fit width from a fixed height. ABI 19.
    pub aspect_ratio: f32,
    /// A checkbox that is neither on nor off (`mixed`, ADR 0034): read as
    /// mixed whatever `checked` says, drawn as a dash by `kui_checkbox`.
    /// ABI 19.
    pub mixed: u32,
    /// A slider's step (`valueStep`, ADR 0034), present when
    /// `KUI_VALUE_STEP` is in `value_set`. ABI 19.
    pub value_step: f32,
    /// A slider's change tag (`onChange`, ADR 0034): the core turns a
    /// press, a drag, the arrows, PageUp / PageDown and Home / End into
    /// `{kind:"change", value, phase, tag}`. Borrowed while the node
    /// opens, like every other tag. ABI 19.
    pub on_change: *const KuiValue,
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
pub const KUI_VALUE_CARET_SOLID: u32 = 1 << 5;
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
    /// Logical px, viewport coordinates, cut to the node's clip (zero-size
    /// on the clip's edge when wholly clipped).
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
/// A checkbox that is neither on nor off (`KuiSpec.mixed`, ADR 0034); set
/// beside `KUI_ACCESS_CHECKED_SET`, whose `KUI_ACCESS_CHECKED` it outranks.
pub const KUI_ACCESS_MIXED: u32 = 1 << 20;
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

/// KUI_SCROLLBAR_* is the position in `schema::SCROLLBARS` plus one (0 =
/// unset, which is the stock visible bar).
pub const KUI_SCROLLBAR_VISIBLE: u32 = 1;
pub const KUI_SCROLLBAR_HIDDEN: u32 = 2;
pub const KUI_SCROLLBAR_AUTO: u32 = 3;

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
/// `KuiSpec.value_step` holds (ADR 0034).
pub const KUI_VALUE_STEP: u32 = 1 << 6;

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

/// The palette a frame paints with ([out] for `kui_theme`, [in] for
/// `kui_theme_set`): one `0xRRGGBBAA` per role, derived from what the host
/// reported through `kui_env_set_system` unless it pinned something else.
/// See `docs/adr/0019-a-theme-derived-from-appearance-and-accent.md` and
/// the Theme table in `docs/props.md`.
///
/// The roles are `kui_core::schema::THEME_ROLES` field for field, in that
/// order, and `theme_struct_covers_every_role` pins the two together. A
/// role added there fails that test until it is appended here — and an
/// append here **bumps `KUI_ABI_VERSION`**, by the rule in `abi`: the
/// `size` handshake keeps the append from corrupting a host that sets it
/// (`write_out` fills only what was reserved), but a host that skipped
/// `kui_abi_version()` would take the short write unaware, which is what
/// ABI 4 bumped for. The [in] direction has no handshake at all —
/// `kui_theme_set` reads the struct the host filled, so an appended field
/// is read past an old host's shorter one — which is the second reason.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiTheme {
    /// [out] reservation; see `KUI_THEME_INIT`. Ignored by
    /// `kui_theme_set`, which reads the struct the host filled.
    pub size: u32,
    /// Which base this came from: `KUI_APPEARANCE_UNKNOWN` (0, the dark
    /// base without claiming the user chose it) / `_LIGHT` / `_DARK`.
    pub appearance: u32,
    /// What a disabled control's opacity is multiplied by.
    pub disabled_opacity: f32,
    pub bg: u32,
    pub surface: u32,
    pub raised: u32,
    pub sunken: u32,
    pub border: u32,
    pub border_strong: u32,
    pub fg: u32,
    pub muted: u32,
    pub faint: u32,
    pub accent: u32,
    pub accent_hover: u32,
    pub accent_pressed: u32,
    pub on_accent: u32,
    pub accent_soft: u32,
    pub selection: u32,
    pub focus_ring: u32,
    pub hover: u32,
    pub pressed: u32,
    pub success: u32,
    pub warning: u32,
    pub danger: u32,
    pub scrollbar: u32,
    pub scrollbar_active: u32,
}

// Hand-written for the same reason `KuiTextMetrics`'s is: a zeroed `size`
// is the one value the handshake refuses.
impl Default for KuiTheme {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            appearance: 0,
            disabled_opacity: 0.0,
            bg: 0,
            surface: 0,
            raised: 0,
            sunken: 0,
            border: 0,
            border_strong: 0,
            fg: 0,
            muted: 0,
            faint: 0,
            accent: 0,
            accent_hover: 0,
            accent_pressed: 0,
            on_accent: 0,
            accent_soft: 0,
            selection: 0,
            focus_ring: 0,
            hover: 0,
            pressed: 0,
            success: 0,
            warning: 0,
            danger: 0,
            scrollbar: 0,
            scrollbar_active: 0,
        }
    }
}

// SAFETY: `repr(C)` with `size: u32` first.
unsafe impl OutParam for KuiTheme {
    const ABI_V1_SIZE: u32 = abi_through!(KuiTheme, scrollbar_active, u32);
    fn size_mut(&mut self) -> &mut u32 {
        &mut self.size
    }
}

/// The sizes the stock widgets are built from (`kui_metrics`,
/// `kui_metrics_set`; backlog T2): `kui_core::schema::METRIC_ROLES` field
/// for field, in that order, and `metrics_struct_covers_every_role` pins
/// the two together as `KuiTheme` is pinned to its roles. Logical px,
/// before the scale factor. An append here bumps `KUI_ABI_VERSION`, by
/// the rule `KuiTheme` states.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiMetrics {
    /// [out] reservation; see `KUI_METRICS_INIT`. Ignored by
    /// `kui_metrics_set`, which reads the struct the host filled.
    pub size: u32,
    pub control_text: f32,
    pub chrome_text: f32,
    pub hint_text: f32,
    pub radius: f32,
    pub radius_inner: f32,
    pub control_pad_x: f32,
    pub control_pad_y: f32,
    pub field_pad_x: f32,
    pub field_pad_y: f32,
    pub hint_pad_x: f32,
    pub hint_pad_y: f32,
    pub menu_pad_x: f32,
    pub menu_pad_y: f32,
    pub menu_width: f32,
    pub menu_bar_h: f32,
    pub titlebar_h: f32,
}

impl Default for KuiMetrics {
    fn default() -> Self {
        Self::of(&kui_core::Metrics::default())
    }
}

impl KuiMetrics {
    /// A core `Metrics` as the host reads it, through the role table so
    /// the two cannot disagree on a field.
    pub(crate) fn of(m: &kui_core::Metrics) -> Self {
        let mut out = Self {
            size: std::mem::size_of::<Self>() as u32,
            control_text: 0.0,
            chrome_text: 0.0,
            hint_text: 0.0,
            radius: 0.0,
            radius_inner: 0.0,
            control_pad_x: 0.0,
            control_pad_y: 0.0,
            field_pad_x: 0.0,
            field_pad_y: 0.0,
            hint_pad_x: 0.0,
            hint_pad_y: 0.0,
            menu_pad_x: 0.0,
            menu_pad_y: 0.0,
            menu_width: 0.0,
            menu_bar_h: 0.0,
            titlebar_h: 0.0,
        };
        for role in kui_core::schema::METRIC_ROLES {
            *out.field_mut(role.name) = (role.get)(m);
        }
        out
    }

    /// The host's struct as a core `Metrics`, through the same table.
    pub(crate) fn to_core(self) -> kui_core::Metrics {
        let mut m = kui_core::Metrics::default();
        for role in kui_core::schema::METRIC_ROLES {
            (role.set)(&mut m, *self.field(role.name));
        }
        m
    }

    fn field(&self, name: &str) -> &f32 {
        match name {
            "control_text" => &self.control_text,
            "chrome_text" => &self.chrome_text,
            "hint_text" => &self.hint_text,
            "radius" => &self.radius,
            "radius_inner" => &self.radius_inner,
            "control_pad_x" => &self.control_pad_x,
            "control_pad_y" => &self.control_pad_y,
            "field_pad_x" => &self.field_pad_x,
            "field_pad_y" => &self.field_pad_y,
            "hint_pad_x" => &self.hint_pad_x,
            "hint_pad_y" => &self.hint_pad_y,
            "menu_pad_x" => &self.menu_pad_x,
            "menu_pad_y" => &self.menu_pad_y,
            "menu_width" => &self.menu_width,
            "menu_bar_h" => &self.menu_bar_h,
            "titlebar_h" => &self.titlebar_h,
            other => panic!("METRIC_ROLES names a metric KuiMetrics lacks: {other}"),
        }
    }

    fn field_mut(&mut self, name: &str) -> &mut f32 {
        match name {
            "control_text" => &mut self.control_text,
            "chrome_text" => &mut self.chrome_text,
            "hint_text" => &mut self.hint_text,
            "radius" => &mut self.radius,
            "radius_inner" => &mut self.radius_inner,
            "control_pad_x" => &mut self.control_pad_x,
            "control_pad_y" => &mut self.control_pad_y,
            "field_pad_x" => &mut self.field_pad_x,
            "field_pad_y" => &mut self.field_pad_y,
            "hint_pad_x" => &mut self.hint_pad_x,
            "hint_pad_y" => &mut self.hint_pad_y,
            "menu_pad_x" => &mut self.menu_pad_x,
            "menu_pad_y" => &mut self.menu_pad_y,
            "menu_width" => &mut self.menu_width,
            "menu_bar_h" => &mut self.menu_bar_h,
            "titlebar_h" => &mut self.titlebar_h,
            other => panic!("METRIC_ROLES names a metric KuiMetrics lacks: {other}"),
        }
    }
}

// SAFETY: `repr(C)` with `size: u32` first.
unsafe impl OutParam for KuiMetrics {
    const ABI_V1_SIZE: u32 = abi_through!(KuiMetrics, titlebar_h, f32);
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

/// One installed or loaded font family (`kui_system_fonts`), as the font
/// database read its faces (backlog F97): what `SystemFont` says, laid
/// out for C. `family` and `weights` are borrowed until the next
/// `kui_system_fonts` on the same context.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiSystemFont {
    /// The name `kui_font_add_system` takes.
    pub family: KuiStr,
    /// The weights its faces come in (400 regular, 700 bold), sorted,
    /// each once; `weight_count` of them.
    pub weights: *const u16,
    pub weight_count: u32,
    /// 1 when every face says it is fixed-pitch.
    pub monospaced: u32,
    /// 1 when it has an italic or an oblique face.
    pub italic: u32,
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
    /// its end. Appended after `paused` without a bump, under the [in]
    /// rule as it then stood (`abi.rs`); zero is the old behaviour.
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
    /// (a zeroed `KuiStr`) is the font's defaults. Appended without a
    /// bump, under the [in] rule as it then stood (`abi.rs`).
    pub features: KuiStr,
    /// `KUI_DECO_UNDERLINE` | `KUI_DECO_STRIKETHROUGH`: lines where the
    /// face puts them, over every glyph. Paint only. Appended without a
    /// bump, the same way (backlog C22).
    pub decoration: u32,
    /// The underline's own colour as `0xRRGGBBAA`, 0 for the text's;
    /// non-zero implies `KUI_DECO_UNDERLINE` (backlog K4). ABI 17.
    pub underline_color: u32,
    /// `KUI_UNDERLINE_SOLID` / `_WAVY` / `_DOTTED`; a non-solid style
    /// implies `KUI_DECO_UNDERLINE` (backlog K4). ABI 17.
    pub underline_style: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiSpan {
    pub text: KuiStr,
    /// 0xRRGGBBAA; 0 = inherit the paragraph color
    pub color: u32,
    /// `KUI_SPAN_BOLD` | `KUI_SPAN_ITALIC` | `KUI_SPAN_UNDERLINE` |
    /// `KUI_SPAN_STRIKETHROUGH`
    pub flags: u32,
    /// 0xRRGGBBAA behind the span's glyphs alone, one rect per line the
    /// span covers; 0 = none. Its append is ABI 8: spans travel as an
    /// array, so the stride moved (backlog C22).
    pub bg: u32,
    /// The underline's own colour, 0 for the span's (backlog K4); non-zero
    /// implies `KUI_SPAN_UNDERLINE`. ABI 17.
    pub underline_color: u32,
    /// `KUI_UNDERLINE_*`; non-solid implies `KUI_SPAN_UNDERLINE`. ABI 17.
    pub underline_style: u32,
}

/// One colour token as `kui_tokens_set` reads it
/// (`docs/adr/0027-tokens-beside-the-theme.md`): a name and a value per
/// base, `0xRRGGBBAA` each — the same value twice for a colour that does
/// not follow the appearance. [in], and it travels as an array, so an
/// append here moves the stride and is an ABI bump (the `KuiSpan` rule).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiColorToken {
    pub name: KuiStr,
    pub light: u32,
    pub dark: u32,
}

/// One length token: a name and logical px, before the scale factor.
/// [in], array-carried like `KuiColorToken`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiLengthToken {
    pub name: KuiStr,
    pub value: f32,
}

/// One step of a derived colour token's recipe as `kui_tokens_derive`
/// reads it (`docs/adr/0028-derived-tokens.md`): the verb as one of the
/// `KUI_OP_*` numbers, the number it takes, and — for `KUI_OP_MIX` and
/// `KUI_OP_READABLE` only — the colour token or role the verb names,
/// empty otherwise. [in], array-carried, so an append moves the stride
/// and is an ABI bump.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiColorOp {
    pub op: u8,
    pub t: f32,
    pub other: KuiStr,
}

/// One derived colour token: a name, the colour token or theme role it
/// derives from, and its chain of ops in order — none for an alias.
/// [in], array-carried like `KuiColorToken`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiDerivedToken {
    pub name: KuiStr,
    pub from: KuiStr,
    pub ops: *const KuiColorOp,
    pub op_count: usize,
}

/// The verbs of `KuiColorOp.op`, numbered as `kui_core::ColorOp::VERBS`
/// lists them.
pub const KUI_OP_LIFT: u8 = 0;
pub const KUI_OP_DARKEN: u8 = 1;
pub const KUI_OP_RAISE: u8 = 2;
pub const KUI_OP_ALPHA: u8 = 3;
pub const KUI_OP_MIX: u8 = 4;
pub const KUI_OP_READABLE: u8 = 5;

/// Where a `kui_reply` from inside `kui_ext_on_event` sends what it is
/// given: an opaque handle the library hands the plugin on the event, and
/// takes back when the callback returns.
///
/// It is a struct with a function pointer rather than a `Vec` the plugin
/// pushes into, and that is the whole point. A plugin may be linked against
/// a *different copy of this library* than its host — on Windows it has to
/// be, because a DLL cannot leave `kui_reply` undefined and resolve it from
/// the executable the way ELF does — and then the `kui_reply` it calls is
/// not the one the host is collecting from. A process-global sink (which is
/// what this was until ABI 10: a `thread_local` keyed by the event) has one
/// per copy, so every reply landed in a list nobody read and the host saw
/// silence. A function pointer *into the host's copy*, carried on the event,
/// has none of that: the plugin's `kui_reply` forwards, the host's code does
/// the work, and only this two-field header has to have the same layout on
/// both sides.
///
/// `push` is cleared when the callback returns, so a plugin that stored the
/// event and calls later gets `false` rather than a reply nobody asked for —
/// best-effort, not a guarantee, since the event itself is only alive for
/// the call (like its payload, and like the `KuiCtx` a view is handed).
#[repr(C)]
pub struct KuiReplySink {
    /// Called with the sink and the value; false if the sink is closed.
    pub(crate) push: Option<unsafe extern "C" fn(*mut KuiReplySink, *const KuiValue) -> bool>,
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
    /// Where `kui_reply` sends a reply to this event, or NULL when there is
    /// nowhere to send one — every event a host polls with
    /// `kui_poll_event`, and any event outside a plugin's
    /// `kui_ext_on_event`. See [`KuiReplySink`].
    ///
    /// **Appended in ABI 10**, after `window`, so a host that reserved the
    /// older layout still passes [`out_accepts`] and still gets every byte
    /// it knows about. A host has no use for it: it is the library's own
    /// channel to a plugin, and the plugin passes the event straight back.
    pub reply_sink: *mut KuiReplySink,
    /// The key of the slot whose fill drew the node — what `kui_key_of`
    /// answers for the slot's full name — or 0 for a node the host drew
    /// itself (backlog K2). **Appended 2026-09-15**, after `reply_sink`,
    /// under the [out] rule and without a bump: a host reserving the older
    /// layout never sees it.
    pub slot: u64,
}

impl Default for KuiEvent {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            origin: 0,
            key: 0,
            payload: std::ptr::null(),
            window: 0,
            reply_sink: std::ptr::null_mut(),
            slot: 0,
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

/// How `kui_run_with` opens its window ([in]; backlog AR27): the
/// `Launcher` options a Rust host has and Node's `WindowOptions` carry,
/// as one struct. Read literally, so start from `KUI_RUN_CONFIG_INIT`,
/// which is every zero, or pass NULL for exactly that — a 960x640 native
/// window, unbounded, antialiasing chosen by the GPU, diagnostics as the
/// build has them.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct KuiRunConfig {
    /// Initial inner size, logical px; zero is the default. Clamped into
    /// the bounds below, as the OS would. `KUI_WINDOW=WxH` in the
    /// environment still overrides.
    pub width: f32,
    pub height: f32,
    /// Smallest and largest inner size the user may resize to, logical
    /// px; a zero side is unbounded, so a lone `min_h` stands. A max
    /// below its min loses to it, as on the OS side.
    pub min_w: f32,
    pub min_h: f32,
    pub max_w: f32,
    pub max_h: f32,
    /// `KUI_CHROME_*`: native decorations, the custom titlebar a
    /// `kui_titlebar` draws, or none.
    pub chrome: u32,
    /// `KUI_TEXT_AA_*`; `KUI_TEXT_AA=gray|subpixel` in the environment
    /// still overrides, for an A/B by hand.
    pub text_aa: u32,
    /// `KUI_DIAG_*`: whether the core runs its checks and the runner
    /// prints them to stderr. The window's setting, over whatever
    /// `kui_set_diagnostics` set on the context handed in; the default is
    /// the build's — on in a debug build, off in release — as `kui_run`
    /// always had it.
    pub diagnostics: u32,
    /// Frames queued ahead of the one on screen (backlog C47); zero is
    /// the default, two. `KUI_FRAME_LATENCY` in the environment still
    /// overrides. ABI 19.
    pub frame_latency: u32,
}

/// `KUI_CHROME_NATIVE`: the OS's decorations.
pub const KUI_CHROME_NATIVE: u32 = 0;
/// `KUI_CHROME_CUSTOM`: undecorated; the view draws a `kui_titlebar` and
/// the runner synthesizes edge resizing and double-click maximize.
pub const KUI_CHROME_CUSTOM: u32 = 1;
/// `KUI_CHROME_BORDERLESS`: no decorations and no chrome expectations.
pub const KUI_CHROME_BORDERLESS: u32 = 2;
/// `KUI_TEXT_AA_AUTO`: LCD subpixel coverage when the GPU can blend per
/// channel, grayscale otherwise.
pub const KUI_TEXT_AA_AUTO: u32 = 0;
/// `KUI_TEXT_AA_GRAYSCALE`.
pub const KUI_TEXT_AA_GRAYSCALE: u32 = 1;
/// `KUI_TEXT_AA_SUBPIXEL`.
pub const KUI_TEXT_AA_SUBPIXEL: u32 = 2;
/// `KUI_DIAG_DEFAULT`: the build's — on in debug, off in release.
pub const KUI_DIAG_DEFAULT: u32 = 0;
/// `KUI_DIAG_ON`.
pub const KUI_DIAG_ON: u32 = 1;
/// `KUI_DIAG_OFF`.
pub const KUI_DIAG_OFF: u32 = 2;

/// A `KuiRunConfig` read: what `kui_run_with` tells the launcher, in the
/// launcher's own terms. Separate from the launcher so the reading is a
/// test without a window.
#[cfg(any(feature = "runner", test))]
#[derive(Debug, PartialEq, Default)]
pub(crate) struct RunOptions {
    pub size: Option<(f64, f64)>,
    pub min_size: Option<(f64, f64)>,
    pub max_size: Option<(f64, f64)>,
    /// `KUI_CHROME_*`, checked.
    pub chrome: u32,
    /// `KUI_TEXT_AA_*`, checked.
    pub text_aa: u32,
    /// `None` is the build's default.
    pub diagnostics: Option<bool>,
    /// `None` is the launcher's default.
    pub frame_latency: Option<u32>,
}

/// A max side left at zero is unbounded: a bound no display reaches, as
/// Node's `maxWidth` alone is.
#[cfg(any(feature = "runner", test))]
pub(crate) const UNBOUNDED_SIZE: f64 = 65_535.0;

/// Reads a config the way `kui_window_declare` reads its own — literally,
/// NULL for the defaults — except that a word this build does not have is
/// refused with its reason rather than degraded: a window that opened
/// native when asked for the custom chrome would draw its titlebar under
/// the OS's, which is the bug AR27 was filed for.
#[cfg(any(feature = "runner", test))]
pub(crate) fn run_options_of(c: Option<&KuiRunConfig>) -> Result<RunOptions, String> {
    let Some(c) = c else {
        return Ok(RunOptions::default());
    };
    let side = |v: f32, what: &str| -> Result<f64, String> {
        if v.is_finite() && v >= 0.0 {
            Ok(f64::from(v))
        } else {
            Err(format!(
                "KuiRunConfig.{what} must be a finite, non-negative size, not {v}"
            ))
        }
    };
    let (w, h) = (side(c.width, "width")?, side(c.height, "height")?);
    let size =
        match (w > 0.0, h > 0.0) {
            (true, true) => Some((w, h)),
            (false, false) => None,
            _ => return Err(
                "KuiRunConfig: width and height go together (a min or max side may stand alone)"
                    .into(),
            ),
        };
    let (min_w, min_h) = (side(c.min_w, "min_w")?, side(c.min_h, "min_h")?);
    let min_size = (min_w > 0.0 || min_h > 0.0).then_some((min_w, min_h));
    let (max_w, max_h) = (side(c.max_w, "max_w")?, side(c.max_h, "max_h")?);
    let unbounded = |v: f64| if v > 0.0 { v } else { UNBOUNDED_SIZE };
    let max_size = (max_w > 0.0 || max_h > 0.0).then_some((unbounded(max_w), unbounded(max_h)));
    if c.chrome > KUI_CHROME_BORDERLESS {
        return Err(format!(
            "KuiRunConfig.chrome must be KUI_CHROME_NATIVE, KUI_CHROME_CUSTOM or KUI_CHROME_BORDERLESS, not {}",
            c.chrome
        ));
    }
    if c.text_aa > KUI_TEXT_AA_SUBPIXEL {
        return Err(format!(
            "KuiRunConfig.text_aa must be KUI_TEXT_AA_AUTO, KUI_TEXT_AA_GRAYSCALE or KUI_TEXT_AA_SUBPIXEL, not {}",
            c.text_aa
        ));
    }
    let diagnostics = match c.diagnostics {
        KUI_DIAG_DEFAULT => None,
        KUI_DIAG_ON => Some(true),
        KUI_DIAG_OFF => Some(false),
        other => {
            return Err(format!(
                "KuiRunConfig.diagnostics must be KUI_DIAG_DEFAULT, KUI_DIAG_ON or KUI_DIAG_OFF, not {other}"
            ));
        }
    };
    Ok(RunOptions {
        size,
        min_size,
        max_size,
        chrome: c.chrome,
        text_aa: c.text_aa,
        diagnostics,
        frame_latency: (c.frame_latency > 0).then_some(c.frame_latency),
    })
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
/// `KUI_CMD_REDRAW`: draw `window` again — another window's input changed
/// what it shows (`docs/adr/0024`, decision 7). A host that redraws every
/// window on every event may ignore it.
pub const KUI_CMD_REDRAW: u32 = 8;

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
        WindowCommand::Redraw(_) => (KUI_CMD_REDRAW, 0, KuiWindowConfig::default()),
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
    /// Which entry of `KuiDrawData::clips` clips this quad. An index and
    /// not the clip itself since ABI 11: a clip is thirty-two bytes and a
    /// frame has a handful of them, so carrying it per quad cost every
    /// quad of every frame for a value nearly all of them share.
    pub clip: u32,
    /// Atlas texels: x, y, w, h. `KUI_QUAD_SEGMENT`: the endpoints as
    /// float bits (see `kui_core::Quad::segment_ends`).
    pub uv: [u32; 4],
}

/// One clip a frame's quads name, in physical pixels. Mirrors
/// `kui_core::Clip`, so `KuiDrawData::clips` is a cast and not a copy.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiClip {
    /// Clip rect: x, y, w, h. Pixels outside are transparent.
    pub rect: [f32; 4],
    /// Corner radii (physical px), clockwise from the top-left: pixels
    /// outside the rounded clip are transparent too. All zero — every clip
    /// of a frame with no rounded clipper — is the plain rect clip.
    pub radius: [f32; 4],
}

/// One `KUI_QUAD_FRAGMENT`'s draw, addressed by that quad's `uv[0]`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiFragmentDraw {
    /// The registered handle, for `kui_fragment_source` and for keying a
    /// renderer's pipeline cache.
    pub fragment: u64,
    /// What the node declared, zero-padded to sixteen.
    pub params: [f32; 16],
    /// Where the draw's `image` is (backlog V1): `KUI_FRAGMENT_IMAGE_NONE`,
    /// `_ATLAS` (bind the atlas, as for any fragment) or `_TEXTURE` (bind
    /// the texture `image_texture` names, as for a `KUI_QUAD_TEXTURE`
    /// quad). Added in ABI 15.
    pub image_source: u32,
    /// The `textures` index the draw reads, when `image_source` is
    /// `KUI_FRAGMENT_IMAGE_TEXTURE`; 0 otherwise.
    pub image_texture: u32,
    /// The texel rect the shader is given as `FragmentIn::image`, in the
    /// atlas or in that texture; zero with no image.
    pub image_uv: [u32; 4],
}

/// `KuiFragmentDraw::image_source`: no image.
pub const KUI_FRAGMENT_IMAGE_NONE: u32 = 0;
/// `KuiFragmentDraw::image_source`: the image is in the atlas.
pub const KUI_FRAGMENT_IMAGE_ATLAS: u32 = 1;
/// `KuiFragmentDraw::image_source`: the image has a texture of its own.
pub const KUI_FRAGMENT_IMAGE_TEXTURE: u32 = 2;

/// One `KUI_QUAD_TEXTURE`'s draw, addressed by that quad's `uv[0]`
/// (ADR 0025, decision 3).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiTextureDraw {
    /// The image handle, for `kui_image_pixels` and for keying a
    /// renderer's texture cache.
    pub image: u64,
    /// Moves with every `kui_image_update`; a renderer that uploaded this
    /// revision has nothing to do.
    pub rev: u32,
    pub width: u32,
    pub height: u32,
    /// The texel rect to show, `[x, y, w, h]` in the image's own texels —
    /// the whole image, or the crop a `fit = cover` made.
    pub uv: [u32; 4],
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
    /// Changes either way between frames: a page extended for one frame
    /// goes back to its size at the next (backlog F99). Size the texture
    /// to it, not to the largest seen.
    pub atlas_size: u32,
    /// Re-upload the atlas texture when either of these changes/sets.
    pub atlas_dirty: bool,
    pub atlas_epoch: u64,
    /// One per `KUI_QUAD_FRAGMENT` quad, indexed by its `uv[0]`; null and
    /// zero on a frame that draws none. Added in ABI 9.
    pub fragments: *const KuiFragmentDraw,
    pub fragment_count: usize,
    /// The frame clock in seconds, for a fragment's `time`.
    pub time: f32,
    /// The clips the quads index through `KuiQuad::clip`. Never empty on a
    /// frame that drew anything: entry zero clips nothing. Added in
    /// ABI 11.
    pub clips: *const KuiClip,
    pub clip_count: usize,
    /// One per `KUI_QUAD_TEXTURE` quad, indexed by its `uv[0]`; null and
    /// zero on a frame that draws none. Added in ABI 14.
    pub textures: *const KuiTextureDraw,
    pub texture_count: usize,
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
            fragments: std::ptr::null(),
            fragment_count: 0,
            time: 0.0,
            clips: std::ptr::null(),
            clip_count: 0,
            textures: std::ptr::null(),
            texture_count: 0,
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

/// [in] One cell of a `kui_cells` grid: a Unicode scalar, colours as
/// `0xRRGGBBAA` (a `bg` of 0 is none), `KUI_CELL_*` attribute bits.
/// Travels as an array, so a change here is an ABI bump (backlog C20).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct KuiCell {
    pub ch: u32,
    pub fg: u32,
    pub bg: u32,
    pub flags: u32,
    /// The underline's own colour (SGR 58), 0 for `fg` (backlog K4).
    /// ABI 17: cells travel as an array, so the stride moved.
    pub ul: u32,
}

/// [out] What choosing a context-menu row left for the host
/// (`kui_take_menu_action`): the clipboard, which is the host's in this
/// library. `KUI_MENU_ACTION_SET_CLIPBOARD` carries the text to put there;
/// `KUI_MENU_ACTION_PASTE` carries nothing and asks for what is there,
/// which the host delivers back with `kui_input_paste` (or
/// `kui_input_commit`); `KUI_MENU_ACTION_SET_CLIPBOARD_SECRET` carries a
/// secret to put there marked concealed and transient.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiMenuAction {
    /// [out] reservation; see `KUI_MENU_ACTION_INIT`.
    pub size: u32,
    /// A `KUI_MENU_ACTION_*` kind.
    pub kind: u32,
    /// Borrowed until the next `kui_take_menu_action` on this context.
    pub text: KuiStr,
    /// The same selection with the formatting the core knows about, for a
    /// host offering a second clipboard flavour (ADR 0017, decision 7).
    /// Empty when there is none to carry — and never a *replacement* for
    /// `text`: a clipboard whose only flavour is HTML pastes markup into
    /// every plain-text field on the machine.
    pub html: KuiStr,
    /// `KUI_MENU_ACTION_LOOK_UP` only: where to anchor the panel — the
    /// baseline origin of the selection's first line, logical viewport
    /// px. Zero for every other kind.
    pub x: f32,
    pub y: f32,
}

impl Default for KuiMenuAction {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            kind: 0,
            text: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
            html: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
            x: 0.0,
            y: 0.0,
        }
    }
}

// SAFETY: `repr(C)` with `size: u32` first.
unsafe impl OutParam for KuiMenuAction {
    const ABI_V1_SIZE: u32 = abi_through!(KuiMenuAction, y, f32);
    fn size_mut(&mut self) -> &mut u32 {
        &mut self.size
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

/// [out] The rect a node was laid out at (`kui_layout_of`): logical px in
/// viewport coordinates, the `layout` event's numbers without the event.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiLayoutRect {
    /// [out] reservation; see `KUI_LAYOUT_RECT_INIT`.
    pub size: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Default for KuiLayoutRect {
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
unsafe impl OutParam for KuiLayoutRect {
    const ABI_V1_SIZE: u32 = abi_through!(KuiLayoutRect, h, f32);
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

// ---------------------------------------------------------------------------
// The plain-constant enums the header spells and the entry points read.
//
// Each of these used to be a literal at the one site that read it (`kind =
// 3`, `flags & 8`), with the header the only place the number had a name.
// Named here so `mod abi_parity` can pin every one of them to the header
// by name, the way it pins `KUI_CMD_*`: a value renumbered on either side
// fails the C build instead of meaning something else at runtime.

/// `KUI_SPAN_*`: the flags on a `KuiSpan`.
pub const KUI_SPAN_BOLD: u32 = 1 << 0;
pub const KUI_SPAN_ITALIC: u32 = 1 << 1;
pub const KUI_SPAN_UNDERLINE: u32 = 1 << 2;
pub const KUI_SPAN_STRIKETHROUGH: u32 = 1 << 3;

/// `KUI_UNDERLINE_*`: an underline's shape, `KuiTextStyle.underline_style`
/// and `KuiSpan.underline_style` (backlog K4) — `UnderlineStyle`'s
/// discriminants.
pub const KUI_UNDERLINE_SOLID: u32 = 0;
pub const KUI_UNDERLINE_WAVY: u32 = 1;
pub const KUI_UNDERLINE_DOTTED: u32 = 2;

/// `KUI_KMOD_*`: the modifier bits `kui_input_key_down` and its siblings
/// take, and `kui_input_modifiers` reports — the core's own
/// `KeyMods::bits`, which is also what the conformance corpus's
/// `modifiers` step spells, so the header and the corpus cannot drift.
pub const KUI_KMOD_SHIFT: u32 = kui_core::KeyMods::SHIFT;
pub const KUI_KMOD_CTRL: u32 = kui_core::KeyMods::CTRL;
pub const KUI_KMOD_ALT: u32 = kui_core::KeyMods::ALT;
pub const KUI_KMOD_SUPER: u32 = kui_core::KeyMods::SUPER;

/// `KUI_EDIT_*`: the flags `kui_text_edit` takes. `WRAP` is the `wrap`
/// row declared on a field (the mode is `KuiTextStyle.wrap`, whose zero
/// is `KUI_WRAP_WORD`, so the style alone cannot say): the field folds to
/// its width the way a document does and keeps a field's keyboard
/// (backlog F44).
pub const KUI_EDIT_MULTILINE: u32 = 1 << 0;
pub const KUI_EDIT_AUTOFOCUS: u32 = 1 << 1;
pub const KUI_EDIT_WRAP: u32 = 1 << 2;

/// `KUI_MOD_*`: the editing modifiers `kui_input_key` takes — extend the
/// selection, move by word, move by document.
pub const KUI_MOD_SHIFT: u32 = 1 << 0;
pub const KUI_MOD_WORD: u32 = 1 << 1;
pub const KUI_MOD_DOC: u32 = 1 << 2;

/// `KUI_KEY_*`: the editing keys `kui_input_key` takes, in the header's
/// order — which is not `EditKey`'s declaration order, so the table is
/// the pin rather than a cast. `edit_key_of` reads it and `mod
/// abi_parity` emits it.
pub const KUI_EDIT_KEYS: [(&str, EditKey); 16] = [
    ("KUI_KEY_LEFT", EditKey::Left),
    ("KUI_KEY_RIGHT", EditKey::Right),
    ("KUI_KEY_UP", EditKey::Up),
    ("KUI_KEY_DOWN", EditKey::Down),
    ("KUI_KEY_HOME", EditKey::Home),
    ("KUI_KEY_END", EditKey::End),
    ("KUI_KEY_PAGE_UP", EditKey::PageUp),
    ("KUI_KEY_PAGE_DOWN", EditKey::PageDown),
    ("KUI_KEY_BACKSPACE", EditKey::Backspace),
    ("KUI_KEY_DELETE", EditKey::Delete),
    ("KUI_KEY_ENTER", EditKey::Enter),
    ("KUI_KEY_TAB", EditKey::Tab),
    ("KUI_KEY_SELECT_ALL", EditKey::SelectAll),
    ("KUI_KEY_ESCAPE", EditKey::Escape),
    ("KUI_KEY_UNDO", EditKey::Undo),
    ("KUI_KEY_REDO", EditKey::Redo),
];

/// `KUI_MOUSE_*`: `kui_input_mouse_button`'s button, which is
/// `MouseButton::code` — the core owns the numbering, this is its name.
pub const KUI_MOUSE_PRIMARY: u32 = 0;
pub const KUI_MOUSE_SECONDARY: u32 = 1;
pub const KUI_MOUSE_MIDDLE: u32 = 2;
pub const KUI_MOUSE_OTHER: u32 = 3;

/// `KUI_MENU_*`: a `KuiMenuItem.role`, the position in `MenuRole::ALL`.
pub const KUI_MENU_CUSTOM: u32 = 0;
pub const KUI_MENU_SEPARATOR: u32 = 1;
pub const KUI_MENU_CUT: u32 = 2;
pub const KUI_MENU_COPY: u32 = 3;
pub const KUI_MENU_PASTE: u32 = 4;
pub const KUI_MENU_SELECT_ALL: u32 = 5;
pub const KUI_MENU_LOOK_UP: u32 = 6;

/// `KUI_MENU_ITEM_*`: the flags `kui_menu_bar_item` and `kui_menu_item`
/// report on a row.
pub const KUI_MENU_ITEM_ENABLED: u32 = 1 << 0;
pub const KUI_MENU_ITEM_CHECKED: u32 = 1 << 1;

/// `KUI_MENU_ACTION_*`: a `KuiMenuAction.kind`.
pub const KUI_MENU_ACTION_SET_CLIPBOARD: u32 = 0;
pub const KUI_MENU_ACTION_PASTE: u32 = 1;
pub const KUI_MENU_ACTION_LOOK_UP: u32 = 2;
/// A secret for the clipboard, to write marked concealed and transient
/// (`kui_set_clipboard_secret`, backlog F84). A new kind rather than a
/// flags field on `KuiMenuAction`, so the struct — and the ABI — stays
/// as it was; a host that does not know the kind drops the copy, which
/// for a secret is the safe way to fail.
pub const KUI_MENU_ACTION_SET_CLIPBOARD_SECRET: u32 = 3;

/// `KUI_PASTE_*`: the pasteboard's markers on a paste's answer
/// (`kui_input_paste`, backlog F84) — `ClipboardMarks::bits`.
pub const KUI_PASTE_CONCEALED: u32 = 1 << 0;
pub const KUI_PASTE_TRANSIENT: u32 = 1 << 1;

/// `KUI_OWED_*`: the bits of what `kui_owed` returns — `kui_animating`
/// by kind (backlog F64).
pub const KUI_OWED_TRANSITION: u32 = 1 << 0;
pub const KUI_OWED_CYCLE: u32 = 1 << 1;
pub const KUI_OWED_DEPART: u32 = 1 << 2;
pub const KUI_OWED_REQUESTED: u32 = 1 << 3;
pub const KUI_OWED_AUTOSCROLL: u32 = 1 << 4;
pub const KUI_OWED_SCROLL: u32 = 1 << 5;

/// `KUI_COPY_*`: what `kui_request_copy` returns.
pub const KUI_COPY_READY: u32 = 0;
pub const KUI_COPY_ASKED: u32 = 1;
pub const KUI_COPY_NOTHING: u32 = 2;

/// `KUI_AUDIO_*`: a `KuiAudioCommand.kind`.
pub const KUI_AUDIO_PLAY: u32 = 1;
pub const KUI_AUDIO_STOP: u32 = 2;
pub const KUI_AUDIO_SET_VOLUME: u32 = 3;
pub const KUI_AUDIO_PAUSE: u32 = 4;
pub const KUI_AUDIO_RESUME: u32 = 5;
pub const KUI_AUDIO_MASTER_VOLUME: u32 = 6;
pub const KUI_AUDIO_UNLOAD: u32 = 7;

/// `KUI_WINDOW_NONE`: a `KuiSpec.window_role` that is no chrome role.
pub const KUI_WINDOW_NONE: u32 = 0;
