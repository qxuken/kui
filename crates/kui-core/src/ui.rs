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

    /// An editable text node; state retained by key. See `Core::text_edit`.
    pub fn text_edit(&mut self, label: &str, initial: &str, opts: &EditOptions, spec: NodeSpec) -> Key {
        self.core.text_edit(label, initial, opts, spec)
    }

    pub fn is_focused(&self, key: Key) -> bool {
        self.core.is_focused(key)
    }

    pub fn edit_text(&self, key: Key) -> Option<String> {
        self.core.edit_text(key)
    }

    /// Runs layout and emission; results land in `Core::output()`.
    pub fn finish(self) {
        self.core.finish_frame();
    }
}
