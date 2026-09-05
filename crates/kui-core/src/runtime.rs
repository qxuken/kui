//! The `Core`: one window. It owns everything that survives across frames
//! and belongs to this window alone (interaction state, scroll and edit
//! stores, focus) plus the reusable per-frame tree — and the frame-builder
//! state itself. Builder state living here (not in a borrowing wrapper) is
//! what lets flat C bindings drive a frame through one opaque pointer; the
//! Rust `Ui` is a thin safe façade.
//!
//! What must not be duplicated per window — the resource registry, the
//! shaping caches, the glyph atlas and the audio store — lives in the
//! [`crate::session::Session`] a core is constructed against.
//! [`Core::new`] makes a private one, so a single-window app never sees it.

use std::rc::Rc;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::anim::{AnimStore, Slot, Track};
use crate::atlas::GlyphAtlas;
use crate::color::Color;
use crate::depart::{DepartStore, Ghost, GhostContent, Pass, Place, Playback, Replay};
use crate::diag::{Diagnostics, Warning};
use crate::display::{Clip, DisplayList, NO_CLIP, Quad, QuadKind};
use crate::edit::{EditOptions, EditStore};
use crate::env::Env;
use crate::geom::{Rect, Size, Vec2};
use crate::input::{
    EditKey, HitRegion, InputEvent, Interaction, KeyPhase, KeyPress, MouseButton, ScrollAxis,
    ScrollRegion, ScrollbarRegion, UiEvent,
};
use crate::key::Key;
use crate::keyframes::{self, Keyframe};
use crate::layout::{self, TextMeasure};
use crate::resources::{FontId, Resources};
use crate::scroll::ScrollStore;
use crate::session::{
    MAIN_WINDOW_NAME, Session, SharedAudio, SharedResources, WindowChange, WindowDecl,
};
use crate::spec::{NodeSpec, Sizing, TextStyle};
use crate::stats::FrameStats;
use crate::text::{Span, TextMetrics, TextSystem};
use crate::tree::{NIL, NodeContent, OriginId, Tree};
use crate::ui::Ui;
use crate::value::Value;
use crate::window::{WindowConfig, WindowId};

// `impl Core` continues in these, one concern per file (each opens with
// what it holds). Children of this module, so the fields stay private.
mod builder;
mod composites;
mod dispatch;
mod emit;
mod focus;
mod resources_api;
mod scrolling;
mod windows;

/// Wheel line-delta to logical px.
const SCROLL_LINE_PX: f32 = 40.0;

