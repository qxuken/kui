//! Typed messages over the plain-data payload (backlog C50).
//!
//! The IR's payloads are [`Value`]s on purpose: Lua, C and JSX share them.
//! A Rust app gets the exhaustive `match` back on its own side of that
//! contract: `#[derive(Message)]` (the `kui-derive` crate, re-exported by
//! `kui-native` and, behind its `derive` feature, by this crate) turns an enum into
//! a `{kind, …fields}` map and back, so `on_click(Msg::Save)` builds the
//! payload and `ev.message::<Msg>()` reads it.
//!
//! What the derive needs from this module: [`MessageField`], the
//! conversion each field's type has, and [`MessageError`], what a payload
//! that is not one of the enum's says. Both are plain enough to write by
//! hand for a type the derive does not cover.

use crate::value::Value;

/// A type a message field can hold: to a [`Value`] and back. Implemented
/// for the numbers, `bool`, `String`, `Value` itself, `Option` (absent or
/// null is `None`) and `Vec`; `#[derive(Message)]` implements it for the
/// type it derives, so one message can carry another.
pub trait MessageField: Sized {
    fn to_value(self) -> Value;
    /// `None` for a value of the wrong shape.
    fn from_value(v: &Value) -> Option<Self>;
}

/// Why a payload is not the message it was read as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageError {
    /// It has no `kind` string (and, for a string-shaped message, is not a
    /// string).
    NoKind,
    /// Its `kind` names no variant of the type.
    UnknownKind(String),
    /// A field is missing, or holds a value of the wrong shape.
    Field { kind: String, field: &'static str },
}

impl std::fmt::Display for MessageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageError::NoKind => f.write_str("the payload has no `kind`"),
            MessageError::UnknownKind(k) => write!(f, "no message is of kind {k:?}"),
            MessageError::Field { kind, field } => write!(
                f,
                "a {kind:?} message's `{field}` is missing or of the wrong type"
            ),
        }
    }
}

impl std::error::Error for MessageError {}

/// Field `key` of the map `v`, as `T`: what the derive reads each field
/// with. An absent key reads as null, so an `Option` field may be left out.
pub fn field<T: MessageField>(v: &Value, kind: &str, key: &'static str) -> Result<T, MessageError> {
    T::from_value(v.get(key).unwrap_or(&Value::Null)).ok_or_else(|| MessageError::Field {
        kind: kind.to_string(),
        field: key,
    })
}

/// The `kind` of a map payload, for the derive's match.
pub fn kind_of(v: &Value) -> Result<&str, MessageError> {
    v.get_str("kind").ok_or(MessageError::NoKind)
}

impl MessageField for Value {
    fn to_value(self) -> Value {
        self
    }
    fn from_value(v: &Value) -> Option<Self> {
        Some(v.clone())
    }
}

impl MessageField for bool {
    fn to_value(self) -> Value {
        Value::Bool(self)
    }
    fn from_value(v: &Value) -> Option<Self> {
        v.as_bool()
    }
}

impl MessageField for String {
    fn to_value(self) -> Value {
        Value::Str(self)
    }
    fn from_value(v: &Value) -> Option<Self> {
        v.as_str().map(str::to_string)
    }
}

impl MessageField for f64 {
    fn to_value(self) -> Value {
        Value::Float(self)
    }
    fn from_value(v: &Value) -> Option<Self> {
        v.as_float()
    }
}

impl MessageField for f32 {
    fn to_value(self) -> Value {
        Value::float(self)
    }
    fn from_value(v: &Value) -> Option<Self> {
        v.as_float().map(|f| f as f32)
    }
}

/// The integers, through `Value::Int`, refusing one out of the type's range
/// rather than wrapping it.
macro_rules! int_fields {
    ($($t:ty),*) => {$(
        impl MessageField for $t {
            fn to_value(self) -> Value {
                Value::Int(i64::try_from(self).unwrap_or(i64::MAX))
            }
            fn from_value(v: &Value) -> Option<Self> {
                v.as_int().and_then(|n| <$t>::try_from(n).ok())
            }
        }
    )*};
}
int_fields!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl<T: MessageField> MessageField for Option<T> {
    fn to_value(self) -> Value {
        self.map_or(Value::Null, T::to_value)
    }
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Null => Some(None),
            v => T::from_value(v).map(Some),
        }
    }
}

impl<T: MessageField> MessageField for Vec<T> {
    fn to_value(self) -> Value {
        Value::List(self.into_iter().map(T::to_value).collect())
    }
    fn from_value(v: &Value) -> Option<Self> {
        v.as_list()?.iter().map(T::from_value).collect()
    }
}

impl<T: MessageField> MessageField for Box<T> {
    fn to_value(self) -> Value {
        (*self).to_value()
    }
    fn from_value(v: &Value) -> Option<Self> {
        T::from_value(v).map(Box::new)
    }
}

impl crate::input::UiEvent {
    /// The app's message this event carries, as `M`: the payload itself for
    /// a click (what `on_click` was handed), and otherwise the `tag` inside
    /// a core event — a drag, a change, a scroll, a drop, a layout — which
    /// is what `on_drag` and the rest were handed. `None` when neither is
    /// an `M`: a key press, a core event with no tag, another type's.
    ///
    /// The event's own fields stay on `payload` — a drag's `phase` and
    /// `x`/`y`, a change's `value` — so a handler matches on the message and
    /// reads those beside it.
    pub fn message<M>(&self) -> Option<M>
    where
        M: for<'a> TryFrom<&'a Value>,
    {
        M::try_from(&self.payload)
            .ok()
            .or_else(|| self.payload.get("tag").and_then(|t| M::try_from(t).ok()))
    }
}
