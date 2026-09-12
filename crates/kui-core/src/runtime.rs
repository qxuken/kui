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
use crate::depart::{At, DepartStore, Ghost, GhostContent, Place, Playback, Replay};
use crate::diag::{Diagnostics, Warning};
use crate::display::{Clip, ClipId, DisplayList, NO_CLIP_ID, Quad, QuadKind};
use crate::edit::{EditOptions, EditStore};
use crate::env::{Env, SystemEnv};
use crate::geom::{Rect, Size, Vec2};
use crate::input::{
    EditKey, HitRegion, InputEvent, Interaction, KeyCode, KeyPhase, KeyPress, MouseButton,
    ScrollAxis, ScrollRegion, ScrollbarRegion, UiEvent,
};
use crate::key::{Key, LabelIndex};
use crate::keyframes;
use crate::layout::{self, TextMeasure};
use crate::line::Stroke;
use crate::resources::{FontId, Resources};
use crate::scroll::ScrollStore;
use crate::session::{
    MAIN_WINDOW_NAME, Session, SharedAudio, SharedResources, WindowChange, WindowDecl,
};
use crate::spec::{NodeSpec, Sizing, TextStyle};
use crate::stats::FrameStats;
use crate::text::TextHit;
use crate::text::{Span, TextMetrics, TextSystem};
use crate::theme::{Theme, ThemeSource};
use crate::tree::{NIL, NodeContent, OriginId, Tree};
use crate::ui::Ui;
use crate::value::Value;
use crate::window::{WindowConfig, WindowId};

// `impl Core` continues in these, one concern per file (each opens with
// what it holds). Children of this module, so the fields stay private.
mod builder;
pub use builder::Content;
mod composites;
pub mod devtools;
mod dispatch;
mod emit;
mod fills;
mod focus;
pub mod inspect;
mod menu_api;
mod menubar_api;
mod resources_api;
mod scrolling;
mod select_api;
mod windows;

/// Wheel line-delta to logical px.
const SCROLL_LINE_PX: f32 = 40.0;

/// What a `Core::focus_region` call asked to enter, held until the frame
/// finishes (`docs/adr/0022-focus-regions.md`, decision 4): the main ring,
/// a region by key, or one by the label its node declares — the spelling
/// a caller has for a node the last frame did not build.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RegionTarget {
    Main,
    Key(Key),
    Label(String),
}

