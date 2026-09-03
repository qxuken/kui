//! The `Core`: owns everything that survives across frames (text caches,
//! glyph atlas, interaction state, registered resources) plus the reusable
//! per-frame tree — and the frame-builder state itself. Builder state living
//! here (not in a borrowing wrapper) is what lets flat C bindings drive a
//! frame through one opaque pointer; the Rust `Ui` is a thin safe façade.

use rustc_hash::FxHashMap;

use crate::anim::{AnimStore, Slot, Track};
use crate::atlas::GlyphAtlas;
use crate::color::Color;
use crate::diag::{Diagnostics, Warning};
use crate::display::{DisplayList, NO_CLIP, Quad, QuadKind};
use crate::edit::{EditOptions, EditStore};
use crate::env::Env;
use crate::geom::{Rect, Size, Vec2};
use crate::input::{
    EditKey, HitRegion, InputEvent, Interaction, MouseButton, ScrollAxis, ScrollRegion,
    ScrollbarRegion, UiEvent,
};
use crate::key::Key;
use crate::keyframes::{self, Keyframe};
use crate::layout::{self, TextMeasure};
use crate::resources::Resources;
use crate::scroll::ScrollStore;
use crate::spec::{NodeSpec, Sizing, TextStyle};
use crate::stats::FrameStats;
use crate::text::{Span, TextMetrics, TextSystem};
use crate::tree::{NIL, NodeContent, OriginId, Tree};
use crate::ui::Ui;
use crate::value::Value;

/// Wheel line-delta to logical px.
const SCROLL_LINE_PX: f32 = 40.0;
const SCROLLBAR_W: f32 = 4.0;
/// Thumb width while hovered or dragged.
const SCROLLBAR_ACTIVE_W: f32 = 6.0;
/// Grabbable gutter width (wider than the drawn thumb).
const SCROLLBAR_HIT_W: f32 = 10.0;
const SCROLLBAR_INSET: f32 = 2.0;
const SCROLLBAR_MIN: f32 = 24.0;
/// The default focus ring (see `docs/adr/0002-keyboard-focus-as-data.md`):
/// drawn this far outside the focused node, this thick, in this colour,
/// when focus is keyboard-visible and the node styles nothing itself.
const FOCUS_RING_GAP: f32 = 2.0;
const FOCUS_RING_W: f32 = 2.0;
const FOCUS_RING: Color = Color {
    r: 0x7f as f32 / 255.0,
    g: 0x9c as f32 / 255.0,
    b: 0xf5 as f32 / 255.0,
    a: 1.0,
};

pub struct Core {
    pub text: TextSystem,
    pub atlas: GlyphAtlas,
    pub interaction: Interaction,
    pub resources: Resources,
    pub scroll: ScrollStore,
    pub edit: EditStore,
    /// Transition tweens, keyed by node; see `anim`. Fed by `set_time`.
    pub anim: AnimStore,
    /// Playback bookkeeping and the audio command queue; see `audio`.
    pub audio: crate::audio::AudioStore,
    /// Frame timing pushed by the frame driver; see `widgets::latency_graph`.
    pub stats: FrameStats,
    /// Host facts pushed by the frame driver (refresh rate, focus).
    pub env: Env,
    /// Window title declared this frame (immediate-mode: cleared each
    /// `begin_frame`; the driver diffs and applies). None = leave as-is.
    window_title: Option<String>,
    /// Keyboard focus: the one node key input goes to — an editor (the
    /// edit store mirrors it), an `on_key` sink, a control Tab landed on
    /// (see `docs/adr/0002-keyboard-focus-as-data.md`). `set_focus` is
    /// the only writer.
    focus: Option<Key>,
    /// Focus got there by keyboard or assistive technology, so it shows:
    /// the default ring, or the node's `focus_bg`. A mouse press clears it.
    focus_visible: bool,
    /// Nodes that declared focus this frame and last (`set_key_focus`):
    /// a declaration takes focus only when it starts, so a repeated one
    /// does not clobber a Tab press.
    declared_focus: Vec<Key>,
    declared_focus_last: Vec<Key>,
    pub(crate) tree: Tree,
    pub(crate) display: DisplayList,
    pub(crate) viewport: Size,
    pub(crate) scale: f32,
    // Frame-builder state.
    stack: Vec<u32>,
    counters: Vec<u64>,
    origin: OriginId,
    /// Per-node inherited clip (logical), rebuilt each finish_frame.
    clips: Vec<Rect>,
    /// Whether any node this frame clips — lets emission skip clip math
    /// entirely for the common unclipped case.
    any_clip: bool,
    /// Per-node "inside a floating subtree" marker (only filled when needed).
    in_float: Vec<bool>,
    any_float: bool,
    /// Whether any node this frame declared `modal`.
    any_modal: bool,
    /// The frame's modal scope: the tree range `[i, subtree_end(i))` of the
    /// last node declaring `modal`, and its key. Everything outside it is
    /// inert and out of the Tab ring (see
    /// `docs/adr/0003-modal-surfaces.md`). Recomputed by `finish_frame`.
    modal: Option<(usize, usize, Key)>,
    /// The modals declared by the last finished frame, in tree order, each
    /// with the focus it displaced: a modal that stops being declared gives
    /// that focus back.
    modal_focus: Vec<(Key, Option<Key>)>,
    /// Whether any node this frame eases its position (`NodeSpec::slide`,
    /// or an `enter` with an offset).
    any_slide: bool,
    /// Whether any node this frame declared `on_layout` — lets the rect
    /// report skip the tree walk for the common case.
    any_layout: bool,
    /// A view asked for one more frame (`request_frame`); cleared by
    /// `begin_frame`, reported through `animating`.
    frame_requested: bool,
    /// The focused editor's caret rect (logical, viewport coords) as of the
    /// last finish_frame — where drivers should anchor the OS IME window.
    ime_rect: Option<Rect>,
    /// Events raised by the frame driver's own reports rather than by input
    /// — a changed viewport becoming a `resize`. Drained alongside the
    /// interaction's pending queue.
    pending: Vec<UiEvent>,
    /// Whether any frame has begun yet: the first one establishes the
    /// viewport instead of resizing it.
    framed: bool,
    /// Frames begun so far; stamps the per-key stores below.
    frame_no: u64,
    /// The rect last reported for each `on_layout` node and the frame it
    /// was seen: a different rect, or a node not seen last frame, posts a
    /// `layout` event (see `emit_layout_events`).
    layouts: FxHashMap<Key, (Rect, u64)>,
    /// Silent-misconfiguration detection; see `diag`.
    diag: Diagnostics,
    /// The access tree of the last finished frame, built on demand (see
    /// `access_tree`) and stamped with the frame it was built from.
    access: crate::access::AccessTree,
    access_built: u64,
}

/// A node's keyframes flattened per slot for `ease_spec`, built once per
/// node per frame (only for nodes that declare keyframes).
struct Tracks {
    width: Option<Vec<(f32, [f32; 4])>>,
    height: Option<Vec<(f32, [f32; 4])>>,
    bg: Option<Vec<(f32, [f32; 4])>>,
    radius: Option<Vec<(f32, [f32; 4])>>,
}

impl Tracks {
    fn of(spec: &NodeSpec) -> Self {
        let frames = &spec.keyframes;
        let offsets = keyframes::offsets(frames);
        let one = |v: f32| [v, 0.0, 0.0, 0.0];
        let sizing = |base: Sizing, pick: fn(&Keyframe) -> Option<Sizing>| {
            let base = base.amount()?;
            keyframes::track(frames, &offsets, one(base), |k| pick(k)?.amount().map(one))
        };
        let bg = spec.style.bg;
        Tracks {
            width: sizing(spec.layout.width, |k| k.width),
            height: sizing(spec.layout.height, |k| k.height),
            bg: keyframes::track(frames, &offsets, [bg.r, bg.g, bg.b, bg.a], |k| {
                k.bg.map(|c| [c.r, c.g, c.b, c.a])
            }),
            radius: keyframes::track(frames, &offsets, spec.style.radius, |k| {
                k.radius.map(|r| [r; 4])
            }),
        }
    }

    fn get(&self, slot: Slot) -> Option<&Track> {
        match slot {
            Slot::Width => self.width.as_deref(),
            Slot::Height => self.height.as_deref(),
            Slot::Bg => self.bg.as_deref(),
            Slot::Radius => self.radius.as_deref(),
            Slot::Border | Slot::Pos => None,
        }
    }
}

impl Core {
    pub fn new() -> Self {
        Self {
            text: TextSystem::new(),
            atlas: GlyphAtlas::new(),
            interaction: Interaction::default(),
            resources: Resources::default(),
            scroll: ScrollStore::default(),
            edit: EditStore::default(),
            anim: AnimStore::default(),
            audio: crate::audio::AudioStore::default(),
            stats: FrameStats::default(),
            env: Env::default(),
            window_title: None,
            focus: None,
            focus_visible: false,
            declared_focus: Vec::new(),
            declared_focus_last: Vec::new(),
            access: Default::default(),
            access_built: 0,
            tree: Tree::new(),
            display: DisplayList::default(),
            viewport: Size::ZERO,
            scale: 1.0,
            stack: Vec::new(),
            counters: Vec::new(),
            origin: OriginId::HOST,
            clips: Vec::new(),
            any_clip: false,
            in_float: Vec::new(),
            any_float: false,
            any_modal: false,
            modal: None,
            modal_focus: Vec::new(),
            any_slide: false,
            any_layout: false,
            frame_requested: false,
            ime_rect: None,
            pending: Vec::new(),
            framed: false,
            frame_no: 0,
            layouts: FxHashMap::default(),
            diag: Diagnostics::default(),
        }
    }

    // -- Measurement ----------------------------------------------------
    // The layout engine measures text every frame; these hand the same
    // numbers to the view, so it never re-derives them by hand.

    /// Measures `content` in `style` without adding a node: its unwrapped
    /// size, or with `max_w` (logical px) its size once wrapped to that
    /// width — what layout would give a text node with that content and
    /// style, `wrap` / `max_lines` / `ellipsis` included. Logical px at
    /// the scale of the current or last frame (1 before any frame).
    /// Shapes through the text cache, so measuring a string and then
    /// drawing it shapes once.
    pub fn measure_text(
        &mut self,
        content: &str,
        style: &TextStyle,
        max_w: Option<f32>,
    ) -> TextMetrics {
        self.text.measure(content, style, &self.resources, max_w)
    }

