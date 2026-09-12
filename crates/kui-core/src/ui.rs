//! The Rust frame builder: a thin safe façade over `Core`'s flat builder
//! methods (which are also the FFI surface). It never captures user state,
//! so `view(&state)` and `update(&mut state)` can't conflict.

use crate::edit::EditOptions;
use crate::env::Env;
use crate::geom::{Rect, Size, Vec2};
use crate::key::Key;
use crate::line::Stroke;
use crate::runtime::Core;
use crate::slot::{Fill, Slot};
use crate::spec::{NodeSpec, TextStyle};
use crate::text::Span;
use crate::tree::OriginId;
use crate::value::Value;
use crate::window::{WindowCommand, WindowConfig, WindowId};

pub struct Ui<'a> {
    core: &'a mut Core,
    /// What fills the slots this frame declares (`Core::frame_with`);
    /// `None` for a frame begun without one, and inside a fill — an
    /// extension's `slot` declares and fills nothing.
    filler: Option<&'a mut dyn Fill>,
}

impl<'a> Ui<'a> {
    pub(crate) fn new(core: &'a mut Core) -> Self {
        Self { core, filler: None }
    }

    /// [`Self::wrap`] with something to fill the slots the frame declares
    /// — for a frontend that drives `Core` directly and keeps an
    /// extension list of its own (the C context, a headless Node one), so
    /// its `slot` and `finish` are this type's rather than a copy of them.
    pub fn with_filler(core: &'a mut Core, filler: &'a mut dyn Fill) -> Self {
        Self {
            core,
            filler: Some(filler),
        }
    }

    /// Escape hatch to the underlying core (e.g. for FFI view callbacks).
    pub fn core(&mut self) -> &mut Core {
        self.core
    }

    /// The inverse escape hatch: wraps a borrowed core mid-frame so foreign
    /// frontends that drive `Core` directly (FFI, Node) can call `widgets::*`.
    pub fn wrap(core: &'a mut Core) -> Self {
        Self { core, filler: None }
    }

    /// Declares a slot here, with no parameters: whatever fills it draws
    /// now, as children of the node this view is inside, at this
    /// position among its siblings. `name` is the full name,
    /// `namespace/slot` — the namespace the host gave the extension when
    /// it loaded it, and the slot in the extension's own vocabulary
    /// (`"fs/panel"`). `"ns/root"` is the fill that follows the host's
    /// view for an extension listing no slots, and declaring it moves
    /// that fill here. See
    /// `docs/adr/0014-slots-an-extension-fills-in-place.md`.
    pub fn slot(&mut self, name: &str) {
        self.slot_with(name, &crate::slot::NULL_PARAMS);
    }

    /// `slot` with parameters the extension reads this frame
    /// (`Slot::params`); a `Value` because it is the type that already
    /// crosses to an extension. Nothing is retained — pass what is true
    /// this frame, every frame.
    ///
    /// Whether the slot was declared: false for a name this frame already
    /// declared (which warns, `duplicate-slot`) or outside a frame. True
    /// whether or not anything filled it — a host with nothing loaded
    /// still gets a placed, empty node to lay out around.
    pub fn slot_with(&mut self, name: &str, params: &Value) -> bool {
        let Some(key) = self.core.begin_slot(name) else {
            return false;
        };
        if let Some(filler) = self.filler.as_deref_mut() {
            filler.fill(name, key, params, &mut Ui::new(self.core));
        }
        true
    }

    /// Whether the full name `name` was declared this frame so far.
    pub fn slot_declared(&self, name: &str) -> bool {
        self.core.slot_declared(name)
    }

    /// Loads `ext` under `namespace` into the list filling this frame's
    /// slots, and answers with the origin it got. This is how an
    /// extension hosts an extension of its own: the guest asks mid-frame,
    /// when it knows what it wants, and the plugin lands in the same list
    /// as the host's own — one namespace map, one origin per extension,
    /// however deep the loading went (`crate::slot`).
    ///
    /// Fails when the namespace is taken, or empty with an extension that
    /// names itself nothing, exactly as
    /// `Extensions::push_as` does, and when this frame was begun without
    /// a filler (`Core::frame`) or with one that is not a list.
    pub fn add_extension(
        &mut self,
        namespace: &str,
        ext: Box<dyn crate::runtime::Extension>,
    ) -> Result<OriginId, String> {
        match self.filler.as_deref_mut() {
            Some(filler) => filler.add(namespace, ext),
            None => Err(format!(
                "cannot load `{namespace}`: this frame declares no slots to fill"
            )),
        }
    }

