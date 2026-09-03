//! Pointer shape as derived data. Nothing declares a cursor for the common
//! cases: the core resolves one per frame from whatever the pointer is
//! over ([`crate::runtime::Core::cursor_shape`]) — an editor is a caret, a
//! button is a hand, a drag source is a grab — and the frame driver hands
//! that to the real window. A view can still override it with the `cursor`
//! prop where the derivation cannot know (a splitter that resizes, a
//! disabled node that wants to say so).
//!
//! The core never touches a device: headless drivers simply never read.

/// The shape the pointer takes. Spelled the way CSS and the platform
/// toolkits do, so a driver maps it one-to-one (winit's `CursorIcon`, the
/// Web's `cursor`, GTK's names) instead of interpreting it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorShape {
    /// The plain arrow: plain boxes, window chrome, anything unclaimed.
    #[default]
    Default,
    /// I-beam: text that can be selected or edited.
    Text,
    /// The pointing hand: something that acts when clicked.
    Pointer,
    /// Open hand: something that can be dragged, but is not being dragged.
    Grab,
    /// Closed hand: a drag is in flight (the pointer is captured).
    Grabbing,
    /// Refused: an inert control that wants to say why the click did
    /// nothing. Only ever declared — the core never derives it, because a
    /// `disabled` node is far more often just quiet.
    NotAllowed,
    /// Horizontal resize (a vertical splitter, the left/right window edge).
    EwResize,
    /// Vertical resize (a horizontal splitter, the top/bottom window edge).
    NsResize,
    /// Diagonal resize, top-left/bottom-right.
    NwseResize,
    /// Diagonal resize, top-right/bottom-left.
    NeswResize,
}

impl CursorShape {
    /// Every shape, in `schema::CURSORS` order (the `cursor` prop's).
    pub const ALL: &'static [CursorShape] = &[
        CursorShape::Default,
        CursorShape::Text,
        CursorShape::Pointer,
        CursorShape::Grab,
        CursorShape::Grabbing,
        CursorShape::NotAllowed,
        CursorShape::EwResize,
        CursorShape::NsResize,
        CursorShape::NwseResize,
        CursorShape::NeswResize,
    ];

    /// The camelCase spelling every binding uses.
    pub fn name(self) -> &'static str {
        match self {
            CursorShape::Default => "default",
            CursorShape::Text => "text",
            CursorShape::Pointer => "pointer",
            CursorShape::Grab => "grab",
            CursorShape::Grabbing => "grabbing",
            CursorShape::NotAllowed => "notAllowed",
            CursorShape::EwResize => "ewResize",
            CursorShape::NsResize => "nsResize",
            CursorShape::NwseResize => "nwseResize",
            CursorShape::NeswResize => "neswResize",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The schema's name list is the wire order: an index means the same
    /// shape in every binding, so the two cannot drift.
    #[test]
    fn schema_names_are_all_in_order() {
        let names: Vec<&str> = CursorShape::ALL.iter().map(|c| c.name()).collect();
        assert_eq!(names, crate::schema::CURSORS);
    }
}
