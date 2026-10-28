//! Channels for Lua (ADR 0065): `channel { name = {...}, ms = {...} }`
//! makes one, a handle in props, params or an event arrives as the same
//! userdata, and a script reads it by index — `#ch`, `ch:get(i, "ms")`,
//! `ch:row(i)` — without the rows ever becoming Lua tables, and
//! `ch:reduce(col, from, to, n)` buckets a range for a chart in Rust
//! (backlog C95), `ch:append(spec, cap)` the next revision with more
//! rows (backlog C147), and `ch:filter(col, match, rows)`, `ch:sort(col,
//! descending, rows)` and `ch:take(rows)` a view built in Rust (backlog
//! C148). Row indices are 0-based, as a list's `index` and an `on_range`
//! event's are.
//!
//! [`from_lua`] and [`to_lua`] are `kui_value::lua`'s conversions with a
//! channel crossing as its handle, both ways.

use kui_value::Value as KValue;

use crate::{self as channel, Cell, Channel, Column};
use mlua::{Lua, MetaMethod, Table, UserData, UserDataFields, UserDataMethods, Value};

/// The userdata: one revision of a channel, held.
#[derive(Clone)]
pub struct LuaChannel(pub Channel);

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
        m.add_method(
            "lanes",
            |_, this, (start, len, lane): (Value, Value, Option<Value>)| {
                let s = column_index(&this.0, &start)?;
                let l = (!len.is_nil())
                    .then(|| column_index(&this.0, &len))
                    .transpose()?;
                let d = lane
                    .filter(|v| !v.is_nil())
                    .map(|c| column_index(&this.0, &c))
                    .transpose()?;
                this.0
                    .lanes(s, l, d)
                    .map(|lanes| LuaLanes(lanes, std::cell::RefCell::new(Vec::new())))
                    .map_err(|e| mlua::Error::runtime(format!("lanes: {e}")))
            },
        );
        m.add_method("append", |_, this, (spec, cap): (Table, Option<usize>)| {
            let cols = columns_of(&spec)?;
            this.0
                .append(cols, cap)
                .map(LuaChannel)
                .map_err(|e| mlua::Error::runtime(format!("append: {e}")))
        });
        m.add_method(
            "filter",
            |lua, this, (col, m, within): (Value, Table, Option<Table>)| {
                let c = column_index(&this.0, &col)?;
                let m = match_of(&m)?;
                let within = within.map(|t| rows_of(&t)).transpose()?;
                let rows = this
                    .0
                    .filter(c, &m, within.as_deref())
                    .map_err(|e| mlua::Error::runtime(format!("filter: {e}")))?;
                lua.create_sequence_from(rows)
            },
        );
        m.add_method(
            "sort",
            |lua, this, (col, descending, within): (Value, Option<bool>, Option<Table>)| {
                let c = column_index(&this.0, &col)?;
                let within = within.map(|t| rows_of(&t)).transpose()?;
                let rows = this
                    .0
                    .sort(c, descending.unwrap_or(false), within.as_deref())
                    .map_err(|e| mlua::Error::runtime(format!("sort: {e}")))?;
                lua.create_sequence_from(rows)
            },
        );
        m.add_method("take", |_, this, rows: Table| {
            this.0
                .take(&rows_of(&rows)?)
                .map(LuaChannel)
                .map_err(|e| mlua::Error::runtime(format!("take: {e}")))
        });
        m.add_method("publish", |_, this, spec: Table| {
            let cols = columns_of(&spec)?;
            this.0
                .publish(cols)
                .map(LuaChannel)
                .map_err(|e| mlua::Error::runtime(format!("publish: {e}")))
        });
    }
}

