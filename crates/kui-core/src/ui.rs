//! The Rust frame builder: a thin safe façade over `Core`'s flat builder
//! methods (which are also the FFI surface). It never captures user state,
//! so `view(&state)` and `update(&mut state)` can't conflict.

use crate::edit::EditOptions;
use crate::env::Env;
use crate::geom::{Size, Vec2};
use crate::key::Key;
use crate::runtime::Core;
use crate::spec::{NodeSpec, TextStyle};
use crate::text::Span;
use crate::tree::OriginId;
use crate::window::WindowCommand;

pub struct Ui<'a> {
    core: &'a mut Core,
}

impl<'a> Ui<'a> {
    pub(crate) fn new(core: &'a mut Core) -> Self {
        Self { core }
    }

    /// Escape hatch to the underlying core (e.g. for FFI view callbacks).
    pub fn core(&mut self) -> &mut Core {
        self.core
    }

    /// The inverse escape hatch: wraps a borrowed core mid-frame so foreign
    /// frontends that drive `Core` directly (FFI, Node) can call `widgets::*`.
    pub fn wrap(core: &'a mut Core) -> Self {
        Self { core }
    }

    pub fn viewport(&self) -> Size {
        self.core.viewport
    }

    /// Host facts pushed by the frame driver (refresh rate, focus).
    pub fn env(&self) -> Env {
        self.core.env
    }

    /// Declares this frame's window title (declare every frame you care;
    /// the driver diffs and applies changes).
    pub fn window_title(&mut self, title: &str) {
        self.core.set_window_title(title);
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
    /// of hand-tuned constants.
    pub fn measure_text(
        &mut self,
        content: &str,
        style: &TextStyle,
        max_w: Option<f32>,
    ) -> crate::text::TextMetrics {
        self.core.measure_text(content, style, max_w)
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

    pub fn open(&mut self, spec: NodeSpec) -> Key {
        self.core.open(spec)
    }

    pub fn open_keyed(&mut self, label: &str, spec: NodeSpec) -> Key {
        self.core.open_keyed(label, spec)
    }

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

    pub fn text(&mut self, content: &str, style: TextStyle) {
        self.core.text_node(content, style);
    }

    /// A paragraph of styled spans, shaped and wrapped as one flow.
    pub fn rich_text(&mut self, spans: &[Span<'_>], base: TextStyle) {
        self.core.rich_text_node(spans, base);
    }

    /// A registered image; see `Core::image_node` for sizing semantics.
    pub fn image(&mut self, id: crate::resources::ImageId, spec: NodeSpec) {
        self.core.image_node(id, spec);
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
    pub fn focus_next(&mut self) {
        self.core.focus_next(true);
    }

    /// Shift-Tab: the previous focusable node.
    pub fn focus_prev(&mut self) {
        self.core.focus_next(false);
    }

    pub fn edit_text(&self, key: Key) -> Option<String> {
        self.core.edit_text(key)
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

    /// Runs layout and emission; results land in `Core::output()`.
    pub fn finish(self) {
        self.core.finish_frame();
    }
}