pub struct Core {
    /// The caches and registries this window shares with the rest of its
    /// session. Everything a frame needs from it is borrowed inside a
    /// method and dropped before it returns; see `session`'s module doc.
    session: Session,
    /// This window's shaped-text cache and rasterizer. Not the session's:
    /// its cache entries are stamped with `atlas`'s epoch, and `TextId`
    /// indexes its per-frame list.
    pub text: TextSystem,
    /// The frame's cell grids and their glyph tables (backlog C20).
    pub cells: crate::cells::CellStore,
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
    /// Where this window's palette comes from; see [`crate::theme`].
    /// `Derived` unless the app said otherwise, so an app that never
    /// mentions themes still follows the OS.
    theme_source: ThemeSource,
    /// `theme_source` resolved against `env.system`, re-resolved at the
    /// start of every frame. Read by the stock widgets, by the core's own
    /// chrome (ring, scrollbar, selection) and by any view that asks.
    theme: Theme,
    /// The sizes the stock widgets are built from (backlog T2): the
    /// palette's other axis, set by the app or [`Metrics::default`].
    metrics: crate::metrics::Metrics,
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
    /// The `.str`-keyed nodes this frame and last, with their labels
    /// (`open_keyed`): what `key_of` resolves a name through. The same
    /// swap-and-clear pair as the focus declarations, so a frame that
    /// keys nothing costs two clears.
    key_labels: LabelIndex,
    key_labels_last: LabelIndex,
    /// The slots this frame declared (`begin_slot`), by name: what the
    /// `"root"` fill and the `unknown-slot` check read, and what makes a
    /// second declaration of one name a `duplicate-slot`. Cleared each
    /// frame; never compared to the last one, since a slot is a position
    /// and not a declaration the core diffs.
    slot_labels: LabelIndex,
    /// The key namespace of the fill in progress (ADR 0014 decision 4):
    /// while the node stack is exactly `ns_depth` deep, a child's key is
    /// derived from `ns_key` instead of from the node it is opened under,
    /// so an extension's nodes are keyed by the slot and the extension
    /// rather than by whatever the host built around them. `usize::MAX`
    /// when no fill is in progress — one compare on the auto-key path.
    ns_depth: usize,
    ns_key: Key,
    /// Between `begin_frame` and `finish_frame`: `key_labels` is partial
    /// and `key_labels_last` is the last whole frame, and `key_of` reads
    /// both; outside a build `key_labels` is the whole last frame and is
    /// the only one read.
    building: bool,
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
    /// Whether `finish_frame` copies the frame into `inspected` (see
    /// `runtime/inspect.rs`); off unless a devtool asked.
    inspect: bool,
    inspected: Vec<inspect::NodeInfo>,
    /// The devtools' hold on this window's frame (`docs/adr/0024`): the
    /// tree index of the app container the host's tree is wrapped in
    /// while the panel is docked, whether this core draws the panel's own
    /// window, and the theme source the host had before the panel
    /// overrode it.
    dt_app: Option<usize>,
    dt_window: bool,
    dt_saved_theme: Option<ThemeSource>,
    /// The panel was built at `begin_frame` (a left dock precedes the
    /// app's container in tree order), so `finish` must not build again.
    dt_built: bool,
    /// The host's viewport in window coordinates: the whole window, or
    /// what the dock leaves of it while the panel is docked (ADR 0024).
    /// What `Core::viewport` reports, what a `resize` is measured on,
    /// what the host's viewport floats resolve against, and the origin
    /// every coordinate the host is handed or hands in is relative to.
    dt_area: Rect,
    /// The previous frame's tree, kept only while a frame declares `exit`
    /// — `begin_frame` swaps the two buffers instead of clearing one, so
    /// the frame that notices a node gone still has the node. Empty (and
    /// untouched) for every frame that declares no exit.
    prev_tree: Tree,
    /// The frame's strokes, indexed by the `line` nodes' `LineId`s; the
    /// previous frame's kept alongside on the same terms as `prev_tree`.
    pub(crate) lines: crate::line::LineStore,
    pub(crate) fragments: crate::fragment::FragmentList,
    /// The stock polygon fragment's handle, once a `polygon` node has
    /// asked for it this session (ADR 0025, decision 6). Forgotten by
    /// `remove_fragment` if a host removes it, so the next node registers
    /// it again rather than drawing nothing — and not re-checked per node,
    /// which was a session lock per polygon and cost more than the six
    /// segment quads a closed stroke of the same outline emits.
    pub(crate) stock_polygon: Option<crate::resources::FragmentId>,
    /// Texture-backed images removed since the last frame began, handed
    /// to the next display list as `dropped_textures` so a backend frees
    /// them; kept here because a removal can land between frames, after
    /// the list was cleared.
    pub(crate) dropped_images: Vec<crate::resources::ImageId>,
    /// The hit shapes the frame being emitted builds beside its regions
    /// (ADR 0026), handed to `interaction` with them at the end of
    /// emission; the previous frame's buffers, cleared, in between.
    pub(crate) hit_shapes: crate::input::HitShapes,
    pub(crate) display: DisplayList,
    pub(crate) viewport: Size,
    pub(crate) scale: f32,
    // Frame-builder state.
    stack: Vec<u32>,
    counters: Vec<u64>,
    origin: OriginId,
    /// Per-node inherited clip (logical), rebuilt each finish_frame.
    clips: Vec<Clip>,
    /// The `DisplayList::clips` index each of those became, so a node
    /// whose clip is its parent's names the entry the parent already
    /// interned instead of asking again. Parallel to `clips`.
    clip_ids: Vec<ClipId>,
    /// Per-node inherited group opacity (the product down the ancestors),
    /// rebuilt each finish_frame and only materialized when something
    /// actually fades.
    opacity: Vec<f32>,
    /// Per node, the layer it paints in: the index of its nearest floating
    /// ancestor-or-self, `NIL` in flow (ADR 0023). Only filled on a frame
    /// that floats something.
    float_root: Vec<u32>,
    /// The float layers as the last frame painted them, bottom to top:
    /// each root's key and its rank among that frame's float roots in
    /// tree order. A root the next frame keeps stays where it is, one it
    /// opens goes on top, one it closes leaves — so the stack is the
    /// order the layers opened in (ADR 0023, decision 3). Bounded by the
    /// frame's own float count; nothing to evict.
    float_stack: Vec<(Key, u32)>,
    /// The hover hints of the nodes open right now that declared a
    /// `tooltip` (`Core::hint`), each with the stack depth it was opened
    /// at, so `close` knows whose turn it is. Only nodes that declared one
    /// are here: a frame without a tooltip pays one length check per close.
    hints: Vec<(usize, Key, String)>,
    /// The context menu this window has open, the keys the stock renderer
    /// gave its rows (so their clicks can be told from the app's), and
    /// what choosing one left for the host to do. See
    /// `docs/adr/0017-selection-as-a-scope.md`, decision 5.
    menu: Option<crate::menu::Menu>,
    menu_actions: Vec<crate::menu::MenuAction>,
    /// The editor that held focus when the menu opened, since the menu's
    /// own rows take focus from it — what Cut, Copy and Select All act on.
    menu_editor: Option<Key>,
    /// Whether the host draws menus itself (`set_native_menus`).
    native_menus: bool,
    /// The application menu this frame has in force, and a count bumped
    /// whenever it changes, so a driver diffs against one integer rather
    /// than against a tree (`docs/adr/0018-a-menu-bar-the-app-declares.md`).
    /// `None` is a declaration nobody has made; an empty `MenuBar` is one
    /// that took the bar away.
    menu_bar: Option<crate::menu::MenuBar>,
    menu_bar_rev: u64,
    /// Who declared it, and so who hears the events its items post — the
    /// host, or the extension whose view declared the bar.
    menu_bar_origin: OriginId,
    /// Which of the drawn bar's menus is open, and the bar's root this
    /// frame — where a menu-bar event lands. Its titles and rows are known
    /// by their origin (`OriginId::MENU_BAR`), not by key.
    menu_bar_open: Option<usize>,
    menu_bar_root: Option<Key>,
    /// Whether the platform owns the menu bar (`set_native_menu_bar`), in
    /// which case the drawn one draws nothing and the driver hands the
    /// declaration over instead.
    native_menu_bar: bool,
    /// Whether the host can show a definition panel
    /// (`set_lookup_available`).
    lookup_available: bool,
    /// The window's text selection outside an editor, and what the
    /// frame resolved it to: `sel_ords` numbers the text nodes of the
    /// selection's scope in emission order (`u32::MAX` for a node
    /// outside it), and `sel_ends` is the pair of ends in reading order,
    /// `None` when this frame builds neither end. See
    /// `docs/adr/0017-selection-as-a-scope.md`.
    selection: Option<crate::select::Selection>,
    /// The window's selection when it is in a `cells` grid instead of in
    /// text. One selection per window: starting either clears the other,
    /// which `Core::set_selection` / `set_cell_selection` enforce in one
    /// place each (ADR 0017, decisions 1 and 4).
    cell_selection: Option<crate::select::CellSelection>,
    /// Whether a `selectionrange` ask is outstanding (ADR 0017, tier 3).
    awaiting_selection: bool,
    /// The drag a press is running through a selection scope, if any: set
    /// on the press inside a scope, cleared on release. The counterpart of
    /// `EditStore::dragging` for text nobody is editing.
    select_dragging: Option<crate::select::SelectDrag>,
    sel_ords: Vec<u32>,
    sel_ends: Option<crate::select::Ends>,
    /// Per-node innermost enclosing selection scope — the key of the
    /// nearest ancestor (or the node itself) declaring `selectable`, and
    /// `None` outside every scope. Filled only on a frame that declares
    /// one at all (`Tree::any_selectable`), which is what keeps ADR 0017
    /// off the frames of apps that never select anything.
    scopes: Vec<Option<Key>>,
    /// Per-node enclosing virtualised row index, filled beside `scopes`:
    /// what places an endpoint whose own node is no longer built.
    rows: Vec<Option<u64>>,
    /// The frame's modal scope: the tree range `[i, subtree_end(i))` of the
    /// last node declaring `modal`, and its key. Everything outside it is
    /// inert and out of the Tab ring (see
    /// `docs/adr/0003-modal-surfaces.md`). Recomputed by `finish_frame`.
    modal: Option<(usize, usize, Key)>,
    /// The modals declared by the last finished frame, in tree order, each
    /// with the focus it displaced: a modal that stops being declared gives
    /// that focus back.
    modal_focus: Vec<(Key, Option<Key>)>,
    /// Per-node inherited opacity while a departing subtree is replayed
    /// (`depart`), reused across ghosts and frames.
    ghost_opacity: Vec<f32>,
    /// Per-node inherited clip and painted rect while a departing subtree
    /// is replayed: the clips its own clippers establish, since a ghost
    /// draws outside every clip its ancestors held (`depart`).
    ghost_clip: Vec<Clip>,
    /// The interned index of each of those, as `clip_ids` is for `clips`.
    ghost_clip_ids: Vec<ClipId>,
    ghost_rect: Vec<Rect>,
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
    /// The OS settings the last frame was begun with. A driver pushes
    /// them into `env` whenever it learns of a change, and the difference
    /// between two frames is what becomes a `system` event — the same
    /// bookkeeping `viewport` does for `resize` (backlog F40).
    system_seen: SystemEnv,
    /// Frames begun so far; stamps the per-key stores below.
    frame_no: u64,
    /// The rect last reported for each `on_layout` node and the frame it
    /// was seen: a different rect, or a node not seen last frame, posts a
    /// `layout` event (see `emit_layout_events`).
    layouts: FxHashMap<Key, (Rect, u64)>,
    /// One-off announcements queued since the last drain (see
    /// [`Self::announce`] and
    /// `docs/adr/0008-live-regions-and-announcements.md`). The fourth of
    /// the four drained channels, and the same shape as the other three:
    /// the core appends, a driver drains, a headless test asserts on what
    /// it drained.
    announcements: Vec<crate::access::Announcement>,
    /// The last announcement's text, the frame it was queued on and the
    /// [`Self::events_answered`] reading then: the same text on two
    /// consecutive frames with no event handed to the app between them is
    /// what an unguarded `ui.announce(...)` in a view looks like, and
    /// `announcement-repeated` says so. The event count is what tells a
    /// window that redraws only on input apart from one shouting every
    /// frame — two Copy presses in a row are two consecutive frames there.
    last_announcement: Option<(String, u64, u64)>,
    /// How many times `handle_input` handed the app at least one event.
    events_answered: u64,
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
    /// The focus region in effect — the key of the node whose subtree Tab
    /// walks — or `None` for the main ring (the tree minus every region).
    /// Follows focus: `set_focus` moves it to the region enclosing the
    /// focused node, a press settles it on the region under the pointer,
    /// and it is kept across a blur so Tab re-enters where the user was.
    region: Option<Key>,
    /// Whether a press settled `region` somewhere other than the focused
    /// node's own region — dead space in the dock, focus on the root sink
    /// the press bubbled to — so the region stops following that focus
    /// until it moves (decision 3's second sentence). Cleared by any
    /// `set_focus` that changes the focus.
    region_held: bool,
    /// The focus each region last held, main (`None`) included: what
    /// `focus_region` lands on when entering it again, and what main gets
    /// back when the region in effect stops being declared.
    region_focus: Vec<(Option<Key>, Option<Key>)>,
    /// A `focus_region` waiting for the frame to finish, for the reason
    /// `pending_focus_step` waits: the ring it enters is the finished
    /// tree's, and the caller may name a node the last frame did not have.
    /// Last writer wins.
    pending_region: Option<RegionTarget>,
    /// Silent-misconfiguration detection; see `diag`.
    diag: Diagnostics,
    /// The access tree of the last finished frame, built on demand (see
    /// `access_tree`) and stamped with the frame it was built from.
    access: crate::access::AccessTree,
    access_built: u64,
    /// The hash of the inputs `self.access` was derived from, so a frame
    /// whose access-relevant state is unchanged keeps it (ADR 0016,
    /// decision 3). `None` when the last frame could not be hashed, which
    /// forces the next derivation.
    access_inputs: Option<u64>,
    /// How many times the access tree has actually been derived, as against
    /// asked for. Tests read it to tell a cache hit from a miss; nothing in
    /// the library acts on it.
    access_rebuilds: u64,
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
    /// This window's palette, as of this frame
    /// (`docs/adr/0019-a-theme-derived-from-appearance-and-accent.md`).
    /// Resolved from [`Core::theme_source`] and `env.system` at the start
    /// of every frame, so it is already right by the time a view runs.
    ///
    /// Frame-stable on purpose: every widget in one frame paints from the
    /// same palette, whatever the driver does to `env` while the view
    /// runs. A host that writes `env.system` *directly* and wants the new
    /// answer before its next frame calls [`Core::refresh_theme`]; every
    /// env setter a binding exposes already does.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Re-resolve the palette from `env.system` now, rather than at the
    /// start of the next frame. What an env setter calls after writing.
    pub fn refresh_theme(&mut self) {
        self.theme = self.theme_source.resolve(&self.env.system);
    }