/// A channel's intervals on lanes (`ch:lanes(start, len, lane)`, ADR
/// 0075): `#lanes` lanes, `lanes.lo` / `lanes.hi` / `lanes.len`, and
/// `lanes:window(lane, v0, v1, min)` the runs of a 0-based lane reaching
/// into the window at the detail where `min` is a run's least width — a
/// list per field, a run per index: `s`, `e`, `row` (0-based; -1 for a
/// strip of several) and `n`.
pub struct LuaLanes(pub crate::Lanes, std::cell::RefCell<Vec<crate::Run>>);

impl UserData for LuaLanes {
    fn add_fields<F: UserDataFields<Self>>(f: &mut F) {
        f.add_field_method_get("lo", |_, this| Ok(this.0.extent().0));
        f.add_field_method_get("hi", |_, this| Ok(this.0.extent().1));
        f.add_field_method_get("len", |_, this| Ok(this.0.len()));
    }

    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Len, |_, this, ()| Ok(this.0.count()));
        m.add_method(
            "window",
            |lua, this, (lane, v0, v1, min): (usize, f64, f64, f64)| {
                let mut runs = this.1.borrow_mut();
                this.0.window(lane, v0, v1, min, &mut runs);
                let t = lua.create_table_with_capacity(0, 4)?;
                let list =
                    |f: &dyn Fn(&crate::Run) -> Value| lua.create_sequence_from(runs.iter().map(f));
                t.raw_set("s", list(&|r| Value::Number(r.start))?)?;
                t.raw_set("e", list(&|r| Value::Number(r.end))?)?;
                t.raw_set(
                    "row",
                    list(&|r| Value::Integer(r.row.map_or(-1, |x| x as i64)))?,
                )?;
                t.raw_set("n", list(&|r| Value::Integer(r.count as i64))?)?;
                Ok(t)
            },
        );
    }
}

/// A list of 0-based rows, as `filter` and `sort` hand them out.
fn rows_of(t: &Table) -> mlua::Result<Vec<usize>> {
    t.sequence_values::<Value>()
        .map(|v| row_index(&v?))
        .collect()
}

/// A filter's match as a script spells it (backlog C148): `{ eq = v }`,
/// `{ contains = "api", case = true }`, or `{ min = 1, max = 5 }` with
/// either end left out.
fn match_of(t: &Table) -> mlua::Result<channel::Match> {
    if let Some(text) = t.get::<Option<String>>("contains")? {
        return Ok(channel::Match::Contains {
            text,
            case: t.get::<Option<bool>>("case")?.unwrap_or(false),
        });
    }
    let eq: Value = t.get("eq")?;
    if !eq.is_nil() {
        return Ok(channel::Match::Eq(kui_value::lua::from_lua(&eq)?));
    }
    let (min, max) = (t.get::<Option<f64>>("min")?, t.get::<Option<f64>>("max")?);
    if min.is_some() || max.is_some() {
        return Ok(channel::Match::Range { min, max });
    }
    Err(mlua::Error::runtime(
        "a filter is { eq = v }, { contains = text, case = bool } or { min = a, max = b }",
    ))
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
        let column = column_from(&list)?
            .map_err(|e| mlua::Error::runtime(format!("channel column `{name}`: {e}")))?;
        cols.push((name, column));
    }
    cols.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(cols)
}

