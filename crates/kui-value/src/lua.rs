//! A [`Value`] to and from an `mlua` value, with the `lua` feature.
//!
//! A table with sequence entries is a [`Value::List`], any other table a
//! [`Value::Map`] with string keys; nil, booleans, integers, floats and
//! strings map one to one. An empty table is a map unless it is marked a
//! list ([`mark_array`]), which an empty list converted the other way is,
//! so it round-trips. A function is refused; a userdata is refused unless
//! the caller's hook knows it — `kui-channel`'s knows a channel, which
//! crosses as its handle (ADR 0065).
//!
//! ```
//! use kui_value::{Value, lua};
//!
//! let state = mlua::Lua::new();
//! let v: mlua::Value = state.load(r#"{ kind = "toggle", index = 2 }"#).eval()?;
//! let payload = lua::from_lua(&v)?;
//! assert_eq!(payload.get_str("kind"), Some("toggle"));
//! // A table's keys come back in Lua's order, not the map's.
//! let back = lua::from_lua(&lua::to_lua(&state, &payload)?)?;
//! assert_eq!((back.get_str("kind"), back.get_int("index")), (Some("toggle"), Some(2)));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use mlua::{AnyUserData, Lua, Table};

use crate::Value;

/// How deep a Lua value may nest before [`from_lua`] refuses it. A table
/// holding itself, or one nested deeper than this, is an error rather than
/// a stack overflow.
pub const MAX_DEPTH: usize = 64;

/// What a userdata is as a value, for a caller that knows some: `None`
/// for one it does not, which is refused.
pub type FromUserData<'a> = &'a dyn Fn(&AnyUserData) -> Option<mlua::Result<Value>>;

/// What a map is in Lua, for a caller that hands some maps over as
/// something else (a channel's handle as the channel): `None` for an
/// ordinary map, made a table.
pub type ToLuaMap<'a> = &'a dyn Fn(&Lua, &Value) -> Option<mlua::Result<mlua::Value>>;

/// Tables nested this deep before [`Path`] records which they are. No
/// payload a view means is this deep, and a table that holds itself passes
/// it and is caught a lap of its cycle later: asking each table its
/// identity costs a size table per row per frame 57 ns, +2.7% on kui-lua's
/// `lua_1000_rows/table per frame`.
const TRACKED_PAST: usize = 8;

/// How deep the conversion is, and past [`TRACKED_PAST`] the tables it is
/// inside.
struct Path {
    depth: usize,
    seen: Vec<*const std::ffi::c_void>,
}

fn bad(msg: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::runtime(msg.to_string())
}

/// A Lua value as a [`Value`]; a function or a userdata is refused.
pub fn from_lua(v: &mlua::Value) -> mlua::Result<Value> {
    from_lua_with(v, &|_| None)
}

/// [`from_lua`], with `userdata` saying what the userdata it knows are.
pub fn from_lua_with(v: &mlua::Value, userdata: FromUserData<'_>) -> mlua::Result<Value> {
    convert(
        v,
        &mut Path {
            depth: 0,
            seen: Vec::new(),
        },
        userdata,
    )
}

/// A table met again on its own path holds itself. A table met twice off
/// the path (the same list under two keys) is copied twice.
fn convert(v: &mlua::Value, path: &mut Path, userdata: FromUserData<'_>) -> mlua::Result<Value> {
    Ok(match v {
        mlua::Value::Nil => Value::Null,
        mlua::Value::Boolean(b) => Value::Bool(*b),
        mlua::Value::Integer(i) => Value::Int(*i),
        mlua::Value::Number(n) => Value::Float(*n),
        mlua::Value::String(s) => Value::Str(s.to_str()?.to_string()),
        mlua::Value::Table(t) => {
            let tracked = path.depth >= TRACKED_PAST;
            if tracked {
                let at = t.to_pointer();
                if path.seen.contains(&at) {
                    return Err(bad("a table that holds itself cannot be a value"));
                }
                path.seen.push(at);
            }
            if path.depth >= MAX_DEPTH {
                return Err(bad(format!("a value nested past {MAX_DEPTH} tables")));
            }
            path.depth += 1;
            let len = t.raw_len();
            let value = if len > 0 {
                let mut list = Vec::with_capacity(len);
                for item in t.sequence_values::<mlua::Value>() {
                    list.push(convert(&item?, path, userdata)?);
                }
                Value::List(list)
            } else {
                let mut map = Vec::new();
                for pair in t.pairs::<String, mlua::Value>() {
                    let (k, v) = pair?;
                    map.push((k, convert(&v, path, userdata)?));
                }
                if map.is_empty() && is_marked_array(t) {
                    Value::List(Vec::new())
                } else {
                    Value::Map(map)
                }
            };
            path.depth -= 1;
            if tracked {
                path.seen.pop();
            }
            value
        }
        mlua::Value::UserData(ud) => match userdata(ud) {
            Some(v) => v?,
            None => return Err(refused(v)),
        },
        other => return Err(refused(other)),
    })
}

