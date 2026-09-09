//! `Core`'s selection surface: what a host, a binding or the pointer
//! model does to the window's selection, and what it reads back
//! (`docs/adr/0017-selection-as-a-scope.md`).
//!
//! Every query here answers from the frame that finished while one is
//! being built, the way `text_hit` and `caret_rect` do: a press is made
//! against the layout the user could see, not against the one being
//! assembled in response to it.

use crate::geom::Vec2;
use crate::key::Key;
use crate::runtime::Core;
use crate::select::{Endpoint, Selection};

impl Core {
    /// The window's text selection outside an editor, if it has one.
    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }

    /// Sets it. The scope is a node that declared `selectable`; the two
    /// ends are addresses inside it (a node key and a byte in that node's
    /// own text). Ends the frame cannot resolve paint nothing rather than
    /// something else, so setting a selection against a tree that has
    /// since changed is safe.
    ///
    /// Clears the focused editor's own selection: there is one selection
    /// per window (ADR 0017, decision 1).
    pub fn set_selection(&mut self, sel: Selection) {
        self.selection = Some(sel);
        self.collapse_editor_selection();
    }

    /// Drops the selection. Returns whether there was one.
    pub fn clear_selection(&mut self) -> bool {
        self.selection.take().is_some()
    }

    /// Selects every run in `scope`, first byte to last — what Select All
    /// does inside one. `false` when the scope drew no text.
    pub fn select_all_in(&mut self, scope: Key) -> bool {
        let runs = self.text.scope_runs(scope, self.building);
        let (Some(first), Some(last)) = (runs.first(), runs.last()) else {
            return false;
        };
        let sel = Selection::new(
            scope,
            Endpoint::new(first.place.key, 0),
            Endpoint::new(last.place.key, last.text.content().len()),
        );
        drop(runs);
        self.set_selection(sel);
        true
    }

    /// Where `point` (logical viewport px) lands inside `scope`, as the
    /// address a selection end is made of. `None` when the scope drew
    /// nothing the pointer could land in — an off-screen run is part of
    /// the scope's text but is under no pointer.
    pub fn selection_hit(&self, scope: Key, point: Vec2) -> Option<Endpoint> {
        let (node, byte) = self.text.scope_hit(scope, point, self.building)?;
        Some(Endpoint::new(node, byte))
    }

    /// The selected text, assembled across every run the selection
    /// covers — including runs the frame built but never drew, which is
    /// what makes a selection that ran past the bottom of a scroller copy
    /// what the reader dragged over (ADR 0017, tier 2).
    ///
    /// `None` with no selection; an empty string when the selection is
    /// empty or its ends no longer resolve.
    pub fn selection_text(&self) -> Option<String> {
        let sel = self.selection?;
        let prev = self.building;
        let from = self
            .text
            .scope_offset(sel.scope, sel.anchor.node, sel.anchor.byte, prev)?;
        let to = self
            .text
            .scope_offset(sel.scope, sel.focus.node, sel.focus.byte, prev)?;
        Some(self.text.scope_slice(sel.scope, from, to, prev))
    }

    /// Starts a selection at `point` inside `scope` — the press half of a
    /// drag-select. Both ends land together, so nothing is selected until
    /// the pointer moves.
    pub fn begin_selection(&mut self, scope: Key, point: Vec2) -> bool {
        let Some(at) = self.selection_hit(scope, point) else {
            return false;
        };
        self.set_selection(Selection::new(scope, at, at));
        true
    }

    /// Moves the live end of the selection to `point` — the motion half.
    /// The anchor stays where the press put it, so dragging back past it
    /// selects the other way rather than starting again.
    pub fn extend_selection(&mut self, point: Vec2) -> bool {
        let Some(sel) = self.selection else {
            return false;
        };
        let Some(focus) = self.selection_hit(sel.scope, point) else {
            return false;
        };
        if focus == sel.focus {
            return false;
        }
        self.selection = Some(Selection { focus, ..sel });
        true
    }

    /// Selects the word under `point` inside `scope` — a double click,
    /// and (on macOS) a force click. `false` when the point lands in no
    /// run, or in one with no word under it.
    pub fn select_word_at(&mut self, scope: Key, point: Vec2) -> bool {
        let Some(at) = self.selection_hit(scope, point) else {
            return false;
        };
        let Some((from, to)) = self.text.word_at(scope, at.node, at.byte, self.building) else {
            return false;
        };
        self.set_selection(Selection::new(
            scope,
            Endpoint::new(at.node, from),
            Endpoint::new(at.node, to),
        ));
        true
    }

    /// Selects the whole run under `point` — a triple click, which takes
    /// the line a label is. `false` when the point lands in no run.
    pub fn select_run_at(&mut self, scope: Key, point: Vec2) -> bool {
        let Some(at) = self.selection_hit(scope, point) else {
            return false;
        };
        let len = self
            .text
            .scope_runs(scope, self.building)
            .into_iter()
            .find(|r| r.place.key == at.node)
            .map(|r| r.text.content().len());
        let Some(len) = len else { return false };
        self.set_selection(Selection::new(
            scope,
            Endpoint::new(at.node, 0),
            Endpoint::new(at.node, len),
        ));
        true
    }

    /// Collapses the focused editor's selection, so a window never shows
    /// two. The caret stays where it was: the editor keeps its focus and
    /// its insertion point, and only the highlight goes.
    fn collapse_editor_selection(&mut self) {
        if let Some(key) = self.edit.focused() {
            self.edit.collapse_selection(key);
        }
    }
}
