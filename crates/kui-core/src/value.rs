//! [`Value`]: the plain-data payload type for everything that crosses an
//! event or binding boundary.
//!
//! A click payload, a drag event's fields, a readback for a script: all of
//! them are a `Value`, so Rust, Lua, Node and C see one shape. A Rust app
//! that wants an exhaustive `match` over its own payloads puts a typed
//! enum on top with [`crate::message`].
//!
//! ```rust
//! use kui_core::Value;
//!
//! let v = Value::map([
//!     ("kind", Value::str("drag")),
//!     ("x", Value::from(1.5f32)),
//!     ("n", Value::from(4usize)),
//!     ("on", true.into()),
//! ]);
//! assert_eq!(v.get_str("kind"), Some("drag"));
//! assert_eq!(v.get_f32("x"), Some(1.5));
//! assert_eq!(v.get_float("n"), Some(4.0)); // an int reads as a float
//! assert_eq!(v.get_str("x"), None); // the wrong type is None
//! assert_eq!(Value::from("save"), Value::Str("save".into()));
//! ```

/// A dynamically typed value: null, bool, int, float, string, list or an
/// ordered string-keyed map. See the [module docs](self) for an example.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

impl Value {
    /// A string value.
    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(s.into())
    }

    /// A map from `(key, value)` pairs, in the order given.
    pub fn map(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
        Value::Map(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    /// The entry under `key` of a map; `None` for a missing key or a
    /// value that is not a map.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The number as an integer; a float is truncated.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    /// The number as a float; an int converts.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// What kind of value this is, for an error message.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "nil",
            Value::Bool(_) => "boolean",
            Value::Int(_) | Value::Float(_) => "number",
            Value::Str(_) => "string",
            Value::List(_) => "list",
            Value::Map(_) => "map",
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// `get(key)` then `as_str`: a map entry read as a string.
    #[inline]
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// `get(key)` then `as_int`.
    #[inline]
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Value::as_int)
    }

    /// `get(key)` then `as_float`.
    #[inline]
    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::as_float)
    }

    /// `get(key)` then `as_float`, as the `f32` geometry is in — a drag's
    /// `x`, a scroll's `dy`.
    #[inline]
    pub fn get_f32(&self, key: &str) -> Option<f32> {
        self.get_float(key).map(|v| v as f32)
    }

    /// `get(key)` then `as_bool`.
    #[inline]
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }
}

/// How a binding spells the handles inside a readback (a node key, a
/// resource id) when a shape crosses as a [`Value`]. A JS number cannot
/// hold a 64-bit handle and a Lua integer can, so Node writes sixteen hex
/// digits ([`Handles::HEX`]) and Lua the integer itself ([`Handles::INT`]).
#[derive(Clone, Copy)]
pub struct Handles {
    pub key: fn(crate::key::Key) -> Value,
    pub id: fn(u64) -> Value,
}

impl Handles {
    /// A handle as the integer it is — Lua's spelling, and the one
    /// `schema::ENV_FIELDS` uses for a focus key.
    pub const INT: Handles = Handles {
        key: |k| Value::Int(k.0 as i64),
        id: |id| Value::Int(id as i64),
    };
    /// A handle as sixteen hex digits — Node's spelling.
    pub const HEX: Handles = Handles {
        key: |k| Value::Str(format!("{:016x}", k.0)),
        id: |id| Value::Str(format!("{id:016x}")),
    };

    pub fn opt_key(&self, k: Option<crate::key::Key>) -> Value {
        k.map_or(Value::Null, self.key)
    }
}

impl Value {
    /// A float, as a readback spells one.
    pub fn float(v: f32) -> Value {
        Value::Float(v as f64)
    }

    /// An `Option` as the value or `Null`: a readback keeps every key,
    /// so a reader destructures a stable shape.
    pub fn opt<T>(v: Option<T>, f: impl FnOnce(T) -> Value) -> Value {
        v.map_or(Value::Null, f)
    }

    pub fn opt_str(s: &Option<String>) -> Value {
        Value::opt(s.as_ref(), |s| Value::Str(s.clone()))
    }

    pub fn opt_float(v: Option<f32>) -> Value {
        Value::opt(v, Value::float)
    }

    pub fn opt_usize(v: Option<usize>) -> Value {
        Value::opt(v, |v| Value::Int(v as i64))
    }

    pub fn opt_bool(v: Option<bool>) -> Value {
        Value::opt(v, Value::Bool)
    }

    pub fn list(items: impl IntoIterator<Item = Value>) -> Value {
        Value::List(items.into_iter().collect())
    }

    pub fn floats(items: &[f32]) -> Value {
        Value::list(items.iter().map(|v| Value::float(*v)))
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Int(v.into())
    }
}

impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Value::Int(v.into())
    }
}

/// An index or a count. One past `i64::MAX` cannot be a payload's number,
/// so it saturates there rather than wrapping negative.
impl From<usize> for Value {
    fn from(v: usize) -> Self {
        Value::Int(i64::try_from(v).unwrap_or(i64::MAX))
    }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::float(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_get_finds_entries() {
        let v = Value::map([("kind", Value::str("inc")), ("by", Value::Int(2))]);
        assert_eq!(v.get("kind").and_then(Value::as_str), Some("inc"));
        assert_eq!(v.get("by").and_then(Value::as_int), Some(2));
        assert_eq!(v.get("missing"), None);
    }

    #[test]
    fn numeric_coercions() {
        assert_eq!(Value::Int(3).as_float(), Some(3.0));
        assert_eq!(Value::Float(3.7).as_int(), Some(3));
        assert_eq!(Value::str("x").as_int(), None);
    }
}