pub struct Core {
    /// The caches and registries this window shares with the rest of its
    /// session. Everything a frame needs from it is borrowed inside a
    /// method and dropped before it returns; see `session`'s module doc.
    session: Session,
    /// This window's shaped-text cache and rasterizer. Not the session's:
    /// its cache entries are stamped with `atlas`'s epoch, and `TextId`
    /// indexes its per-frame list.
    pub text: TextSystem,
    /// This window's glyph atlas — the CPU side of its renderer's texture,
    /// handed out by `output`.
    pub atlas: GlyphAtlas,
    /// The session's resource registry. A font, image or sound registered
    /// through it is registered for every window in the session.
    pub resources: SharedResources,
    /// The session's audio store: one device for the process, so playback
    /// bookkeeping and the command queue are shared, not per window.
    pub audio: SharedAudio,
    /// The family names of the session's fonts as of this window's last
    /// frame, so `font_family` can lend one out. Refreshed from
    /// `SessionState::fonts_rev`.
    font_names: FxHashMap<FontId, std::rc::Rc<str>>,
    fonts_rev: u64,
    pub interaction: Interaction,
    pub scroll: ScrollStore,
    pub edit: EditStore,
    /// Transition tweens, keyed by node; see `anim`. Fed by `set_time`.
    pub anim: AnimStore,
    /// Subtrees the view stopped declaring, played out and then dropped;
    /// see `depart`. Empty unless something declares `exit`.
    pub depart: DepartStore,
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
    /// Windows this frame and last declared (`declare_window`): the same
    /// edge-triggered shape as the focus pair, one level up. The session's
    /// registry diffs the union across every core at `finish_frame`; the
    /// pair here is what makes a frame that changed nothing cost nothing.
    declared_windows: Vec<WindowDecl>,
    declared_windows_last: Vec<WindowDecl>,
    /// The presses delivered to the current focus and not yet released,
    /// in press order. A `KeyUp` is routed only when its press is in here
    /// (so a sink never sees a release it did not see the press of), and
    /// focus leaving synthesizes the missing releases from it.
    keys_held: Vec<KeyPress>,
    pub(crate) tree: Tree,
    /// The previous frame's tree, kept only while a frame declares `exit`
    /// — `begin_frame` swaps the two buffers instead of clearing one, so
    /// the frame that notices a node gone still has the node. Empty (and
    /// untouched) for every frame that declares no exit.
    prev_tree: Tree,
    pub(crate) display: DisplayList,
    pub(crate) viewport: Size,
    pub(crate) scale: f32,
    // Frame-builder state.
    stack: Vec<u32>,
    counters: Vec<u64>,
    origin: OriginId,
    /// Per-node inherited clip (logical), rebuilt each finish_frame.
    clips: Vec<Clip>,
    /// Whether any node this frame clips — lets emission skip clip math
    /// entirely for the common unclipped case.
    any_clip: bool,
    /// Whether any node this frame clips *and* has a radius, so the clip
    /// its descendants inherit is rounded. Separate from `any_clip`: the
    /// per-corner bookkeeping is skipped for the ordinary square clip.
    any_rounded_clip: bool,
    /// Per-node inherited group opacity (the product down the ancestors),
    /// rebuilt each finish_frame and only materialized when something
    /// actually fades.
    opacity: Vec<f32>,
    any_opacity: bool,
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
    /// Whether any node this frame declared a workable `exit`. Gates the
    /// tree swap and the key diff, so a frame with no exits pays one bool.
    any_exit: bool,
    /// Per-node inherited opacity while a departing subtree is replayed
    /// (`depart`), reused across ghosts and frames.
    ghost_opacity: Vec<f32>,
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
    /// A `reveal(key)` waiting for a layout to resolve against: the next
    /// `finish_frame` scrolls the node's scrolling ancestor to show it,
    /// then clears this. Last writer wins.
    pending_reveal: Option<Key>,
    /// Type-ahead inside a composite (`docs/adr/0007`, decision 9): the
    /// characters typed so far, and the frame clock reading of the last
    /// keystroke. The buffer is cleared at the start of the first frame
    /// more than [`TYPE_AHEAD_SECS`] after it, so input routing stays
    /// timeless — the aging happens where time already lives. With no
    /// clock set `type_ahead_at` is None and every keystroke starts a
    /// fresh search, which is the useful half of type-ahead.
    type_ahead: String,
    type_ahead_at: Option<f64>,
    /// Reused by the composite walks (the ring, arrow motion), so a frame
    /// with composites in it pays one allocation rather than one each.
    items_scratch: Vec<usize>,
    /// A `focus_next` / `focus_prev` waiting for a frame to walk: `true`
    /// forward. The Tab ring is the finished tree's, and a request made
    /// while a frame is being built has no tree to walk yet (`begin_frame`
    /// cleared it), so the step is held until `finish_frame` — after the
    /// modal scope is resolved, which is what scopes the ring. Last writer
    /// wins, and an applied step beats a `set_focus` from the same frame.
    pending_focus_step: Option<bool>,
    /// Silent-misconfiguration detection; see `diag`.
    diag: Diagnostics,
    /// The access tree of the last finished frame, built on demand (see
    /// `access_tree`) and stamped with the frame it was built from.
    access: crate::access::AccessTree,
    access_built: u64,
}

/// How long a type-ahead search buffer survives without a keystroke
/// (`docs/adr/0007-composite-keyboard-patterns.md`, decision 9). Aged at
/// the start of a frame, so a core with no clock never ages one.
const TYPE_AHEAD_SECS: f64 = 1.0;

/// One step along a list of `len` items from `at`, wrapping or clamping.
/// None = the step ran off the end of a list that clamps.
fn step(at: usize, delta: isize, len: usize, wrap: bool) -> Option<usize> {
    let n = len as isize;
    let p = at as isize + delta;
    if (0..n).contains(&p) {
        return Some(p as usize);
    }
    wrap.then(|| (((p % n) + n) % n) as usize)
}

impl Core {
    /// A core with a session of its own — one window, nothing shared.
    pub fn new() -> Self {
        Self::new_in(&Session::new())
    }

    /// A core joining an existing session: it draws with the same fonts,
    /// images, sounds, shaping caches and glyph atlas as every other core
    /// constructed against `session`, and plays through the same audio
    /// device. Everything else — tree, focus, scroll, viewport — is this
    /// window's alone.
    pub fn new_in(session: &Session) -> Self {
        let mut core = Self {
            session: session.clone(),
            text: TextSystem::new(),
            atlas: GlyphAtlas::new(),
            resources: SharedResources::new(session),
            audio: SharedAudio::new(session),
            font_names: FxHashMap::default(),
            fonts_rev: u64::MAX,
            interaction: Interaction::default(),
            scroll: ScrollStore::default(),
            edit: EditStore::default(),
            anim: AnimStore::default(),
            depart: DepartStore::default(),
            stats: FrameStats::default(),
            env: Env::default(),
            window_title: None,
            focus: None,
            focus_visible: false,
            declared_focus: Vec::new(),
            declared_focus_last: Vec::new(),
            declared_windows: Vec::new(),
            declared_windows_last: Vec::new(),
            keys_held: Vec::new(),
            access: Default::default(),
            access_built: 0,
            tree: Tree::new(),
            prev_tree: Tree::new(),
            display: DisplayList::default(),
            viewport: Size::ZERO,
            scale: 1.0,
            stack: Vec::new(),
            counters: Vec::new(),
            origin: OriginId::HOST,
            clips: Vec::new(),
            any_clip: false,
            any_rounded_clip: false,
            opacity: Vec::new(),
            any_opacity: false,
            in_float: Vec::new(),
            any_float: false,
            any_modal: false,
            modal: None,
            modal_focus: Vec::new(),
            type_ahead: String::new(),
            type_ahead_at: None,
            items_scratch: Vec::new(),
            any_slide: false,
            any_layout: false,
            any_exit: false,
            ghost_opacity: Vec::new(),
            frame_requested: false,
            pending_reveal: None,
            pending_focus_step: None,
            ime_rect: None,
            pending: Vec::new(),
            framed: false,
            frame_no: 0,
            layouts: FxHashMap::default(),
            diag: Diagnostics::default(),
        };
        core.sync_font_names();
        core
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
        let sess = &mut *self.session.state();
        self.text
            .measure(content, style, &sess.resources, &mut sess.fonts, max_w)
    }

