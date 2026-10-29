//! `kui.sqlite`: a database on a thread of its own (rusqlite, SQLite
//! bundled). `db:query(sql, params)` answers rows as tables by column name,
//! `db:exec(sql, params)` the change count and last row id, each an
//! operation, so a task waits on it without blocking the frame.
//! `db:transaction(fn)` is Lua over `exec` (lua/runtime.lua).

use std::sync::mpsc;

use kui_core::Value;
use mlua::{Lua, Table, UserData, UserDataMethods};

use crate::ops::Ops;

use super::{fail, shared, tried, try_fn};

type Job = Box<dyn FnOnce(&mut rusqlite::Connection) + Send>;

struct Db {
    jobs: mpsc::Sender<Job>,
    ops: Ops,
}

fn to_sql(v: &Value) -> rusqlite::types::Value {
    use rusqlite::types::Value as S;
    match v {
        Value::Null => S::Null,
        Value::Bool(b) => S::Integer(*b as i64),
        Value::Int(i) => S::Integer(*i),
        Value::Float(f) => S::Real(*f),
        Value::Str(s) => S::Text(s.clone()),
        other => {
            let mut out = String::new();
            super::formats::write_json(&mut out, other, false, 0);
            S::Text(out)
        }
    }
}

fn from_sql(v: rusqlite::types::ValueRef<'_>) -> Value {
    use rusqlite::types::ValueRef as R;
    match v {
        R::Null => Value::Null,
        R::Integer(i) => Value::Int(i),
        R::Real(f) => Value::Float(f),
        R::Text(t) => Value::Str(String::from_utf8_lossy(t).into_owned()),
        R::Blob(b) => Value::Str(String::from_utf8_lossy(b).into_owned()),
    }
}

/// Binds `params`: a list for `?`, a map for `:name` (with or without the
/// colon).
fn bind(stmt: &mut rusqlite::Statement<'_>, params: &Value) -> rusqlite::Result<()> {
    match params {
        Value::List(items) => {
            for (i, v) in items.iter().enumerate() {
                stmt.raw_bind_parameter(i + 1, to_sql(v))?;
            }
        }
        Value::Map(entries) => {
            for (k, v) in entries {
                let name = if k.starts_with([':', '@', '$']) {
                    k.clone()
                } else {
                    format!(":{k}")
                };
                if let Some(i) = stmt.parameter_index(&name)? {
                    stmt.raw_bind_parameter(i, to_sql(v))?;
                }
            }
        }
        Value::Null => {}
        single => stmt.raw_bind_parameter(1, to_sql(single))?,
    }
    Ok(())
}

/// A database's error with its code: `constraint`, `busy` (another
/// connection holds the lock), `sql` (the statement is wrong: its syntax,
/// a table or column that is not there), `denied`, `unavailable` (the
/// file cannot be opened), `invalid` (not a database, a parameter that
/// does not fit), `full`, `too_long`, else `failed`.
fn sql_err(e: rusqlite::Error) -> String {
    use rusqlite::ErrorCode as C;
    let code = match e.sqlite_error_code() {
        Some(C::ConstraintViolation) => "constraint",
        Some(C::DatabaseBusy | C::DatabaseLocked) => "busy",
        Some(C::Unknown) => "sql",
        Some(C::PermissionDenied | C::ReadOnly | C::AuthorizationForStatementDenied) => "denied",
        Some(C::CannotOpen) => "unavailable",
        Some(C::NotADatabase | C::DatabaseCorrupt | C::TypeMismatch | C::ParameterOutOfRange) => {
            "invalid"
        }
        Some(C::DiskFull) => "full",
        Some(C::TooBig) => "too_long",
        Some(_) => "failed",
        None => match e {
            rusqlite::Error::SqlInputError { .. } => "sql",
            rusqlite::Error::InvalidParameterName(_)
            | rusqlite::Error::InvalidParameterCount(..)
            | rusqlite::Error::InvalidColumnType(..)
            | rusqlite::Error::InvalidColumnIndex(_)
            | rusqlite::Error::InvalidColumnName(_)
            | rusqlite::Error::MultipleStatement => "invalid",
            _ => "failed",
        },
    };
    super::coded(code, e)
}

fn query(conn: &mut rusqlite::Connection, sql: &str, params: &Value) -> Result<Value, String> {
    let mut stmt = conn.prepare_cached(sql).map_err(sql_err)?;
    bind(&mut stmt, params).map_err(sql_err)?;
    let names: Vec<String> = stmt
        .column_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    let mut rows = stmt.raw_query();
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(sql_err)? {
        let mut m = Vec::with_capacity(names.len());
        for (i, n) in names.iter().enumerate() {
            m.push((n.clone(), from_sql(row.get_ref(i).map_err(sql_err)?)));
        }
        out.push(Value::Map(m));
    }
    Ok(Value::List(out))
}

fn exec(conn: &mut rusqlite::Connection, sql: &str, params: &Value) -> Result<Value, String> {
    let changes = if matches!(params, Value::Null) && sql.contains(';') {
        // Several statements and nothing to bind: a schema, a migration.
        conn.execute_batch(sql).map_err(sql_err)?;
        conn.changes() as i64
    } else {
        let mut stmt = conn.prepare_cached(sql).map_err(sql_err)?;
        bind(&mut stmt, params).map_err(sql_err)?;
        stmt.raw_execute().map_err(sql_err)? as i64
    };
    Ok(Value::map([
        ("changes", Value::Int(changes)),
        ("last_id", Value::Int(conn.last_insert_rowid())),
    ]))
}

impl UserData for Db {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        for (name, write) in [("__try_query", false), ("__try_exec", true)] {
            m.add_method(
                name,
                move |lua, this, (sql, params): (String, Option<mlua::Value>)| {
                    let start = || {
                        let params = match params {
                            Some(p) => kui_lua::lua_to_value(&p)?,
                            None => Value::Null,
                        };
                        let id = this.ops.id();
                        let ops = this.ops.clone();
                        this.jobs
                            .send(Box::new(move |conn| {
                                let a = if write {
                                    exec(conn, &sql, &params)
                                } else {
                                    query(conn, &sql, &params)
                                };
                                ops.answer(id, a.map_err(|e| format!("{e} (in: {sql})")));
                            }))
                            .map_err(|_| super::fail_as("closed", "the database is closed"))?;
                        Ok(id)
                    };
                    tried(lua, start())
                },
            );
        }
    }
}

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let sqlite = lua.create_table()?;
    sqlite.set(
        "try_open",
        try_fn(lua, |lua, path: Option<String>| {
            let s = shared(lua);
            let target = path.as_deref().unwrap_or(":memory:");
            let conn = if target == ":memory:" {
                rusqlite::Connection::open_in_memory()
            } else {
                rusqlite::Connection::open(s.path(target))
            }
            .map_err(|e| {
                fail(sql_err(e).replacen("] ", &format!("] sqlite.open {target}: "), 1))
            })?;
            let _ = conn.pragma_update(None, "journal_mode", "WAL");
            let (tx, rx) = mpsc::channel::<Job>();
            std::thread::Builder::new()
                .name("kui-sqlite".into())
                .spawn(move || {
                    let mut conn = conn;
                    for job in rx {
                        job(&mut conn);
                    }
                })
                .map_err(|e| fail(format!("sqlite.open: {e}")))?;
            Ok(Db {
                jobs: tx,
                ops: s.ops.clone(),
            })
        })?,
    )?;
    kui.set("sqlite", sqlite)?;
    Ok(())
}