    /// `measure_text` for a rich-text paragraph.
    pub fn measure_rich_text(
        &mut self,
        spans: &[Span<'_>],
        base: &TextStyle,
        max_w: Option<f32>,
    ) -> TextMetrics {
        self.text.measure_rich(spans, base, &self.resources, max_w)
    }

    // -- Diagnostics ----------------------------------------------------

    /// Drains the warnings raised since the last drain (see [`crate::diag`]):
    /// silent misconfigurations the core noticed while finishing frames,
    /// each distinct (code, node) pair once. Windowed runners print them;
    /// headless tests assert on them.
    pub fn take_warnings(&mut self) -> Vec<Warning> {
        self.diag.take()
    }

    /// Turns the diagnostic checks on or off. A bare `Core` has them on;
    /// drivers set them for the build they are in (the runner: debug on,
    /// release off; Node loops: off under `NODE_ENV=production`; a
    /// standalone C context: off until asked).
    pub fn set_diagnostics(&mut self, on: bool) {
        self.diag.enabled = on;
    }

    pub fn diagnostics(&self) -> bool {
        self.diag.enabled
    }

    /// Feeds one input event; returns any UI events it resolved to,
    /// hit-tested against the previous frame's layout.
    pub fn handle_input(&mut self, ev: InputEvent) -> Vec<UiEvent> {
        let mut out = std::mem::take(&mut self.pending);
        match ev {
            InputEvent::Scroll(delta) => {
                // Wheel up (positive y) reveals earlier content: offset decreases.
                if let Some(key) = self.interaction.scroll_target() {
                    self.scroll.scroll_by(key, Vec2::new(-delta.x, -delta.y));
                }
            }
            InputEvent::Text(s) => {
                if let Some(key) = self.edit.focused() {
                    if self.edit.apply_text(key, &s, self.text.font_system_mut()) {
                        self.push_edit_event(key, "changed", &mut out);
                    }
                } else if s == " "
                    && let Some(i) = self.focused_control()
                {
                    // Space presses the focused control (a sink would have
                    // taken the press as data; an editor took the text).
                    self.click_node(self.tree.keys[i], &mut out);
                }
            }
            InputEvent::Preedit(s, cursor) => {
                if let Some(key) = self.edit.focused() {
                    self.edit
                        .set_preedit(key, &s, cursor, self.text.font_system_mut());
                }
            }
            InputEvent::Key(ek, mods) => {
                // A modal owns Escape: it asks to go away, and nothing
                // else happens (see `docs/adr/0003-modal-surfaces.md`).
                // The core closes nothing — the app stops declaring it.
                if ek == EditKey::Escape
                    && let Some(key) = self.modal()
                {
                    self.dismiss(key, "escape", &mut out);
                    return out;
                }
                // Tab walks the focus ring (Shift-Tab backwards) unless a
                // multiline editor holds focus — there Tab stays
                // indentation — or a key sink does: a sink is an app that
                // owns its keyboard, Tab included (it hands focus on with
                // `focus_next`). With nothing focused Tab enters the ring.
                let sink = self.focused_sink();
                let traverse = ek == EditKey::Tab
                    && match self.edit.focused() {
                        Some(k) => !self.edit.is_multiline(k),
                        None => !sink,
                    };
                if traverse {
                    self.focus_next(!mods.shift);
                } else if let Some(key) = self.edit.focused() {
                    let (changed, submit) =
                        self.edit
                            .apply_key(key, ek, mods, self.text.font_system_mut());
                    if changed {
                        self.push_edit_event(key, "changed", &mut out);
                    }
                    if submit {
                        self.push_edit_event(key, "submit", &mut out);
                    }
                    if ek == EditKey::Escape {
                        self.set_focus(None);
                    }
                } else if let Some(i) = self.focused_control() {
                    // A control that is neither an editor nor a sink:
                    // Enter presses it, the arrows nudge a slider (the
                    // same events assistive technology produces), Escape
                    // lets go.
                    use crate::access::AccessAction;
                    let slider = self.tree.specs[i].role == Some(crate::access::Role::Slider);
                    match ek {
                        EditKey::Enter => self.click_node(self.tree.keys[i], &mut out),
                        EditKey::Escape => self.set_focus(None),
                        EditKey::Right | EditKey::Up if slider => {
                            self.nudge(i, AccessAction::Increment, &mut out)
                        }
                        EditKey::Left | EditKey::Down if slider => {
                            self.nudge(i, AccessAction::Decrement, &mut out)
                        }
                        _ => {}
                    }
                }
            }
            InputEvent::KeyDown(kp) => {
                // The focused edit widget owns the keyboard (it takes the
                // Text/EditKey path); otherwise the focused sink, if it
                // still exists in the last frame, gets the press as data.
                if self.edit.focused().is_none()
                    && let Some(focus) = self.focus
                    && let Some(h) = self
                        .interaction
                        .hits
                        .iter()
                        .rev()
                        .find(|h| h.key == focus && h.key_sink.is_some())
                {
                    let mut payload = kp.to_value();
                    if let Some(tag) = &h.key_sink
                        && *tag != Value::Null
                        && let Value::Map(entries) = &mut payload
                    {
                        entries.push(("tag".to_string(), tag.clone()));
                    }
                    out.push(UiEvent {
                        origin: h.origin,
                        key: h.key,
                        payload,
                    });
                }
            }
            InputEvent::MouseDown { button, clicks } => {
                // Only the primary button moves anything: a secondary
                // press asks for a context menu where it landed and leaves
                // focus, the caret and the scrollbars exactly as they were
                // (a right-click on a selection has to keep it).
                let primary = button == MouseButton::Primary;
                // Scrollbars win over everything under them (they draw on
                // top): a thumb press starts a drag, a track press jumps
                // there first. Neither blurs the focused edit.
                if primary
                    && let Some(p) = self.interaction.cursor()
                    && let Some(bar) = self.interaction.scrollbar_at(p)
                {
                    let (pos, thumb_start) = match bar.axis {
                        ScrollAxis::X => (p.x, bar.thumb.x),
                        ScrollAxis::Y => (p.y, bar.thumb.y),
                    };
                    let grab = if pos >= thumb_start && pos <= thumb_start + bar.bar_len {
                        pos - thumb_start
                    } else {
                        let center = bar.bar_len / 2.0;
                        let off = bar.offset_for(p, center);
                        self.set_scroll_axis(bar.key, bar.axis, off);
                        center
                    };
                    self.interaction.scrollbar_drag = Some((bar.key, bar.axis, grab));
                    return out;
                }
                // Click-to-focus / caret placement / start drag-selection,
                // against the previous frame's layout.
                if let Some(p) = self.interaction.cursor() {
                    let hit = self
                        .interaction
                        .hit_at(p)
                        .map(|h| (h.key, h.edit_origin, h.focusable));
                    // While a modal is up, a press outside it never
                    // touches focus: one that finds no region asks the
                    // modal to go away (a modal is hit-tracked, so its own
                    // background is not "outside"), and one that finds the
                    // only live thing out there — window chrome — is the
                    // platform's business, not the app's.
                    if let Some(key) = self.modal()
                        && !hit.as_ref().is_some_and(|(k, _, _)| self.within_modal(*k))
                    {
                        if hit.is_none() {
                            self.dismiss(key, "outside", &mut out);
                        }
                        self.interaction
                            .handle(InputEvent::MouseDown { button, clicks }, &mut out);
                        return out;
                    }
                    // A press moves focus (to a focusable node) or drops
                    // it; either way it is pointer focus, not shown.
                    if primary {
                        match hit {
                            Some((key, Some(origin), true)) => {
                                self.set_focus(Some(key));
                                let local = Vec2::new(p.x - origin.x, p.y - origin.y);
                                self.edit
                                    .click(key, local, clicks, self.text.font_system_mut());
                                self.edit.dragging = Some((key, origin));
                            }
                            Some((key, None, true)) => self.set_focus(Some(key)),
                            _ => self.set_focus(None),
                        }
                        self.focus_visible = false;
                    }
                }
                self.interaction
                    .handle(InputEvent::MouseDown { button, clicks }, &mut out);
            }
            InputEvent::CursorMoved(p) => {
                if let Some((key, axis, grab)) = self.interaction.scrollbar_drag
                    && let Some(bar) = self
                        .interaction
                        .scrollbars
                        .iter()
                        .rev()
                        .find(|b| b.key == key && b.axis == axis)
                        .copied()
                {
                    // Thumb drag: geometry is last frame's, which is fine —
                    // track length only changes with the container.
                    let off = bar.offset_for(p, grab);
                    self.set_scroll_axis(key, axis, off);
                }
                if let Some((key, origin)) = self.edit.dragging {
                    let local = Vec2::new(p.x - origin.x, p.y - origin.y);
                    self.edit.drag(key, local, self.text.font_system_mut());
                }
                self.interaction
                    .handle(InputEvent::CursorMoved(p), &mut out);
            }
            InputEvent::MouseUp { button } => {
                if button == MouseButton::Primary {
                    self.edit.dragging = None;
                    self.interaction.scrollbar_drag = None;
                }
                self.interaction
                    .handle(InputEvent::MouseUp { button }, &mut out);
            }
            InputEvent::Access(req) => self.handle_access(req, &mut out),
            other => self.interaction.handle(other, &mut out),
        }
        self.flush_sound_requests();
        // Input moves focus, carets and scroll offsets: an access tree
        // derived earlier this frame no longer describes it.
        self.access_built = 0;
        out
    }

    /// Resolves a request from assistive technology against the last
    /// frame the way the pointer or keyboard equivalent would be (see
    /// [`crate::access::AccessAction`]).
    fn handle_access(&mut self, req: crate::access::AccessRequest, out: &mut Vec<UiEvent>) {
        use crate::access::AccessAction;
        let key = req.key;
        let idx = self.tree.keys.iter().position(|k| *k == key);
        match req.action {
            AccessAction::Click => self.click_node(key, out),
            AccessAction::Focus => {
                // The reader's cursor lands where Tab would — on a node it
                // can see (decoration is not in its tree); show it.
                let exposed = self.access_tree().get(key).is_some();
                if exposed && idx.is_some_and(|i| crate::access::focusable(&self.tree, i)) {
                    self.set_focus(Some(key));
                    self.focus_visible = true;
                }
            }
            AccessAction::Blur => {
                if self.focus == Some(key) {
                    self.set_focus(None);
                }
            }
            AccessAction::SetValue => {
                if self.edit.contains(key) {
                    let value = req.value.unwrap_or_default();
                    if self.edit.text(key).as_deref() != Some(value.as_str()) {
                        self.set_edit_text(key, &value);
                        self.push_edit_event(key, "changed", out);
                    }
                } else if let Some(i) = idx
                    && crate::access::is_custom_editor(&self.tree, i)
                {
                    // The app owns the text: hand the request over as data.
                    let mut entries = vec![
                        ("kind".to_string(), Value::str("access")),
                        ("action".to_string(), Value::str(req.action.name())),
                        (
                            "text".to_string(),
                            Value::str(req.value.unwrap_or_default()),
                        ),
                    ];
                    if let Some(tag) = self.access_tag(i) {
                        entries.push(("tag".to_string(), tag));
                    }
                    out.push(UiEvent {
                        origin: self.tree.origins[i],
                        key,
                        payload: Value::Map(entries),
                    });
                }
            }
            AccessAction::Increment | AccessAction::Decrement => {
                let Some(i) = idx else { return };
                self.nudge(i, req.action, out);
            }
            AccessAction::SetTextSelection | AccessAction::ReplaceSelectedText => {
                let Some(i) = idx else { return };
                if self.edit.contains(key) {
                    match req.action {
                        AccessAction::SetTextSelection => {
                            let (Some(anchor), Some(focus)) = (req.anchor, req.focus) else {
                                return;
                            };
                            // Run positions resolve against the tree of
                            // the last frame, which is what the request
                            // was made from.
                            let tree = self.access_tree();
                            let Some(node) = tree.get(key) else { return };
                            let (Some(a), Some(f)) =
                                (node.line_offset(anchor), node.line_offset(focus))
                            else {
                                return;
                            };
                            self.edit.set_selection(key, a, f);
                        }
                        _ => {
                            let text = req.value.unwrap_or_default();
                            if self
                                .edit
                                .replace_selection(key, &text, self.text.font_system_mut())
                            {
                                self.push_edit_event(key, "changed", out);
                            }
                        }
                    }
                } else if crate::access::is_custom_editor(&self.tree, i) {
                    // The app owns the text: hand the request over as data.
                    let mut entries = vec![
                        ("kind".to_string(), Value::str("access")),
                        ("action".to_string(), Value::str(req.action.name())),
                    ];
                    if let Some(text) = req.value {
                        entries.push(("text".to_string(), Value::str(text)));
                    }
                    if let (Some(anchor), Some(focus)) = (req.anchor, req.focus) {
                        let tree = self.access_tree();
                        let Some(node) = tree.get(key) else { return };
                        let (Some(a), Some(f)) =
                            (node.line_offset(anchor), node.line_offset(focus))
                        else {
                            return;
                        };
                        let pos = |(line, offset): (usize, usize)| {
                            Value::map([
                                ("line", Value::Int(line as i64)),
                                ("offset", Value::Int(offset as i64)),
                            ])
                        };
                        entries.push(("anchor".to_string(), pos(a)));
                        entries.push(("focus".to_string(), pos(f)));
                    }
                    let spec = &self.tree.specs[i];
                    let tag = spec
                        .on_click
                        .clone()
                        .or_else(|| spec.on_drag.clone())
                        .or_else(|| spec.on_key.clone());
                    if let Some(tag) = tag.filter(|t| *t != Value::Null) {
                        entries.push(("tag".to_string(), tag));
                    }
                    out.push(UiEvent {
                        origin: self.tree.origins[i],
                        key,
                        payload: Value::Map(entries),
                    });
                }
            }
            AccessAction::ScrollIntoView => {
                let Some(i) = idx else { return };
                let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
                self.scroll_rect_into_view(i, rect, false);
            }
            AccessAction::ScrollUp
            | AccessAction::ScrollDown
            | AccessAction::ScrollLeft
            | AccessAction::ScrollRight => {
                let Some(i) = idx else { return };
                let size = self.tree.size[i];
                let delta = match req.action {
                    AccessAction::ScrollUp => Vec2::new(0.0, -size.h * 0.8),
                    AccessAction::ScrollDown => Vec2::new(0.0, size.h * 0.8),
                    AccessAction::ScrollLeft => Vec2::new(-size.w * 0.8, 0.0),
                    _ => Vec2::new(size.w * 0.8, 0.0),
                };
                self.scroll.scroll_by(key, delta);
            }
        }
    }

    /// The `tag` an `access` event on node `i` carries: its click payload,
    /// else its drag or key tag; None when there is none (or it is null).
    fn access_tag(&self, i: usize) -> Option<Value> {
        let spec = &self.tree.specs[i];
        spec.on_click
            .clone()
            .or_else(|| spec.on_drag.clone())
            .or_else(|| spec.on_key.clone())
            .filter(|t| *t != Value::Null)
    }

    /// The access tree of the last finished frame (see [`crate::access`]):
    /// derived on the first call after a frame, then reused. A driver that
    /// never asks pays nothing.
    pub fn access_tree(&mut self) -> &crate::access::AccessTree {
        if self.access_built != self.frame_no {
            self.access = crate::access::build(
                &self.tree,
                &crate::access::Sources {
                    text: &self.text,
                    edit: &self.edit,
                    scroll: &self.scroll,
                    title: self.window_title.as_deref(),
                    focus: self.focus,
                    modal: self.modal(),
                    viewport: self.viewport,
                    scale: self.scale,
                },
            );
            self.access_built = self.frame_no;
        }
        &self.access
    }

    /// Turns the sounds nodes asked for (`click_sound` / `hover_sound`)
    /// into play commands. Declarative sounds carry no tag, so they never
    /// report `ended`.
    fn flush_sound_requests(&mut self) {
        for sound in self.interaction.take_sound_requests() {
            self.audio.play(
                OriginId::HOST,
                Key::ROOT,
                sound,
                crate::audio::PlayOptions::default(),
            );
        }
    }

    /// Emits one node's quads and registers its hit/scroll regions.
    fn emit_node(
        &mut self,
        i: usize,
        rect: Rect,
        clip: Rect,
        scale: f32,
        hits: &mut Vec<HitRegion>,
        scroll_regions: &mut Vec<ScrollRegion>,
    ) {
        // Behind a modal a node still draws, and stops taking input.
        let interactive = self.interactive(i);
        let spec = &self.tree.specs[i];
        let style = spec.style;
        if style.bg.is_visible() || (style.border_w > 0.0 && style.border_color.is_visible()) {
            self.display.quads.push(Quad {
                rect: rect.scaled(scale),
                color: style.bg,
                border_color: style.border_color,
                radius: style.radius.map(|r| r * scale),
                border_w: style.border_w * scale,
                kind: QuadKind::Solid,
                uv: [0; 4],
                clip: clip.scaled(scale),
            });
        }
        if spec.hover_tracked() && interactive {
            let parent = self.tree.parent[i];
            let parent_rect = if parent == NIL {
                Rect::new(0.0, 0.0, self.viewport.w, self.viewport.h)
            } else {
                let p = parent as usize;
                Rect::from_pos_size(self.tree.pos[p], self.tree.size[p])
            };
            // A disabled node keeps hover (a tooltip can say why) and loses
            // every interaction: it emits nothing and takes no focus.
            let live = !spec.disabled;
            hits.push(HitRegion {
                key: self.tree.keys[i],
                origin: self.tree.origins[i],
                rect,
                clip,
                payload: spec.on_click.clone().filter(|_| live),
                drag: spec.on_drag.clone().filter(|_| live),
                parent_rect,
                key_sink: spec.on_key.clone().filter(|_| live),
                context_menu: spec.on_context_menu.clone().filter(|_| live),
                focusable: crate::access::focusable(&self.tree, i),
                edit_origin: None,
                window: spec.window,
                hover: spec.on_hover.clone(),
                group: spec.hover_group,
                click_sound: spec.click_sound.filter(|_| live),
                hover_sound: spec.hover_sound,
            });
        }
        if spec.layout.scroll_x || spec.layout.scroll_y {
            // A container behind a modal keeps its scrollbar drawn and
            // refuses the wheel and the thumb.
            scroll_regions.push(ScrollRegion {
                key: self.tree.keys[i],
                rect,
                clip,
                inert: !interactive,
            });
        }
        match self.tree.content[i] {
            NodeContent::Text(tid) => {
                self.text.emit(
                    tid,
                    self.tree.pos[i],
                    self.tree.size[i],
                    clip.scaled(scale),
                    &mut self.atlas,
                    &mut self.display.quads,
                );
            }
            NodeContent::Edit(key) => {
                let pad = spec.layout.padding;
                let content_origin = Vec2::new(rect.x + pad.l, rect.y + pad.t);
                if interactive {
                    hits.push(HitRegion {
                        key,
                        origin: self.tree.origins[i],
                        rect,
                        clip,
                        payload: None,
                        drag: None,
                        parent_rect: rect,
                        edit_origin: Some(content_origin),
                        key_sink: None,
                        context_menu: None,
                        focusable: !spec.disabled,
                        window: None,
                        hover: None,
                        group: None,
                        click_sound: None,
                        hover_sound: None,
                    });
                }
                let focused = self.edit.focused() == Some(key);
                let origin_phys = Vec2::new(
                    (content_origin.x * scale).round(),
                    (content_origin.y * scale).round(),
                );
                self.edit.emit(
                    key,
                    origin_phys,
                    focused,
                    clip.scaled(scale),
                    &mut self.text,
                    &mut self.atlas,
                    &mut self.display.quads,
                );
            }
            NodeContent::Image(id) => {
                if let Some(entry) = self.resources.images.get(id)
                    && let Some(slot) =
                        self.atlas
                            .get_or_insert_image(id, entry.width, entry.height, &entry.rgba)
                {
                    self.display.quads.push(Quad {
                        rect: rect.scaled(scale),
                        // White = untinted; radius rounds like a solid.
                        color: Color::WHITE,
                        border_color: Color::TRANSPARENT,
                        radius: spec.style.radius.map(|r| r * scale),
                        border_w: 0.0,
                        kind: QuadKind::Image,
                        uv: [slot.x, slot.y, slot.w, slot.h],
                        clip: clip.scaled(scale),
                    });
                }
            }
            NodeContent::Container => {}
        }
    }

    /// Moves keyboard focus to the next / previous focusable node in tree
    /// order (from the last laid-out frame; see `access::focusable`),
    /// wrapping around; with no current focus, enters the first (or last,
    /// going backwards). What Tab does. The landing node scrolls into
    /// view, and the focus shows (ring or `focus_bg`), as keyboard focus
    /// should.
    pub fn focus_next(&mut self, forward: bool) {
        let ring = self.focus_ring();
        if ring.is_empty() {
            return;
        }
        let at = self
            .focus
            .and_then(|cur| ring.iter().position(|(_, k)| *k == cur));
        let (i, key) = match at {
            Some(p) if forward => ring[(p + 1) % ring.len()],
            Some(p) => ring[(p + ring.len() - 1) % ring.len()],
            None if forward => ring[0],
            None => ring[ring.len() - 1],
        };
        self.set_focus(Some(key));
        self.focus_visible = true;
        let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
        self.scroll_rect_into_view(i, rect, false);
    }

    /// The Tab ring: (tree index, key) of every focusable node of the
    /// last frame in tree order, `role="none"` subtrees skipped whole —
    /// and, when a modal is in effect, of its subtree only (see
    /// `docs/adr/0003-modal-surfaces.md`).
    fn focus_ring(&self) -> Vec<(usize, Key)> {
        let mut out = Vec::new();
        let (mut i, end) = match self.modal {
            Some((start, end, _)) => (start, end),
            None => (0, self.tree.len()),
        };
        while i < end {
            if self.tree.specs[i].role == Some(crate::access::Role::None) {
                i = self.tree.subtree_end(i);
                continue;
            }
            if crate::access::focusable(&self.tree, i) {
                out.push((i, self.tree.keys[i]));
            }
            i += 1;
        }
        out
    }

    /// The frame's modal scope: the tree range of the last node declaring
    /// `modal`, and its key. The last one wins, so a confirm declared
    /// inside (or after) a dialog is the one in effect and the dialog
    /// under it is as inert as the app under the dialog.
    fn modal_scope(&self) -> Option<(usize, usize, Key)> {
        let i = (0..self.tree.len())
            .rev()
            .find(|&i| self.tree.specs[i].modal.is_some())?;
        Some((i, self.tree.subtree_end(i), self.tree.keys[i]))
    }

    /// The key of the modal in effect this frame, if any.
    pub fn modal(&self) -> Option<Key> {
        self.modal.map(|(_, _, key)| key)
    }

    /// Whether node `i` takes input: everything does, until a modal is in
    /// effect — then its subtree does, and so does window chrome (the
    /// window's own controls belong to the platform, not to the dialog).
    fn interactive(&self, i: usize) -> bool {
        match self.modal {
            Some((start, end, _)) => {
                (start..end).contains(&i) || self.tree.specs[i].window.is_some()
            }
            None => true,
        }
    }

    /// Whether node `key` is inside the frame's modal scope (nothing is
    /// outside one when there is no modal).
    fn within_modal(&self, key: Key) -> bool {
        let Some((start, end, _)) = self.modal else {
            return true;
        };
        self.tree.keys[start..end].contains(&key)
    }

    /// Focus follows the modal: a modal that appears remembers the focus
    /// it displaces and takes focus into itself; one that stops being
    /// declared gives that focus back. Run once per frame, after the
    /// scope is known.
    fn resolve_modal_focus(&mut self) {
        // This frame's modals in tree order, each carrying the focus it
        // displaced when it first appeared.
        let mut now: Vec<(Key, Option<Key>)> = Vec::new();
        if self.any_modal {
            for i in 0..self.tree.len() {
                if self.tree.specs[i].modal.is_none() {
                    continue;
                }
                let key = self.tree.keys[i];
                let saved = self
                    .modal_focus
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map_or(self.focus, |(_, f)| *f);
                now.push((key, saved));
            }
        }
        // The outermost modal that went away hands its focus back, so a
        // dialog and the confirm inside it closing together land where
        // the dialog was opened from.
        let closed = self
            .modal_focus
            .iter()
            .find(|(k, _)| !now.iter().any(|(n, _)| n == k))
            .map(|(_, saved)| saved.filter(|key| self.tree.keys.contains(key)));
        if let Some(saved) = closed {
            // Exactly what it displaced, nothing included: leaving focus
            // on the dismissed modal's own button would be a focus on a
            // node that is not there any more.
            self.set_focus(saved);
        }
        self.modal_focus = now;
        // Containment: focus outside the scope enters it (at its first
        // focusable node), or is dropped when it holds none. Not
        // `focus_visible`: the app showed the modal, nobody pressed a key.
        if self.modal.is_some() && !self.focus.is_some_and(|k| self.within_modal(k)) {
            let first = self.focus_ring().first().map(|(_, k)| *k);
            self.set_focus(first);
        }
    }

    /// The focused node's index in the last frame, if it is there.
    fn focus_index(&self) -> Option<usize> {
        let key = self.focus?;
        self.tree.keys.iter().position(|k| *k == key)
    }

    /// Whether the focused node is a key sink (it owns its keys).
    fn focused_sink(&self) -> bool {
        self.focus_index()
            .is_some_and(|i| self.tree.specs[i].on_key.is_some())
    }

    /// The focused node when it is a control the core presses itself:
    /// not an editor, not a key sink, and still focusable.
    fn focused_control(&self) -> Option<usize> {
        let i = self.focus_index()?;
        let spec = &self.tree.specs[i];
        let editor = matches!(self.tree.content[i], NodeContent::Edit(_));
        (!editor && spec.on_key.is_none() && crate::access::focusable(&self.tree, i)).then_some(i)
    }

    /// Activates node `key` the way a pointer click would — against the
    /// last frame's hit regions, so a disabled node emits nothing — for
    /// Enter, Space and an assistive-technology `click`. Focus follows
    /// into an editor or a sink, as a click's would.
    fn click_node(&mut self, key: Key, out: &mut Vec<UiEvent>) {
        let Some(h) = self.interaction.hits.iter().rev().find(|h| h.key == key) else {
            return;
        };
        let (origin, payload, window, sound) =
            (h.origin, h.payload.clone(), h.window, h.click_sound);
        let takes_focus = h.focusable && (h.edit_origin.is_some() || h.key_sink.is_some());
        if let Some(sound) = sound {
            self.interaction.sound_requests.push(sound);
        }
        match (window, payload) {
            (Some(crate::window::WindowRole::Button(b)), _) => {
                self.interaction.window_commands.push(b.command())
            }
            (None, Some(payload)) => out.push(UiEvent {
                origin,
                key,
                payload,
            }),
            _ => {}
        }
        if takes_focus {
            self.set_focus(Some(key));
        }
    }

    /// The modal `key` was asked to go away — Escape, or a press outside
    /// it. Reaches the app as `{kind="dismiss", reason, tag}` on the modal
    /// node; what happens next is the app's, since only it can stop
    /// declaring the node (see `docs/adr/0003-modal-surfaces.md`).
    fn dismiss(&mut self, key: Key, reason: &str, out: &mut Vec<UiEvent>) {
        let Some(i) = self.tree.keys.iter().position(|k| *k == key) else {
            return;
        };
        let mut entries = vec![
            ("kind".to_string(), Value::str("dismiss")),
            ("reason".to_string(), Value::str(reason)),
        ];
        if let Some(tag) = self.tree.specs[i]
            .modal
            .clone()
            .filter(|t| *t != Value::Null)
        {
            entries.push(("tag".to_string(), tag));
        }
        out.push(UiEvent {
            origin: self.tree.origins[i],
            key,
            payload: Value::Map(entries),
        });
    }

    /// A slider nudge on node `i`: the core cannot know what a step means,
    /// so it reaches the app as `{kind="access", action, tag}` — from the
    /// arrow keys and from assistive technology alike.
    fn nudge(&mut self, i: usize, action: crate::access::AccessAction, out: &mut Vec<UiEvent>) {
        if self.tree.specs[i].disabled {
            return;
        }
        let mut entries = vec![
            ("kind".to_string(), Value::str("access")),
            ("action".to_string(), Value::str(action.name())),
        ];
        if let Some(tag) = self.access_tag(i) {
            entries.push(("tag".to_string(), tag));
        }
        out.push(UiEvent {
            origin: self.tree.origins[i],
            key: self.tree.keys[i],
            payload: Value::Map(entries),
        });
    }

    /// Sets one axis of a container's scroll offset, keeping the other.
    fn set_scroll_axis(&mut self, key: Key, axis: ScrollAxis, value: f32) {
        let mut off = self.scroll.offset(key);
        match axis {
            ScrollAxis::X => off.x = value,
            ScrollAxis::Y => off.y = value,
        }
        self.scroll.set(key, off);
    }

    fn push_edit_event(&self, key: Key, kind: &str, out: &mut Vec<UiEvent>) {
        out.push(UiEvent {
            origin: self.edit.origin_of(key).unwrap_or(OriginId::HOST),
            key,
            payload: Value::map([("kind", kind.into())]),
        });
    }

    /// Selected text of the focused editor (for clipboard integration).
    pub fn copy_selection(&self) -> Option<String> {
        self.edit.copy_selection(self.edit.focused()?)
    }

    /// Cuts the focused editor's selection, returning the removed text.
    pub fn cut_selection(&mut self) -> Option<String> {
        let key = self.edit.focused()?;
        let text = self.edit.copy_selection(key)?;
        self.edit.delete_selection(key, self.text.font_system_mut());
        Some(text)
    }

    /// Whether `key` holds keyboard focus — any node (see `focus`).
    pub fn is_focused(&self, key: Key) -> bool {
        self.focus == Some(key)
    }

    /// The node holding keyboard focus: an editor, an `on_key` sink, or
    /// a control Tab (or assistive technology, or `set_focus`) put it on.
    pub fn focus(&self) -> Option<Key> {
        self.focus
    }

    /// Whether focus got where it is by keyboard or assistive technology
    /// rather than a click — when it shows (the default ring, or the
    /// node's `focus_bg`).
    pub fn focus_visible(&self) -> bool {
        self.focus_visible
    }

    /// Moves keyboard focus (None blurs). The one writer: the edit store
    /// mirrors it for editor keys, and a landing editor scrolls its caret
    /// into view. Any node can be focused this way; only focusable ones
    /// (see `access::focusable`) are reached by Tab.
    pub fn set_focus(&mut self, key: Option<Key>) {
        let edit_key = key.filter(|k| self.edit.contains(*k));
        if self.focus != key
            && let Some(k) = edit_key
        {
            self.edit.caret_moved = Some(k);
        }
        self.focus = key;
        self.edit.set_focus(edit_key);
    }

    /// Current text of an editor by key.
    pub fn edit_text(&self, key: Key) -> Option<String> {
        self.edit.text(key)
    }

    pub fn set_edit_text(&mut self, key: Key, text: &str) {
        let fs = self.text.font_system_mut();
        self.edit.set_text(key, text, fs, &self.resources);
    }

    // -- Fonts ----------------------------------------------------------

    /// Registers a font from its file bytes (TTF/OTF/TTC); `None` when the
    /// data holds no usable face. Shape with it via `TextStyle::font`.
    pub fn add_font_data(&mut self, data: Vec<u8>) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::Source;
        let db = self.text.font_system_mut().db_mut();
        let ids = db.load_font_source(Source::Binary(std::sync::Arc::new(data)));
        let family = db.face(*ids.first()?)?.families.first()?.0.clone();
        Some(self.resources.add_font(family, ids.to_vec()))
    }

