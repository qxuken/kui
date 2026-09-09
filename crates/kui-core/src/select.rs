//! The window's text selection outside an editor: what a `selectable`
//! node scopes, what a press-drag across it produces, and what a copy
//! reads (`docs/adr/0017-selection-as-a-scope.md`).
//!
//! There is one of these per window, and an editor's own selection is the
//! other half of the same exclusivity: starting one clears the other, the
//! way moving focus clears the last focus. What is kept here is two
//! *addresses* — a node key and a byte inside that node's own text — and
//! never a byte into a concatenation, an index into a frame's vectors or
//! a handle into the shaped-text cache. A frame that no longer builds the
//! node an address names resolves it to nothing and paints nothing, which
//! is the honest answer; a frame that builds it again resolves it again.

use crate::color::Color;
use crate::key::Key;

/// What a selection is painted under. The editor's own default
/// (`EditOptions::accent`), because a selection over a label and one over
/// a field sitting side by side must not be two different blues.
pub const TINT: Color = Color {
    r: 0x3b as f32 / 255.0,
    g: 0x5b as f32 / 255.0,
    b: 0xd4 as f32 / 255.0,
    a: 0x66 as f32 / 255.0,
};

/// One end of a selection: the node whose text it lands in, and a byte
/// offset into *that node's* content (not into the scope's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub node: Key,
    pub byte: usize,
    /// The data index of the virtualised row this end is in, when it is in
    /// one (`open_indexed`). Recorded when the end is made, and the only
    /// thing that can place it once its row stops being built: a key says
    /// *which* node, an index says *where in the data* — and a frame that
    /// never built the node can still answer the second question (ADR
    /// 0017, decision 3).
    pub row: Option<u64>,
}

impl Endpoint {
    pub fn new(node: Key, byte: usize) -> Self {
        Self {
            node,
            byte,
            row: None,
        }
    }

    /// The same end, in the virtualised row `row`.
    pub fn in_row(mut self, row: Option<u64>) -> Self {
        self.row = row;
        self
    }
}

/// The window's selection: a scope and two ends of it. `anchor` is where
/// the press landed and `focus` is where the pointer is now, so the pair
/// is *directed* — dragging back past the anchor selects the other way
/// without the two swapping, which is what keeps a drag from feeling like
/// it jumps when it crosses its own start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// The `selectable` node the selection lives inside.
    pub scope: Key,
    pub anchor: Endpoint,
    pub focus: Endpoint,
}

impl Selection {
    pub fn new(scope: Key, anchor: Endpoint, focus: Endpoint) -> Self {
        Self {
            scope,
            anchor,
            focus,
        }
    }

    /// A selection of no text — a click that placed both ends together.
    /// It still exists (the scope is where the next Shift-click or drag
    /// extends from), and it copies nothing.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.focus
    }
}

/// What a drag-select moves by. A press sets it from the click count the
/// driver counted, the way every text UI does: one click drags by
/// characters, two by words, three by whole runs.
///
/// The unit is not just a rounding of the live end — the *anchor* rounds
/// too, and outwards. A double-click-drag that turns back on itself keeps
/// the word it started in whole, which is what makes the gesture feel
/// like it is selecting words rather than snapping to them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Grain {
    #[default]
    Char,
    Word,
    /// One text node's whole content — a label, a paragraph. cosmic-text
    /// calls this a line; here a run is the thing a triple click takes.
    Run,
}

/// One end of a selection in a cell grid: an *absolute* line (the grid's
/// `origin_line` plus the row) and a column. Absolute because a grid is
/// one screenful of an app's own history, so a row number means a
/// different line after every scroll (ADR 0017, decision 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellEnd {
    pub line: u64,
    pub col: usize,
}

impl CellEnd {
    pub fn new(line: u64, col: usize) -> Self {
        Self { line, col }
    }
}

/// A selection inside one `cells` grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellSelection {
    /// The grid it lives in.
    pub node: Key,
    pub anchor: CellEnd,
    pub focus: CellEnd,
    /// Rectangular rather than linewise: the column range is the same on
    /// every line, which is how a terminal selects a column of output.
    /// Held by a modifier while dragging, the way every terminal does it.
    pub block: bool,
}

impl CellSelection {
    pub fn new(node: Key, anchor: CellEnd, focus: CellEnd) -> Self {
        Self {
            node,
            anchor,
            focus,
            block: false,
        }
    }

    pub fn block(mut self, on: bool) -> Self {
        self.block = on;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.focus
    }

    /// The two ends in reading order.
    pub fn ordered(&self) -> (CellEnd, CellEnd) {
        if self.anchor <= self.focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        }
    }

    /// The columns selected on `line`, as a half-open range, or `None`
    /// when the line is outside the selection. Linewise by default — a
    /// line in the middle runs edge to edge, which `cols` gives — and a
    /// block selection is the same column range on every line it covers.
    pub fn cols_on(&self, line: u64, cols: usize) -> Option<(usize, usize)> {
        let (a, b) = self.ordered();
        if line < a.line || line > b.line {
            return None;
        }
        if self.block {
            let (lo, hi) = (a.col.min(b.col), a.col.max(b.col));
            return (lo < hi).then_some((lo.min(cols), hi.min(cols)));
        }
        let from = if line == a.line { a.col } else { 0 };
        let to = if line == b.line { b.col } else { cols };
        let (from, to) = (from.min(cols), to.min(cols));
        (from < to).then_some((from, to))
    }
}