    /// Writes what the user set in the OS and re-resolves the palette from
    /// it, so a host that pushes the appearance and reads the theme back
    /// before its next frame sees the answer (ADR 0019). The one door for
    /// a binding's env setter: writing `env.system` by hand and forgetting
    /// the refresh was a decision each of them had to remember.
    pub fn set_system(&mut self, system: SystemEnv) {
        self.env.system = system;
        self.refresh_theme();
    }

    /// The env reading's inputs (`schema::ENV_FIELDS`): the stored env and
    /// the frame's own facts — viewport, scale, focus — that ride in the
    /// same reading.
    pub fn env_facts(&self) -> crate::schema::EnvFacts {
        crate::schema::EnvFacts {
            env: self.env,
            viewport: self.viewport,
            scale: self.scale,
            focus: self.focus(),
            focus_visible: self.focus_visible(),
            region: self.region(),
        }
    }

    /// Whether anyone actually *chose* the accent — the OS reported one,
    /// or the app set or pinned one — as opposed to the palette falling
    /// back to kui's own blue.
    ///
    /// The question the `accent` row asks before it repaints anything:
    /// that row has always meant "the accent colour where there is one,
    /// the `bg` I declared where there is not", so a view can name its
    /// own fallback and a host that knows nothing changes nothing. The
    /// theme widened *where* the accent comes from without widening
    /// *whether* there is one.
    pub fn has_accent(&self) -> bool {
        match self.theme_source {
            ThemeSource::Derived => self.env.system.accent.is_some(),
            ThemeSource::DerivedWithAccent(_) | ThemeSource::Pinned(_) => true,
        }
    }

