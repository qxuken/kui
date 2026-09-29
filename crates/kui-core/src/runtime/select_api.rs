//! `Core`'s selection surface: what a host, a binding or the pointer
//! model does to the window's selection, and what it reads back
//! (`docs/adr/0017-selection-as-a-scope.md`).
//!
//! Every query here answers from the frame that finished while one is
//! being built, the way `text_hit` and `caret_rect` do: a press is made
//! against the layout the user could see, not against the one being
//! assembled in response to it.

use crate::geom::{Rect, Vec2};
use crate::input::{EditKey, Mods};
use crate::key::Key;
use crate::runtime::Core;
use crate::select::{
    CellEnd, CellSelection, CopyRequest, DragAnchor, Endpoint, Grain, RangeEnd, SelectDrag,
    Selection, grained_edges, unbuilt_row_is_after,
};
use crate::tree::NodeContent;
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
        Some(CellEnd::new(
            self.cells.origin_line(id, self.building) + row as u64,
            col,
        ))
    }

    /// The grid a keyed node drew this frame, if it drew one.
    pub(crate) fn cells_id_of(&mut self, key: Key) -> Option<crate::cells::CellsId> {
        self.cells_id_of_ref(key)
    }

    pub(crate) fn cells_id_of_ref(&self, key: Key) -> Option<crate::cells::CellsId> {
        // Off the cell store rather than off the tree: a host reading the
        // selection from inside its own `view` is asking while this
        // frame's tree is half-built, and the grid it means is last
        // frame's — the same rule the text places follow.
        self.cells.find(key, self.building)
    }

    /// The top-left of the cells themselves, which is the node's box
    /// moved in by its padding — the same corner the grid is painted
    /// from, so a hit test and the glyphs agree about where row 0 is.
    pub(crate) fn cells_origin(&self, i: usize) -> Vec2 {
        let pad = self.tree.specs[i].layout.padding;
        let pos = self.tree.pos[i];
        Vec2::new(pos.x + pad.l, pos.y + pad.t)
    }

    /// Where `point` lands in that grid, as a row and column clamped to
    /// it — the one arithmetic, so the `cell` a click's payload carries
    /// (`attach_pointer`), a selection and an app's own hit test agree.
    pub(crate) fn cell_row_col(&mut self, key: Key, point: Vec2) -> Option<(usize, usize)> {
        // Off the tree, not off the store: a hit test needs the node's
        // box, and only a built frame has one. Which is also why this one
        // reads the drawn frame rather than `building` — nothing hit-tests
        // a frame that is still being declared.
        let i = self.tree.index_of(key)?;
        let crate::tree::NodeContent::Cells(id) = self.tree.content[i] else {
            return None;
        };
        let cell = {
            let sess = &mut *self.session.state();
            self.cells
                .cell_size(id, false, &sess.resources, &mut sess.fonts)
        };
        let (rows, cols) = self.cells.dims(id, false);
        let pos = self.cells_origin(i);
        let col = ((point.x - pos.x) / cell.w.max(f32::EPSILON)).floor();
        let row = ((point.y - pos.y) / cell.h.max(f32::EPSILON)).floor();
        Some((
            (row.max(0.0) as usize).min(rows.saturating_sub(1)),
            (col.max(0.0) as usize).min(cols.saturating_sub(1)),
        ))
    }

    /// Selects the word under `point` in the grid `key` — a double click
    /// on a terminal. Answers the span it took, as an absolute line and a
    /// half-open column range, so a drag that follows can round to it.
    pub fn select_word_in_cells(
        &mut self,
        key: Key,
        point: Vec2,
        block: bool,
    ) -> Option<(u64, usize, usize)> {
        let (row, col) = self.cell_row_col(key, point)?;
        let id = self.cells_id_of_ref(key)?;
        let (from, to) = self.cells.word_at(id, row, col, self.building)?;
        let line = self.cells.origin_line(id, self.building) + row as u64;
        self.set_cell_selection(
            CellSelection::new(key, CellEnd::new(line, from), CellEnd::new(line, to)).block(block),
        );
        Some((line, from, to))
    }

    /// Selects the whole row under `point` — a triple click. Edge to edge,
    /// the way a line in the middle of a linewise selection runs; the
    /// copy is what trims the blanks off the end of it.
    pub fn select_line_in_cells(
        &mut self,
        key: Key,
        point: Vec2,
        block: bool,
    ) -> Option<(u64, usize, usize)> {
        let (row, _) = self.cell_row_col(key, point)?;
        let id = self.cells_id_of_ref(key)?;
        let (_, cols) = self.cells.dims(id, self.building);
        let line = self.cells.origin_line(id, self.building) + row as u64;
        self.set_cell_selection(
            CellSelection::new(key, CellEnd::new(line, 0), CellEnd::new(line, cols)).block(block),
        );
        Some((line, 0, cols))
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

    /// Moves the live end of a cell selection by whatever the press armed
    /// it with: cells, words, or whole rows. The *anchor* rounds outwards
    /// too, so a double-click-drag that turns back on itself keeps the
    /// word it started in whole — the same rule the text side follows.
    pub(crate) fn extend_cell_selection_grained(
        &mut self,
        drag: crate::select::SelectDrag,
        point: Vec2,
    ) -> bool {
        let Some(crate::select::DragAnchor::Cells(a_line, a_from, a_to)) =
            drag.anchor.filter(|_| drag.grain != Grain::Char)
        else {
            return self.extend_cell_selection(point);
        };
        let Some(sel) = self.cell_selection else {
            return false;
        };
        let Some((row, col)) = self.cell_row_col(sel.node, point) else {
            return false;
        };
        let Some(id) = self.cells_id_of_ref(sel.node) else {
            return false;
        };
        let (_, cols) = self.cells.dims(id, self.building);
        let line = self.cells.origin_line(id, self.building) + row as u64;
        // The unit under the live end.
        let (f_from, f_to) = match drag.grain {
            Grain::Word => match self.cells.word_at(id, row, col, self.building) {
                Some(span) => span,
                None => return false,
            },
            _ => (0, cols),
        };
        // A block selection is ordered by column alone, because that is
        // the only axis its two ends disagree on.
        let backwards = if sel.block {
            col < a_from
        } else {
            (line, col) < (a_line, a_from)
        };
        let (a_edge, f_edge) = grained_edges((a_from, a_to), (f_from, f_to), backwards);
        let next = CellSelection::new(
            sel.node,
            CellEnd::new(a_line, a_edge),
            CellEnd::new(line, f_edge),
        )
        .block(sel.block);
        if next == sel {
            return false;
        }
        self.cell_selection = Some(next);
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
        let (rows, cols) = self.cells.dims(id, self.building);
        let origin = self.cells.origin_line(id, self.building);
        let mut out = String::new();
        let mut first = true;
        let mut any = false;
        for row in 0..rows {
            let line = origin + row as u64;
            let Some((from, to)) = sel.cols_on(line, cols) else {
                continue;
            };
            if !first {
                out.push('\n');
            }
            first = false;
            any = true;
            let mut text = String::new();
            for col in from..to {
                match self.cells.cell_char(id, row, col, self.building) {
                    // The cell after a wide glyph is the app's spacer, and
                    // copying it would put a blank in the middle of a word.
                    Some((_, true)) => {}
                    Some((ch, _)) => text.push(ch),
                    None => {}
                }
            }
            out.push_str(text.trim_end());
        }
        // Nothing of the grid is inside the selection — it is scrolled
        // away entirely — so there is nothing to copy. `Some("")` here
        // would let Cmd-C wipe whatever was on the clipboard.
        any.then_some(out)
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
        // A grid selects in cells: the whole screen it was given, from
        // its first absolute line to its last.
        if let Some(id) = self.cells_id_of_ref(scope) {
            let (rows, cols) = self.cells.dims(id, self.building);
            if rows == 0 || cols == 0 {
                return false;
            }
            let origin = self.cells.origin_line(id, self.building);
            self.set_cell_selection(CellSelection::new(
                scope,
                CellEnd::new(origin, 0),
                CellEnd::new(origin + rows as u64 - 1, cols),
            ));
            return true;
        }
        // A virtual list selects its *data*: rows `0..count`, whether the
        // frame built them or not (ADR 0017, tier 3). An end in a row the
        // frame built is that row's first or last run, as a drag would
        // have made it; one in a row it did not build is placed by its
        // index alone, on a node no run matches — the scope's — with the
        // last row's end spelled `ROW_END`, since nothing here knows how
        // long a row it never laid out is.
        if let Some(count) = self.row_count_in(scope) {
            if count == 0 {
                return false;
            }
            let last_row = count - 1;
            let runs = self.text.scope_runs(scope, self.building);
            let (mut first, mut last) = (None, None);
            for run in &runs {
                match self.row_of(run.place.key) {
                    Some(0) if first.is_none() => first = Some(run.place.key),
                    Some(r) if r == last_row => {
                        last = Some((run.place.key, run.text.content().len()));
                    }
                    _ => {}
                }
            }
            drop(runs);
            let anchor = first.map_or(Endpoint::new(scope, 0), |k| Endpoint::new(k, 0));
            let focus = last.map_or(Endpoint::new(scope, crate::select::ROW_END), |(k, len)| {
                Endpoint::new(k, len)
            });
            self.set_selection(Selection::new(
                scope,
                anchor.in_row(Some(0)),
                focus.in_row(Some(last_row)),
            ));
            return true;
        }
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

    /// The `rowCount` declared on `scope` or on a node inside it, if any:
    /// the size of the virtual list a Select All in that scope spans. The
    /// first in tree order where two lists share one scope, which is not
    /// a shape Select All can serve anyway.
    fn row_count_in(&self, scope: Key) -> Option<u64> {
        if self.tree.row_counts.is_empty() {
            return None;
        }
        let top = self.tree.index_of(scope)?;
        self.tree
            .row_counts
            .iter()
            .find(|(node, _)| {
                let mut i = *node as usize;
                loop {
                    if i == top {
                        return true;
                    }
                    match self.tree.parent[i] {
                        crate::tree::NIL => return false,
                        p => i = p as usize,
                    }
                }
            })
            .map(|(_, n)| *n)
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
        // Off the tree the key is looked up in, rather than off the map
        // the last *emission* filled: during a build those are two
        // different trees, and an index into one says nothing about the
        // other. Free where it does not apply — a frame with no
        // virtualised rows answers on the first line.
        if self.tree.indexed.is_empty() {
            return None;
        }
        let mut i = self.tree.index_of(node)?;
        loop {
            if let Some(&(_, row)) = self.tree.indexed.iter().find(|(n, _)| *n as usize == i) {
                return Some(row);
            }
            match self.tree.parent[i] {
                crate::tree::NIL => return None,
                p => i = p as usize,
            }
        }
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
    /// row's text — **in reading order**: `from` precedes `to` whichever
    /// way the drag was made, so an app answering a `selectionrange` ask
    /// can iterate `from..=to` (the clipboard examples do). `None` when
    /// there is no selection, or when neither end is in a virtualised row
    /// (nothing to ask about: the core has it all). The directed pair is
    /// [`Self::selection_ends`].
    pub fn selection_range(&self) -> Option<(RangeEnd, RangeEnd)> {
        let sel = self.selection?;
        let (a, f) = (sel.anchor, sel.focus);
        if a.row.is_none() && f.row.is_none() {
            return None;
        }
        let end = |e: Endpoint| RangeEnd {
            row: e.row,
            byte: e.byte,
        };
        if self.end_precedes(sel.scope, f, a) {
            Some((end(f), end(a)))
        } else {
            Some((end(a), end(f)))
        }
    }

    /// Whether `x` comes before `y` in the scope's reading order — the
    /// question a backwards drag makes of two ends. Both built this frame:
    /// by their offset in the scope's concatenation, the order the drag
    /// itself is decided by. Both in virtualised rows: by row, then byte.
    /// One built and one in a row the frame never built: the unbuilt row
    /// is placed after the scope's built rows or before them by the one
    /// rule `resolve_selection` paints by (`unbuilt_row_is_after`), so
    /// the highlight and the answer agree. Nothing to compare by: the
    /// pair keeps its order — `false` here, since the caller asks whether
    /// the focus precedes the anchor.
    fn end_precedes(&self, scope: Key, x: Endpoint, y: Endpoint) -> bool {
        let prev = self.building;
        let ox = self.text.scope_offset(scope, x.node, x.byte, prev);
        let oy = self.text.scope_offset(scope, y.node, y.byte, prev);
        match (ox, oy, x.row, y.row) {
            (Some(ox), Some(oy), ..) => ox < oy,
            (_, _, Some(rx), Some(ry)) => (rx, x.byte) < (ry, y.byte),
            (Some(_), None, _, Some(ry)) => unbuilt_row_is_after(ry, self.last_built_row_in(scope)),
            (None, Some(_), Some(rx), _) => {
                !unbuilt_row_is_after(rx, self.last_built_row_in(scope))
            }
            _ => false,
        }
    }

    /// The row of the last text run the frame built inside `scope` that
    /// carries one — what an end the frame did not build is placed
    /// against. This scope's rows only: a second virtual list on screen
    /// says nothing about where a row of this one sits.
    fn last_built_row_in(&self, scope: Key) -> Option<u64> {
        (0..self.tree.len()).rev().find_map(|i| {
            (self.scopes.get(i).copied().flatten() == Some(scope)
                && matches!(self.tree.content[i], NodeContent::Text(_)))
            .then(|| self.rows.get(i).copied().flatten())
            .flatten()
        })
    }

    /// The selection's two ends as the drag made them — the anchor where
    /// the press landed, the focus where the pointer is — each as the
    /// data index of the virtualised row it is in (`None` outside every
    /// virtualised row) and the byte inside that node's own text. The
    /// directed pair, unlike [`Self::selection_range`]'s: what a test or
    /// a model that mirrors the selection reads, and what says whether a
    /// Shift-press kept the anchor (ADR 0029). `None` with no text
    /// selection; a grid's is `cell_selection`.
    pub fn selection_ends(&self) -> Option<(RangeEnd, RangeEnd)> {
        let sel = self.selection?;
        Some((
            RangeEnd {
                row: sel.anchor.row,
                byte: sel.anchor.byte,
            },
            RangeEnd {
                row: sel.focus.row,
                byte: sel.focus.byte,
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
                ("from", from.to_value()),
                ("to", to.to_value()),
            ]),
            slot: None,
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

    // -- The clipboard, for an app that owns its text ---------------------
    // A key sink hears the raw `Ctrl-c` / `Ctrl-v` and brings its own
    // bindings — and had nowhere to bind them to (backlog C33): the only
    // ways onto the system clipboard were a menu's Copy and Paste. These
    // are those two actions with a door on them. The clipboard stays the
    // host's: the core never reads it, and what a paste brings back
    // arrives as input, the way a menu's Paste does.

    /// Puts `text` on the system clipboard — queued as the
    /// `MenuAction::SetClipboard` a menu's Copy produces, for the host to
    /// apply at its next drain (the runner's is after every input and
    /// every frame). `html` is a second flavour beside the text for a
    /// host that offers one, never in place of it.
    pub fn set_clipboard(&mut self, text: impl Into<String>, html: Option<String>) {
        self.menu_actions
            .push(crate::menu::MenuAction::SetClipboard {
                text: text.into(),
                html,
            });
    }

    /// Puts a secret on the system clipboard the way a password manager
    /// does (backlog F84) — queued as `MenuAction::SetClipboardSecret`,
    /// which the runner writes marked concealed and transient, so a
    /// clipboard manager neither shows nor keeps it. What the marks are on
    /// each platform is on the action. The text alone: a secret has no
    /// second flavour to offer.
    pub fn set_clipboard_secret(&mut self, text: impl Into<String>) {
        self.menu_actions
            .push(crate::menu::MenuAction::SetClipboardSecret { text: text.into() });
    }

    /// Asks for what is on the clipboard — queued as the
    /// `MenuAction::Paste` a menu's Paste produces. The host reads the
    /// clipboard and hands the text back as `InputEvent::Paste` (or a
    /// bare `InputEvent::Commit`), which reaches a focused editor as
    /// typing and a focused sink as `{kind:"text", text, tag}` (backlog
    /// C17), with `concealed: true` / `transient: true` beside the text
    /// when the pasteboard marked it so (backlog F84) — so the app that asked
    /// inserts it the way it inserts a committed IME string, and never
    /// sees the clipboard any other way. The read stays on the driver's
    /// side, where the permission lives.
    ///
    /// One ask at a time: while a paste is outstanding — queued, or taken
    /// by the driver and not yet answered — a second ask is dropped, so a
    /// view that asks on every frame until the answer lands asks once
    /// (backlog AR34; both Rust examples carried this guard themselves).
    /// The answer is the `Paste` (or `Commit`) the driver sends, an empty
    /// one when the clipboard held nothing, and [`Core::awaiting_paste`]
    /// reads the state.
    pub fn request_paste(&mut self) {
        self.queue_paste();
    }

    /// Whether a paste ask is outstanding: asked and not yet answered
    /// with a `Paste` or a `Commit`.
    pub fn awaiting_paste(&self) -> bool {
        self.awaiting_paste
    }

    /// Asks the host for a file dialog (backlog C51): an Open, a Save or
    /// a folder picker, which the host shows as the platform's own. The
    /// answer is an event, `{kind:"files", paths, tag}` — the `drop`
    /// payload's shape, `paths` empty when the user cancelled — delivered
    /// to whoever asked: the host from its own view or between frames, the
    /// extension from inside its fill. A host drains the ask with
    /// [`Core::take_file_requests`] and answers with `InputEvent::Files`;
    /// the runner does both.
    ///
    /// One ask at a time, as for a paste: while one is outstanding —
    /// queued, or taken and not yet answered — another is dropped and this
    /// returns false, so a view that asks every frame until the answer
    /// lands asks once. Between frames it asks for the frame that hands
    /// the ask to the host.
    #[track_caller]
    pub fn request_files(&mut self, dialog: crate::dialog::FileDialog) -> bool {
        if self.file_ask.pending() {
            return false;
        }
        self.file_ask = crate::dialog::FileAsk::Queued(dialog, self.origin);
        if !self.building {
            self.owe_frame("request_files");
        }
        true
    }

    /// Whether a file dialog asked for is still unanswered.
    pub fn awaiting_files(&self) -> bool {
        self.file_ask.pending()
    }

    /// The file dialog asked for and not yet taken — at most one — for the
    /// host to show. Taking it keeps the ask outstanding until the answer.
    pub fn take_file_requests(&mut self) -> Vec<crate::dialog::FileDialog> {
        match std::mem::take(&mut self.file_ask) {
            crate::dialog::FileAsk::Queued(dialog, origin) => {
                self.file_ask = crate::dialog::FileAsk::Taken(dialog.tag.clone(), origin);
                vec![dialog]
            }
            other => {
                self.file_ask = other;
                Vec::new()
            }
        }
    }

    /// The one place a `Paste` is queued — the app's ask and a menu's
    /// Paste row alike — so the gate is one.
    pub(crate) fn queue_paste(&mut self) {
        if self.awaiting_paste {
            return;
        }
        self.awaiting_paste = true;
        self.menu_actions.push(crate::menu::MenuAction::Paste);
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

    /// A keyboard's selection in a `selectable` scope (backlog AR28):
    /// Shift with an arrow, Home or End on a focused node inside `scope`
    /// — the scope itself when it is focusable, a control inside it —
    /// moves the selection's focus the way the stock editor's Shift-
    /// motions move its caret: a character (a word with `mods.word`)
    /// left or right through the scope's runs in order, Home and End to
    /// the scope's first and last byte. Nothing selected yet, the anchor
    /// is placed at the scope's start, so Shift-End from a freshly
    /// focused label selects it whole. Returns whether the selection
    /// changed. Answered from the frame that finished, like a drag; the
    /// endpoints carry their virtual rows like every other selection, so
    /// a copy past the built range asks the app as ADR 0017's tier 3
    /// does. Up and Down are not motions here: a scope has no line
    /// geometry a caret could keep a column in.
    pub fn keyboard_select(&mut self, scope: Key, key: EditKey, mods: Mods) -> bool {
        let prev = self.building;
        // Every character of the scope with its offset in the
        // concatenation and the run it is in: what the motions step
        // through. A run's end is a word's end — the concatenation has no
        // separator, and a copy puts a newline there.
        let chars: Vec<(usize, char, usize)> = self
            .text
            .scope_runs(scope, prev)
            .iter()
            .enumerate()
            .flat_map(|(n, r)| {
                let base = r.base;
                r.text
                    .content()
                    .char_indices()
                    .map(move |(i, c)| (base + i, c, n))
                    .collect::<Vec<_>>()
            })
            .collect();
        let total = chars.last().map_or(0, |(o, c, _)| o + c.len_utf8());
        let sel = self.selection.filter(|s| s.scope == scope);
        let at = |e: Endpoint| self.text.scope_offset(scope, e.node, e.byte, prev);
        let (anchor, focus) = match sel {
            Some(s) => match (at(s.anchor), at(s.focus)) {
                (Some(a), Some(f)) => (a, f),
                _ => (0, 0),
            },
            None => (0, 0),
        };
        let word = mods.word;
        let ws = |i: usize| chars[i].1.is_whitespace();
        let next = match key {
            EditKey::Right => {
                let mut i = chars
                    .iter()
                    .position(|(o, ..)| *o >= focus)
                    .unwrap_or(chars.len());
                if word {
                    while i < chars.len() && ws(i) {
                        i += 1;
                    }
                    let run = chars.get(i).map(|c| c.2);
                    while i < chars.len() && !ws(i) && Some(chars[i].2) == run {
                        i += 1;
                    }
                    chars.get(i).map_or(total, |(o, ..)| *o)
                } else {
                    chars.get(i).map_or(total, |(o, c, _)| o + c.len_utf8())
                }
            }
            EditKey::Left => {
                let mut i = chars
                    .iter()
                    .rposition(|(o, ..)| *o < focus)
                    .map_or(0, |i| i + 1);
                if word {
                    while i > 0 && ws(i - 1) {
                        i -= 1;
                    }
                    let run = (i > 0).then(|| chars[i - 1].2);
                    while i > 0 && !ws(i - 1) && Some(chars[i - 1].2) == run {
                        i -= 1;
                    }
                    chars.get(i).map_or(total, |(o, ..)| *o)
                } else if i == 0 {
                    0
                } else {
                    chars[i - 1].0
                }
            }
            EditKey::Home => 0,
            EditKey::End => total,
            _ => return false,
        };
        if sel.is_some() && next == focus {
            return false;
        }
        let Some(anchor) = self.endpoint_at_offset(scope, anchor, prev) else {
            return false;
        };
        let Some(focus) = self.endpoint_at_offset(scope, next, prev) else {
            return false;
        };
        self.set_selection(Selection::new(scope, anchor, focus));
        true
    }

    /// The endpoint at `offset` in the scope's concatenation: the run it
    /// falls in and the byte inside that run's text — the last run's end
    /// for the offset past everything. `None` for a scope with no runs.
    fn endpoint_at_offset(&self, scope: Key, offset: usize, prev: bool) -> Option<Endpoint> {
        let runs = self.text.scope_runs(scope, prev);
        let run = runs
            .iter()
            .find(|r| {
                let (start, end) = r.span();
                offset >= start && offset < end
            })
            .or_else(|| runs.last())?;
        let node = run.place.key;
        let byte = offset
            .saturating_sub(run.base)
            .min(run.text.content().len());
        Some(Endpoint::new(node, byte).in_row(self.row_of(node)))
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
        let Some(crate::select::DragAnchor::Bytes(anode, a_from, a_to)) = drag.anchor else {
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
        // Which side of the anchor the live end is on, in the scope's
        // concatenation.
        let prev = self.building;
        let ga = self.text.scope_offset(sel.scope, anode, a_from, prev);
        let gf = self.text.scope_offset(sel.scope, hit.node, f_from, prev);
        let (Some(ga), Some(gf)) = (ga, gf) else {
            return false;
        };
        let (arow, frow) = (self.row_of(anode), self.row_of(hit.node));
        let (a_edge, f_edge) = grained_edges((a_from, a_to), (f_from, f_to), gf < ga);
        let next = Selection::new(
            sel.scope,
            Endpoint::new(anode, a_edge).in_row(arow),
            Endpoint::new(hit.node, f_edge).in_row(frow),
        );
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

    /// Arms a drag-select at a press inside `scope`, with what the click
    /// count says it moves by (`Grain::of_clicks`) and the span the press
    /// itself took, which both ends of the drag round outwards to. A grid
    /// selects in cells and a paragraph in bytes (ADR 0017, decision 4):
    /// this is where the two are told apart, once, and the anchor carries
    /// the answer for the drag. In a grid, Alt makes it the rectangular
    /// selection every terminal has. With `extend` — a Shift-press in
    /// the scope the selection is in — the anchor is kept and the press
    /// is the live end (ADR 0029, decision 3). Whether a drag was armed.
    pub(crate) fn arm_select_drag(
        &mut self,
        scope: Key,
        point: Vec2,
        clicks: u8,
        extend: bool,
    ) -> bool {
        let grain = if extend {
            Grain::Char
        } else {
            Grain::of_clicks(clicks)
        };
        let armed = if extend {
            // The anchor stays; the live end is the press, and the drag
            // goes on from there by characters, whatever the click count
            // — armed whether or not the press moved the end (one on the
            // focus itself moves nothing and still drags on). The
            // window's one selection is this one, so a focused editor's
            // collapses as `set_selection` would have it.
            let drag = SelectDrag {
                scope,
                grain,
                anchor: None,
            };
            self.extend_select_drag(drag, point);
            self.collapse_editor_selection();
            Some(None)
        } else if self.cells_id_of_ref(scope).is_some() {
            let block = self.interaction.modifiers().alt;
            match grain {
                Grain::Char => self
                    .begin_cell_selection(scope, point, block)
                    .then_some(None),
                Grain::Word => self
                    .select_word_in_cells(scope, point, block)
                    .map(|(l, f, t)| Some(DragAnchor::Cells(l, f, t))),
                Grain::Run => self
                    .select_line_in_cells(scope, point, block)
                    .map(|(l, f, t)| Some(DragAnchor::Cells(l, f, t))),
            }
        } else {
            match grain {
                Grain::Char => self.begin_selection(scope, point).then_some(None),
                Grain::Word => self
                    .select_word_at(scope, point)
                    .map(|(n, f, t)| Some(DragAnchor::Bytes(n, f, t))),
                Grain::Run => self
                    .select_run_at(scope, point)
                    .map(|(n, f, t)| Some(DragAnchor::Bytes(n, f, t))),
            }
        };
        if let Some(anchor) = armed {
            self.select_dragging = Some(SelectDrag {
                scope,
                grain,
                anchor,
            });
        }
        armed.is_some()
    }

    /// Moves the live end of the drag [`Self::arm_select_drag`] started,
    /// in whichever geometry its scope has.
    pub(crate) fn extend_select_drag(&mut self, drag: SelectDrag, point: Vec2) -> bool {
        if self.cells_id_of_ref(drag.scope).is_some() {
            self.extend_cell_selection_grained(drag, point)
        } else {
            self.extend_selection_grained(drag, point)
        }
    }

    /// Selects the word under `point` in `scope`, whichever way the scope
    /// addresses itself — what a double click takes, and what a force
    /// click takes before it asks for a definition. `false` when there
    /// was no word there.
    pub fn select_word_under(&mut self, scope: Key, point: Vec2) -> bool {
        if self.cells_id_of_ref(scope).is_some() {
            self.select_word_in_cells(scope, point, false).is_some()
        } else {
            self.select_word_at(scope, point).is_some()
        }
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
