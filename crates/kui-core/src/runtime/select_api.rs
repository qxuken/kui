//! `Core`'s selection surface: what a host, a binding or the pointer
//! model does to the window's selection, and what it reads back
//! (`docs/adr/0017-selection-as-a-scope.md`).
//!
//! Every query here answers from the frame that finished while one is
//! being built, the way `text_hit` and `caret_rect` do: a press is made
//! against the layout the user could see, not against the one being
//! assembled in response to it.

use crate::geom::{Rect, Vec2};
use crate::key::Key;
use crate::runtime::Core;
use crate::select::{CellEnd, CellSelection, Endpoint, Selection};

impl Core {
    /// The window's text selection outside an editor, if it has one.
    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }

    /// The window's selection when it lives in a `cells` grid.
    pub fn cell_selection(&self) -> Option<CellSelection> {
        self.cell_selection
    }

    /// Sets it, clearing whatever else the window had selected.
    pub fn set_cell_selection(&mut self, sel: CellSelection) {
        self.cell_selection = Some(sel);
        self.selection = None;
        self.collapse_editor_selection();
    }

    /// Where `point` lands in the grid `key` drew, as an absolute line and
    /// a column — the address a cell selection's end is. `None` when the
    /// node drew no grid.
    pub fn cell_at(&mut self, key: Key, point: Vec2) -> Option<CellEnd> {
        let (row, col) = self.cell_row_col(key, point)?;
        let id = self.cells_id_of(key)?;
        Some(CellEnd::new(self.cells.origin_line(id) + row as u64, col))
    }

    /// The grid a keyed node drew this frame, if it drew one.
    pub(crate) fn cells_id_of(&mut self, key: Key) -> Option<crate::cells::CellsId> {
        self.cells_id_of_ref(key)
    }

    pub(crate) fn cells_id_of_ref(&self, key: Key) -> Option<crate::cells::CellsId> {
        let i = self.tree.keys.iter().position(|k| *k == key)?;
        match self.tree.content[i] {
            crate::tree::NodeContent::Cells(id) => Some(id),
            _ => None,
        }
    }

    /// Where `point` lands in that grid, as a row and column clamped to
    /// it — the same arithmetic a `cell` payload on a click uses, so a
    /// selection and an app's own hit test agree.
    fn cell_row_col(&mut self, key: Key, point: Vec2) -> Option<(usize, usize)> {
        let i = self.tree.keys.iter().position(|k| *k == key)?;
        let crate::tree::NodeContent::Cells(id) = self.tree.content[i] else {
            return None;
        };
        let cell = {
            let sess = &mut *self.session.state();
            self.cells.cell_size(id, &sess.resources, &mut sess.fonts)
        };
        let (rows, cols) = self.cells.dims(id);
        let pos = self.tree.pos[i];
        let col = ((point.x - pos.x) / cell.w.max(f32::EPSILON)).floor();
        let row = ((point.y - pos.y) / cell.h.max(f32::EPSILON)).floor();
        Some((
            (row.max(0.0) as usize).min(rows.saturating_sub(1)),
            (col.max(0.0) as usize).min(cols.saturating_sub(1)),
        ))
    }

    /// Starts a cell selection at `point` in the grid `key`.
    pub fn begin_cell_selection(&mut self, key: Key, point: Vec2, block: bool) -> bool {
        let Some(at) = self.cell_at(key, point) else {
            return false;
        };
        self.set_cell_selection(CellSelection::new(key, at, at).block(block));
        true
    }

    /// Moves the live end of a cell selection to `point`.
    pub fn extend_cell_selection(&mut self, point: Vec2) -> bool {
        let Some(sel) = self.cell_selection else {
            return false;
        };
        let Some(focus) = self.cell_at(sel.node, point) else {
            return false;
        };
        if focus == sel.focus {
            return false;
        }
        self.cell_selection = Some(CellSelection { focus, ..sel });
        true
    }

    /// The selected cells as text: one line per grid row it covers, each
    /// with its trailing blanks trimmed — the rule that makes a copied
    /// screen paste like text instead of like a rectangle of spaces.
    ///
    /// Only what the grid *holds*: a selection whose ends reach into the
    /// scrollback copies the lines on screen, because the lines behind it
    /// were never handed to the core (ADR 0017, decision 3, tier 3).
    pub fn cell_selection_text(&self) -> Option<String> {
        let sel = self.cell_selection?;
        let id = self.cells_id_of_ref(sel.node)?;
        let (rows, cols) = self.cells.dims(id);
        let origin = self.cells.origin_line(id);
        let mut out = String::new();
        let mut first = true;
        for row in 0..rows {
            let line = origin + row as u64;
            let Some((from, to)) = sel.cols_on(line, cols) else {
                continue;
            };
            if !first {
                out.push('\n');
            }
            first = false;
            let mut text = String::new();
            for col in from..to {
                match self.cells.cell_char(id, row, col) {
                    // The cell after a wide glyph is the app's spacer, and
                    // copying it would double the character.
                    Some((_, true)) => text.push(' '),
                    Some((ch, _)) => text.push(ch),
                    None => {}
                }
            }
            out.push_str(text.trim_end());
        }
        (!out.is_empty() || !sel.is_empty()).then_some(out)
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
        self.cell_selection = None;
        self.collapse_editor_selection();
    }

    /// Drops the selection. Returns whether there was one.
    pub fn clear_selection(&mut self) -> bool {
        self.selection.take().is_some() | self.cell_selection.take().is_some()
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

    /// The box the window's selection occupies, logical viewport px —
    /// what a platform panel about that selection is anchored to. The
    /// union of the drawn runs it covers, so a selection that runs off
    /// the screen is anchored by the part the reader can see.
    ///
    /// `None` with no selection, an empty one, or one whose runs the
    /// frame never drew.
    pub fn selection_rect(&self) -> Option<Rect> {
        let sel = self.selection?;
        let prev = self.building;
        let from = self
            .text
            .scope_offset(sel.scope, sel.anchor.node, sel.anchor.byte, prev)?;
        let to = self
            .text
            .scope_offset(sel.scope, sel.focus.node, sel.focus.byte, prev)?;
        self.text.scope_selection_rect(sel.scope, from, to, prev)
    }

    /// Where a platform panel about the selection should point: the
    /// baseline origin of its first line, logical viewport px. See
    /// `TextSystem::scope_selection_anchor`.
    pub fn selection_anchor(&self) -> Option<Vec2> {
        let sel = self.selection?;
        let prev = self.building;
        let from = self
            .text
            .scope_offset(sel.scope, sel.anchor.node, sel.anchor.byte, prev)?;
        let to = self
            .text
            .scope_offset(sel.scope, sel.focus.node, sel.focus.byte, prev)?;
        self.text.scope_selection_anchor(sel.scope, from, to, prev)
    }

    /// The selection as HTML — the same text `selection_text` gives, with
    /// the bold, the italic and the span colours it was declared with
    /// (ADR 0017, decision 7). `None` with no text selection; a cells
    /// selection has no styling to carry and answers `None` too.
    ///
    /// Meant as the *second* clipboard flavour, beside the plain text and
    /// never instead of it: an editor that understands HTML takes the
    /// formatting, and everything else takes the words.
    pub fn selection_html(&self) -> Option<String> {
        let sel = self.selection?;
        let prev = self.building;
        let from = self
            .text
            .scope_offset(sel.scope, sel.anchor.node, sel.anchor.byte, prev)?;
        let to = self
            .text
            .scope_offset(sel.scope, sel.focus.node, sel.focus.byte, prev)?;
        let html = self.text.scope_html(sel.scope, from, to, prev);
        (!html.is_empty()).then_some(html)
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