    /// Where the palette comes from. [`ThemeSource::Derived`] by default.
    pub fn theme_source(&self) -> ThemeSource {
        self.theme_source
    }

    /// Change where the palette comes from. Takes effect on the next
    /// frame, and immediately for anything reading [`Core::theme`] after
    /// this call, so a host may set it before its first frame or in a
    /// handler and get the same answer either way.
    pub fn set_theme_source(&mut self, source: ThemeSource) {
        self.theme_source = source;
        self.refresh_theme();
    }

    /// Pin a palette: this exact [`Theme`], following neither the OS's
    /// appearance nor its accent. Shorthand for
    /// [`ThemeSource::Pinned`].
    pub fn set_theme(&mut self, theme: Theme) {
        self.set_theme_source(ThemeSource::Pinned(theme));
    }

    /// Keep following the OS's light/dark, but paint this accent instead
    /// of the OS's. Shorthand for [`ThemeSource::DerivedWithAccent`], and
    /// what an app with a brand colour wants.
    pub fn set_accent(&mut self, accent: Color) {
        self.set_theme_source(ThemeSource::DerivedWithAccent(accent));
    }

    /// Go back to following the OS for both — the default.
    pub fn derive_theme(&mut self) {
        self.set_theme_source(ThemeSource::Derived);
    }

