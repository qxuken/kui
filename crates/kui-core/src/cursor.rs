//! Pointer shape as declared data. A view says what the pointer is over
//! a node with the `cursor` prop — a button is a hand because it declared
//! one, a handle a grab because it declared one — and the core resolves
//! which declaration is under the pointer per frame
//! ([`crate::runtime::Core::cursor_shape`]), which the frame driver hands
//! to the real window. Nothing is inferred from what a node *does*: an
//! `on_click` node with no `cursor` is the plain arrow, as a native
//! button is, and so is an `on_drag` node. The one shape the core implies
//! is the I-beam over an editor or a selection scope, the way every
//! desktop marks text that can be taken. The stock button declares
//! `Pointer` for itself, so `<button>` is a hand in every binding without
//! the app saying so.
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
    /// nothing.
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