    /// Registers a font file (TTF/OTF/TTC) by path, memory-mapped by the
    /// font database; `None` when it cannot be read or holds no usable
    /// face. Shape with it via `TextStyle::font`.
    pub fn load_font_file(
        &mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::Source;
        let db = self.text.font_system_mut().db_mut();
        let ids = db.load_font_source(Source::File(path.into()));
        let family = db.face(*ids.first()?)?.families.first()?.0.clone();
        Some(self.resources.add_font(family, ids.to_vec()))
    }

    /// Loads every font file under `dir` (recursively) into the font
    /// database, so their families become available to `add_system_font`
    /// by name; returns how many faces were added. A bundled `fonts/`
    /// folder next to the app is the usual case.
    pub fn load_fonts_dir(&mut self, dir: impl AsRef<std::path::Path>) -> usize {
        let db = self.text.font_system_mut().db_mut();
        let before = db.len();
        db.load_fonts_dir(dir);
        db.len().saturating_sub(before)
    }

    /// The handle for a font family by name (`"Menlo"`, `"Antonio"`) —
    /// installed on the system or loaded with `load_fonts_dir` /
    /// `load_font_file`; `None` when no face matches (see
    /// `system_font_families`). Idempotent: the same family gets the same
    /// handle, so views can call it every frame.
    pub fn add_system_font(&mut self, name: &str) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::{Family, Query};
        let db = self.text.font_system().db();
        let query = Query {
            families: &[Family::Name(name)],
            ..Default::default()
        };
        let id = db.query(&query)?;
        // The canonical spelling, so the style matches the way fontdb does.
        let family = db.face(id)?.families.first()?.0.clone();
        if let Some((id, _)) = self
            .resources
            .fonts
            .iter()
            .find(|(_, f)| f.faces.is_empty() && f.family == family)
        {
            return Some(id);
        }
        Some(self.resources.add_font(family, Vec::new()))
    }

    /// Forgets a registered font; faces loaded from bytes leave the font
    /// database. Styles still naming it shape as sans-serif.
    pub fn remove_font(&mut self, id: crate::resources::FontId) {
        if let Some(entry) = self.resources.remove_font(id) {
            let db = self.text.font_system_mut().db_mut();
            for face in entry.faces {
                db.remove_face(face);
            }
        }
    }

    /// The registered family name behind a font handle, if it is live.
    pub fn font_family(&self, id: crate::resources::FontId) -> Option<&str> {
        self.resources.font_family(id)
    }

    /// Family names of every installed font the core can see (sorted,
    /// deduplicated) — what `add_system_font` accepts.
    pub fn system_font_families(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .text
            .font_system()
            .db()
            .faces()
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Declares a node focused this frame (None blurs at once). The
    /// declaration is edge-triggered: the node takes focus on the first
    /// frame it is declared and keeps being declared without effect
    /// afterwards, so a view that repeats it every frame (an app that
    /// owns its keyboard, a `keyFocus` prop) does not clobber the focus a
    /// Tab press or a click moved. Programmatic focus keeps the modality
    /// of the last input (it shows after keyboard use, not after a
    /// click). To move focus at any time, `set_focus`.
    pub fn set_key_focus(&mut self, key: Option<Key>) {
        let Some(k) = key else {
            self.set_focus(None);
            return;
        };
        if !self.declared_focus.contains(&k) {
            self.declared_focus.push(k);
        }
        if !self.declared_focus_last.contains(&k) {
            self.set_focus(Some(k));
        }
    }

    /// The node holding keyboard focus (the same as `focus`; kept from
    /// when only key sinks and editors could).
    pub fn key_focus(&self) -> Option<Key> {
        self.focus
    }

    /// Queues a window command as if chrome had produced it, so apps can
    /// close/minimize/maximize from a keymap or command line. Drained by
    /// the frame driver with the rest.
    pub fn push_window_command(&mut self, cmd: crate::window::WindowCommand) {
        self.interaction.window_commands.push(cmd);
    }

    /// Drains window intents produced by chrome nodes since the last drain.
    /// Frame drivers call this after each input dispatch and apply the
    /// commands to the real window; headless drivers may simply never call.
    pub fn take_window_commands(&mut self) -> Vec<crate::window::WindowCommand> {
        std::mem::take(&mut self.interaction.window_commands)
    }

    // -- Audio ----------------------------------------------------------
    // Sounds are resources, playback is commands the driver drains; see
    // `audio`. Nothing here touches a device.

    /// Registers a sound from its encoded file bytes (wav/ogg/mp3/flac —
    /// the driver's backend decodes; the core only keeps the bytes).
    pub fn add_sound(&mut self, bytes: Vec<u8>) -> crate::resources::SoundId {
        self.resources.add_sound(bytes)
    }

    /// Forgets a sound; the driver drops its decoded copy. Playbacks
    /// already running keep going.
    pub fn remove_sound(&mut self, id: crate::resources::SoundId) {
        if self.resources.remove_sound(id).is_some() {
            self.audio.unload(id);
        }
    }

    /// Starts a playback; the returned id addresses it in `stop` /
    /// `set_volume` / `pause` / `resume`. With a tag in the options, the
    /// playback finishing on its own comes back as
    /// `{kind="sound", phase="ended", playback, tag}` on the current
    /// origin's root — the host's, or the extension's during its view.
    pub fn play(
        &mut self,
        sound: crate::resources::SoundId,
        opts: crate::audio::PlayOptions,
    ) -> crate::audio::PlaybackId {
        self.audio.play(self.origin, Key::ROOT, sound, opts)
    }

    /// Stops a playback, fading over `fade_ms` (0 = at once). A stopped
    /// playback never reports `ended`.
    pub fn stop(&mut self, playback: crate::audio::PlaybackId, fade_ms: f32) {
        self.audio.stop(playback, fade_ms);
    }

    /// Sets a playback's volume (linear amplitude), tweening over `tween_ms`.
    pub fn set_volume(&mut self, playback: crate::audio::PlaybackId, volume: f32, tween_ms: f32) {
        self.audio.set_volume(playback, volume, tween_ms);
    }

    pub fn pause(&mut self, playback: crate::audio::PlaybackId, fade_ms: f32) {
        self.audio.pause(playback, fade_ms);
    }

    pub fn resume(&mut self, playback: crate::audio::PlaybackId, fade_ms: f32) {
        self.audio.resume(playback, fade_ms);
    }

    /// Sets the master volume (linear amplitude), tweening over `tween_ms`.
    pub fn set_master_volume(&mut self, volume: f32, tween_ms: f32) {
        self.audio.master_volume(volume, tween_ms);
    }

    /// An `audio` node: a playback retained by key for as long as the view
    /// keeps declaring it — present means playing (once, or looped),
    /// gone means stopped; `volume` / `paused` changes apply live, a
    /// changed `src` restarts. The key is auto-assigned from the tree
    /// position; see `audio_node_keyed` for a stable label. Draws nothing
    /// and takes no layout space.
    pub fn audio_node(&mut self, spec: crate::audio::AudioSpec) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.auto_key();
        self.audio.declare(key, self.origin, spec);
        key
    }

    /// `audio_node` with a label-derived key (stable across reorders).
    pub fn audio_node_keyed(&mut self, label: &str, spec: crate::audio::AudioSpec) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.child_key(label);
        self.audio.declare(key, self.origin, spec);
        key
    }

    /// Drains the audio commands queued since the last drain. Frame
    /// drivers apply them to a real device after each input dispatch and
    /// after each frame; headless drivers may simply never call.
    pub fn take_audio_commands(&mut self) -> Vec<crate::audio::AudioCommand> {
        self.audio.take_commands()
    }

    /// The driver reports a playback finished on its own (not stopped).
    /// A tagged playback becomes an `ended` event, pending like a `resize`
    /// (see `take_pending_events`).
    pub fn audio_ended(&mut self, playback: crate::audio::PlaybackId) {
        if let Some(ev) = self.audio.ended(playback) {
            self.pending.push(ev);
        }
    }

    /// Wheel line-deltas (e.g. winit's LineDelta) to logical px.
    pub fn lines_to_px(lines: f32) -> f32 {
        lines * SCROLL_LINE_PX
    }

    /// Rasterizes outline glyphs as LCD subpixel coverage
    /// (`QuadKind::GlyphSubpixel`) instead of alpha masks. Drivers set it
    /// from what their renderer can blend per channel; flipping it drops
    /// the glyph atlas so every glyph re-rasterizes in the new mode.
    pub fn set_subpixel_text(&mut self, on: bool) {
        if self.text.set_subpixel(on) {
            self.atlas.clear();
        }
    }

    pub fn subpixel_text(&self) -> bool {
        self.text.subpixel()
    }

    /// The frame clock for transitions: monotonic seconds, any origin.
    /// Drivers set it before every frame; a driver that never does gets
    /// snapping instead of animation.
    pub fn set_time(&mut self, now_secs: f64) {
        self.anim.set_time(now_secs);
    }

    /// True when the last frame left a transition mid-flight, or a view
    /// asked for another frame — drivers schedule one without waiting for
    /// input.
    pub fn animating(&self) -> bool {
        self.anim.animating() || self.frame_requested
    }

    /// Asks the driver for one more frame right after this one. A view
    /// that sets up a transition by drawing a starting state (a new split
    /// drawn collapsed so it can slide open) needs the next frame to come
    /// without waiting for input — the starting state itself snaps, so
    /// nothing is mid-flight yet to request it.
    pub fn request_frame(&mut self) {
        self.frame_requested = true;
    }

    /// Starts a frame. Build the tree through the returned `Ui` (or the
    /// `Core` builder methods directly), then `finish_frame()`.
    pub fn frame(&mut self, viewport: Size, scale: f32) -> Ui<'_> {
        self.begin_frame(viewport, scale);
        Ui::new(self)
    }

    /// The viewport (logical px) the current frame was begun with. Changes
    /// to it arrive as `resize` events (see `take_pending_events`).
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// The device pixel ratio the current frame was begun with.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// The finished frame's draw data: display list plus the glyph atlas the
    /// renderer mirrors (mutable so it can clear the dirty flag).
    pub fn output(&mut self) -> (&DisplayList, &mut GlyphAtlas) {
        (&self.display, &mut self.atlas)
    }

    // -- Frame builder ------------------------------------------------------
    // Flat, non-panicking, callable through FFI. Misuse (close past the root,
    // building outside a frame) is ignored rather than UB or panic.

    pub fn begin_frame(&mut self, viewport: Size, scale: f32) {
        // A window that changed size is a fact the driver reports, so the
        // core turns it into data like any other: `{kind="resize", width,
        // height, scale}` on the root, pending for the driver to route
        // after the frame. The first frame establishes the viewport rather
        // than resizing it.
        if self.framed && (viewport != self.viewport || scale != self.scale) {
            self.pending.push(UiEvent {
                origin: OriginId::HOST,
                key: Key::ROOT,
                payload: Value::map([
                    ("kind", Value::str("resize")),
                    ("width", Value::Float(viewport.w as f64)),
                    ("height", Value::Float(viewport.h as f64)),
                    ("scale", Value::Float(scale as f64)),
                ]),
            });
        }
        self.framed = true;
        self.frame_no += 1;
        if self.frame_no.is_multiple_of(240) {
            let cutoff = self.frame_no.saturating_sub(300);
            self.layouts.retain(|_, (_, seen)| *seen >= cutoff);
        }
        self.viewport = viewport;
        self.scale = scale;
        self.window_title = None;
        // Last frame's focus declarations are what this frame's are
        // compared against (see `set_key_focus`).
        std::mem::swap(&mut self.declared_focus, &mut self.declared_focus_last);
        self.declared_focus.clear();
        self.tree.clear();
        self.display.clear();
        self.text.begin_frame(scale);
        self.anim.begin_frame();
        self.tree.push(
            NIL,
            Key::ROOT,
            OriginId::HOST,
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0)),
            NodeContent::Container,
        );
        self.stack.clear();
        self.stack.push(0);
        self.counters.clear();
        self.counters.push(0);
        self.origin = OriginId::HOST;
        self.any_clip = false;
        self.any_float = false;
        self.any_modal = false;
        self.any_slide = false;
        self.any_layout = false;
        self.frame_requested = false;
    }

    /// Declares this frame's window title. Like all frame state it's data:
    /// the driver diffs against what's applied and only then touches the
    /// window. Undeclared frames leave the title alone; last writer wins.
    pub fn set_window_title(&mut self, title: &str) {
        self.window_title = Some(title.to_string());
    }

    /// The title declared this frame, if any (for the frame driver).
    pub fn window_title(&self) -> Option<&str> {
        self.window_title.as_deref()
    }

    /// Tags subsequently created nodes with an origin (set by the runner
    /// before handing the frame to an extension).
    pub fn set_origin(&mut self, origin: OriginId) {
        self.origin = origin;
    }

    /// Replaces the implicit root's spec (e.g. to make the top level a row).
    /// Root sizing is resolved against the viewport regardless.
    pub fn configure_root(&mut self, mut spec: NodeSpec) {
        if !self.tree.is_empty() {
            self.ease_spec(Key::ROOT, &mut spec);
            if spec.on_layout.is_some() {
                self.any_layout = true;
            }
            self.tree.specs[0] = spec;
        }
    }

    /// Replaces a transitioning node's animatable values with this frame's
    /// eased ones. Nodes without a transition cost one branch. A slot the
    /// node's keyframes name is sampled from its cycle instead of tweened.
    fn ease_spec(&mut self, key: Key, spec: &mut NodeSpec) {
        let Some(t) = spec.transition else {
            return;
        };
        let anim = &mut self.anim;
        let tracks = (!spec.keyframes.is_empty()).then(|| Tracks::of(spec));
        let track = |slot: Slot| tracks.as_ref().and_then(|k| k.get(slot));
        let enter = spec.enter.unwrap_or_default();
        let mut sizing = |slot: Slot, s: Sizing, from: Option<Sizing>| {
            let Some(v) = s.amount() else {
                return s;
            };
            let from = from.and_then(|f| f.amount()).map(|f| [f, 0.0, 0.0, 0.0]);
            let eased = match track(slot) {
                Some(track) => anim.sample(track, t).map_or(v, |v| v[0]),
                None => anim.drive(key, slot, from, [v, 0.0, 0.0, 0.0], t, true)[0],
            };
            s.with_amount(eased)
        };
        spec.layout.width = sizing(Slot::Width, spec.layout.width, enter.width);
        spec.layout.height = sizing(Slot::Height, spec.layout.height, enter.height);
        let mut color = |slot: Slot, c: Color, from: Option<Color>| {
            let target = [c.r, c.g, c.b, c.a];
            let from = from.map(|f| [f.r, f.g, f.b, f.a]);
            let v = match track(slot) {
                Some(track) => anim.sample(track, t).unwrap_or(target),
                None => anim.drive(key, slot, from, target, t, true),
            };
            Color {
                r: v[0],
                g: v[1],
                b: v[2],
                a: v[3],
            }
        };
        spec.style.bg = color(Slot::Bg, spec.style.bg, enter.bg);
        spec.style.border_color = color(Slot::Border, spec.style.border_color, None);
        spec.style.radius = match track(Slot::Radius) {
            Some(track) => anim.sample(track, t).unwrap_or(spec.style.radius),
            None => anim.drive(
                key,
                Slot::Radius,
                enter.radius.map(|r| [r; 4]),
                spec.style.radius,
                t,
                true,
            ),
        };
    }

    /// The root node's key — for hover/press queries or `set_key_focus` when
    /// the root itself declares the interaction (e.g. a root-level key sink).
    pub fn root_key(&self) -> Key {
        self.tree.keys.first().copied().unwrap_or(Key(0))
    }

    fn current(&self) -> u32 {
        self.stack.last().copied().unwrap_or(0)
    }

    fn auto_key(&mut self) -> Key {
        let parent = self.current() as usize;
        let i = self.counters.last().copied().unwrap_or(0);
        if let Some(c) = self.counters.last_mut() {
            *c += 1;
        }
        self.tree.keys[parent].index(i)
    }

    /// The key a child labeled `label` would get — usable before creating it,
    /// e.g. to check hover state for styling.
    pub fn child_key(&self, label: &str) -> Key {
        self.tree.keys[self.current() as usize].str(label)
    }

    pub fn is_hovered(&self, key: Key) -> bool {
        self.interaction.is_hovered(key)
    }

    pub fn is_pressed(&self, key: Key) -> bool {
        self.interaction.is_pressed(key)
    }

    /// Whether any member of hover group `group` (see
    /// `NodeSpec::hover_group`) is hovered.
    pub fn is_group_hovered(&self, group: u64) -> bool {
        self.interaction.is_group_hovered(group)
    }

    /// Whether hover group `group` is pressed (press started on a member,
    /// pointer still over one).
    pub fn is_group_pressed(&self, group: u64) -> bool {
        self.interaction.is_group_pressed(group)
    }

    /// Events raised outside `handle_input`: the `resize` a changed
    /// viewport produced at `begin_frame`, and `on_hover` enter/leave
    /// caused by a finished frame changing what sits under a still cursor.
    /// Frame drivers route these after `finish_frame`; they also ride along
    /// with the next `handle_input` result, so a driver that never calls
    /// this merely sees them a little later.
    pub fn take_pending_events(&mut self) -> Vec<UiEvent> {
        let mut out = std::mem::take(&mut self.pending);
        out.append(&mut self.interaction.take_pending());
        out
    }

    /// Swaps in the hover / pressed / focus background the spec declares
    /// for the node's (or its group's) current state: pressed wins over
    /// keyboard-visible focus wins over hover. A disabled node keeps its
    /// plain `bg`. Runs before easing so a `transition` tweens between
    /// the states.
    fn resolve_hover_style(&self, key: Key, spec: &mut NodeSpec) {
        if spec.disabled
            || (spec.hover_bg.is_none() && spec.pressed_bg.is_none() && spec.focus_bg.is_none())
        {
            return;
        }
        let group = spec.hover_group;
        let pressed = self.interaction.is_pressed(key)
            || group.is_some_and(|g| self.interaction.is_group_pressed(g));
        let hovered = pressed
            || self.interaction.is_hovered(key)
            || group.is_some_and(|g| self.interaction.is_group_hovered(g));
        let focused = self.focus_visible && self.focus == Some(key);
        if pressed && let Some(c) = spec.pressed_bg {
            spec.style.bg = c;
        } else if focused && let Some(c) = spec.focus_bg {
            spec.style.bg = c;
        } else if hovered && let Some(c) = spec.hover_bg {
            spec.style.bg = c;
        }
    }

    /// Physical modifier state as of the last `InputEvent::Modifiers`.
    pub fn modifiers(&self) -> crate::input::KeyMods {
        self.interaction.modifiers()
    }

    pub fn open(&mut self, spec: NodeSpec) -> Key {
        let key = self.auto_key();
        self.open_with_key(key, spec);
        key
    }

    pub fn open_keyed(&mut self, label: &str, spec: NodeSpec) -> Key {
        let key = self.child_key(label);
        self.open_with_key(key, spec);
        key
    }

    fn open_with_key(&mut self, key: Key, mut spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        self.resolve_hover_style(key, &mut spec);
        self.ease_spec(key, &mut spec);
        if spec.layout.clips() {
            self.any_clip = true;
        }
        if spec.transition.is_some() && (spec.slide || spec.enter.is_some_and(|e| e.offsets())) {
            self.any_slide = true;
        }
        if spec.layout.float.is_some() {
            self.any_float = true;
        }
        if spec.modal.is_some() {
            self.any_modal = true;
        }
        if spec.on_layout.is_some() {
            self.any_layout = true;
        }
        let parent = self.current();
        let idx = self
            .tree
            .push(parent, key, self.origin, spec, NodeContent::Container);
        self.stack.push(idx);
        self.counters.push(0);
    }

    pub fn close(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
            self.counters.pop();
        }
    }

    pub fn text_node(&mut self, content: &str, style: TextStyle) {
        if self.tree.is_empty() {
            return;
        }
        let tid = self.text.add(content, &style, &self.resources);
        let key = self.auto_key();
        let parent = self.current();
        self.tree.push(
            parent,
            key,
            self.origin,
            NodeSpec::default(),
            NodeContent::Text(tid),
        );
    }

    /// An editable text node. State (buffer, cursor, selection) is retained
    /// by key across frames; edits arrive via `handle_input` and come back to
    /// the host as "changed"/"submit" events. Read with `edit_text`.
    pub fn text_edit(
        &mut self,
        label: &str,
        initial: &str,
        opts: &EditOptions,
        mut spec: NodeSpec,
    ) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.child_key(label);
        self.ease_spec(key, &mut spec);
        if spec.on_layout.is_some() {
            self.any_layout = true;
        }
        self.edit.declare(
            key,
            initial,
            opts,
            self.origin,
            self.scale,
            self.text.font_system_mut(),
            &self.resources,
        );
        // Autofocus takes the keyboard only while nothing holds it — never
        // from a control Tab landed on.
        if opts.autofocus && self.focus.is_none() && !spec.disabled {
            self.set_focus(Some(key));
        }
        let parent = self.current();
        self.tree
            .push(parent, key, self.origin, spec, NodeContent::Edit(key));
        key
    }

    /// A registered image (see `Resources::add_image`). Fit sizing takes
    /// the image's pixel dimensions as logical px; a Fit height against a
    /// resolved width preserves the aspect ratio. `style.radius` rounds the
    /// corners.
    pub fn image_node(&mut self, id: crate::resources::ImageId, mut spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        let key = self.auto_key();
        self.resolve_hover_style(key, &mut spec);
        self.ease_spec(key, &mut spec);
        if spec.on_layout.is_some() {
            self.any_layout = true;
        }
        let parent = self.current();
        self.tree
            .push(parent, key, self.origin, spec, NodeContent::Image(id));
    }

    /// Unregisters an image and forgets its atlas slot.
    pub fn remove_image(&mut self, id: crate::resources::ImageId) {
        self.resources.remove_image(id);
        self.atlas.evict_image(id);
    }

    /// A paragraph of styled spans, shaped and wrapped as one flow.
    pub fn rich_text_node(&mut self, spans: &[Span<'_>], base: TextStyle) {
        if self.tree.is_empty() {
            return;
        }
        let tid = self.text.add_rich(spans, &base, &self.resources);
        let key = self.auto_key();
        let parent = self.current();
        self.tree.push(
            parent,
            key,
            self.origin,
            NodeSpec::default(),
            NodeContent::Text(tid),
        );
    }

    /// Runs layout and emission into `output()`, and installs this frame's
    /// hit and scroll regions for input handling.
    pub fn finish_frame(&mut self) {
        // Tolerate unclosed containers (an FFI caller may have bailed early).
        self.stack.truncate(1);
        self.counters.truncate(1);

        {
            let mut measure = Measure {
                text: &mut self.text,
                edit: &mut self.edit,
                resources: &self.resources,
            };
            layout::compute(
                &mut self.tree,
                &mut measure,
                &mut self.scroll,
                self.viewport,
            );
        }
        self.scroll_caret_into_view();
        if self.any_slide {
            self.ease_positions();
        }
        // Positions are final: report the rects views asked about, and
        // look for the misconfigurations that would otherwise fail silently.
        if self.any_layout {
            self.emit_layout_events();
        }
        self.diag
            .check(&self.tree, &self.text, &self.edit, self.frame_no);
        // The frame's modal scope, and the focus it moves: emission reads
        // it (everything outside is inert) and so does the Tab ring.
        self.modal = if self.any_modal {
            self.modal_scope()
        } else {
            None
        };
        self.resolve_modal_focus();

        let scale = self.scale;
        let mut hits: Vec<HitRegion> = self.interaction.take_hit_buffer();
        let mut scroll_regions: Vec<ScrollRegion> = Vec::new();
        self.display.viewport = Size::new(self.viewport.w * scale, self.viewport.h * scale);
        self.display.scale = scale;

        // configure_root can also introduce a clipper.
        let any_clip =
            self.any_clip || (!self.tree.is_empty() && self.tree.specs[0].layout.clips());
        let any_float = self.any_float;

        // inherited clip per node (logical): ancestors only, not the node
        // itself. Only materialized when something actually clips.
        self.clips.clear();
        if any_clip {
            self.clips.resize(self.tree.len(), NO_CLIP);
        }
        self.in_float.clear();
        if any_float {
            self.in_float.resize(self.tree.len(), false);
        }

        // Pass 1: clip/float propagation + in-flow emission (preorder =
        // paint order; parents precede children).
        for i in 0..self.tree.len() {
            let parent = self.tree.parent[i];
            let floats_here = self.tree.specs[i].layout.float.is_some();
            if any_float {
                self.in_float[i] = floats_here || (parent != NIL && self.in_float[parent as usize]);
            }
            let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
            let clip = if !any_clip {
                NO_CLIP
            } else {
                // Floating nodes escape ancestor clips.
                let clip = if parent == NIL || floats_here {
                    NO_CLIP
                } else {
                    let p = parent as usize;
                    if self.tree.specs[p].layout.clips() {
                        self.clips[p]
                            .intersect(&Rect::from_pos_size(self.tree.pos[p], self.tree.size[p]))
                    } else {
                        self.clips[p]
                    }
                };
                self.clips[i] = clip;
                clip
            };
            if any_float && self.in_float[i] {
                continue; // deferred to the float pass
            }
            // Entirely clipped away: skip drawing and hit-testing.
            let visible = rect.intersect(&clip);
            if visible.w <= 0.0 || visible.h <= 0.0 {
                continue;
            }
            self.emit_node(i, rect, clip, scale, &mut hits, &mut scroll_regions);
        }

        // Pass 2: floating subtrees, on top of all in-flow content (their
        // hit regions land last too, so they're topmost for input).
        if any_float {
            for i in 0..self.tree.len() {
                if !self.in_float[i] {
                    continue;
                }
                let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
                let clip = if any_clip { self.clips[i] } else { NO_CLIP };
                let visible = rect.intersect(&clip);
                if visible.w <= 0.0 || visible.h <= 0.0 {
                    continue;
                }
                self.emit_node(i, rect, clip, scale, &mut hits, &mut scroll_regions);
            }
        }

        // Scrollbars, on top of content: indicator quads plus the hit
        // regions that make their thumbs draggable.
        let cursor = self.interaction.cursor();
        let mut scrollbars: Vec<ScrollbarRegion> = Vec::new();
        for r in &scroll_regions {
            let i = self
                .tree
                .keys
                .iter()
                .position(|k| *k == r.key)
                .expect("scroll region from this frame");
            let max = self.tree.scroll_max[i];
            let offset = self.scroll.offset(r.key);
            let clip = self.clips[i].scaled(scale);
            if max.y > 0.0 {
                let track_h = r.rect.h - 2.0 * SCROLLBAR_INSET;
                let bar_h = (track_h * r.rect.h / (r.rect.h + max.y)).max(SCROLLBAR_MIN);
                let t = (offset.y / max.y).clamp(0.0, 1.0);
                let track = Rect::new(
                    r.rect.x + r.rect.w - SCROLLBAR_HIT_W,
                    r.rect.y + SCROLLBAR_INSET,
                    SCROLLBAR_HIT_W,
                    track_h,
                );
                let active = self.interaction.is_scrollbar_dragging(r.key, ScrollAxis::Y)
                    || cursor.is_some_and(|p| track.contains(p));
                let w = if active {
                    SCROLLBAR_ACTIVE_W
                } else {
                    SCROLLBAR_W
                };
                let thumb = Rect::new(
                    r.rect.x + r.rect.w - w - SCROLLBAR_INSET,
                    r.rect.y + SCROLLBAR_INSET + t * (track_h - bar_h),
                    w,
                    bar_h,
                );
                self.display
                    .quads
                    .push(scrollbar_quad(thumb, scale, clip, active));
                scrollbars.push(ScrollbarRegion {
                    key: r.key,
                    axis: ScrollAxis::Y,
                    thumb,
                    track,
                    bar_len: bar_h,
                    max: max.y,
                    inert: r.inert,
                });
            }
            if max.x > 0.0 {
                let track_w = r.rect.w - 2.0 * SCROLLBAR_INSET;
                let bar_w = (track_w * r.rect.w / (r.rect.w + max.x)).max(SCROLLBAR_MIN);
                let t = (offset.x / max.x).clamp(0.0, 1.0);
                let track = Rect::new(
                    r.rect.x + SCROLLBAR_INSET,
                    r.rect.y + r.rect.h - SCROLLBAR_HIT_W,
                    track_w,
                    SCROLLBAR_HIT_W,
                );
                let active = self.interaction.is_scrollbar_dragging(r.key, ScrollAxis::X)
                    || cursor.is_some_and(|p| track.contains(p));
                let w = if active {
                    SCROLLBAR_ACTIVE_W
                } else {
                    SCROLLBAR_W
                };
                let thumb = Rect::new(
                    r.rect.x + SCROLLBAR_INSET + t * (track_w - bar_w),
                    r.rect.y + r.rect.h - w - SCROLLBAR_INSET,
                    bar_w,
                    w,
                );
                self.display
                    .quads
                    .push(scrollbar_quad(thumb, scale, clip, active));
                scrollbars.push(ScrollbarRegion {
                    key: r.key,
                    axis: ScrollAxis::X,
                    thumb,
                    track,
                    bar_len: bar_w,
                    max: max.x,
                    inert: r.inert,
                });
            }
        }

        self.emit_focus_ring(scale);

        self.interaction.set_hits(hits);
        // A new frame can move a hover-sound node under a still cursor.
        self.flush_sound_requests();
        self.audio.reconcile();
        self.interaction.scroll_regions = scroll_regions;
        self.interaction.scrollbars = scrollbars;
        self.ime_rect = self.focused_caret_rect();
    }

    /// After layout: nodes that `slide` ease from last frame's position
    /// toward where layout put them, carrying their subtree along (hit
    /// regions come from the same positions, so input follows the motion).
    /// Nodes whose `enter` has an offset start that far away on first
    /// sight and ease in the same way; without `slide` that entrance is
    /// all their position ever eases. Preorder means a parent shifts before
    /// its children are visited, so nested sliders ease relative to an
    /// already-eased parent.
    fn ease_positions(&mut self) {
        for i in 0..self.tree.len() {
            let spec = &self.tree.specs[i];
            let Some(t) = spec.transition else {
                continue;
            };
            let enter = spec.enter.filter(|e| e.offsets());
            if !spec.slide && enter.is_none() {
                continue;
            }
            let key = self.tree.keys[i];
            let target = self.tree.pos[i];
            let from = enter.map(|e| [target.x + e.dx, target.y + e.dy, 0.0, 0.0]);
            let v = self.anim.drive(
                key,
                Slot::Pos,
                from,
                [target.x, target.y, 0.0, 0.0],
                t,
                spec.slide,
            );
            let d = Vec2::new(v[0] - target.x, v[1] - target.y);
            if d.x == 0.0 && d.y == 0.0 {
                continue;
            }
            let end = self.tree.subtree_end(i);
            for p in &mut self.tree.pos[i..end] {
                p.x += d.x;
                p.y += d.y;
            }
        }
    }

    /// After layout: every `on_layout` node whose rect differs from the one
    /// last reported for its key — or that was not seen last frame — posts
    /// `{kind="layout", x, y, w, h, parent, tag}`, pending like a `resize`.
    /// A frame that leaves a node where it was posts nothing, so a view
    /// that stores the rect in its model and redraws does not loop.
    fn emit_layout_events(&mut self) {
        let frame_no = self.frame_no;
        for i in 0..self.tree.len() {
            let Some(tag) = &self.tree.specs[i].on_layout else {
                continue;
            };
            let key = self.tree.keys[i];
            let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
            let changed = match self.layouts.get(&key) {
                Some((last, seen)) if *seen + 1 == frame_no => *last != rect,
                _ => true,
            };
            self.layouts.insert(key, (rect, frame_no));
            if !changed {
                continue;
            }
            let parent = self.tree.parent[i];
            let parent_rect = if parent == NIL {
                Rect::new(0.0, 0.0, self.viewport.w, self.viewport.h)
            } else {
                let p = parent as usize;
                Rect::from_pos_size(self.tree.pos[p], self.tree.size[p])
            };
            let rect_value = |r: Rect| {
                Value::map([
                    ("x", Value::Float(r.x as f64)),
                    ("y", Value::Float(r.y as f64)),
                    ("w", Value::Float(r.w as f64)),
                    ("h", Value::Float(r.h as f64)),
                ])
            };
            let mut entries = vec![
                ("kind".to_string(), Value::str("layout")),
                ("x".to_string(), Value::Float(rect.x as f64)),
                ("y".to_string(), Value::Float(rect.y as f64)),
                ("w".to_string(), Value::Float(rect.w as f64)),
                ("h".to_string(), Value::Float(rect.h as f64)),
                ("parent".to_string(), rect_value(parent_rect)),
            ];
            if *tag != Value::Null {
                entries.push(("tag".to_string(), tag.clone()));
            }
            self.pending.push(UiEvent {
                origin: self.tree.origins[i],
                key,
                payload: Value::Map(entries),
            });
        }
    }

    /// After content and scrollbars: the default focus ring around the
    /// keyboard-visibly focused node, on top of everything, in the same
    /// display list every binding draws. Not for editors (the caret shows
    /// focus), key sinks (an app surface styles itself, through
    /// `is_focused` / `focus_visible`) or nodes declaring `focus_bg`.
    fn emit_focus_ring(&mut self, scale: f32) {
        if !self.focus_visible {
            return;
        }
        let Some(i) = self.focus_index() else {
            return;
        };
        let spec = &self.tree.specs[i];
        let editor = matches!(self.tree.content[i], NodeContent::Edit(_))
            || spec.role.is_some_and(crate::access::Role::is_editor);
        if editor || spec.on_key.is_some() || spec.focus_bg.is_some() || spec.disabled {
            return;
        }
        let node = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
        let rect = Rect::new(
            node.x - FOCUS_RING_GAP,
            node.y - FOCUS_RING_GAP,
            node.w + 2.0 * FOCUS_RING_GAP,
            node.h + 2.0 * FOCUS_RING_GAP,
        );
        let clip = self.clips.get(i).copied().unwrap_or(NO_CLIP);
        let visible = rect.intersect(&clip);
        if visible.w <= 0.0 || visible.h <= 0.0 {
            return;
        }
        self.display.quads.push(Quad {
            rect: rect.scaled(scale),
            color: Color::TRANSPARENT,
            border_color: FOCUS_RING,
            radius: spec.style.radius.map(|r| (r + FOCUS_RING_GAP) * scale),
            border_w: FOCUS_RING_W * scale,
            kind: QuadKind::Solid,
            uv: [0; 4],
            clip: clip.scaled(scale),
        });
    }

    /// See the `ime_rect` field. None when no editor is focused.
    pub fn ime_rect(&self) -> Option<Rect> {
        self.ime_rect
    }

    fn focused_caret_rect(&mut self) -> Option<Rect> {
        let key = self.edit.focused()?;
        let i = (0..self.tree.len()).find(|&i| self.tree.content[i] == NodeContent::Edit(key))?;
        let caret = self.edit.caret_rect(key, self.text.font_system_mut())?;
        let pad = self.tree.specs[i].layout.padding;
        Some(Rect::new(
            self.tree.pos[i].x + pad.l + caret.x / self.scale,
            self.tree.pos[i].y + pad.t + caret.y / self.scale,
            caret.w / self.scale,
            caret.h / self.scale,
        ))
    }

    /// After layout: if the focused edit's caret moved this frame, nudge the
    /// nearest scrollable ancestor so the caret stays visible, then re-run
    /// the positions pass with the adjusted offset (positions is the only
    /// pass scroll offsets feed into, so nothing else needs recomputing).
    fn scroll_caret_into_view(&mut self) {
        let Some(key) = self.edit.caret_moved.take() else {
            return;
        };
        if self.edit.focused() != Some(key) {
            return;
        }
        let Some(i) =
            (0..self.tree.len()).find(|&i| self.tree.content[i] == NodeContent::Edit(key))
        else {
            return;
        };
        let Some(caret_phys) = self.edit.caret_rect(key, self.text.font_system_mut()) else {
            return;
        };
        let pad = self.tree.specs[i].layout.padding;
        let caret = Rect::new(
            self.tree.pos[i].x + pad.l + caret_phys.x / self.scale,
            self.tree.pos[i].y + pad.t + caret_phys.y / self.scale,
            caret_phys.w / self.scale,
            caret_phys.h / self.scale,
        );
        self.scroll_rect_into_view(i, caret, true);
    }

    /// Nudges the nearest scrolling ancestor of node `i` so `rect`
    /// (viewport coordinates) is inside it. With `relayout`, re-runs the
    /// positions pass so this frame already shows the new offset
    /// (positions is the only pass scroll offsets feed into); without it
    /// the next frame does.
    fn scroll_rect_into_view(&mut self, i: usize, rect: Rect, relayout: bool) {
        // Slack so the target isn't glued to the container edge.
        const MARGIN: f32 = 4.0;
        let mut a = self.tree.parent[i];
        while a != NIL {
            let spec = self.tree.specs[a as usize].layout;
            if spec.scroll_x || spec.scroll_y {
                let view =
                    Rect::from_pos_size(self.tree.pos[a as usize], self.tree.size[a as usize]);
                let mut delta = Vec2::ZERO;
                if spec.scroll_y {
                    if rect.y < view.y + MARGIN {
                        delta.y = rect.y - (view.y + MARGIN);
                    } else if rect.y + rect.h > view.y + view.h - MARGIN {
                        delta.y = rect.y + rect.h - (view.y + view.h - MARGIN);
                    }
                }
                if spec.scroll_x {
                    if rect.x < view.x + MARGIN {
                        delta.x = rect.x - (view.x + MARGIN);
                    } else if rect.x + rect.w > view.x + view.w - MARGIN {
                        delta.x = rect.x + rect.w - (view.x + view.w - MARGIN);
                    }
                }
                if delta.x != 0.0 || delta.y != 0.0 {
                    self.scroll.scroll_by(self.tree.keys[a as usize], delta);
                    if relayout {
                        layout::positions(&mut self.tree, &mut self.scroll, self.viewport);
                    }
                }
                return;
            }
            a = self.tree.parent[a as usize];
        }
    }
}

