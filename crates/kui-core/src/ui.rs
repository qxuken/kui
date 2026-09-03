//! The Rust frame builder: a thin safe façade over `Core`'s flat builder
//! methods (which are also the FFI surface). It never captures user state,
//! so `view(&state)` and `update(&mut state)` can't conflict.

use crate::edit::EditOptions;
use crate::env::Env;
use crate::geom::Size;
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

    pub fn is_focused(&self, key: Key) -> bool {
        self.core.is_focused(key)
    }

    pub fn edit_text(&self, key: Key) -> Option<String> {
        self.core.edit_text(key)
    }

    /// Routes full-keyboard input at this node — it must have declared
    /// `on_key` in its spec. Key presses then arrive in `on_event` as
    /// `{kind="key", code, ctrl, alt, shift, super, text, repeat, tag}`.
    /// Declare every frame you care, like the window title.
    pub fn take_key_focus(&mut self, key: Key) {
        self.core.set_key_focus(Some(key));
    }

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
