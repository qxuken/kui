//! A data channel (ADR 0065): bulk data the core owns and a handle names,
//! written once and read by index from anywhere that holds the handle — an
//! extension's fill, another Lua state, a worker thread — so a hundred
//! thousand rows cross into a component as one small value instead of a
//! copy every time they change.
//!
//! A channel is a table of named, typed columns of one length, or a run of
//! bytes. It is immutable per revision: [`Channel::publish`] makes the next
//! revision under the same identity, and a reader holding an older one
//! keeps it until it lets go. Its handle is a value —
//! `{"$channel": id, "rev": rev}` — so it rides in props, params and
//! messages like any other, and [`Channel::resolve`] turns one back into the
//! latest live revision of that identity.
//!
//! ```rust
//! use kui_core::channel::{Cell, Channel, Column};
//!
//! let ch = Channel::table(vec![
//!     ("name".into(), Column::strs(["build", "test", "deploy"])),
//!     ("ms".into(), Column::F64(vec![12.5, 30.0, 4.0])),
//! ])
//! .unwrap();
//! assert_eq!(ch.len(), 3);
//! let handle = ch.handle();
//! let again = Channel::resolve(&handle).unwrap();
//! let ms = again.col("ms").unwrap();
//! assert_eq!(again.get(1, ms), Cell::F64(30.0));
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, Weak};

use rustc_hash::FxHashMap;

use crate::value::Value;

/// The key a handle's identity rides under.
pub const HANDLE_KEY: &str = "$channel";
/// The key its revision rides under.
pub const REV_KEY: &str = "rev";

/// One column: every row's value of one field, all of one type.
#[derive(Clone, Debug, PartialEq)]
pub enum Column {
    F64(Vec<f64>),
    I64(Vec<i64>),
    Bool(Vec<bool>),
    Str(Strs),
}

impl Column {
    /// A string column from anything that yields strings.
    pub fn strs<S: AsRef<str>>(items: impl IntoIterator<Item = S>) -> Column {
        Column::Str(Strs::from_iter(items))
    }

    pub fn len(&self) -> usize {
        match self {
            Column::F64(v) => v.len(),
            Column::I64(v) => v.len(),
            Column::Bool(v) => v.len(),
            Column::Str(s) => s.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The column's type, as the bindings name it.
    pub fn kind(&self) -> &'static str {
        match self {
            Column::F64(_) => "number",
            Column::I64(_) => "integer",
            Column::Bool(_) => "boolean",
            Column::Str(_) => "string",
        }
    }

    /// Row `i`'s cell; `Cell::Null` past the end.
    pub fn get(&self, i: usize) -> Cell<'_> {
        match self {
            Column::F64(v) => v.get(i).map_or(Cell::Null, |x| Cell::F64(*x)),
            Column::I64(v) => v.get(i).map_or(Cell::Null, |x| Cell::I64(*x)),
            Column::Bool(v) => v.get(i).map_or(Cell::Null, |x| Cell::Bool(*x)),
            Column::Str(s) => s.get(i).map_or(Cell::Null, Cell::Str),
        }
    }
}

/// A string column: one buffer and where each string ends, so a hundred
/// thousand names are two allocations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Strs {
    bytes: String,
    ends: Vec<u32>,
}

impl Strs {
    pub fn push(&mut self, s: &str) {
        self.bytes.push_str(s);
        self.ends.push(self.bytes.len() as u32);
    }

    pub fn len(&self) -> usize {
        self.ends.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }

    pub fn get(&self, i: usize) -> Option<&str> {
        let end = *self.ends.get(i)? as usize;
        let start = if i == 0 { 0 } else { self.ends[i - 1] as usize };
        self.bytes.get(start..end)
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        (0..self.len()).filter_map(|i| self.get(i))
    }
}

impl<S: AsRef<str>> FromIterator<S> for Strs {
    fn from_iter<I: IntoIterator<Item = S>>(items: I) -> Self {
        let mut out = Strs::default();
        for s in items {
            out.push(s.as_ref());
        }
        out
    }
}

/// One cell, borrowed from its channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cell<'a> {
    Null,
    F64(f64),
    I64(i64),
    Bool(bool),
    Str(&'a str),
}