/// One list as a column, straight from the Lua values: what
/// `channel::column_of` makes of the same list as `Value`s, without a
/// `Value` — and a `String` — a cell on the way. A gallery's log, 9,000
/// lines republished every 400 ms, spent half its main thread making
/// them (backlog C143).
fn column_from(list: &Table) -> mlua::Result<Result<Column, channel::ChannelError>> {
    enum Build {
        Empty,
        Ints(Vec<i64>),
        Floats(Vec<f64>),
        Bools(Vec<bool>),
        Strs(channel::Strs),
    }
    let kind = |v: &Value| match v {
        Value::Nil => "nothing",
        Value::Boolean(_) => "a boolean",
        Value::Integer(_) | Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Table(_) => "a list",
        _ => "a value",
    };
    let bad = |i: usize, v: &Value, want: &str| {
        Ok(Err(channel::ChannelError(format!(
            "row {} is {}, in a column of {want}s",
            i + 1,
            kind(v)
        ))))
    };
    let mut b = Build::Empty;
    for (i, v) in list.sequence_values::<Value>().enumerate() {
        let v = v?;
        b = match (b, &v) {
            (Build::Empty, Value::Integer(x)) => Build::Ints(vec![*x]),
            (Build::Empty, Value::Number(x)) => Build::Floats(vec![*x]),
            (Build::Empty, Value::Boolean(x)) => Build::Bools(vec![*x]),
            (Build::Empty, Value::String(x)) => {
                let mut out = channel::Strs::default();
                out.push(&x.to_str()?);
                Build::Strs(out)
            }
            (Build::Empty, _) => return bad(0, &v, "number, boolean or string"),
            (Build::Ints(mut out), Value::Integer(x)) => {
                out.push(*x);
                Build::Ints(out)
            }
            (Build::Ints(out), Value::Number(x)) => {
                let mut f: Vec<f64> = out.into_iter().map(|n| n as f64).collect();
                f.push(*x);
                Build::Floats(f)
            }
            (Build::Floats(mut out), Value::Integer(x)) => {
                out.push(*x as f64);
                Build::Floats(out)
            }
            (Build::Floats(mut out), Value::Number(x)) => {
                out.push(*x);
                Build::Floats(out)
            }
            (Build::Ints(_) | Build::Floats(_), _) => return bad(i, &v, "number"),
            (Build::Bools(mut out), Value::Boolean(x)) => {
                out.push(*x);
                Build::Bools(out)
            }
            (Build::Bools(_), _) => return bad(i, &v, "boolean"),
            (Build::Strs(mut out), Value::String(x)) => {
                out.push(&x.to_str()?);
                Build::Strs(out)
            }
            (Build::Strs(_), _) => return bad(i, &v, "string"),
        };
    }
    Ok(Ok(match b {
        Build::Empty => Column::F64(Vec::new()),
        Build::Ints(v) => Column::I64(v),
        Build::Floats(v) => Column::F64(v),
        Build::Bools(v) => Column::Bool(v),
        Build::Strs(v) => Column::Str(v),
    }))
}

/// Registers `channel(spec)` in a script's globals.
pub fn register(lua: &Lua) -> mlua::Result<()> {
    let make = lua.create_function(|_, spec: Table| {
        Channel::table(columns_of(&spec)?)
            .map(LuaChannel)
            .map_err(|e| mlua::Error::runtime(format!("channel: {e}")))
    })?;
    lua.globals().set("channel", make)
}

/// A handle as the script sees it: the channel when someone holds it,
/// nil when nobody does.
pub fn handle_to_lua(lua: &Lua, v: &KValue) -> Option<mlua::Result<Value>> {
    if !channel::is_handle(v) {
        return None;
    }
    Some(match Channel::resolve(v) {
        Some(ch) => lua.create_userdata(LuaChannel(ch)).map(Value::UserData),
        None => Ok(Value::Nil),
    })
}

/// A userdata as a value, when it is a channel: its handle.
pub fn userdata_value(ud: &mlua::AnyUserData) -> Option<mlua::Result<KValue>> {
    ud.is::<LuaChannel>()
        .then(|| ud.borrow::<LuaChannel>().map(|c| c.0.handle()))
}

/// A Lua value as a [`KValue`], a channel as its handle
/// ([`kui_value::lua::from_lua`] with [`userdata_value`]).
pub fn from_lua(v: &Value) -> mlua::Result<KValue> {
    kui_value::lua::from_lua_with(v, &userdata_value)
}

/// A [`KValue`] as a Lua value, a channel's handle as the channel
/// ([`kui_value::lua::to_lua`] with [`handle_to_lua`]).
pub fn to_lua(lua: &Lua, v: &KValue) -> mlua::Result<Value> {
    kui_value::lua::to_lua_with(lua, v, &handle_to_lua)
}