/// One end of the range an app is asked to fill in
/// (`Core::selection_range`): the data index of the row it is in, and the
/// byte inside that row's own text. An end outside every virtualised row
/// has no index — it is text the core built and can answer for itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangeEnd {
    pub row: Option<u64>,
    pub byte: usize,
}

/// What asking for a copy answered (`Core::request_copy`).
///
/// The third case is the one this type exists for: a selection can reach
/// rows a virtual list never built, and the core will not invent them
/// (ADR 0017, decision 3). It asks the app instead — a `selectionrange`
/// event on the scope — and the answer arrives later as a clipboard
/// action, so a copy over a gap is the app's own text rather than a
/// silent hole in the middle of one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CopyRequest {
    /// The core had all of it; here it is.
    Ready(String),
    /// The app was asked and has not answered yet
    /// (`Core::answer_selection_range`).
    Asked,
    /// Nothing is selected.
    Nothing,
}

/// A drag-select in flight: which scope it is in, what it moves by, and
/// the span the press itself selected — the word a double click took, the
/// run a triple click took — which both ends round outwards to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SelectDrag {
    pub scope: Key,
    pub grain: Grain,
    /// `(node, from, to)` in that node's own bytes. `None` for a
    /// character drag, which has nothing to round to, and for a grid,
    /// which drags in cells.
    pub anchor: Option<(Key, usize, usize)>,
}

/// Where one text node's content sits in a selection: the two ends
/// resolved against *this* node, in its own bytes.
///
/// Resolved by ordinal rather than by a running byte offset, because an
/// ordinal is knowable while the frame is still being emitted and a
/// global offset is not — the runs after this one have not been placed
/// yet, and the anchor may be one of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ends {
    /// Ordinal of the earlier end's node within the scope, and the byte
    /// inside it.
    pub start: (u32, usize),
    pub end: (u32, usize),
}

impl Ends {
    /// The two ends in reading order: by ordinal, and by byte within one
    /// node.
    pub(crate) fn ordered(a: (u32, usize), b: (u32, usize)) -> Self {
        if a <= b {
            Self { start: a, end: b }
        } else {
            Self { start: b, end: a }
        }
    }

    /// The byte range selected in the node at ordinal `ord`, whose own
    /// content is `len` bytes. `None` when the node is outside the
    /// selection entirely.
    pub(crate) fn range_in(&self, ord: u32, len: usize) -> Option<(usize, usize)> {
        if ord < self.start.0 || ord > self.end.0 {
            return None;
        }
        let from = if ord == self.start.0 {
            self.start.1.min(len)
        } else {
            0
        };
        let to = if ord == self.end.0 {
            self.end.1.min(len)
        } else {
            len
        };
        (from < to).then_some((from, to))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(a: (u64, usize), b: (u64, usize)) -> CellSelection {
        CellSelection::new(Key::ROOT, CellEnd::new(a.0, a.1), CellEnd::new(b.0, b.1))
    }

    #[test]
    fn a_linewise_cell_selection_runs_edge_to_edge_in_the_middle() {
        let s = cells((10, 3), (12, 5));
        assert_eq!(s.cols_on(10, 80), Some((3, 80)));
        assert_eq!(s.cols_on(11, 80), Some((0, 80)));
        assert_eq!(s.cols_on(12, 80), Some((0, 5)));
        assert_eq!(s.cols_on(13, 80), None);
        assert_eq!(s.cols_on(9, 80), None);
    }

    #[test]
    fn a_block_selection_is_the_same_columns_on_every_line() {
        let s = cells((10, 6), (12, 2)).block(true);
        for line in 10..=12 {
            assert_eq!(s.cols_on(line, 80), Some((2, 6)));
        }
        assert_eq!(s.cols_on(13, 80), None);
    }

    #[test]
    fn a_backwards_drag_selects_the_same_thing() {
        assert_eq!(
            cells((12, 5), (10, 3)).ordered(),
            cells((10, 3), (12, 5)).ordered()
        );
    }

    #[test]
    fn ends_order_by_ordinal_then_byte() {
        let e = Ends::ordered((2, 5), (0, 9));
        assert_eq!(e.start, (0, 9));
        assert_eq!(e.end, (2, 5));
        let same = Ends::ordered((1, 7), (1, 2));
        assert_eq!(same.start, (1, 2));
        assert_eq!(same.end, (1, 7));
    }

    #[test]
    fn a_middle_node_is_selected_whole() {
        let e = Ends::ordered((0, 3), (2, 4));
        assert_eq!(e.range_in(1, 10), Some((0, 10)));
        assert_eq!(e.range_in(0, 10), Some((3, 10)));
        assert_eq!(e.range_in(2, 10), Some((0, 4)));
        assert_eq!(e.range_in(3, 10), None);
    }

    #[test]
    fn an_empty_range_selects_nothing() {
        // Both ends in one node, at the same byte: a click, not a drag.
        let e = Ends::ordered((1, 4), (1, 4));
        assert_eq!(e.range_in(1, 10), None);
    }

    #[test]
    fn ends_clamp_to_the_content_they_land_in() {
        // The node shrank since the address was taken.
        let e = Ends::ordered((0, 2), (0, 99));
        assert_eq!(e.range_in(0, 5), Some((2, 5)));
    }
}