    /// `measure_text` for a rich-text paragraph.
    pub fn measure_rich_text(
        &mut self,
        spans: &[Span<'_>],
        base: &TextStyle,
        max_w: Option<f32>,
    ) -> TextMetrics {
        let sess = &mut *self.session.state();
        self.text
            .measure_rich(spans, base, &sess.resources, &mut sess.fonts, max_w)
    }

    // -- Diagnostics ----------------------------------------------------

    /// Drains the warnings raised since the last drain (see [`crate::diag`]):
    /// silent misconfigurations the core noticed while finishing frames,
    /// each distinct (code, node) pair once. Windowed runners print them;
    /// headless tests assert on them.
    pub fn take_warnings(&mut self) -> Vec<Warning> {
        // Handles of other sessions resolve where the registry can see
        // them and this core cannot (shaping, the audio backend), so the
        // registry keeps them and the core draining warnings reports them.
        for f in self.session.state().resources.take_foreign() {
            self.diag.raise(crate::diag::foreign_resource(&f));
        }
        self.diag.take()
    }

    /// Raises a warning a binding built (see [`crate::diag::unknown_prop`]):
    /// a frontend sees declarations the tree walk cannot, because a prop
    /// name nothing claims never becomes part of a node. Behind the same
    /// [`Self::set_diagnostics`] gate and the same once-per-(code, key)
    /// dedup as the checks, so a binding may raise one per node per frame.
    pub fn warn(&mut self, warning: Warning) {
        self.diag.raise(warning);
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
        self.anim.animating() || self.depart.animating() || self.frame_requested
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

    /// The session this core draws from. Hand it to `Core::new_in` to open
    /// another window sharing its fonts, images, sounds and glyph atlas.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Runs an edit-store operation against the session's font system —
    /// the one the text cache shapes with, so an editor and a text node
    /// measure the same. The session borrow lasts exactly the call.
    fn edit_with_fonts<T>(
        &mut self,
        f: impl FnOnce(&mut EditStore, &mut cosmic_text::FontSystem) -> T,
    ) -> T {
        let sess = &mut *self.session.state();
        f(&mut self.edit, &mut sess.fonts)
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

    pub fn begin_frame(&mut self, viewport: Size, scale: f32) {
        self.age_type_ahead();
        // A window that changed size is a fact the driver reports, so the
        // core turns it into data like any other: `{kind="resize", width,
        // height, scale}` on the root, pending for the driver to route
        // after the frame. The first frame establishes the viewport rather
        // than resizing it.
        if self.framed && (viewport != self.viewport || scale != self.scale) {
            self.pending.push(UiEvent {
                origin: OriginId::HOST,
                window: WindowId::MAIN,
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
        // And the window declarations, which `finish_frame` diffs the same
        // way (see `declare_window`).
        std::mem::swap(&mut self.declared_windows, &mut self.declared_windows_last);
        self.declared_windows.clear();
        // A frame that declared an `exit` may be the last one some node is
        // ever seen in, so it is kept whole: the two tree buffers swap
        // roles instead of one being cleared, which costs an allocation
        // that already existed and no copying. Nothing else keeps it — a
        // frame with no exits empties the spare, so a stale tree can never
        // be diffed against.
        let keep_prev = self.any_exit;
        if keep_prev {
            std::mem::swap(&mut self.tree, &mut self.prev_tree);
        } else {
            self.prev_tree.clear();
        }
        self.tree.clear();
        self.display.clear();
        // The text list goes with the tree: a kept frame's text nodes carry
        // that frame's `TextId`s, and nothing else can resolve them.
        self.text
            .begin_frame(&mut self.session.state().fonts, scale, keep_prev);
        self.sync_font_names();
        self.anim.begin_frame();
        self.depart.begin_frame();
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
        self.any_rounded_clip = false;
        self.any_opacity = false;
        self.any_float = false;
        self.any_modal = false;
        self.any_slide = false;
        self.any_layout = false;
        self.any_exit = false;
        self.frame_requested = false;
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
