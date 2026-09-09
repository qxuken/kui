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
use crate::select::{CellEnd, CellSelection, CopyRequest, Endpoint, Grain, RangeEnd, Selection};
use crate::value::Value;

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
        let (fk, lk, llen) = (first.place.key, last.place.key, last.text.content().len());
        drop(runs);
        let sel = Selection::new(
            scope,
            Endpoint::new(fk, 0).in_row(self.row_of(fk)),
            Endpoint::new(lk, llen).in_row(self.row_of(lk)),
        );
        self.set_selection(sel);
        true
    }

    /// Where `point` (logical viewport px) lands inside `scope`, as the
    /// address a selection end is made of. `None` when the scope drew
    /// nothing the pointer could land in — an off-screen run is part of
    /// the scope's text but is under no pointer.
    pub fn selection_hit(&self, scope: Key, point: Vec2) -> Option<Endpoint> {
        let (node, byte) = self.text.scope_hit(scope, point, self.building)?;
        Some(Endpoint::new(node, byte).in_row(self.row_of(node)))
    }

    /// The virtualised row a node sits in, if any — what an endpoint keeps
    /// so it can be placed after its row stops being built.
    pub(crate) fn row_of(&self, node: Key) -> Option<u64> {
        let i = self.tree.keys.iter().position(|k| *k == node)?;
        self.rows.get(i).copied().flatten()
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

    /// The selection's two ends as the app's own addresses — the data
    /// index of the virtualised row each is in, and the byte inside that
    /// row's text. `None` when there is no selection, or when neither end
    /// is in a virtualised row (nothing to ask about: the core has it
    /// all).
    pub fn selection_range(&self) -> Option<(RangeEnd, RangeEnd)> {
        let sel = self.selection?;
        let (a, f) = (sel.anchor, sel.focus);
        (a.row.is_some() || f.row.is_some()).then_some((
            RangeEnd {
                row: a.row,
                byte: a.byte,
            },
            RangeEnd {
                row: f.row,
                byte: f.byte,
            },
        ))
    }

    /// Whether the core can answer a copy on its own: both ends resolve
    /// against runs this frame built.
    fn selection_is_whole(&self) -> bool {
        let Some(sel) = self.selection else {
            return false;
        };
        let prev = self.building;
        self.text
            .scope_offset(sel.scope, sel.anchor.node, sel.anchor.byte, prev)
            .is_some()
            && self
                .text
                .scope_offset(sel.scope, sel.focus.node, sel.focus.byte, prev)
                .is_some()
    }

    /// Asks for the selection as text, and says how the answer will come.
    ///
    /// [`CopyRequest::Ready`] is the ordinary case: everything selected is
    /// text the core shaped, so it hands it over. [`CopyRequest::Asked`]
    /// is a selection that reaches rows a virtual list never built — the
    /// core posts `{kind:"selectionrange", from:{index, byte}, to:{index,
    /// byte}}` on the scope and waits for
    /// [`Self::answer_selection_range`], because the rows behind that gap
    /// are the app's and only the app has them.
    ///
    /// The event goes out with the frame's pending events, so a host that
    /// calls this outside `handle_input` drains `take_pending_events`
    /// after it.
    pub fn request_copy(&mut self) -> CopyRequest {
        if self.selection.is_none() && self.cell_selection.is_none() {
            return match self
                .edit
                .focused()
                .and_then(|k| self.edit.copy_selection(k))
            {
                Some(text) => CopyRequest::Ready(text),
                None => CopyRequest::Nothing,
            };
        }
        if self.cell_selection.is_some() || self.selection_is_whole() {
            return match self.copy_selection() {
                Some(text) => CopyRequest::Ready(text),
                None => CopyRequest::Nothing,
            };
        }
        let Some(sel) = self.selection else {
            return CopyRequest::Nothing;
        };
        let Some((from, to)) = self.selection_range() else {
            // Not virtualised and not resolvable: nothing to ask anyone
            // about, and nothing to hand over.
            return CopyRequest::Nothing;
        };
        let end = |row: Option<u64>, byte: usize| {
            Value::map([
                ("index", row.map_or(Value::Null, |r| Value::Int(r as i64))),
                ("byte", Value::Int(byte as i64)),
            ])
        };
        self.pending.push(crate::input::UiEvent {
            // The scope's own origin: an extension that declared the
            // list is the one that can answer for its rows.
            origin: self
                .tree
                .keys
                .iter()
                .position(|k| *k == sel.scope)
                .map_or(crate::tree::OriginId::HOST, |i| self.tree.origins[i]),
            window: crate::window::WindowId::MAIN,
            key: sel.scope,
            payload: Value::map([
                ("kind", Value::str("selectionrange")),
                ("from", end(from.row, from.byte)),
                ("to", end(to.row, to.byte)),
            ]),
        });
        self.awaiting_selection = true;
        CopyRequest::Asked
    }

    /// The app's answer to a `selectionrange` ask: the text for the range
    /// it was asked about, whole. Queues it for the clipboard the way a
    /// menu's Copy does, and is ignored when nothing asked — a stale
    /// answer cannot overwrite what somebody copied since.
    pub fn answer_selection_range(&mut self, text: &str) -> bool {
        if !std::mem::take(&mut self.awaiting_selection) {
            return false;
        }
        self.menu_actions
            .push(crate::menu::MenuAction::SetClipboard {
                text: text.to_string(),
                html: None,
            });
        true
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

    /// The motion half of a drag that is moving by *words* or by whole
    /// runs: the live end rounds outwards to its own word (or run), and so
    /// does the anchor, so the word the press took stays whole however far
    /// back over itself the drag turns.
    ///
    /// This is what a double-click-and-drag does in every text UI, and
    /// what the stock `<edit>` gets for free from cosmic-text's
    /// `Selection::Word`; a `selectable` scope is the one that had to be
    /// taught (ADR 0017).
    pub(crate) fn extend_selection_grained(
        &mut self,
        drag: crate::select::SelectDrag,
        point: Vec2,
    ) -> bool {
        let grain = drag.grain;
        if grain == Grain::Char {
            return self.extend_selection(point);
        }
        let Some(sel) = self.selection else {
            return false;
        };
        let Some(hit) = self.selection_hit(sel.scope, point) else {
            return false;
        };
        let Some((anode, a_from, a_to)) = drag.anchor else {
            return self.extend_selection(point);
        };
        // The unit under the live end, in that node's own bytes.
        let (f_from, f_to) = match grain {
            Grain::Word => match self
                .text
                .word_at(sel.scope, hit.node, hit.byte, self.building)
            {
                Some(span) => span,
                None => return false,
            },
            _ => match self
                .text
                .scope_runs(sel.scope, self.building)
                .into_iter()
                .find(|r| r.place.key == hit.node)
                .map(|r| r.text.content().len())
            {
                Some(len) => (0, len),
                None => return false,
            },
        };
        // Which side of the anchor the live end is on decides which edge
        // of each unit the selection runs between.
        let prev = self.building;
        let ga = self.text.scope_offset(sel.scope, anode, a_from, prev);
        let gf = self.text.scope_offset(sel.scope, hit.node, f_from, prev);
        let (Some(ga), Some(gf)) = (ga, gf) else {
            return false;
        };
        let (arow, frow) = (self.row_of(anode), self.row_of(hit.node));
        let next = if gf < ga {
            // Backwards: from the far edge of the anchor's unit to the
            // near edge of the live one.
            Selection::new(
                sel.scope,
                Endpoint::new(anode, a_to).in_row(arow),
                Endpoint::new(hit.node, f_from).in_row(frow),
            )
        } else {
            Selection::new(
                sel.scope,
                Endpoint::new(anode, a_from).in_row(arow),
                Endpoint::new(hit.node, f_to).in_row(frow),
            )
        };
        if Some(next) == self.selection {
            return false;
        }
        self.selection = Some(next);
        true
    }

    /// Selects the word under `point` inside `scope` — a double click,
    /// and (on macOS) a force click. Answers the span it took, in the
    /// node's own bytes, so a drag that follows can round to it.
    pub fn select_word_at(&mut self, scope: Key, point: Vec2) -> Option<(Key, usize, usize)> {
        let at = self.selection_hit(scope, point)?;
        let (from, to) = self.text.word_at(scope, at.node, at.byte, self.building)?;
        let row = self.row_of(at.node);
        self.set_selection(Selection::new(
            scope,
            Endpoint::new(at.node, from).in_row(row),
            Endpoint::new(at.node, to).in_row(row),
        ));
        Some((at.node, from, to))
    }

    /// Selects the whole run under `point` — a triple click, which takes
    /// the line a label is. Answers the span, like `select_word_at`.
    pub fn select_run_at(&mut self, scope: Key, point: Vec2) -> Option<(Key, usize, usize)> {
        let at = self.selection_hit(scope, point)?;
        let len = self
            .text
            .scope_runs(scope, self.building)
            .into_iter()
            .find(|r| r.place.key == at.node)
            .map(|r| r.text.content().len())?;
        let row = self.row_of(at.node);
        self.set_selection(Selection::new(
            scope,
            Endpoint::new(at.node, 0).in_row(row),
            Endpoint::new(at.node, len).in_row(row),
        ));
        Some((at.node, 0, len))
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