/// Combined measurer handed to the layout pass: static text through the
/// shape cache, editors through the edit store (sharing one FontSystem),
/// images through the resource registry.
struct Measure<'a> {
    text: &'a mut TextSystem,
    edit: &'a mut EditStore,
    resources: &'a Resources,
}

impl TextMeasure for Measure<'_> {
    fn intrinsic(&mut self, id: crate::tree::TextId) -> Size {
        self.text.intrinsic(id)
    }

    fn wrapped(&mut self, id: crate::tree::TextId, max_w: f32) -> Size {
        self.text.wrapped(id, max_w)
    }

    fn edit_intrinsic(&mut self, key: Key) -> Size {
        self.edit.intrinsic(key, self.text.font_system_mut())
    }

    fn edit_wrapped(&mut self, key: Key, max_w: f32) -> Size {
        self.edit.wrapped(key, max_w, self.text.font_system_mut())
    }

    fn image_size(&mut self, id: crate::resources::ImageId) -> Size {
        self.resources
            .images
            .get(id)
            .map_or(Size::ZERO, |e| Size::new(e.width as f32, e.height as f32))
    }
}

fn scrollbar_quad(bar: Rect, scale: f32, clip: Rect, active: bool) -> Quad {
    Quad {
        rect: bar.scaled(scale),
        color: Color::rgba(1.0, 1.0, 1.0, if active { 0.4 } else { 0.18 }),
        border_color: Color::TRANSPARENT,
        radius: [bar.w.min(bar.h) / 2.0 * scale; 4],
        border_w: 0.0,
        kind: QuadKind::Solid,
        uv: [0; 4],
        clip,
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

/// A frontend that draws into the shared tree each frame — the trait the
/// runner uses to host Lua (or any other) extensions without knowing what
/// they are. Origins are assigned by the runner.
pub trait Extension {
    fn name(&self) -> &str;
    fn view(&mut self, ui: &mut Ui<'_>) -> Result<(), String>;
    fn on_event(&mut self, ev: &UiEvent);
}