impl Cell<'_> {
    /// The cell as a value.
    pub fn to_value(self) -> Value {
        match self {
            Cell::Null => Value::Null,
            Cell::F64(x) => Value::Float(x),
            Cell::I64(x) => Value::Int(x),
            Cell::Bool(x) => Value::Bool(x),
            Cell::Str(s) => Value::str(s),
        }
    }

    /// A number cell as an `f64`.
    pub fn as_f64(self) -> Option<f64> {
        match self {
            Cell::F64(x) => Some(x),
            Cell::I64(x) => Some(x as f64),
            _ => None,
        }
    }
}

/// What a channel holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    /// Named columns of one length.
    Table {
        names: Vec<String>,
        cols: Vec<Column>,
    },
    /// A run of bytes.
    Bytes(Vec<u8>),
}

/// Why a channel could not be made.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelError(pub String);

impl std::fmt::Display for ChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ChannelError {}

struct Inner {
    id: u64,
    rev: u64,
    data: Data,
    len: usize,
}

/// One revision of a channel, held: cheap to clone, and the data stays as
/// long as some clone (here or in another binding) does.
#[derive(Clone)]
pub struct Channel(Arc<Inner>);

impl std::fmt::Debug for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channel")
            .field("id", &self.0.id)
            .field("rev", &self.0.rev)
            .field("len", &self.0.len)
            .finish()
    }
}

impl PartialEq for Channel {
    /// The same identity at the same revision: what a replay compares.
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id && self.0.rev == other.0.rev
    }
}

/// Every identity's latest revision, weakly: a channel nobody holds is
/// gone, and its handle resolves to nothing.
static REGISTRY: LazyLock<Mutex<FxHashMap<u64, Weak<Inner>>>> =
    LazyLock::new(|| Mutex::new(FxHashMap::default()));
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn registry() -> std::sync::MutexGuard<'static, FxHashMap<u64, Weak<Inner>>> {
    REGISTRY.lock().unwrap_or_else(|e| e.into_inner())
}

fn table_len(names: &[String], cols: &[Column]) -> Result<usize, ChannelError> {
    if names.len() != cols.len() {
        return Err(ChannelError(format!(
            "{} names for {} columns",
            names.len(),
            cols.len()
        )));
    }
    let mut seen = std::collections::HashSet::new();
    for n in names {
        if !seen.insert(n.as_str()) {
            return Err(ChannelError(format!("column `{n}` is named twice")));
        }
    }
    let len = cols.first().map_or(0, Column::len);
    if let Some((i, c)) = cols.iter().enumerate().find(|(_, c)| c.len() != len) {
        return Err(ChannelError(format!(
            "column `{}` has {} rows and `{}` has {len}: every column of a channel is one length",
            names[i],
            c.len(),
            names[0]
        )));
    }
    Ok(len)
}

impl Channel {
    fn register(id: u64, rev: u64, data: Data, len: usize) -> Channel {
        let inner = Arc::new(Inner { id, rev, data, len });
        let mut reg = registry();
        // A sweep of the gone, now and then: the map holds weak entries,
        // and one per identity ever made would grow with a busy writer.
        if reg.len() > 256 && reg.len().is_power_of_two() {
            reg.retain(|_, w| w.strong_count() > 0);
        }
        reg.insert(id, Arc::downgrade(&inner));
        Channel(inner)
    }

    /// A table of named columns of one length.
    pub fn table(cols: Vec<(String, Column)>) -> Result<Channel, ChannelError> {
        let (names, cols): (Vec<String>, Vec<Column>) = cols.into_iter().unzip();
        let len = table_len(&names, &cols)?;
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Ok(Self::register(id, 1, Data::Table { names, cols }, len))
    }

    /// A run of bytes.
    pub fn bytes(bytes: Vec<u8>) -> Channel {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let len = bytes.len();
        Self::register(id, 1, Data::Bytes(bytes), len)
    }

