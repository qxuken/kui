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
}

impl Endpoint {
    pub fn new(node: Key, byte: usize) -> Self {
        Self { node, byte }
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
