//! Dynamic values: the payload type for events crossing the host/extension
//! boundary. Everything in the IR that scripts can produce or consume is
//! expressible as a `Value`.

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
    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(s.into())
    }

    pub fn map(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
        Value::Map(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

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
}

/// How a binding spells the handles inside a readback — a node key, a
/// resource id — when a shape crosses as a [`Value`] (backlog AR1). The
/// shape is the core's; the spelling of a 64-bit handle is not, because
/// a JS number cannot hold one and a Lua integer can: Node writes sixteen
/// hex digits, the way its `key`/`font`/`sound` arguments already read,
/// and Lua writes the integer its `key` arguments already are. C reads
/// the structs and never sees a `Value`.
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