    /// The next revision under this identity, holding `cols`: a handle to
    /// the identity resolves to it from now on, and this revision stays
    /// readable for as long as it is held.
    pub fn publish(&self, cols: Vec<(String, Column)>) -> Result<Channel, ChannelError> {
        let (names, cols): (Vec<String>, Vec<Column>) = cols.into_iter().unzip();
        let len = table_len(&names, &cols)?;
        Ok(Self::register(
            self.0.id,
            self.0.rev + 1,
            Data::Table { names, cols },
            len,
        ))
    }

    /// [`Self::publish`] for bytes.
    pub fn publish_bytes(&self, bytes: Vec<u8>) -> Channel {
        let len = bytes.len();
        Self::register(self.0.id, self.0.rev + 1, Data::Bytes(bytes), len)
    }

    pub fn id(&self) -> u64 {
        self.0.id
    }

    pub fn rev(&self) -> u64 {
        self.0.rev
    }

    /// Rows for a table, bytes for bytes.
    pub fn len(&self) -> usize {
        self.0.len
    }

    pub fn is_empty(&self) -> bool {
        self.0.len == 0
    }

    pub fn data(&self) -> &Data {
        &self.0.data
    }

    /// The column names, in order; empty for bytes.
    pub fn names(&self) -> &[String] {
        match &self.0.data {
            Data::Table { names, .. } => names,
            Data::Bytes(_) => &[],
        }
    }

    /// The index of the column called `name`.
    pub fn col(&self, name: &str) -> Option<usize> {
        self.names().iter().position(|n| n == name)
    }

    /// The column at `i`.
    pub fn column(&self, i: usize) -> Option<&Column> {
        match &self.0.data {
            Data::Table { cols, .. } => cols.get(i),
            Data::Bytes(_) => None,
        }
    }

    /// The column called `name`.
    pub fn column_named(&self, name: &str) -> Option<&Column> {
        self.column(self.col(name)?)
    }

    /// Row `row` of column `col`; `Cell::Null` out of range.
    pub fn get(&self, row: usize, col: usize) -> Cell<'_> {
        self.column(col).map_or(Cell::Null, |c| c.get(row))
    }

    /// The bytes, for a bytes channel.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match &self.0.data {
            Data::Bytes(b) => Some(b),
            Data::Table { .. } => None,
        }
    }

    /// The handle: `{"$channel": id, "rev": rev}`, a value that rides
    /// anywhere a value does.
    pub fn handle(&self) -> Value {
        handle_value(self.0.id, self.0.rev)
    }

    /// The latest live revision of the identity `v` names, if `v` is a
    /// handle and some side still holds one.
    pub fn resolve(v: &Value) -> Option<Channel> {
        let (id, _) = handle_of(v)?;
        registry().get(&id)?.upgrade().map(Channel)
    }
}

/// A handle's value for an identity and revision.
pub fn handle_value(id: u64, rev: u64) -> Value {
    Value::map([
        (HANDLE_KEY, Value::Int(id as i64)),
        (REV_KEY, Value::Int(rev as i64)),
    ])
}

/// The identity and revision a handle names, or `None` for any other value.
pub fn handle_of(v: &Value) -> Option<(u64, u64)> {
    let Value::Map(entries) = v else {
        return None;
    };
    let mut id = None;
    let mut rev = 0;
    for (k, x) in entries {
        match (k.as_str(), x) {
            (HANDLE_KEY, Value::Int(i)) => id = Some(*i as u64),
            (REV_KEY, Value::Int(r)) => rev = *r as u64,
            _ => return None,
        }
    }
    id.map(|id| (id, rev))
}

/// Whether `v` is a channel handle.
pub fn is_handle(v: &Value) -> bool {
    handle_of(v).is_some()
}