    /// The sizes the stock widgets are built from — the palette's other
    /// axis (`crate::metrics`, backlog T2). [`Metrics::default`] until
    /// the app sets one; nothing in the OS is followed.
    pub fn metrics(&self) -> &crate::metrics::Metrics {
        &self.metrics
    }

    /// Makes `metrics` the frame's: every stock widget from the next node
    /// on is built from it, and `ui.metrics()` reads it back. Logical px,
    /// before `env.scale`; a density is the app's to choose
    /// (`Metrics::compact`, `Metrics::scaled`).
    pub fn set_metrics(&mut self, metrics: crate::metrics::Metrics) {
        self.metrics = metrics;
    }

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
            cells: crate::cells::CellStore::new(),
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
            theme_source: ThemeSource::Derived,
            theme: Theme::default(),
            metrics: crate::metrics::Metrics::default(),
            window_title: None,
            focus: None,
            focus_visible: false,
            declared_focus: Vec::new(),
            declared_focus_last: Vec::new(),
            key_labels: LabelIndex::default(),
            key_labels_last: LabelIndex::default(),
            slot_labels: LabelIndex::default(),
            ns_depth: usize::MAX,
            ns_key: Key::ROOT,
            dt_app: None,
            dt_window: false,
            dt_saved_theme: None,
            dt_built: false,
            dt_area: Rect::new(0.0, 0.0, 0.0, 0.0),
            building: false,
            declared_windows: Vec::new(),
            declared_windows_last: Vec::new(),
            keys_held: Vec::new(),
            access: Default::default(),
            access_built: 0,
            access_inputs: None,
            access_rebuilds: 0,
            tree: Tree::new(),
            inspect: false,
            inspected: Vec::new(),
            prev_tree: Tree::new(),
            lines: Default::default(),
            fragments: Default::default(),
            stock_polygon: None,
            dropped_images: Vec::new(),
            hit_shapes: Default::default(),
            display: DisplayList::default(),
            viewport: Size::ZERO,
            scale: 1.0,
            stack: Vec::new(),
            counters: Vec::new(),
            origin: OriginId::HOST,
            clips: Vec::new(),
            clip_ids: Vec::new(),
            opacity: Vec::new(),
            float_root: Vec::new(),
            float_stack: Vec::new(),
            hints: Vec::new(),
            menu: None,
            menu_actions: Vec::new(),
            menu_editor: None,
            native_menus: false,
            menu_bar: None,
            menu_bar_rev: 0,
            menu_bar_origin: OriginId::HOST,
            menu_bar_open: None,
            menu_bar_root: None,
            native_menu_bar: false,
            lookup_available: false,
            selection: None,
            cell_selection: None,
            awaiting_selection: false,
            select_dragging: None,
            sel_ords: Vec::new(),
            sel_ends: None,
            scopes: Vec::new(),
            rows: Vec::new(),
            modal: None,
            modal_focus: Vec::new(),
            type_ahead: String::new(),
            type_ahead_at: None,
            items_scratch: Vec::new(),
            ghost_opacity: Vec::new(),
            ghost_clip: Vec::new(),
            ghost_clip_ids: Vec::new(),
            ghost_rect: Vec::new(),
            frame_requested: false,
            pending_reveal: None,
            pending_focus_step: None,
            region: None,
            region_held: false,
            region_focus: Vec::new(),
            pending_region: None,
            ime_rect: None,
            pending: Vec::new(),
            framed: false,
            system_seen: SystemEnv::default(),
            frame_no: 0,
            layouts: FxHashMap::default(),
            announcements: Vec::new(),
            last_announcement: None,
            events_answered: 0,
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

