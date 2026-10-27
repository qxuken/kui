//! [`Value`], the plain-data payload, lives in `kui-value` (ADR 0075,
//! decision 3) and is re-exported here; what spells a node key inside one
//! is the core's, since a key is.

pub use kui_value::Value;

use crate::key::Key;

/// How a binding spells the handles inside a readback (a node key, a
/// resource id) when a shape crosses as a [`Value`]. A JS number cannot
/// hold a 64-bit handle and a Lua integer can, so Node writes sixteen hex
/// digits ([`Handles::HEX`]) and Lua the integer itself ([`Handles::INT`]).
#[derive(Clone, Copy)]
pub struct Handles {
    pub key: fn(Key) -> Value,
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

    pub fn opt_key(&self, k: Option<Key>) -> Value {
        k.map_or(Value::Null, self.key)
    }
}