/// A column from a list of values all of one kind — numbers (integers
/// stay integers only when every one is), booleans or strings — what the
/// bindings build from their own arrays. `None` names the first value that
/// breaks the kind.
pub fn column_of(values: &[Value]) -> Result<Column, ChannelError> {
    let Some(first) = values.first() else {
        return Ok(Column::F64(Vec::new()));
    };
    let bad = |i: usize, want: &str| {
        Err(ChannelError(format!(
            "row {} is {}, in a column of {want}s",
            i + 1,
            match &values[i] {
                Value::Null => "nothing",
                Value::Bool(_) => "a boolean",
                Value::Int(_) | Value::Float(_) => "a number",
                Value::Str(_) => "a string",
                Value::List(_) => "a list",
                Value::Map(_) => "a map",
            }
        )))
    };
    match first {
        Value::Int(_) | Value::Float(_) => {
            if values.iter().all(|v| matches!(v, Value::Int(_))) {
                Ok(Column::I64(
                    values.iter().map(|v| v.as_int().unwrap_or(0)).collect(),
                ))
            } else {
                let mut out = Vec::with_capacity(values.len());
                for (i, v) in values.iter().enumerate() {
                    match v {
                        Value::Int(x) => out.push(*x as f64),
                        Value::Float(x) => out.push(*x),
                        _ => return bad(i, "number"),
                    }
                }
                Ok(Column::F64(out))
            }
        }
        Value::Bool(_) => {
            let mut out = Vec::with_capacity(values.len());
            for (i, v) in values.iter().enumerate() {
                match v {
                    Value::Bool(b) => out.push(*b),
                    _ => return bad(i, "boolean"),
                }
            }
            Ok(Column::Bool(out))
        }
        Value::Str(_) => {
            let mut out = Strs::default();
            for (i, v) in values.iter().enumerate() {
                match v {
                    Value::Str(s) => out.push(s),
                    _ => return bad(i, "string"),
                }
            }
            Ok(Column::Str(out))
        }
        _ => bad(0, "number, boolean or string"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_reads_by_row_and_column() {
        let ch = Channel::table(vec![
            ("a".into(), Column::I64(vec![1, 2, 3])),
            ("b".into(), Column::strs(["x", "", "zz"])),
            ("c".into(), Column::Bool(vec![true, false, true])),
        ])
        .unwrap();
        assert_eq!(ch.len(), 3);
        assert_eq!(ch.get(2, 0), Cell::I64(3));
        assert_eq!(ch.get(1, 1), Cell::Str(""));
        assert_eq!(ch.get(2, 1), Cell::Str("zz"));
        assert_eq!(ch.get(3, 1), Cell::Null);
        assert_eq!(ch.column_named("c").unwrap().kind(), "boolean");
    }

    #[test]
    fn columns_of_two_lengths_or_one_name_twice_are_refused() {
        assert!(
            Channel::table(vec![
                ("a".into(), Column::I64(vec![1])),
                ("b".into(), Column::I64(vec![1, 2])),
            ])
            .is_err()
        );
        assert!(
            Channel::table(vec![
                ("a".into(), Column::I64(vec![1])),
                ("a".into(), Column::I64(vec![2])),
            ])
            .is_err()
        );
    }

    #[test]
    fn a_handle_resolves_while_held_and_follows_the_latest_revision() {
        let first = Channel::table(vec![("n".into(), Column::F64(vec![1.0]))]).unwrap();
        let h = first.handle();
        assert_eq!(Channel::resolve(&h), Some(first.clone()));
        let second = first
            .publish(vec![("n".into(), Column::F64(vec![1.0, 2.0]))])
            .unwrap();
        assert_eq!(second.id(), first.id());
        assert_eq!(second.rev(), 2);
        // The old handle names the identity; it resolves to the latest.
        assert_eq!(Channel::resolve(&h).unwrap().len(), 2);
        // The old revision stays readable while held.
        assert_eq!(first.len(), 1);
        let h2 = second.handle();
        drop(first);
        drop(second);
        assert!(Channel::resolve(&h2).is_none(), "nobody holds it");
        assert!(handle_of(&Value::map([("$channel", Value::str("x"))])).is_none());
    }

    #[test]
    fn a_column_is_built_from_values_of_one_kind() {
        let ints = column_of(&[Value::Int(1), Value::Int(2)]).unwrap();
        assert_eq!(ints, Column::I64(vec![1, 2]));
        let mixed = column_of(&[Value::Int(1), Value::Float(2.5)]).unwrap();
        assert_eq!(mixed, Column::F64(vec![1.0, 2.5]));
        let err = column_of(&[Value::str("a"), Value::Int(1)]).unwrap_err();
        assert!(err.0.contains("row 2 is a number"), "{err}");
    }
}