fn refused(v: &mlua::Value) -> mlua::Error {
    mlua::Error::runtime(format!("cannot convert {} to event payload", v.type_name()))
}

/// A [`Value`] as a Lua value: the inverse of [`from_lua`].
pub fn to_lua(lua: &Lua, v: &Value) -> mlua::Result<mlua::Value> {
    to_lua_with(lua, v, &|_, _| None)
}

/// [`to_lua`], with `map` handing some maps over as something else.
pub fn to_lua_with(lua: &Lua, v: &Value, map: ToLuaMap<'_>) -> mlua::Result<mlua::Value> {
    Ok(match v {
        Value::Null => mlua::Value::Nil,
        Value::Bool(b) => mlua::Value::Boolean(*b),
        Value::Int(i) => mlua::Value::Integer(*i),
        Value::Float(f) => mlua::Value::Number(*f),
        Value::Str(s) => mlua::Value::String(lua.create_string(s)?),
        Value::List(items) => {
            let t = lua.create_table_with_capacity(items.len(), 0)?;
            for (i, item) in items.iter().enumerate() {
                t.set(i + 1, to_lua_with(lua, item, map)?)?;
            }
            // An empty list stays one on its way back.
            if items.is_empty() {
                mark_array(lua, &t)?;
            }
            mlua::Value::Table(t)
        }
        Value::Map(entries) => {
            if let Some(other) = map(lua, v) {
                return other;
            }
            let t = lua.create_table_with_capacity(0, entries.len())?;
            for (k, v) in entries {
                t.set(k.as_str(), to_lua_with(lua, v, map)?)?;
            }
            mlua::Value::Table(t)
        }
    })
}

/// The metatable that says an empty table is a list, one per state.
const ARRAY_MARK: &str = "kui.array";

/// Marks `t` a list, so that empty it converts to `[]` and not `{}`: the
/// one thing a Lua table cannot say for itself. An empty list converted
/// to Lua comes back marked, so it round-trips.
pub fn mark_array(lua: &Lua, t: &Table) -> mlua::Result<()> {
    let mt = match lua.named_registry_value::<Option<Table>>(ARRAY_MARK)? {
        Some(mt) => mt,
        None => {
            let mt = lua.create_table()?;
            mt.raw_set("__kui_array", true)?;
            lua.set_named_registry_value(ARRAY_MARK, &mt)?;
            mt
        }
    };
    if t.metatable().is_none() {
        t.set_metatable(Some(mt))?;
    }
    Ok(())
}

/// Whether `t` was marked a list ([`mark_array`]).
pub fn is_marked_array(t: &Table) -> bool {
    t.metatable()
        .is_some_and(|mt| mt.raw_get::<bool>("__kui_array").unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_round_trips_and_an_empty_list_stays_one() {
        let lua = Lua::new();
        let v = Value::map([
            ("kind", "inc".into()),
            ("by", Value::Int(2)),
            ("w", Value::List(vec![Value::Float(0.5)])),
            ("none", Value::List(Vec::new())),
            ("on", true.into()),
        ]);
        let back = from_lua(&to_lua(&lua, &v).unwrap()).unwrap();
        assert_eq!(back.get("none"), Some(&Value::List(Vec::new())));
        assert_eq!(back.get_int("by"), Some(2));
        assert_eq!(back.get_bool("on"), Some(true));
    }

    #[test]
    fn a_table_that_holds_itself_and_a_function_are_refused() {
        let lua = Lua::new();
        let r: mlua::Value = lua
            .load("local r = { x = 1 } r.r = r return r")
            .eval()
            .unwrap();
        let e = from_lua(&r).unwrap_err().to_string();
        assert!(e.contains("holds itself"), "{e}");
        let f: mlua::Value = lua.load("return function() end").eval().unwrap();
        assert!(from_lua(&f).is_err());
    }
}
