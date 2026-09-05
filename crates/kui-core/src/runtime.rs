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
use crate::depart::{DepartStore, Ghost, GhostContent, Playback};
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

mod composites;
mod dispatch;
mod emit;
mod focus;
mod scrolling;

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

/// A node's keyframes flattened per slot for `ease_spec`, built once per
/// node per frame (only for nodes that declare keyframes).
struct Tracks {
    width: Option<Vec<(f32, [f32; 4])>>,
    height: Option<Vec<(f32, [f32; 4])>>,
    bg: Option<Vec<(f32, [f32; 4])>>,
    radius: Option<Vec<(f32, [f32; 4])>>,
    opacity: Option<Vec<(f32, [f32; 4])>>,
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
            opacity: keyframes::track(frames, &offsets, one(spec.style.opacity), |k| {
                k.opacity.map(one)
            }),
        }
    }

    fn get(&self, slot: Slot) -> Option<&Track> {
        match slot {
            Slot::Width => self.width.as_deref(),
            Slot::Height => self.height.as_deref(),
            Slot::Bg => self.bg.as_deref(),
            Slot::Radius => self.radius.as_deref(),
            Slot::Opacity => self.opacity.as_deref(),
            // Keyframing a shadow would need stops for four more numbers
            // and a color; a shadow tweens with `transition` and no more.
            Slot::Border | Slot::Pos | Slot::Shadow | Slot::ShadowColor => None,
        }
    }
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

    /// Turns the sounds nodes asked for (`click_sound` / `hover_sound`)
    /// into play commands. Declarative sounds carry no tag, so they never
    /// report `ended`.
    fn flush_sound_requests(&mut self) {
        let mut sess = self.session.state();
        for sound in self.interaction.take_sound_requests() {
            sess.audio.play(
                OriginId::HOST,
                Key::ROOT,
                sound,
                crate::audio::PlayOptions::default(),
            );
        }
    }

    // -- Fonts ----------------------------------------------------------

    /// Registers a font from its file bytes (TTF/OTF/TTC); `None` when the
    /// data holds no usable face. Shape with it via `TextStyle::font`.
    pub fn add_font_data(&mut self, data: Vec<u8>) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::Source;
        let id = {
            let sess = &mut *self.session.state();
            let db = sess.fonts.db_mut();
            let ids = db.load_font_source(Source::Binary(std::sync::Arc::new(data)));
            let family = db.face(*ids.first()?)?.families.first()?.0.clone();
            sess.fonts_rev += 1;
            sess.resources.add_font(family, ids.to_vec())
        };
        self.sync_font_names();
        Some(id)
    }

    /// Registers a font file (TTF/OTF/TTC) by path, memory-mapped by the
    /// font database; `None` when it cannot be read or holds no usable
    /// face. Shape with it via `TextStyle::font`.
    pub fn load_font_file(
        &mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::Source;
        let id = {
            let sess = &mut *self.session.state();
            let db = sess.fonts.db_mut();
            let ids = db.load_font_source(Source::File(path.into()));
            let family = db.face(*ids.first()?)?.families.first()?.0.clone();
            sess.fonts_rev += 1;
            sess.resources.add_font(family, ids.to_vec())
        };
        self.sync_font_names();
        Some(id)
    }

    /// Loads every font file under `dir` (recursively) into the font
    /// database, so their families become available to `add_system_font`
    /// by name; returns how many faces were added. A bundled `fonts/`
    /// folder next to the app is the usual case.
    pub fn load_fonts_dir(&mut self, dir: impl AsRef<std::path::Path>) -> usize {
        let sess = &mut *self.session.state();
        let db = sess.fonts.db_mut();
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
        let id = {
            let sess = &mut *self.session.state();
            let db = sess.fonts.db();
            let query = Query {
                families: &[Family::Name(name)],
                ..Default::default()
            };
            let id = db.query(&query)?;
            // The canonical spelling, so the style matches the way fontdb does.
            let family = db.face(id)?.families.first()?.0.clone();
            if let Some((id, _)) = sess
                .resources
                .fonts
                .iter()
                .find(|(_, f)| f.faces.is_empty() && f.family == family)
            {
                return Some(id);
            }
            sess.fonts_rev += 1;
            sess.resources.add_font(family, Vec::new())
        };
        self.sync_font_names();
        Some(id)
    }

    /// Forgets a registered font; faces loaded from bytes leave the font
    /// database. Styles still naming it shape as sans-serif.
    pub fn remove_font(&mut self, id: crate::resources::FontId) {
        {
            let sess = &mut *self.session.state();
            let Some(entry) = sess.resources.remove_font(id) else {
                return;
            };
            sess.fonts_rev += 1;
            let db = sess.fonts.db_mut();
            for face in entry.faces {
                db.remove_face(face);
            }
        }
        self.sync_font_names();
    }

    /// The registered family name behind a font handle, if it is live.
    /// Read from this window's mirror of the session's fonts (see
    /// `session`'s module doc), which every registration refreshes and so
    /// does every frame — a font another window registered mid-frame shows
    /// up here on the next one.
    pub fn font_family(&self, id: crate::resources::FontId) -> Option<&str> {
        self.font_names.get(&id).map(|s| &**s)
    }

    /// Family names of every installed font the core can see (sorted,
    /// deduplicated) — what `add_system_font` accepts.
    pub fn system_font_families(&self) -> Vec<String> {
        let sess = self.session.state();
        let mut names: Vec<String> = sess
            .fonts
            .db()
            .faces()
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    // -- Declared windows ---------------------------------------------------
    // `docs/adr/0004-multi-window.md`, decisions 4-6: a window's existence
    // is declared the way a title is, the session diffs the union of what
    // every live window's frame declared, and the diff's `Open` / `Close`
    // go out through the same queue chrome commands do.

    /// Declares that a window named `name` exists this frame.
    ///
    /// The window opens on the first frame any core declares it — that
    /// frame's `finish_frame` queues a [`crate::WindowCommand::Open`] with
    /// the id the core assigned and raises `{kind:"window",
    /// phase:"opened", name, id}` — and closes, with a `Close` and a
    /// `phase:"closed"`, on the first frame none does. Declared every
    /// frame it costs nothing after the first.
    ///
    /// `config` is read on that opening edge and never again: re-declaring
    /// a live window at another size changes nothing, because the user
    /// owns its geometry once it exists. Where two declarations of one
    /// name disagree on that edge, the lowest declaring window's first
    /// declaration wins and [`crate::diag::DUPLICATE_WINDOW_CONFIG`]
    /// says so. A window the user closed (see [`Self::window_closed`])
    /// does not reopen while it is still declared — the declaration has
    /// to stop and start again — and keeps raising
    /// [`crate::diag::WINDOW_DECLARED_WHILE_CLOSED`] until it does.
    /// `"main"` names the window the launcher opened and is always live.
    pub fn declare_window(&mut self, name: &str, config: WindowConfig) {
        if let Some(d) = self.declared_windows.iter_mut().find(|d| &*d.name == name) {
            // First declaration wins within a frame; a disagreement is
            // remembered for the opening edge to report.
            d.conflict |= d.config != config;
            return;
        }
        self.declared_windows.push(WindowDecl {
            name: Rc::from(name),
            config,
            origin: self.origin,
            conflict: false,
        });
    }

    /// The windows declared while the current frame was built, for the
    /// scene corpus's coverage derivation: a declaration leaves no node.
    pub(crate) fn declared_windows(&self) -> &[WindowDecl] {
        &self.declared_windows
    }

    /// The name of the window this core draws: `"main"` for
    /// `WindowId::MAIN`, else the name the declaration that opened
    /// `env.window.id` used. A driver that gave a core an id the session
    /// never opened gets the id spelled out, so the answer is never empty.
    pub fn window_name(&self) -> Rc<str> {
        let id = self.env.window.id;
        if id == WindowId::MAIN {
            return Rc::from(MAIN_WINDOW_NAME);
        }
        self.session
            .state()
            .windows
            .name_of(id)
            .unwrap_or_else(|| Rc::from(format!("window-{}", id.0).as_str()))
    }

    /// Every window of the session that is open right now — main first,
    /// then in the order they opened — as `(id, name)`. What the
    /// declaration diff has asked for, not what the driver has shown:
    /// the two agree once the driver drains its commands.
    pub fn windows(&self) -> Vec<(WindowId, Rc<str>)> {
        self.session.state().windows.live()
    }

    /// The driver reports that the OS closed window `id` — its close
    /// button, a keyboard shortcut, the window manager. The window is gone
    /// and stays gone while its name is still declared (the app has to stop
    /// declaring it and start again to reopen it; see
    /// [`Self::declare_window`]); its own declarations leave the union, so
    /// whatever only it declared closes too. Raises `{kind:"window",
    /// phase:"closed", name, id}` for the app, pending like a resize.
    /// Nothing happens for the main window (closing it ends the app) or
    /// for a window the diff already closed.
    pub fn window_closed(&mut self, id: WindowId) {
        let mut changes = Vec::new();
        let name = {
            let sess = &mut *self.session.state();
            let Some(name) = sess.windows.os_closed(id) else {
                return;
            };
            sess.windows.diff(&mut changes);
            name
        };
        self.push_window_event("closed", &name, id);
        self.apply_window_changes(changes);
    }

    /// Hands this frame's declarations to the session and takes back what
    /// the union's diff decided. Runs at `finish_frame`; skipped whole when
    /// this frame declared what the last one did and no window is sitting
    /// closed-but-declared, which is every frame of a single-window app.
    fn sync_windows(&mut self) {
        let changed = self.declared_windows != self.declared_windows_last;
        let mut changes = Vec::new();
        let mut still_closed: Vec<Rc<str>> = Vec::new();
        {
            let sess = &mut *self.session.state();
            let reg = &mut sess.windows;
            if !changed && !reg.any_closed() {
                return;
            }
            if changed {
                reg.set_slot(self.env.window.id, &self.declared_windows);
                reg.diff(&mut changes);
            }
            still_closed.extend(reg.closed_among(&self.declared_windows));
        }
        for name in still_closed {
            self.diag
                .raise(crate::diag::window_declared_while_closed(&name));
        }
        self.apply_window_changes(changes);
    }

    /// Turns the diff's decisions into what a driver and an app see: a
    /// command in the queue, a `{kind:"window"}` event, and the conflict
    /// warning where the opening edge found one.
    fn apply_window_changes(&mut self, changes: Vec<WindowChange>) {
        for change in changes {
            match change {
                WindowChange::Opened {
                    id,
                    name,
                    origin,
                    config,
                    conflict,
                } => {
                    self.interaction
                        .window_commands
                        .push(crate::window::WindowCommand::Open { id, origin, config });
                    self.push_window_event("opened", &name, id);
                    if conflict {
                        self.diag.raise(crate::diag::duplicate_window_config(&name));
                    }
                }
                WindowChange::Closed { id, name } => {
                    self.interaction
                        .window_commands
                        .push(crate::window::WindowCommand::Close(id));
                    self.push_window_event("closed", &name, id);
                }
            }
        }
    }

    /// `{kind:"window", phase, name, id}` on the root, pending for the
    /// driver to route after the frame (or with the next input). `id` is
    /// in the payload because `UiEvent::window` says which core reported
    /// it, and the diff runs on whichever core finished its frame.
    fn push_window_event(&mut self, phase: &str, name: &str, id: WindowId) {
        self.pending.push(UiEvent {
            origin: OriginId::HOST,
            window: WindowId::MAIN,
            key: Key::ROOT,
            payload: Value::map([
                ("kind", Value::str("window")),
                ("phase", Value::str(phase)),
                ("name", Value::str(name)),
                ("id", Value::Int(id.0 as i64)),
            ]),
        });
    }

    /// Queues a window command as if chrome had produced it, so apps can
    /// close/minimize/maximize from a keymap or command line. Drained by
    /// the frame driver with the rest.
    pub fn push_window_command(&mut self, cmd: crate::window::WindowCommand) {
        self.interaction.window_commands.push(cmd);
    }

    /// Asks the driver to resize `window` to `size` (logical px). Queued
    /// the way `reveal` and `play` queue theirs: a request the driver
    /// applies on its next pump — after this input dispatch if called from
    /// a handler, after this frame if called from a view — and one a
    /// headless driver never applies, since it never drains. The window
    /// answers through the ordinary `resize` event, with the size it
    /// actually became. Until ADR 0004's step 3 lets a frame declare more
    /// windows, `WindowId::MAIN` is the only one there is.
    pub fn set_window_size(&mut self, window: crate::window::WindowId, size: crate::geom::Size) {
        self.interaction
            .window_commands
            .push(crate::window::WindowCommand::SetSize { window, size });
    }

    /// Asks the driver to give `window` keyboard focus; queued like
    /// [`Core::set_window_size`]. Whether the window manager agrees shows
    /// up as `env.focused` on the frames that follow, not as a reply.
    pub fn focus_window(&mut self, window: crate::window::WindowId) {
        self.interaction
            .window_commands
            .push(crate::window::WindowCommand::Focus(window));
    }

    /// Drains window intents queued since the last drain — by chrome nodes,
    /// by `push_window_command`, `set_window_size` and `focus_window` — in
    /// the order they were queued. Frame drivers call this after each input
    /// dispatch and each frame and apply the commands to the real window;
    /// headless drivers may simply never call.
    pub fn take_window_commands(&mut self) -> Vec<crate::window::WindowCommand> {
        std::mem::take(&mut self.interaction.window_commands)
    }

    // -- Audio ----------------------------------------------------------
    // Sounds are resources, playback is commands the driver drains; see
    // `audio`. Nothing here touches a device.

    /// Registers a sound from its encoded file bytes (wav/ogg/mp3/flac —
    /// the driver's backend decodes; the core only keeps the bytes).
    pub fn add_sound(&mut self, bytes: Vec<u8>) -> crate::resources::SoundId {
        self.session.state().resources.add_sound(bytes)
    }

    /// Forgets a sound; the driver drops its decoded copy. Playbacks
    /// already running keep going.
    pub fn remove_sound(&mut self, id: crate::resources::SoundId) {
        let sess = &mut *self.session.state();
        if sess.resources.remove_sound(id).is_some() {
            sess.audio.unload(id);
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
        let origin = self.origin;
        self.session
            .state()
            .audio
            .play(origin, Key::ROOT, sound, opts)
    }

    /// Stops a playback, fading over `fade_ms` (0 = at once). A stopped
    /// playback never reports `ended`.
    pub fn stop(&mut self, playback: crate::audio::PlaybackId, fade_ms: f32) {
        self.session.state().audio.stop(playback, fade_ms);
    }

    /// Sets a playback's volume (linear amplitude), tweening over `tween_ms`.
    pub fn set_volume(&mut self, playback: crate::audio::PlaybackId, volume: f32, tween_ms: f32) {
        self.session
            .state()
            .audio
            .set_volume(playback, volume, tween_ms);
    }

    pub fn pause(&mut self, playback: crate::audio::PlaybackId, fade_ms: f32) {
        self.session.state().audio.pause(playback, fade_ms);
    }

    pub fn resume(&mut self, playback: crate::audio::PlaybackId, fade_ms: f32) {
        self.session.state().audio.resume(playback, fade_ms);
    }

    /// Sets the master volume (linear amplitude), tweening over `tween_ms`.
    pub fn set_master_volume(&mut self, volume: f32, tween_ms: f32) {
        self.session.state().audio.master_volume(volume, tween_ms);
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
        let origin = self.origin;
        self.session.state().audio.declare(key, origin, spec);
        key
    }

    /// `audio_node` with a label-derived key (stable across reorders).
    pub fn audio_node_keyed(&mut self, label: &str, spec: crate::audio::AudioSpec) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.child_key(label);
        let origin = self.origin;
        self.session.state().audio.declare(key, origin, spec);
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
        let ended = self.session.state().audio.ended(playback);
        if let Some(ev) = ended {
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

    /// Re-reads the session's font family names into the mirror
    /// `font_family` lends from, when a registration has moved since.
    fn sync_font_names(&mut self) {
        let sess = self.session.state();
        if sess.fonts_rev == self.fonts_rev {
            return;
        }
        self.fonts_rev = sess.fonts_rev;
        self.font_names.clear();
        for (id, entry) in sess.resources.fonts.iter() {
            self.font_names.insert(id, entry.family.as_str().into());
        }
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
            if spec.exit.is_some() && spec.transition.is_some() {
                self.any_exit = true;
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
        spec.style.shadow.color = color(Slot::ShadowColor, spec.style.shadow.color, None);
        let sh = spec.style.shadow;
        let geom = anim.drive(
            key,
            Slot::Shadow,
            None,
            [sh.dx, sh.dy, sh.blur, sh.spread],
            t,
            true,
        );
        spec.style.shadow.dx = geom[0];
        spec.style.shadow.dy = geom[1];
        spec.style.shadow.blur = geom[2].max(0.0);
        spec.style.shadow.spread = geom[3];
        spec.style.opacity = match track(Slot::Opacity) {
            Some(track) => anim.sample(track, t).map_or(spec.style.opacity, |v| v[0]),
            None => anim.drive(
                key,
                Slot::Opacity,
                enter.opacity.map(|o| [o, 0.0, 0.0, 0.0]),
                [spec.style.opacity, 0.0, 0.0, 0.0],
                t,
                true,
            )[0],
        }
        .clamp(0.0, 1.0);
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

    /// The key the `i`th child gets from auto-keying — what `open_indexed`
    /// opens with, usable before the node exists.
    pub fn child_key_index(&self, i: u64) -> Key {
        self.tree.keys[self.current() as usize].index(i)
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
        self.stamp(&mut out);
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

    /// `open_keyed` in the sibling-index namespace: the key auto-keying
    /// would have given the `i`th child. A list that builds only rows
    /// 900..930 opens each with its *data* index, so row 900 keeps the key
    /// it has when the whole list is built — hover, focus, edit buffers and
    /// tweens follow the row instead of the slot it happens to occupy.
    pub fn open_indexed(&mut self, i: u64, spec: NodeSpec) -> Key {
        let key = self.child_key_index(i);
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
            self.any_rounded_clip |= spec.style.radius != crate::display::SQUARE;
        }
        if spec.style.opacity < 1.0 {
            self.any_opacity = true;
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
        if spec.exit.is_some() && spec.transition.is_some() {
            self.any_exit = true;
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
        let tid = {
            let sess = &mut *self.session.state();
            self.text
                .add(content, &style, &sess.resources, &mut sess.fonts)
        };
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
        if spec.exit.is_some() && spec.transition.is_some() {
            self.any_exit = true;
        }
        {
            let origin = self.origin;
            let scale = self.scale;
            let sess = &mut *self.session.state();
            self.edit.declare(
                key,
                initial,
                opts,
                origin,
                scale,
                &mut sess.fonts,
                &sess.resources,
            );
        }
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
        if spec.exit.is_some() && spec.transition.is_some() {
            self.any_exit = true;
        }
        let parent = self.current();
        self.tree
            .push(parent, key, self.origin, spec, NodeContent::Image(id));
    }

    /// Unregisters an image and forgets its atlas slot.
    pub fn remove_image(&mut self, id: crate::resources::ImageId) {
        self.session.state().resources.remove_image(id);
        self.atlas.evict_image(id);
    }

    /// A paragraph of styled spans, shaped and wrapped as one flow.
    pub fn rich_text_node(&mut self, spans: &[Span<'_>], base: TextStyle) {
        if self.tree.is_empty() {
            return;
        }
        let tid = {
            let sess = &mut *self.session.state();
            self.text
                .add_rich(spans, &base, &sess.resources, &mut sess.fonts)
        };
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
