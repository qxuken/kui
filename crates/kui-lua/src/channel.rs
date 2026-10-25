//! Channels for Lua (ADR 0065): `channel { name = {...}, ms = {...} }`
//! makes one, a handle in props, params or an event arrives as the same
//! userdata, and a script reads it by index — `#ch`, `ch:get(i, "ms")`,
//! `ch:row(i)` — without the rows ever becoming Lua tables, and
//! `ch:reduce(col, from, to, n)` buckets a range for a chart in Rust
//! (backlog C95). Row indices are 0-based, as a list's `index` and an
//! `on_range` event's are.

use kui_core::Value as KValue;
use kui_core::channel::{self, Cell, Channel, Column};
use mlua::{Lua, MetaMethod, Table, UserData, UserDataFields, UserDataMethods, Value};

/// The userdata: one revision of a channel, held.
#[derive(Clone)]
pub(crate) struct LuaChannel(pub(crate) Channel);

fn cell_to_lua(lua: &Lua, c: Cell<'_>) -> mlua::Result<Value> {
    Ok(match c {
        Cell::Null => Value::Nil,
        Cell::F64(x) => Value::Number(x),
        Cell::I64(x) => Value::Integer(x),
        Cell::Bool(x) => Value::Boolean(x),
        Cell::Str(s) => Value::String(lua.create_string(s)?),
    })
}

/// A column named or numbered (0-based) as a script can.
fn column_index(ch: &Channel, col: &Value) -> mlua::Result<usize> {
    match col {
        Value::String(s) => {
            let name = s.to_str()?;
            ch.col(&name).ok_or_else(|| {
                mlua::Error::runtime(format!(
                    "the channel has no column {:?}; it has {}",
                    &*name,
                    ch.names().join(", ")
                ))
            })
        }
        Value::Integer(i) if *i >= 0 => Ok(*i as usize),
        other => Err(mlua::Error::runtime(format!(
            "a channel's column is a name or a 0-based number, not {}",
            other.type_name()
        ))),
    }
}

fn row_index(i: &Value) -> mlua::Result<usize> {
    match i {
        Value::Integer(i) if *i >= 0 => Ok(*i as usize),
        Value::Number(n) if *n >= 0.0 && n.is_finite() => Ok(*n as usize),
        other => Err(mlua::Error::runtime(format!(
            "a channel's row is a 0-based number, not {}",
            other.type_name()
        ))),
    }
}

impl UserData for LuaChannel {
    fn add_fields<F: UserDataFields<Self>>(f: &mut F) {
        f.add_field_method_get("rev", |_, this| Ok(this.0.rev()));
        f.add_field_method_get("id", |_, this| Ok(this.0.id()));
        f.add_field_method_get("names", |lua, this| {
            lua.create_sequence_from(this.0.names().iter().map(String::as_str))
        });
    }

    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Len, |_, this, ()| Ok(this.0.len()));
        m.add_meta_method(MetaMethod::Eq, |_, this, other: mlua::AnyUserData| {
            Ok(other.borrow::<LuaChannel>().is_ok_and(|o| o.0 == this.0))
        });
        m.add_method("get", |lua, this, (row, col): (Value, Value)| {
            let c = column_index(&this.0, &col)?;
            cell_to_lua(lua, this.0.get(row_index(&row)?, c))
        });
        m.add_method("row", |lua, this, row: Value| {
            let i = row_index(&row)?;
            let t = lua.create_table_with_capacity(0, this.0.names().len())?;
            for (c, name) in this.0.names().iter().enumerate() {
                t.raw_set(name.as_str(), cell_to_lua(lua, this.0.get(i, c))?)?;
            }
            Ok(t)
        });
        m.add_method("column", |lua, this, col: Value| {
            let c = column_index(&this.0, &col)?;
            let column = this.0.column(c).expect("an index column_index answered");
            let t = lua.create_table_with_capacity(column.len(), 0)?;
            for i in 0..column.len() {
                t.raw_set(i + 1, cell_to_lua(lua, column.get(i))?)?;
            }
            Ok(t)
        });
        m.add_method(
            "reduce",
            |lua, this, (col, from, to, n): (Value, Value, Value, Value)| {
                let c = column_index(&this.0, &col)?;
                let buckets = this
                    .0
                    .reduce(c, row_index(&from)?, row_index(&to)?, row_index(&n)?)
                    .map_err(|e| mlua::Error::runtime(format!("reduce: {e}")))?;
                reduction_to_lua(lua, &buckets)
            },
        );
        m.add_method("publish", |_, this, spec: Table| {
            let cols = columns_of(&spec)?;
            this.0
                .publish(cols)
                .map(LuaChannel)
                .map_err(|e| mlua::Error::runtime(format!("publish: {e}")))
        });
    }
}

/// A reduction as a script reads it (backlog C95): one flat list per
/// field, a bucket per index — `r.min[i]`, `r.max_row[i]` — rather than a
/// table per bucket, so 720 buckets are eight tables and not 720, and the
/// row fields 0-based as every row a channel names is.
fn reduction_to_lua(lua: &Lua, buckets: &[channel::Bucket]) -> mlua::Result<Table> {
    let t = lua.create_table_with_capacity(0, 8)?;
    let field = |name: &str, get: &dyn Fn(&channel::Bucket) -> Value| -> mlua::Result<()> {
        t.raw_set(name, lua.create_sequence_from(buckets.iter().map(get))?)
    };
    field("min", &|b| Value::Number(b.min))?;
    field("max", &|b| Value::Number(b.max))?;
    field("first", &|b| Value::Number(b.first))?;
    field("last", &|b| Value::Number(b.last))?;
    field("min_row", &|b| Value::Integer(b.min_row as i64))?;
    field("max_row", &|b| Value::Integer(b.max_row as i64))?;
    field("first_row", &|b| Value::Integer(b.first_row as i64))?;
    field("last_row", &|b| Value::Integer(b.last_row as i64))?;
    Ok(t)
}

/// A spec's columns: each key a name, each value a list of numbers,
/// booleans or strings, all of one kind, the columns in name order.
fn columns_of(spec: &Table) -> mlua::Result<Vec<(String, Column)>> {
    let mut cols = Vec::new();
    for pair in spec.pairs::<String, Table>() {
        let (name, list) = pair?;
        let values = list
            .sequence_values::<Value>()
            .map(|v| v.and_then(|v| crate::lua_to_value(&v)))
            .collect::<mlua::Result<Vec<KValue>>>()?;
        let column = channel::column_of(&values)
            .map_err(|e| mlua::Error::runtime(format!("channel column `{name}`: {e}")))?;
        cols.push((name, column));
    }
    cols.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(cols)
}

/// Registers `channel(spec)` in a script's globals.
pub(crate) fn register(lua: &Lua) -> mlua::Result<()> {
    let make = lua.create_function(|_, spec: Table| {
        Channel::table(columns_of(&spec)?)
            .map(LuaChannel)
            .map_err(|e| mlua::Error::runtime(format!("channel: {e}")))
    })?;
    lua.globals().set("channel", make)
}

/// A handle as the script sees it: the channel when someone holds it,
/// nil when nobody does.
pub(crate) fn handle_to_lua(lua: &Lua, v: &KValue) -> Option<mlua::Result<Value>> {
    if !channel::is_handle(v) {
        return None;
    }
    Some(match Channel::resolve(v) {
        Some(ch) => lua.create_userdata(LuaChannel(ch)).map(Value::UserData),
        None => Ok(Value::Nil),
    })
}
