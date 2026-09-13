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

/// What a selection is painted under when nothing else says — the dark
/// base's tint, and what kui painted before there were themes. The live
/// value is `theme.selection`, which both a `selectable` scope and an
/// editor read, because a selection over a label and one over a field
/// sitting side by side must not be two different blues. This constant
/// stays as the floor an [`crate::edit::EditOptions`] the core never
/// stamped falls back to (ADR 0019).
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
    /// In a `cells` grid it is one whole row, edge to edge, which is what
    /// a triple click takes in every terminal.
    Run,
}

impl Grain {
    /// The grain a press arms, from the click count the driver counted:
    /// one click (or none counted) a character, two the word under it,
    /// three or more the whole run.
    pub(crate) fn of_clicks(clicks: u8) -> Self {
        match clicks {
            0 | 1 => Grain::Char,
            2 => Grain::Word,
            _ => Grain::Run,
        }
    }
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
        if self.block {
            if line < a.line || line > b.line {
                return None;
            }
            let (lo, hi) = (a.col.min(b.col), a.col.max(b.col));
            return (lo < hi).then_some((lo.min(cols), hi.min(cols)));
        }
        clip_to_unit((a.line, a.col), (b.line, b.col), line, cols)
    }
}

/// The part of one unit — a text node of `len` bytes, a grid row of `len`
/// columns — that a selection running from `start` to `end` covers, as a
/// half-open range in that unit's own offsets. `None` when the unit is
/// outside the selection. The unit is named by whatever orders the
/// scope's units (an ordinal, an absolute line); the arithmetic is the
/// same for both geometries ADR 0017 decision 4 keeps apart (AR3): the
/// first unit runs from the start's offset, the last to the end's, and
/// every unit between runs edge to edge.
pub(crate) fn clip_to_unit<U: Ord + Copy>(
    start: (U, usize),
    end: (U, usize),
    unit: U,
    len: usize,
) -> Option<(usize, usize)> {
    if unit < start.0 || unit > end.0 {
        return None;
    }
    let from = if unit == start.0 { start.1 } else { 0 };
    let to = if unit == end.0 { end.1 } else { len };
    let (from, to) = (from.min(len), to.min(len));
    (from < to).then_some((from, to))
}

/// The edges a grained drag runs between: the anchor's unit and the live
/// end's unit are each `(from, to)`, and which side of the anchor the
/// live end is on decides which edge of each the selection takes —
/// backwards, from the far edge of the anchor's unit to the near edge of
/// the live one; forwards, the reverse. So the unit the press took stays
/// whole however far back over itself the drag turns, in bytes or in
/// cells alike (AR3). Answers `(anchor edge, live edge)`.
pub(crate) fn grained_edges(
    anchor: (usize, usize),
    live: (usize, usize),
    backwards: bool,
) -> (usize, usize) {
    if backwards {
        (anchor.1, live.0)
    } else {
        (anchor.0, live.1)
    }
}

/// Where a selection end in a virtualised row the frame did not build
/// sits against the rows it did: after every built run of the scope iff
/// its row is past the last built row that carries one (`last`), before
/// them otherwise — below the first row, or in a hole, which a contiguous
/// virtual window does not have. The one rule the highlight paints by
/// (`resolve_selection`) and a `selectionrange` ask orders by
/// (`selection_range`). With no built row to compare against, nothing is
/// after: the start is the honest boundary.
pub(crate) fn unbuilt_row_is_after(row: u64, last: Option<u64>) -> bool {
    last.is_some_and(|hi| row > hi)
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

impl RangeEnd {
    /// The end as data — `{index, byte}`, the index null outside every
    /// virtualised row: the shape a `selectionrange` ask carries and a
    /// binding's `selection_ends` reads back, spelled once.
    pub fn to_value(self) -> crate::value::Value {
        use crate::value::Value;
        Value::map([
            (
                "index",
                self.row.map_or(Value::Null, |r| Value::Int(r as i64)),
            ),
            ("byte", Value::Int(self.byte as i64)),
        ])
    }
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

/// The span the press itself selected — the word a double click took, the
/// run or row a triple click took — which both ends of the drag round
/// outwards to. Two shapes because the two kinds of scope address
/// themselves differently, and a drag is only ever in one of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DragAnchor {
    /// `(node, from, to)`, in that node's own bytes.
    Bytes(Key, usize, usize),
    /// `(line, from, to)`: a half-open column range on one absolute line
    /// of a `cells` grid.
    Cells(u64, usize, usize),
}

/// A drag-select in flight: which scope it is in, what it moves by, and
/// the span the press itself selected — the word a double click took, the
/// run a triple click took — which both ends round outwards to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SelectDrag {
    pub scope: Key,
    pub grain: Grain,
    /// `None` for a character drag, which has nothing to round to.
    pub anchor: Option<DragAnchor>,
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
        clip_to_unit(self.start, self.end, ord, len)
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