    /// Runs `f` as the fill of `slot` under `origin`: nodes it opens are
    /// tagged with the origin, keyed under the slot's key, and closed
    /// for it if it leaves any open. What a `Fill` implementation calls
    /// per extension; see `Core::fill`.
    pub fn fill(&mut self, origin: OriginId, slot: &Slot<'_>, f: impl FnOnce(&mut Ui<'_>)) {
        self.core.fill(slot, origin, f);
    }

    /// `fill`, with `filler` answering the slots the fill declares — an
    /// extension hosting extensions of its own. `Extensions::fill_one`
    /// passes itself, which is what makes one namespace map do for every
    /// level; see `crate::slot`.
    pub fn fill_within(
        &mut self,
        origin: OriginId,
        slot: &Slot<'_>,
        filler: &mut dyn Fill,
        f: impl FnOnce(&mut Ui<'_>),
    ) {
        self.core.fill_within(slot, origin, Some(filler), f);
    }

    /// The origin the nodes opened right now are tagged with:
    /// `OriginId::HOST` in the host's own view, the filling extension's
    /// inside a fill. What records who declared a slot, and so where the
    /// replies of whatever fills it go.
    pub fn origin(&self) -> OriginId {
        self.core.origin()
    }

    /// The viewport this frame lays out into: the window, less the
    /// devtools' dock while the panel is docked (`Core::viewport`).
    pub fn viewport(&self) -> Size {
        self.core.viewport()
    }

    /// Host facts pushed by the frame driver (refresh rate, focus).
    /// The env reading's inputs, the frame's facts included
    /// (`Core::env_facts`): what a binding's `env` table is filled from.
    pub fn env_facts(&self) -> crate::schema::EnvFacts {
        self.core.env_facts()
    }

    pub fn env(&self) -> Env {
        self.core.env
    }

    /// This frame's palette: the named colours the stock widgets paint
    /// with, derived from `env.system` unless the app pinned something
    /// else (`docs/adr/0019-a-theme-derived-from-appearance-and-accent.md`).
    ///
    /// By value, because it is [`Copy`] and a view that took a reference
    /// could not then touch `ui` — which is the whole of what a view
    /// does. `let t = ui.theme();` at the top of a widget is the idiom.
    pub fn theme(&self) -> crate::theme::Theme {
        *self.core.theme()
    }

    /// Whether anyone chose the theme's accent, or it is kui's fallback
    /// blue — see [`Core::has_accent`](crate::runtime::Core::has_accent).
    pub fn has_accent(&self) -> bool {
        self.core.has_accent()
    }

    /// The sizes the stock widgets are built from
    /// (`crate::metrics::Metrics`, backlog T2), by value like the theme
    /// and for the same reason. What a view reads to make its own
    /// controls agree with the stock ones on a radius and a padding.
    pub fn metrics(&self) -> crate::metrics::Metrics {
        *self.core.metrics()
    }

    /// Declares this frame's window title (declare every frame you care;
    /// the driver diffs and applies changes).
    pub fn window_title(&mut self, title: &str) {
        self.core.set_window_title(title);
    }

    /// Declares that a window named `name` exists this frame; see
    /// `Core::declare_window`. It opens on the first frame that declares
    /// it (`config` is read then and never again), stays open while any
    /// window's frame keeps declaring it, and closes when none does.
    /// `view` is then called for it too, with [`Ui::window_name`] saying
    /// which window is being drawn.
    pub fn window(&mut self, name: &str, config: WindowConfig) {
        self.core.declare_window(name, config);
    }

    /// The name of the window this frame is drawing: `"main"` for the one
    /// the launcher opened, else the name the declaration that opened it
    /// used. `env().window.id` is the same window as a number.
    pub fn window_name(&self) -> std::rc::Rc<str> {
        self.core.window_name()
    }

    pub fn set_origin(&mut self, origin: OriginId) {
        self.core.set_origin(origin);
    }

    pub fn configure_root(&mut self, spec: NodeSpec) {
        self.core.configure_root(spec);
    }

    pub fn child_key(&self, label: &str) -> Key {
        self.core.child_key(label)
    }

    /// The key the `i`th child gets from auto-keying; see `open_indexed`.
    pub fn child_key_index(&self, i: u64) -> Key {
        self.core.child_key_index(i)
    }

    pub fn is_hovered(&self, key: Key) -> bool {
        self.core.is_hovered(key)
    }

    pub fn is_pressed(&self, key: Key) -> bool {
        self.core.is_pressed(key)
    }

    /// Whether any member of a hover group (`NodeSpec::hover_group`) is
    /// hovered; the id comes from `NodeSpec::hover_group_id`.
    pub fn is_group_hovered(&self, group: u64) -> bool {
        self.core.is_group_hovered(group)
    }

    pub fn is_group_pressed(&self, group: u64) -> bool {
        self.core.is_group_pressed(group)
    }

    /// Physical modifier state (the host also receives it as a
    /// `{kind="modifiers"}` event whenever it changes).
    pub fn modifiers(&self) -> crate::input::KeyMods {
        self.core.modifiers()
    }

    /// Asks for one more frame after this one; see `Core::request_frame`.
    pub fn request_frame(&mut self) {
        self.core.request_frame();
    }

    /// Measures text the way layout would, without adding a node; see
    /// `Core::measure_text`. Sizing a column to its widest label, or
    /// choosing a tier that fits, is arithmetic on these numbers instead
    /// of hand-tuned constants. The metrics do not scale linearly:
    /// `measured × zoom` is not `measure(size × zoom)`, because shaping
    /// rounds per size, so anything that zooms measures at the size it
    /// draws.
    pub fn measure_text(
        &mut self,
        content: &str,
        style: &TextStyle,
        max_w: Option<f32>,
    ) -> crate::text::TextMetrics {
        self.core.measure_text(content, style, max_w)
    }

    /// Where a point lands in the text node `key` drew, as a byte offset
    /// and a visual line; see `Core::text_hit`. During a build it answers
    /// from the last frame, which is the layout a click was made against.
    pub fn text_hit(&self, key: Key, point: Vec2) -> Option<crate::text::TextHit> {
        self.core.text_hit(key, point)
    }

    /// The window's selected text — a `selectable` scope's, or the
    /// focused editor's; see `Core::copy_selection`.
    pub fn selection_text(&self) -> Option<String> {
        self.core.copy_selection()
    }

    /// The window's selection when it lives in a `cells` grid — its ends
    /// as absolute lines and columns; see `Core::cell_selection`.
    ///
    /// Offered where the text selection offers only [`Self::selection_text`]
    /// because a grid's ends mean something to the app: they are the
    /// session's own line numbers, not byte offsets into runs the app never
    /// laid out (ADR 0017, decision 4).
    pub fn cell_selection(&self) -> Option<crate::select::CellSelection> {
        self.core.cell_selection()
    }

    /// Opens a context menu; see `Core::open_menu`.
    pub fn open_menu(&mut self, menu: crate::menu::Menu) {
        self.core.open_menu(menu);
    }

    /// Closes whatever menu is open; see `Core::close_menu`.
    pub fn close_menu(&mut self) -> bool {
        self.core.close_menu()
    }

    /// Asks for the selection as text; see `Core::request_copy`.
    pub fn request_copy(&mut self) -> crate::select::CopyRequest {
        self.core.request_copy()
    }

    /// Answers a `selectionrange` ask; see `Core::answer_selection_range`.
    pub fn answer_selection_range(&mut self, text: &str) -> bool {
        self.core.answer_selection_range(text)
    }

    /// The selection as HTML; see `Core::selection_html`.
    pub fn selection_html(&self) -> Option<String> {
        self.core.selection_html()
    }

    /// Selects everything in the scope `key` declared; see
    /// `Core::select_all_in`.
    pub fn select_all_in(&mut self, key: Key) -> bool {
        self.core.select_all_in(key)
    }

    /// Drops the window's selection; see `Core::clear_selection`.
    pub fn clear_selection(&mut self) -> bool {
        self.core.clear_selection()
    }

    /// The caret rect for a byte offset in the text node `key` drew; see
    /// `Core::caret_rect`.
    pub fn caret_rect(&self, key: Key, byte: usize) -> Option<Rect> {
        self.core.caret_rect(key, byte)
    }

    /// `measure_text` for a rich-text paragraph.
    pub fn measure_rich_text(
        &mut self,
        spans: &[Span<'_>],
        base: &TextStyle,
        max_w: Option<f32>,
    ) -> crate::text::TextMetrics {
        self.core.measure_rich_text(spans, base, max_w)
    }

    #[inline]
    pub fn open(&mut self, spec: NodeSpec) -> Key {
        self.core.open(spec)
    }

    #[inline]
    pub fn open_keyed(&mut self, label: &str, spec: NodeSpec) -> Key {
        self.core.open_keyed(label, spec)
    }

    /// `open_keyed` by sibling index: the key auto-keying would have given
    /// the `i`th child. A virtualizing list opens each row with its data
    /// index, so a row keeps its identity when the built range slides past
    /// it. See `Core::open_indexed`.
    #[inline]
    pub fn open_indexed(&mut self, i: u64, spec: NodeSpec) -> Key {
        self.core.open_indexed(i, spec)
    }

    #[inline]
    pub fn close(&mut self) {
        self.core.close();
    }

    /// Scoped open/close.
    pub fn with(&mut self, spec: NodeSpec, f: impl FnOnce(&mut Ui<'_>)) -> Key {
        let key = self.open(spec);
        f(self);
        self.close();
        key
    }

    pub fn with_keyed(&mut self, label: &str, spec: NodeSpec, f: impl FnOnce(&mut Ui<'_>)) -> Key {
        let key = self.open_keyed(label, spec);
        f(self);
        self.close();
        key
    }

    /// Scoped `open_indexed`: the `i`th child's auto-key, given to a node
    /// that is not in the `i`th slot.
    pub fn with_indexed(&mut self, i: u64, spec: NodeSpec, f: impl FnOnce(&mut Ui<'_>)) -> Key {
        let key = self.open_indexed(i, spec);
        f(self);
        self.close();
        key
    }

    pub fn text(&mut self, content: &str, style: TextStyle) {
        self.core.text_node(content, style);
    }

    /// A paragraph of styled spans, shaped and wrapped as one flow.
    /// A cell grid — a terminal's screen — as one node; see
    /// `crate::cells` (backlog C20). The spec is the node's own.
    pub fn cells(&mut self, grid: &crate::cells::CellGrid<'_>, spec: NodeSpec) {
        self.core.cells(grid, spec);
    }

    /// [`Self::cells`] under a declared key.
    /// [`Self::cells`] under a data index; see [`Self::open_indexed`].
    pub fn cells_indexed(&mut self, i: u64, grid: &crate::cells::CellGrid<'_>, spec: NodeSpec) {
        self.core.cells_indexed(i, grid, spec);
    }

    pub fn cells_keyed(&mut self, label: &str, grid: &crate::cells::CellGrid<'_>, spec: NodeSpec) {
        self.core.cells_keyed(label, grid, spec);
    }

    pub fn rich_text(&mut self, spans: &[Span<'_>], base: TextStyle) {
        self.core.rich_text_node(spans, base);
    }

    /// A registered image; see `Core::image_node` for sizing semantics.
    pub fn image(&mut self, id: crate::resources::ImageId, spec: NodeSpec) {
        self.core.image_node(id, spec);
    }

    /// [`Self::image`] with its `sampling` and `fit` rows; see
    /// `Core::image_node_with`.
    pub fn image_with(
        &mut self,
        id: crate::resources::ImageId,
        opts: crate::resources::ImageOpts,
        spec: NodeSpec,
    ) {
        self.core.image_node_with(id, opts, spec);
    }

    /// A box a registered WGSL function paints; see `Core::fragment_node`
    /// for what it is, and `Core::add_fragment` for where the handle comes
    /// from. It has no intrinsic size, so give it one.
    ///
    /// `frag` is the handle, or `handle.with_image(img)` for a function
    /// that reads a registered image through `kui_sample(uv)` — a
    /// waveform, a heatmap, an image effect (ADR 0025, decision 7).
    pub fn fragment(
        &mut self,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        self.core.fragment_node(frag, params, spec)
    }

    /// A fragment holding children, which paint over it: a gradient card
    /// with a title and buttons on top of it.
    pub fn fragment_with(
        &mut self,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
        f: impl FnOnce(&mut Ui<'_>),
    ) -> Key {
        let key = self.core.open_fragment(frag, params, spec);
        f(self);
        self.core.close();
        key
    }

    /// [`Self::fragment`] under a label key, for one that transitions or
    /// exits and needs a stable identity across frames.
    pub fn fragment_keyed(
        &mut self,
        label: &str,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        self.core.fragment_node_keyed(label, frag, params, spec)
    }

    /// [`Self::fragment_with`] under a label key.
    pub fn fragment_with_keyed(
        &mut self,
        label: &str,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
        f: impl FnOnce(&mut Ui<'_>),
    ) -> Key {
        let key = self.core.open_fragment_keyed(label, frag, params, spec);
        f(self);
        self.core.close();
        key
    }

    /// A round-capped segment from `from` to `to`, in the parent's box
    /// space; see `Core::line_node` for what it is and is not.
    pub fn line(&mut self, from: Vec2, to: Vec2, stroke: Stroke, spec: NodeSpec) {
        self.core.line_node(&[from, to], stroke, spec);
    }

    /// [`Self::line`] under a label key.
    pub fn line_keyed(
        &mut self,
        label: &str,
        from: Vec2,
        to: Vec2,
        stroke: Stroke,
        spec: NodeSpec,
    ) {
        self.core.line_node_keyed(label, &[from, to], stroke, spec);
    }

    /// A stroke through `points`: a polyline, or a smooth curve through
    /// them with [`Stroke::curve`]; see `Core::line_node`.
    pub fn polyline(&mut self, points: &[Vec2], stroke: Stroke, spec: NodeSpec) {
        self.core.line_node(points, stroke, spec);
    }

    /// [`Self::polyline`] under a label key.
    /// [`Self::polyline`] under a data index; see [`Self::open_indexed`].
    pub fn polyline_indexed(&mut self, i: u64, points: &[Vec2], stroke: Stroke, spec: NodeSpec) {
        self.core.line_node_indexed(i, points, stroke, spec);
    }

    pub fn polyline_keyed(&mut self, label: &str, points: &[Vec2], stroke: Stroke, spec: NodeSpec) {
        self.core.line_node_keyed(label, points, stroke, spec);
    }

    /// A filled polygon through `points` in the parent's box space, the
    /// fill in `spec`'s `bg`; see `Core::polygon_node` for what it is and
    /// is not (ADR 0025, decision 6).
    pub fn polygon(&mut self, points: &[Vec2], spec: NodeSpec) {
        self.core.polygon_node(points, spec);
    }

    /// [`Self::polygon`] under a label key.
    pub fn polygon_keyed(&mut self, label: &str, points: &[Vec2], spec: NodeSpec) {
        self.core.polygon_node_keyed(label, points, spec);
    }

    /// [`Self::polygon`] under a data index; see [`Self::open_indexed`].
    pub fn polygon_indexed(&mut self, i: u64, points: &[Vec2], spec: NodeSpec) {
        self.core.polygon_node_indexed(i, points, spec);
    }

    /// An `audio` node: a playback retained for as long as the view keeps
    /// declaring it; see `Core::audio_node`.
    pub fn audio(&mut self, spec: crate::audio::AudioSpec) -> Key {
        self.core.audio_node(spec)
    }

    /// `audio` with a label-derived key; see `Core::audio_node_keyed`.
    pub fn audio_keyed(&mut self, label: &str, spec: crate::audio::AudioSpec) -> Key {
        self.core.audio_node_keyed(label, spec)
    }

    /// Starts a playback from a view; see `Core::play`. Views run every
    /// frame, so gate it on state that changes once (or use `audio`).
    pub fn play(
        &mut self,
        sound: crate::resources::SoundId,
        opts: crate::audio::PlayOptions,
    ) -> crate::audio::PlaybackId {
        self.core.play(sound, opts)
    }

    /// An editable text node; state retained by key. See `Core::text_edit`.
    pub fn text_edit(
        &mut self,
        label: &str,
        initial: &str,
        opts: &EditOptions,
        spec: NodeSpec,
    ) -> Key {
        self.core.text_edit(label, initial, opts, spec)
    }

    /// Whether `key` holds keyboard focus — any node: an editor, a key
    /// sink, a button Tab landed on (see `Core::focus`).
    pub fn is_focused(&self, key: Key) -> bool {
        self.core.is_focused(key)
    }

    /// The node holding keyboard focus, if any.
    pub fn focused(&self) -> Option<Key> {
        self.core.focus()
    }

    /// Whether focus got where it is by keyboard or assistive technology
    /// (a Tab press, a reader's request) rather than a click — when a
    /// view that styles its own focus should show it.
    pub fn focus_visible(&self) -> bool {
        self.core.focus_visible()
    }

    /// The key of the node opened under `label` — in this frame so far,
    /// then in the last finished one. For a caller that holds only the
    /// label and cannot spell the path (`child_key` is the same question
    /// asked from the parent); see `Core::key_of`.
    pub fn key_of(&mut self, label: &str) -> Option<Key> {
        self.core.key_of(label)
    }

    /// Moves keyboard focus to `key` now (an editor, an `on_key` sink, a
    /// control, a `focusable` node); see `Core::set_focus`.
    pub fn focus(&mut self, key: Key) {
        self.core.set_focus(Some(key));
    }

    /// Drops keyboard focus.
    pub fn blur(&mut self) {
        self.core.set_focus(None);
    }

    /// Moves focus to the next focusable node in tree order, wrapping —
    /// what Tab does. A key sink that binds Tab itself calls this to hand
    /// the keyboard on.
    ///
    /// Deferred, unlike the rest of this handle: a `Ui` only exists while a
    /// frame is being built, and `begin_frame` cleared the tree the Tab
    /// ring is made of, so stepping now would walk an empty ring. The step
    /// is applied at `finish`, against the frame this call is part of — so
    /// a view that declares three rows and asks to step lands on one of
    /// them, without waiting a frame for them to exist. Outside a frame
    /// (a driver handling a key press) `Core::focus_next` steps at once.
    pub fn focus_next(&mut self) {
        self.core.request_focus_step(true);
    }

    /// Shift-Tab: the previous focusable node. Deferred to `finish` for the
    /// reason [`Ui::focus_next`] gives.
    pub fn focus_prev(&mut self) {
        self.core.request_focus_step(false);
    }

    /// Enters a focus region — the node `key` names, declared
    /// `focus_region` — or the main ring for `None`
    /// (`docs/adr/0022-focus-regions.md`): focus lands on what that ring
    /// last held if the node is still there, else its `initial_focus`,
    /// else its first stop, and shows. Deferred to `finish` like
    /// [`Ui::focus_next`], so a view may name the region it is declaring
    /// right now — the dock this frame toggles on. A key the frame does
    /// not declare as a region raises `focus-region-without-node`.
    pub fn focus_region(&mut self, key: Option<Key>) {
        self.core.focus_region(key);
    }

    /// The focus region in effect — the node whose ring Tab walks — or
    /// `None` for the main ring. What a chord that toggles between a dock
    /// and the app reads to know which way it is going.
    pub fn region(&self) -> Option<Key> {
        self.core.region()
    }

    pub fn edit_text(&self, key: Key) -> Option<String> {
        self.core.edit_text(key)
    }

    /// Replaces an editor's text, caret at the end (`Core::set_edit_text`).
    pub fn set_edit_text(&mut self, key: Key, text: &str) {
        self.core.set_edit_text(key, text);
    }

    /// The same by the label the view declares, for a caller with no key
    /// yet — an `update` opening a field the editor has not fired an
    /// event from (`Core::set_edit_text_by_label`, backlog F32).
    pub fn set_edit_text_by_label(&mut self, label: &str, text: &str) {
        self.core.set_edit_text_by_label(label, text);
    }

    /// Says something once, with no node behind it (`Core::announce`).
    /// Takes effect at once, unlike `focus_next`: the queue is not made of
    /// a finished tree.
    ///
    /// A view runs every frame, so a call made from here needs a guard the
    /// app clears — the core reports the unguarded case as
    /// `announcement-repeated`. A region whose message is on screen is the
    /// `live` prop instead
    /// (`docs/adr/0008-live-regions-and-announcements.md`).
    pub fn announce(&mut self, text: &str, live: crate::access::Live) {
        self.core.announce(text, live);
    }

    /// Scrolls whatever contains `key` so the node shows — "scroll to the
    /// selected row", without the container geometry the app cannot see.
    /// Resolved when this frame finishes laying out, so a row the view is
    /// declaring right now reveals fine; a key the frame does not declare,
    /// or one nothing scrollable contains, is a no-op. See `Core::reveal`.
    pub fn reveal(&mut self, key: Key) {
        self.core.reveal(key);
    }

    /// A scroll container's retained offset, clamped as of the last
    /// layout — the number to stash in a model and hand back to
    /// `set_scroll` later. Zero for a node that never scrolled.
    pub fn scroll_offset(&self, key: Key) -> Vec2 {
        self.core.scroll_offset(key)
    }

    /// Sets that offset, the way the wheel would: `Vec2::ZERO` jumps to
    /// the top, a large value to the end (the next layout clamps it).
    pub fn set_scroll(&mut self, key: Key, offset: Vec2) {
        self.core.set_scroll(key, offset);
    }

    /// What the last layout resolved for a scroll container — its box, its
    /// content size and the clamped offset — so a view can build only the
    /// rows that fit and two spacers instead of ten thousand rows. `None`
    /// until a layout has resolved `key` as a container. It describes the
    /// previous frame; see `Core::scroll_geometry`, or
    /// `widgets::virtual_column` for the uniform-row case.
    pub fn scroll_geometry(&self, key: Key) -> Option<crate::scroll::ScrollGeometry> {
        self.core.scroll_geometry(key)
    }

    /// The rect the last frame laid an `on_layout` node out at; see
    /// `Core::layout_of` (backlog C26 step 2).
    pub fn layout_of(&self, key: Key) -> Option<Rect> {
        self.core.layout_of(key)
    }

    /// Declares this node focused: it takes keyboard focus when the
    /// declaration *starts* (the first frame it is made), and a Tab press
    /// afterwards is not clobbered by the view repeating it. An `on_key`
    /// sink then gets presses in `on_event` as `{kind="key", code, ctrl,
    /// alt, shift, super, text, repeat, tag}`. To move focus at any time,
    /// `focus(key)`.
    pub fn take_key_focus(&mut self, key: Key) {
        self.core.set_key_focus(Some(key));
    }

    /// The node holding keyboard focus (the same as `focused`).
    pub fn key_focus(&self) -> Option<Key> {
        self.core.key_focus()
    }

    /// Asks the frame driver to apply a window command (close from a
    /// keymap, minimize from a command line).
    pub fn window_command(&mut self, cmd: WindowCommand) {
        self.core.push_window_command(cmd);
    }

    /// Asks the driver to resize a window (logical px) — a request applied
    /// on the driver's next pump and ignored headlessly, not a declaration:
    /// the user owns a window's size once it exists. `ui.env().window.id`
    /// is the window this view is drawing. See `Core::set_window_size`.
    pub fn set_window_size(&mut self, window: WindowId, size: Size) {
        self.core.set_window_size(window, size);
    }

    /// Asks the driver to give a window keyboard focus; queued the same
    /// way. See `Core::focus_window`.
    pub fn focus_window(&mut self, window: WindowId) {
        self.core.focus_window(window);
    }

    /// Runs layout and emission; results land in `Core::output()`. A frame
    /// begun with a filler lets it finish first: the `"root"` fill, unless
    /// the view declared it, and the `unknown-slot` check. An open context
    /// menu is drawn after both, which is what makes it the frame's modal
    /// scope and its topmost float (ADR 0017, decision 5).
    pub fn finish(self) {
        let Ui { core, filler } = self;
        // Not in the devtools' own window: nothing the host or an
        // extension declares there is built (ADR 0024, decision 6).
        if let Some(filler) = filler
            && !core.devtools_window()
        {
            filler.finish(&mut Ui::new(core));
        }
        // The panel, after the fills and before the menu: the menu is
        // the last thing declared and so the topmost float.
        core.devtools_finish();
        Core::build_menu(&mut Ui::new(core));
        core.finish_frame();
    }
}