    /// Where a point lands in the text node `key` drew: a byte offset into
    /// its content and the visual line, or `None` for a key that is not a
    /// text node or was not drawn (backlog C18). `point` is logical
    /// viewport px — the `x`/`y` a click or drag event carries — so a
    /// custom editor turns the event into a caret position with one call
    /// instead of measuring prefixes or assuming a cell width. Answered
    /// from the frame that finished: between frames that is the layout the
    /// pointer was over, and during a build it is the last one, since the
    /// node being declared has no layout yet. A wrapped node answers in
    /// the width it was drawn at.
    pub fn text_hit(&self, key: Key, point: Vec2) -> Option<TextHit> {
        self.text
            .hit_at(key, point.plus(self.dt_shift()), self.building)
    }

    /// The caret rect for byte `byte` of the text node `key` drew: logical
    /// viewport px, zero wide, one line tall — where a caret, an IME
    /// candidate window or a selection edge goes. `byte` past the content
    /// is the end. Answered from the same frame `text_hit` is.
    pub fn caret_rect(&self, key: Key, byte: usize) -> Option<Rect> {
        let shift = self.dt_shift();
        self.text
            .caret_at(key, byte, self.building)
            .map(|r| Rect::new(r.x - shift.x, r.y - shift.y, r.w, r.h))
    }

    // -- Announcements ---------------------------------------------------

    /// Says something once, with no node behind it: "Saved", "3 results".
    /// Queued for [`Self::take_announcements`], the way `play` queues an
    /// audio command — an announcement is a consequence of an event, and
    /// the frame's tree, which is a function of state, has no place to
    /// keep one (see `docs/adr/0008-live-regions-and-announcements.md`).
    /// A region whose text changes on screen is the other half, and is
    /// the `live` prop instead.
    ///
    /// [`Live::Off`] and an empty string are both no-ops — the first so a
    /// caller can gate politeness without an `if`, the second because
    /// every platform needs a name to say.
    pub fn announce(&mut self, text: &str, live: crate::access::Live) {
        if live == crate::access::Live::Off || text.is_empty() {
            return;
        }
        if let Some((last, frame, answered)) = &self.last_announcement
            && last == text
            && *frame + 1 >= self.frame_no
            && *answered == self.events_answered
        {
            self.diag.raise(crate::diag::announcement_repeated(text));
        }
        self.last_announcement = Some((text.to_string(), self.frame_no, self.events_answered));
        self.announcements.push(crate::access::Announcement {
            text: text.to_string(),
            live,
        });
    }

    /// Drains the announcements queued since the last drain. Windowed
    /// runners drain every frame whether or not assistive technology is
    /// attached, and discard what they cannot deliver, so a real app never
    /// accumulates and nothing is spoken minutes late; headless drivers
    /// assert on what comes back.
    pub fn take_announcements(&mut self) -> Vec<crate::access::Announcement> {
        std::mem::take(&mut self.announcements)
    }

