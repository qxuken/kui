//! Typed messages over the plain-data payload: `kui-value`'s
//! [`MessageField`], [`MessageError`], [`field`] and [`kind_of`] (ADR 0075,
//! decision 3), re-exported here beside what only the core has — an event's
//! data as a field ([`EventField`]) and reading an event's message
//! ([`UiEvent::message`](crate::input::UiEvent::message)).

pub use kui_value::message::*;

use crate::Value;

/// An event's data as a message field: the type the core writes under
/// one field of the handler's message (ADR 0047, decision 2) — a drag's
/// [`Drag`](crate::event::Drag) under `drag`, an editor's
/// [`EditChange`](crate::event::EditChange) under `change`. Its
/// [`MessageField`] half reads and writes that field's value in the shape
/// the core sends, so a message can carry it as any other field.
///
/// What `#[message(on = drag)]` builds a message's arriving twin from
/// (decision 13): the twin's variant has a field of this type, read from
/// [`Self::FIELD`] of the payload. A message declared on another handler
/// has no such field, so it does not decode as the twin.
pub trait EventField: MessageField {
    /// The event this data arrives with.
    const EVENT: crate::input::EventKind;
    /// The field of the message it rides under: [`Self::EVENT`]'s
    /// [`name`](crate::input::EventKind::name).
    const FIELD: &'static str;
}

impl crate::input::UiEvent {
    /// The app's message this event carries, as `M`: the payload, which is
    /// the message the handler declared with the event's data in one field
    /// of it (ADR 0047) — a field `M` does not name is not read, so a
    /// message decodes whichever event brought it. A message that was not
    /// a map is read from under `tag`, where the core kept it beside the
    /// field. `None` for a message that is not an `M`.
    ///
    /// The event's own data is read beside the message: [`Self::drag`]
    /// and the other views, or [`Self::data`].
    pub fn message<M>(&self) -> Option<M>
    where
        M: for<'a> TryFrom<&'a Value>,
    {
        M::try_from(&self.payload)
            .ok()
            .or_else(|| self.payload.get("tag").and_then(|t| M::try_from(t).ok()))
    }
}