    /// Announcements queued and not yet drained (what `pending` is for
    /// audio commands).
    pub fn pending_announcements(&self) -> &[crate::access::Announcement] {
        &self.announcements
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

    /// Every warning this core has raised, drained or not, oldest first —
    /// for a reader that is not the driver. The runner drains
    /// [`Self::take_warnings`] after every frame and prints them, so a
    /// view that wants to *show* them (a development overlay) would
    /// otherwise never see one; this is the log the drain leaves behind.
    pub fn warnings_raised(&self) -> &[Warning] {
        self.diag.raised()
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

    /// The byte budget for the shaped-text cache (backlog C16): every
    /// text a frame draws is shaped once and kept, and past this many
    /// estimated bytes the least recently drawn entries go, down to three
    /// quarters of it, at the start of the next frame. What the last
    /// frame drew is never evicted, so a budget too small for one
    /// screenful costs re-shaping nothing — it only stops keeping what
    /// scrolled away. Default `DEFAULT_TEXT_CACHE_BYTES` (64 MB): a
    /// terminal streaming new lines lowers it, a document viewer that
    /// wants every page it showed to stay warm raises it. The clock that
    /// empties an idle cache after 300 frames is unchanged.
    pub fn set_text_cache_budget(&mut self, bytes: usize) {
        self.text.set_budget(bytes);
    }

    pub fn text_cache_budget(&self) -> usize {
        self.text.budget()
    }

    /// What the shaped-text cache holds, as the estimate the budget is
    /// charged against (a fixed floor per entry plus a per-glyph rate,
    /// calibrated against a counting allocator; see `text.rs`).
    pub fn text_cache_bytes(&self) -> usize {
        self.text.bytes()
    }

    /// How many shaped texts the cache holds (a long line's chunks each
    /// count).
    pub fn text_cache_len(&self) -> usize {
        self.text.len()
    }

    /// How many long lines — no-wrap texts past `LONG_LINE_BYTES`, shaped
    /// in chunks — are held (backlog C19).
    pub fn long_lines(&self) -> usize {
        self.text.long_lines()
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
        self.anim.animating()
            || self.depart.animating()
            || self.frame_requested
            || self.tree.any_animate
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

    /// `frame` with something to fill the slots the view declares — the
    /// runner's extension list (`[Box<dyn Extension>]` is a `Fill`), or a
    /// test's stand-in. `Ui::slot` calls it in place, and `Ui::finish`
    /// lets it fill `"root"` and report unknown slots before layout. See
    /// `docs/adr/0014-slots-an-extension-fills-in-place.md`.
    pub fn frame_with<'a>(
        &'a mut self,
        viewport: Size,
        scale: f32,
        filler: &'a mut dyn crate::slot::Fill,
    ) -> Ui<'a> {
        self.begin_frame(viewport, scale);
        Ui::with_filler(self, filler)
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

    /// The viewport (logical px) the current frame was begun with — the
    /// window, less the devtools' dock while the panel is docked
    /// (`docs/adr/0024`): what the host lays out into. Changes to it
    /// arrive as `resize` events (see `take_pending_events`), a dock
    /// coming, going or resizing among them.
    pub fn viewport(&self) -> Size {
        Size::new(self.dt_area.w, self.dt_area.h)
    }

    /// The device pixel ratio the current frame was begun with.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// The origin nodes opened right now are tagged with: `OriginId::HOST`
    /// in the host's own view, the filling extension's inside a fill (see
    /// `Core::fill`).
    pub fn origin(&self) -> OriginId {
        self.origin
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
        // The host's viewport is what the dock leaves of the window
        // (ADR 0024), so a dock that comes, goes or is dragged is a
        // resize too.
        let area = self.devtools_area(viewport);
        if self.framed
            && (area.w != self.dt_area.w || area.h != self.dt_area.h || scale != self.scale)
        {
            self.pending.push(UiEvent {
                origin: OriginId::HOST,
                window: WindowId::MAIN,
                key: Key::ROOT,
                payload: Value::map([
                    ("kind", Value::str("resize")),
                    ("width", Value::Float(area.w as f64)),
                    ("height", Value::Float(area.h as f64)),
                    ("scale", Value::Float(scale as f64)),
                ]),
            });
        }
        self.dt_area = area;
        // And what the user set in the OS — and whether assistive
        // technology is listening, which rides in the same reading. A
        // driver that learns of a change writes it into `env` and asks
        // for a redraw — which is
        // enough for a host whose view is a function the runner calls
        // every frame, and nothing at all for one that retains the tree
        // it was handed (Node, C, Lua): its `view` runs when a message
        // changes the model, so the change has to *be* a message. The
        // first frame establishes the reading rather than reporting it,
        // the way the viewport does.
        if self.framed && self.env.system != self.system_seen {
            let sys = self.env.system;
            self.pending.push(UiEvent {
                origin: OriginId::HOST,
                window: WindowId::MAIN,
                key: Key::ROOT,
                payload: Value::map([
                    ("kind", Value::str("system")),
                    ("appearance", Value::str(sys.appearance.name())),
                    (
                        "accent",
                        sys.accent
                            .map_or(Value::Null, |c| Value::Int(c.to_hex() as i64)),
                    ),
                    ("motion", Value::str(sys.motion.name())),
                    (
                        "locale",
                        sys.locale.map_or(Value::Null, |l| Value::str(l.as_str())),
                    ),
                    ("assistive", Value::str(sys.assistive.name())),
                ]),
            });
        }
        self.system_seen = self.env.system;
        // The palette is a function of what the OS said and what the app
        // asked for, so it is recomputed rather than invalidated: a
        // couple of dozen float ops once a frame, against a cache that
        // would have to be poked from every writer of `env.system`.
        self.refresh_theme();
        self.framed = true;
        self.frame_no += 1;
        if self.frame_no.is_multiple_of(240) {
            let cutoff = self.frame_no.saturating_sub(300);
            self.layouts.retain(|_, (_, seen)| *seen >= cutoff);
        }
        self.viewport = viewport;
        self.scale = scale;
        self.window_title = None;
        // The drawn menu bar's root is this frame's: a view that stops
        // calling `widgets::menu_bar` leaves nothing behind for the next
        // event to land on. Re-recorded while the widget builds.
        self.menu_bar_root = None;
        // Last frame's focus declarations are what this frame's are
        // compared against (see `set_key_focus`).
        std::mem::swap(&mut self.declared_focus, &mut self.declared_focus_last);
        self.declared_focus.clear();
        // And the labels `key_of` resolves through, the same way.
        std::mem::swap(&mut self.key_labels, &mut self.key_labels_last);
        self.key_labels.clear();
        // Slots are positions, not declarations to diff: one clear.
        self.slot_labels.clear();
        self.ns_depth = usize::MAX;
        self.building = true;
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
        let keep_prev = self.tree.any_exit;
        if keep_prev {
            std::mem::swap(&mut self.tree, &mut self.prev_tree);
        } else {
            self.prev_tree.clear();
        }
        self.tree.clear();
        self.display.clear();
        self.display
            .dropped_textures
            .append(&mut self.dropped_images);
        // The text list goes with the tree: a kept frame's text nodes carry
        // that frame's `TextId`s, and nothing else can resolve them.
        self.text
            .begin_frame(&mut self.session.state().fonts, scale, keep_prev);
        // And the strokes, for the same reason: a kept frame's `line`
        // nodes index that frame's list.
        self.lines.begin_frame(keep_prev);
        self.fragments.begin_frame(keep_prev);
        self.cells.begin_frame(scale);
        self.sync_font_names();
        self.anim.begin_frame();
        self.depart.begin_frame();
        // The two stores that keep state by key across a key's absence:
        // they stamp this frame onto what it declares, and cap what it
        // does not (backlog F26).
        self.edit.begin_frame(self.frame_no);
        self.scroll.begin_frame(self.frame_no);
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
        // A frame that ended with nodes unclosed must not leak its hints
        // into the next one.
        self.hints.clear();
        self.origin = OriginId::HOST;
        self.frame_requested = false;
        self.devtools_begin_frame();
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
///
/// Where it draws is a slot the host declared
/// (`docs/adr/0014-slots-an-extension-fills-in-place.md`): `slots` names
/// the ones it fills, `view` is called once per frame for each of them
/// with which one it is, and an extension naming none is called once
/// after the host's view for the reserved `"root"` slot — the sequence
/// every extension got before slots existed. `on_event` answers with
/// replies: values the runner hands to the host's own `on_event`, with
/// this extension's origin on them.
pub trait Extension {
    fn name(&self) -> &str;
    /// The slot names this extension fills; empty means `"root"`.
    fn slots(&self) -> &[String] {
        &[]
    }
    fn view(&mut self, slot: &crate::slot::Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String>;
    /// One of this extension's events; the values returned are replies
    /// to the host (ADR 0014 decision 6), delivered in order.
    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value>;
}
