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
//! A crate of its own (ADR 0075, decision 3), on `kui-value` alone, so
//! what has no kui core — the recording's reader — makes channels too;
//! `kui-core` re-exports it as `kui_core::channel`. With the `lua`
//! feature, [`lua`] is a channel as a script holds it.
//!
//! ```rust
//! use kui_channel::{Cell, Channel, Column};
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
use std::sync::{Arc, LazyLock, Mutex, OnceLock, Weak};

use rustc_hash::FxHashMap;

use kui_value::Value;

mod lanes;
#[cfg(feature = "lua")]
pub mod lua;

pub use lanes::{Lanes, Run};

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

    /// An empty column of this one's kind.
    fn empty_like(&self) -> Column {
        match self {
            Column::F64(_) => Column::F64(Vec::new()),
            Column::I64(_) => Column::I64(Vec::new()),
            Column::Bool(_) => Column::Bool(Vec::new()),
            Column::Str(_) => Column::Str(Strs::default()),
        }
    }

    /// `other`'s rows `range` after this column's, of the same kind.
    fn extend_rows(&mut self, other: &Column, range: std::ops::Range<usize>) {
        match (self, other) {
            (Column::F64(a), Column::F64(b)) => a.extend_from_slice(&b[range]),
            (Column::I64(a), Column::I64(b)) => a.extend_from_slice(&b[range]),
            (Column::Bool(a), Column::Bool(b)) => a.extend_from_slice(&b[range]),
            (Column::Str(a), Column::Str(b)) => {
                for i in range {
                    a.push(b.get(i).unwrap_or_default());
                }
            }
            _ => unreachable!("runs of one column are of one kind"),
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

/// A run of a table's rows, shared between the revisions that hold it:
/// an append adds one, so a revision costs its new rows and not its old
/// ones (backlog C147). Its rows in the channel are `start..`, its own
/// from `skip` on — a cap drops the oldest from the front.
#[derive(Clone)]
struct Chunk {
    start: usize,
    skip: usize,
    cols: Arc<Vec<Column>>,
}

impl Chunk {
    fn rows(&self) -> usize {
        self.cols.first().map_or(0, Column::len) - self.skip
    }
}

enum Body {
    /// The columns' names, the runs of rows, and the columns whole, made
    /// once when a reader asks for one whole and there is more than one
    /// run (or the first is cut).
    Table {
        names: Vec<String>,
        chunks: Vec<Chunk>,
        flat: OnceLock<Vec<Column>>,
        data: OnceLock<Data>,
    },
    Bytes(Data),
}

struct Inner {
    id: u64,
    rev: u64,
    body: Body,
    len: usize,
}

impl Inner {
    /// The table's columns whole.
    fn flat(&self) -> Option<&[Column]> {
        let Body::Table {
            names,
            chunks,
            flat,
            ..
        } = &self.body
        else {
            return None;
        };
        if let [one] = chunks.as_slice()
            && one.skip == 0
        {
            return Some(&one.cols);
        }
        Some(flat.get_or_init(|| {
            (0..names.len())
                .map(|c| {
                    let mut out = chunks[0].cols[c].empty_like();
                    for ch in chunks {
                        out.extend_rows(&ch.cols[c], ch.skip..ch.skip + ch.rows());
                    }
                    out
                })
                .collect()
        }))
    }
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

/// At most this many runs before a reader by row searches them: past it
/// the runs merge however their sizes stand.
const MAX_CHUNKS: usize = 48;

/// Merges the last two runs while the last is as long as the one before
/// it, so a table appended to a row at a time holds about log₂ of its rows
/// in runs and each row is copied about that many times in all.
fn settle(chunks: &mut Vec<Chunk>) {
    while chunks.len() >= 2 {
        let n = chunks.len();
        let (a, b) = (&chunks[n - 2], &chunks[n - 1]);
        if b.rows() < a.rows() && n <= MAX_CHUNKS {
            break;
        }
        let cols = (0..a.cols.len())
            .map(|c| {
                let mut out = a.cols[c].empty_like();
                out.extend_rows(&a.cols[c], a.skip..a.skip + a.rows());
                out.extend_rows(&b.cols[c], b.skip..b.skip + b.rows());
                out
            })
            .collect();
        let start = a.start;
        chunks.truncate(n - 2);
        chunks.push(Chunk {
            start,
            skip: 0,
            cols: Arc::new(cols),
        });
    }
}

impl Channel {
    fn register(id: u64, rev: u64, body: Body, len: usize) -> Channel {
        let inner = Arc::new(Inner { id, rev, body, len });
        let mut reg = registry();
        // A sweep of the gone, now and then: the map holds weak entries,
        // and one per identity ever made would grow with a busy writer.
        if reg.len() > 256 && reg.len().is_power_of_two() {
            reg.retain(|_, w| w.strong_count() > 0);
        }
        reg.insert(id, Arc::downgrade(&inner));
        Channel(inner)
    }

    fn table_body(names: Vec<String>, cols: Vec<Column>) -> Body {
        Body::Table {
            names,
            chunks: vec![Chunk {
                start: 0,
                skip: 0,
                cols: Arc::new(cols),
            }],
            flat: OnceLock::new(),
            data: OnceLock::new(),
        }
    }

    /// A table of named columns of one length.
    pub fn table(cols: Vec<(String, Column)>) -> Result<Channel, ChannelError> {
        let (names, cols): (Vec<String>, Vec<Column>) = cols.into_iter().unzip();
        let len = table_len(&names, &cols)?;
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Ok(Self::register(id, 1, Self::table_body(names, cols), len))
    }

    /// A run of bytes.
    pub fn bytes(bytes: Vec<u8>) -> Channel {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let len = bytes.len();
        Self::register(id, 1, Body::Bytes(Data::Bytes(bytes)), len)
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
            Self::table_body(names, cols),
            len,
        ))
    }

    /// [`Self::publish`] for bytes.
    pub fn publish_bytes(&self, bytes: Vec<u8>) -> Channel {
        let len = bytes.len();
        Self::register(
            self.0.id,
            self.0.rev + 1,
            Body::Bytes(Data::Bytes(bytes)),
            len,
        )
    }

    /// The next revision under this identity: this one's rows and `rows`
    /// after them (backlog C147). `rows` names every column this table
    /// has, each of the same kind (integers widen into a number column),
    /// all of one length. With `cap`, the oldest rows go past it, so a log
    /// keeps its last `cap` lines. The rows this revision holds are shared
    /// with the next, not copied: an append costs its own rows.
    pub fn append(
        &self,
        rows: Vec<(String, Column)>,
        cap: Option<usize>,
    ) -> Result<Channel, ChannelError> {
        let Body::Table { names, chunks, .. } = &self.0.body else {
            return Err(ChannelError("a channel of bytes takes no rows".into()));
        };
        if rows.len() != names.len() {
            return Err(ChannelError(format!(
                "{} columns appended to a channel of {}: {}",
                rows.len(),
                names.len(),
                names.join(", ")
            )));
        }
        let mut given: Vec<Option<Column>> = vec![None; names.len()];
        let mut n = None;
        for (name, col) in rows {
            let Some(i) = names.iter().position(|x| *x == name) else {
                return Err(ChannelError(format!(
                    "the channel has no column `{name}`; it has {}",
                    names.join(", ")
                )));
            };
            if given[i].is_some() {
                return Err(ChannelError(format!("column `{name}` is appended twice")));
            }
            let have = &chunks[0].cols[i];
            let col = match (have, col) {
                (Column::F64(_), Column::I64(v)) => {
                    Column::F64(v.into_iter().map(|x| x as f64).collect())
                }
                (h, c) if h.kind() == c.kind() => c,
                // No rows have no kind: a binding's empty list reads as
                // numbers, which a string column takes as none of its own.
                (h, c) if c.is_empty() => h.empty_like(),
                (h, c) => {
                    return Err(ChannelError(format!(
                        "column `{name}` holds {}s; {}s were appended to it",
                        h.kind(),
                        c.kind()
                    )));
                }
            };
            match n {
                None => n = Some(col.len()),
                Some(n) if n != col.len() => {
                    return Err(ChannelError(format!(
                        "column `{name}` appends {} rows where another appends {n}",
                        col.len()
                    )));
                }
                Some(_) => {}
            }
            given[i] = Some(col);
        }
        let added = n.unwrap_or(0);
        let mut chunks = chunks.clone();
        let mut len = self.0.len;
        if added > 0 {
            chunks.push(Chunk {
                start: len,
                skip: 0,
                cols: Arc::new(given.into_iter().map(Option::unwrap).collect()),
            });
            len += added;
            settle(&mut chunks);
        }
        if let Some(cap) = cap
            && len > cap
        {
            let mut drop = len - cap;
            len = cap;
            while drop > 0 && !chunks.is_empty() {
                let rows = chunks[0].rows();
                if rows <= drop && chunks.len() > 1 {
                    chunks.remove(0);
                    drop -= rows;
                } else {
                    chunks[0].skip += drop.min(rows);
                    drop = 0;
                }
            }
            let mut start = 0;
            for c in &mut chunks {
                c.start = start;
                start += c.rows();
            }
        }
        Ok(Self::register(
            self.0.id,
            self.0.rev + 1,
            Body::Table {
                names: names.clone(),
                chunks,
                flat: OnceLock::new(),
                data: OnceLock::new(),
            },
            len,
        ))
    }

    /// The next revision under this identity, holding `source`'s rows as
    /// they stand — shared, not copied: a view showing what it views
    /// unfiltered, its handle staying the view's.
    pub fn publish_from(&self, source: &Channel) -> Channel {
        let body = match &source.0.body {
            Body::Table { names, chunks, .. } => Body::Table {
                names: names.clone(),
                chunks: chunks.clone(),
                flat: OnceLock::new(),
                data: OnceLock::new(),
            },
            Body::Bytes(d) => Body::Bytes(d.clone()),
        };
        Self::register(self.0.id, self.0.rev + 1, body, source.0.len)
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

    /// What it holds, whole: a table appended to is put together once,
    /// on the first ask.
    pub fn data(&self) -> &Data {
        match &self.0.body {
            Body::Bytes(d) => d,
            Body::Table { names, data, .. } => data.get_or_init(|| Data::Table {
                names: names.clone(),
                cols: self.0.flat().unwrap_or_default().to_vec(),
            }),
        }
    }

    /// The column names, in order; empty for bytes.
    pub fn names(&self) -> &[String] {
        match &self.0.body {
            Body::Table { names, .. } => names,
            Body::Bytes(_) => &[],
        }
    }

    /// The index of the column called `name`.
    pub fn col(&self, name: &str) -> Option<usize> {
        self.names().iter().position(|n| n == name)
    }

    /// The column at `i`, whole: a table appended to is put together once
    /// a revision, on the first ask; [`Self::get`] reads a row without.
    pub fn column(&self, i: usize) -> Option<&Column> {
        self.0.flat()?.get(i)
    }

    /// The column called `name`.
    pub fn column_named(&self, name: &str) -> Option<&Column> {
        self.column(self.col(name)?)
    }

    /// Row `row` of column `col`; `Cell::Null` out of range.
    pub fn get(&self, row: usize, col: usize) -> Cell<'_> {
        let Body::Table { chunks, .. } = &self.0.body else {
            return Cell::Null;
        };
        if row >= self.0.len {
            return Cell::Null;
        }
        let at = match chunks.as_slice() {
            [one] => one,
            many => &many[many.partition_point(|c| c.start <= row) - 1],
        };
        at.cols
            .get(col)
            .map_or(Cell::Null, |c| c.get(row - at.start + at.skip))
    }

    /// The bytes, for a bytes channel.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match &self.0.body {
            Body::Bytes(Data::Bytes(b)) => Some(b),
            _ => None,
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

/// What [`Channel::filter`] keeps a row for (backlog C148).
#[derive(Clone, Debug, PartialEq)]
pub enum Match {
    /// The cell equals this: a number to a number column (an integer to
    /// an integer one exactly), a string to a string, a boolean to a
    /// boolean.
    Eq(Value),
    /// A string cell holds `text`; `case` false matches it whatever the
    /// letters' case.
    Contains { text: String, case: bool },
    /// A number cell is at least `min` and at most `max`, either left
    /// open; NaN is in no range.
    Range { min: Option<f64>, max: Option<f64> },
}

/// `needle` in `hay`, ASCII letters of either case alike.
fn contains_ascii_blind(hay: &str, needle: &str) -> bool {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    n.is_empty() || h.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n))
}

impl Channel {
    fn column_index(&self, col: usize) -> Result<&Column, ChannelError> {
        self.column(col).ok_or_else(|| {
            ChannelError(format!(
                "the channel has no column {col}; it has {}",
                self.names().len()
            ))
        })
    }

    fn rows_in<'a>(
        &self,
        within: Option<&'a [usize]>,
    ) -> Result<Box<dyn Iterator<Item = usize> + 'a>, ChannelError> {
        let len = self.len();
        Ok(match within {
            Some(rows) => {
                if let Some(r) = rows.iter().find(|&&r| r >= len) {
                    return Err(ChannelError(format!(
                        "row {r} is past the channel's {len} rows"
                    )));
                }
                Box::new(rows.iter().copied())
            }
            None => Box::new(0..len),
        })
    }

    /// The rows whose cell in column `col` matches `m`, in row order —
    /// all of them, or those of `within` in its order (backlog C148): a
    /// filter a worker or a table builds a view from without the rows
    /// crossing into a binding. A match of the wrong kind for the column
    /// is an error.
    pub fn filter(
        &self,
        col: usize,
        m: &Match,
        within: Option<&[usize]>,
    ) -> Result<Vec<usize>, ChannelError> {
        let column = self.column_index(col)?;
        let name = &self.names()[col];
        let rows = self.rows_in(within)?;
        let wrong = |what: &str| {
            Err(ChannelError(format!(
                "column `{name}` holds {}s; {what} matches none",
                column.kind()
            )))
        };
        Ok(match (column, m) {
            (Column::F64(v), Match::Eq(x)) => match x.as_float() {
                Some(x) => rows.filter(|&r| v[r] == x).collect(),
                None => return wrong("that value"),
            },
            (Column::I64(v), Match::Eq(Value::Int(x))) => rows.filter(|&r| v[r] == *x).collect(),
            (Column::I64(v), Match::Eq(Value::Float(x))) => {
                rows.filter(|&r| v[r] as f64 == *x).collect()
            }
            (Column::Bool(v), Match::Eq(Value::Bool(x))) => rows.filter(|&r| v[r] == *x).collect(),
            (Column::Str(v), Match::Eq(Value::Str(x))) => {
                rows.filter(|&r| v.get(r) == Some(x.as_str())).collect()
            }
            (Column::Str(v), Match::Contains { text, case: true }) => rows
                .filter(|&r| v.get(r).is_some_and(|s| s.contains(text.as_str())))
                .collect(),
            (Column::Str(v), Match::Contains { text, case: false }) if text.is_ascii() => rows
                .filter(|&r| v.get(r).is_some_and(|s| contains_ascii_blind(s, text)))
                .collect(),
            (Column::Str(v), Match::Contains { text, case: false }) => {
                let text = text.to_lowercase();
                rows.filter(|&r| v.get(r).is_some_and(|s| s.to_lowercase().contains(&text)))
                    .collect()
            }
            (Column::F64(_) | Column::I64(_), Match::Range { min, max }) => {
                let (lo, hi) = (
                    min.unwrap_or(f64::NEG_INFINITY),
                    max.unwrap_or(f64::INFINITY),
                );
                let at = |r| column.get(r).as_f64().unwrap_or(f64::NAN);
                rows.filter(|&r| {
                    let x = at(r);
                    x >= lo && x <= hi
                })
                .collect()
            }
            (_, Match::Eq(_)) => return wrong("that value"),
            (_, Match::Contains { .. }) => return wrong("a string search"),
            (_, Match::Range { .. }) => return wrong("a range"),
        })
    }

    /// The rows — all, or `within`'s — ordered by column `col`, least
    /// first or with `descending` greatest first (backlog C148). Rows of
    /// one value keep their order; NaN goes last either way; strings
    /// compare by their bytes, `false` before `true`.
    pub fn sort(
        &self,
        col: usize,
        descending: bool,
        within: Option<&[usize]>,
    ) -> Result<Vec<usize>, ChannelError> {
        use std::cmp::Ordering as O;
        let column = self.column_index(col)?;
        let mut rows: Vec<usize> = self.rows_in(within)?.collect();
        let flip = |o: O| if descending { o.reverse() } else { o };
        match column {
            Column::F64(v) => rows.sort_by(|&a, &b| match (v[a].is_nan(), v[b].is_nan()) {
                (false, false) => flip(v[a].total_cmp(&v[b])),
                (a, b) => a.cmp(&b),
            }),
            Column::I64(v) => rows.sort_by(|&a, &b| flip(v[a].cmp(&v[b]))),
            Column::Bool(v) => rows.sort_by(|&a, &b| flip(v[a].cmp(&v[b]))),
            Column::Str(v) => rows.sort_by(|&a, &b| flip(v.get(a).cmp(&v.get(b)))),
        }
        Ok(rows)
    }

    /// The columns of `rows`, in that order, under this channel's names:
    /// what [`Self::take`] makes a channel of, and what a view publishes
    /// as the next revision of its own.
    pub fn take_columns(&self, rows: &[usize]) -> Result<Vec<(String, Column)>, ChannelError> {
        let len = self.len();
        if let Some(r) = rows.iter().find(|&&r| r >= len) {
            return Err(ChannelError(format!(
                "row {r} is past the channel's {len} rows"
            )));
        }
        Ok(self
            .names()
            .iter()
            .enumerate()
            .map(|(c, name)| {
                let have = self.column(c).expect("a named column");
                let col = match have {
                    Column::F64(v) => Column::F64(rows.iter().map(|&r| v[r]).collect()),
                    Column::I64(v) => Column::I64(rows.iter().map(|&r| v[r]).collect()),
                    Column::Bool(v) => Column::Bool(rows.iter().map(|&r| v[r]).collect()),
                    Column::Str(v) => {
                        Column::Str(rows.iter().map(|&r| v.get(r).unwrap_or_default()).collect())
                    }
                };
                (name.clone(), col)
            })
            .collect())
    }

    /// A new channel of `rows`, in that order (backlog C148): a filtered
    /// or sorted view, made in Rust.
    pub fn take(&self, rows: &[usize]) -> Result<Channel, ChannelError> {
        if self.as_bytes().is_some() {
            return Err(ChannelError(
                "a channel of bytes has no rows to take".into(),
            ));
        }
        Channel::table(self.take_columns(rows)?)
    }
}

/// One bucket of a [`Channel::reduce`]: the least and greatest value
/// among its rows, the first and the last, and the row each was read
/// from — a row of the channel, not of the range. A NaN is no value: it
/// is skipped, and a bucket of nothing but NaN reads NaN for all four,
/// every row its own first, so a line drawn through it breaks there.
/// Ties go to the earliest row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bucket {
    pub min: f64,
    pub max: f64,
    pub first: f64,
    pub last: f64,
    pub min_row: usize,
    pub max_row: usize,
    pub first_row: usize,
    pub last_row: usize,
}

impl Channel {
    /// Column `col` over rows `from..to` in `buckets` buckets of as near
    /// one size as whole rows allow, each one's [`Bucket`] (backlog C95):
    /// what a chart draws a hundred thousand points as — per pixel column
    /// the first, least, greatest and last, in row order, which is the
    /// line the rows would draw — without the rows crossing into the
    /// binding. A range with fewer rows than `buckets` gets a bucket a
    /// row; `to` past the end stops at it, and an empty range (`from` at
    /// or past `to` or the end, or no buckets) gets none. The column is
    /// numbers or integers (widened); any other, or no column `col`, is
    /// an error.
    pub fn reduce(
        &self,
        col: usize,
        from: usize,
        to: usize,
        buckets: usize,
    ) -> Result<Vec<Bucket>, ChannelError> {
        let mut out = Vec::new();
        self.reduce_into(col, from, to, buckets, &mut out)?;
        Ok(out)
    }

    /// [`Self::reduce`] into `out`, cleared first: a chart that reduces
    /// every frame keeps one buffer.
    pub fn reduce_into(
        &self,
        col: usize,
        from: usize,
        to: usize,
        buckets: usize,
        out: &mut Vec<Bucket>,
    ) -> Result<(), ChannelError> {
        out.clear();
        let column = self.column(col).ok_or_else(|| {
            ChannelError(format!(
                "the channel has no column {col}; it has {}",
                self.names().len()
            ))
        })?;
        let to = to.min(self.len());
        let n = to.saturating_sub(from);
        let k = buckets.min(n);
        match column {
            // Checked before the range, so a mistake does not wait for
            // the data to show.
            Column::F64(_) | Column::I64(_) if k == 0 => {}
            Column::F64(v) => reduce_rows(&v[from..to], from, k, out, |x| x),
            Column::I64(v) => reduce_rows(&v[from..to], from, k, out, |x| x as f64),
            other => return Err(not_numbers(&self.names()[col], other)),
        }
        Ok(())
    }
}

fn not_numbers(name: &str, c: &Column) -> ChannelError {
    ChannelError(format!(
        "column `{name}` holds {}s; reduce reads a column of numbers",
        c.kind()
    ))
}

/// The buckets of `rows`, which begin at row `base`: `k` of them, bucket
/// `b` holding rows `b·n/k ..(b+1)·n/k`. One pass per bucket, its first
/// and last found from its ends and its extremes from the first on, NaN
/// failing every comparison and so skipped by them.
fn reduce_rows<T: Copy>(
    rows: &[T],
    base: usize,
    k: usize,
    out: &mut Vec<Bucket>,
    f: impl Fn(T) -> f64,
) {
    out.reserve(k);
    let n = rows.len() as u128;
    let edge = |b: usize| (b as u128 * n / k as u128) as usize;
    for b in 0..k {
        let (a, z) = (edge(b), edge(b + 1));
        let bucket = &rows[a..z];
        let Some(i) = bucket.iter().position(|&x| !f(x).is_nan()) else {
            let row = base + a;
            out.push(Bucket {
                min: f64::NAN,
                max: f64::NAN,
                first: f64::NAN,
                last: f64::NAN,
                min_row: row,
                max_row: row,
                first_row: row,
                last_row: row,
            });
            continue;
        };
        let j = bucket.iter().rposition(|&x| !f(x).is_nan()).unwrap_or(i);
        let first = f(bucket[i]);
        let ((min, lo), (max, hi)) = extremes(&bucket[i..], &f);
        let (lo, hi) = (lo + i, hi + i);
        out.push(Bucket {
            min,
            max,
            first,
            last: f(bucket[j]),
            min_row: base + a + lo,
            max_row: base + a + hi,
            first_row: base + a + i,
            last_row: base + a + j,
        });
    }
}

/// The least and greatest of `rows`, which starts with a number, and
/// where each is first found. Four lanes, each a run of every fourth
/// row, so the comparisons of one do not wait on another's: one lane was
/// a comparison's latency a row, 1 ns, the four a quarter of it. A NaN
/// fails every comparison and is passed over; a lane that has seen only
/// NaN holds the first row's number, which is no harm, since that number
/// is in the run.
#[inline]
fn extremes<T: Copy>(rows: &[T], f: &impl Fn(T) -> f64) -> ((f64, usize), (f64, usize)) {
    let x0 = f(rows[0]);
    let mut min = [x0; 4];
    let mut max = [x0; 4];
    let mut lo = [0usize; 4];
    let mut hi = [0usize; 4];
    let (quads, rest) = rows.as_chunks::<4>();
    for (q, quad) in quads.iter().enumerate() {
        for l in 0..4 {
            let x = f(quad[l]);
            let at = q * 4 + l;
            if x < min[l] {
                min[l] = x;
                lo[l] = at;
            }
            if x > max[l] {
                max[l] = x;
                hi[l] = at;
            }
        }
    }
    let mut least = (min[0], lo[0]);
    let mut most = (max[0], hi[0]);
    // The lanes' answers, ties to the earliest row as one pass gives.
    for l in 1..4 {
        if min[l] < least.0 || (min[l] == least.0 && lo[l] < least.1) {
            least = (min[l], lo[l]);
        }
        if max[l] > most.0 || (max[l] == most.0 && hi[l] < most.1) {
            most = (max[l], hi[l]);
        }
    }
    for (r, &x) in rest.iter().enumerate() {
        let x = f(x);
        let at = quads.len() * 4 + r;
        if x < least.0 {
            least = (x, at);
        }
        if x > most.0 {
            most = (x, at);
        }
    }
    (least, most)
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
    fn a_reduction_finds_each_buckets_extremes_and_ends() {
        let ch = Channel::table(vec![(
            "y".into(),
            Column::F64(vec![3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0]),
        )])
        .unwrap();
        let r = ch.reduce(0, 0, 8, 2).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(
            (r[0].min, r[0].min_row),
            (1.0, 1),
            "ties go to the earliest row"
        );
        assert_eq!((r[0].max, r[0].max_row), (4.0, 2));
        assert_eq!(
            (r[0].first, r[0].first_row, r[0].last, r[0].last_row),
            (3.0, 0, 1.0, 3)
        );
        assert_eq!(
            (r[1].min, r[1].min_row, r[1].max, r[1].max_row),
            (2.0, 6, 9.0, 5)
        );
        // Rows are the channel's, not the range's.
        let r = ch.reduce(0, 4, 8, 1).unwrap();
        assert_eq!((r[0].first_row, r[0].max_row, r[0].last_row), (4, 5, 7));
        // Integers widen.
        let ints = Channel::table(vec![("n".into(), Column::I64(vec![5, -2, 7]))]).unwrap();
        let r = ints.reduce(0, 0, 3, 1).unwrap();
        assert_eq!((r[0].min, r[0].max), (-2.0, 7.0));
    }

    #[test]
    fn a_reduction_at_its_edges() {
        let ch = Channel::table(vec![
            ("y".into(), Column::F64(vec![1.0, 2.0, 3.0, 4.0, 5.0])),
            ("s".into(), Column::strs(["a", "b", "c", "d", "e"])),
        ])
        .unwrap();
        // Empty ranges and no buckets: nothing.
        assert!(ch.reduce(0, 3, 3, 10).unwrap().is_empty());
        assert!(ch.reduce(0, 4, 2, 10).unwrap().is_empty());
        assert!(ch.reduce(0, 9, 20, 10).unwrap().is_empty());
        assert!(ch.reduce(0, 0, 5, 0).unwrap().is_empty());
        // More buckets than rows: a bucket a row, `to` stopped at the end.
        let r = ch.reduce(0, 1, 99, 10).unwrap();
        assert_eq!(r.len(), 4);
        for (i, b) in r.iter().enumerate() {
            assert_eq!((b.first_row, b.last_row), (i + 1, i + 1));
            assert_eq!((b.min, b.max, b.last), (b.first, b.first, b.first));
        }
        // Buckets of near one size: 5 rows in 2 are 2 and 3.
        let r = ch.reduce(0, 0, 5, 2).unwrap();
        assert_eq!((r[0].last_row, r[1].first_row), (1, 2));
        // A column that is not numbers, or none, is an error, even empty.
        assert!(ch.reduce(1, 0, 5, 2).unwrap_err().0.contains("strings"));
        assert!(ch.reduce(1, 0, 0, 2).is_err());
        assert!(ch.reduce(7, 0, 5, 2).is_err());
    }

    /// The four lanes answer what one pass would, ties and NaN included,
    /// over lengths that leave every remainder and values drawn from a
    /// few, so ties are common.
    #[test]
    fn the_lanes_agree_with_one_pass() {
        let mut seed = 0x2545_f491u32;
        for len in 1..40usize {
            for _ in 0..20 {
                let rows: Vec<f64> = (0..len)
                    .map(|i| {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        if i > 0 && seed.is_multiple_of(5) {
                            f64::NAN
                        } else {
                            (seed % 7) as f64
                        }
                    })
                    .collect();
                let (mut min, mut max) = ((rows[0], 0), (rows[0], 0));
                for (at, &x) in rows.iter().enumerate() {
                    if x < min.0 {
                        min = (x, at);
                    }
                    if x > max.0 {
                        max = (x, at);
                    }
                }
                assert_eq!(extremes(&rows, &|x| x), (min, max), "{rows:?}");
            }
        }
    }

    #[test]
    fn a_reduction_skips_nan() {
        let nan = f64::NAN;
        let ch = Channel::table(vec![(
            "y".into(),
            Column::F64(vec![nan, 2.0, nan, -1.0, nan, nan, nan, nan, 7.0]),
        )])
        .unwrap();
        let r = ch.reduce(0, 0, 9, 3).unwrap();
        assert_eq!(
            (r[0].first, r[0].first_row, r[0].last, r[0].last_row),
            (2.0, 1, 2.0, 1)
        );
        assert_eq!(
            (r[1].min, r[1].min_row, r[1].max, r[1].max_row),
            (-1.0, 3, -1.0, 3)
        );
        assert_eq!((r[2].first_row, r[2].first), (8, 7.0));
        // A bucket of nothing but NaN reads NaN, at its first row.
        let r = ch.reduce(0, 4, 8, 2).unwrap();
        assert!(
            r.iter()
                .all(|b| b.min.is_nan() && b.max.is_nan() && b.first.is_nan() && b.last.is_nan())
        );
        assert_eq!((r[0].min_row, r[1].last_row), (4, 6));
    }

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

    fn log(n: usize) -> Channel {
        Channel::table(vec![
            ("line".into(), Column::strs((0..n).map(|i| format!("l{i}")))),
            ("ms".into(), Column::F64((0..n).map(|i| i as f64).collect())),
        ])
        .unwrap()
    }

    fn rows(lines: &[&str], ms: &[i64]) -> Vec<(String, Column)> {
        vec![
            ("ms".into(), Column::I64(ms.to_vec())),
            ("line".into(), Column::strs(lines)),
        ]
    }

    /// An append is the next revision with the rows after this one's —
    /// columns by name, integers widened into a number column — and the
    /// old revision keeps its rows (backlog C147).
    #[test]
    fn an_append_adds_rows_and_the_old_revision_keeps_its_own() {
        let a = log(2);
        let b = a.append(rows(&["x", "y"], &[10, 11]), None).unwrap();
        assert_eq!((b.id(), b.rev(), b.len()), (a.id(), 2, 4));
        assert_eq!(b.get(3, 0), Cell::Str("y"));
        assert_eq!(b.get(2, 1), Cell::F64(10.0));
        assert_eq!(a.len(), 2);
        assert_eq!(a.get(2, 0), Cell::Null);
        assert_eq!(Channel::resolve(&a.handle()).unwrap().len(), 4);
        let whole = b.column_named("line").unwrap();
        assert_eq!(whole, &Column::strs(["l0", "l1", "x", "y"]));
    }

    /// A row at a time, past a cap: the last `cap` rows, read by row and
    /// whole alike, and the runs few.
    #[test]
    fn a_capped_log_keeps_its_last_rows() {
        let mut ch = log(0);
        for i in 0..1000i64 {
            ch = ch
                .append(rows(&[&format!("n{i}")], &[i]), Some(300))
                .unwrap();
            if let Body::Table { chunks, .. } = &ch.0.body {
                assert!(chunks.len() <= 12, "{} runs at row {i}", chunks.len());
            }
        }
        assert_eq!(ch.len(), 300);
        assert_eq!(ch.get(0, 0), Cell::Str("n700"));
        assert_eq!(ch.get(299, 1), Cell::F64(999.0));
        let ms = ch.column_named("ms").unwrap();
        assert_eq!(ms.len(), 300);
        assert_eq!(ms.get(150), Cell::F64(850.0));
        for r in [0, 1, 77, 150, 298, 299] {
            assert_eq!(ch.get(r, 1), ms.get(r), "row {r}");
        }
    }

    #[test]
    fn an_append_of_the_wrong_shape_is_refused() {
        let a = log(1);
        let e = |r: Result<Channel, ChannelError>| r.unwrap_err().0;
        assert!(
            e(a.append(vec![("ms".into(), Column::F64(vec![1.0]))], None)).contains("1 columns")
        );
        assert!(e(a.append(rows(&["x"], &[1, 2]), None)).contains("rows"));
        let bools = vec![
            ("ms".into(), Column::Bool(vec![true])),
            ("line".into(), Column::strs(["x"])),
        ];
        assert!(e(a.append(bools, None)).contains("booleans"));
        // No rows have no kind: an empty list, numbers to a binding, is
        // taken by the string column.
        let none = vec![
            ("ms".into(), Column::F64(Vec::new())),
            ("line".into(), Column::F64(Vec::new())),
        ];
        let same = a.append(none, None).unwrap();
        assert_eq!((same.len(), same.get(0, 0)), (1, a.get(0, 0)));
        assert!(
            Channel::bytes(vec![1])
                .append(rows(&["x"], &[1]), None)
                .is_err()
        );
    }

    /// Filters by value, by a string's part and by a range, chained
    /// through `within`; a sort that keeps ties in order and NaN last; and
    /// a take of the rows they chose (backlog C148).
    #[test]
    fn a_filter_a_sort_and_a_take() {
        let ch = Channel::table(vec![
            (
                "name".into(),
                Column::strs(["api/get", "db", "API/put", "api/del", "x"]),
            ),
            ("ms".into(), Column::F64(vec![5.0, 1.0, f64::NAN, 5.0, 9.0])),
            ("n".into(), Column::I64(vec![1, 2, 3, 4, 5])),
        ])
        .unwrap();
        let api = Match::Contains {
            text: "api".into(),
            case: false,
        };
        assert_eq!(ch.filter(0, &api, None).unwrap(), [0, 2, 3]);
        let exact = Match::Contains {
            text: "api".into(),
            case: true,
        };
        assert_eq!(ch.filter(0, &exact, None).unwrap(), [0, 3]);
        let fast = Match::Range {
            min: None,
            max: Some(5.0),
        };
        let both = ch.filter(1, &fast, Some(&[0, 2, 3])).unwrap();
        assert_eq!(both, [0, 3], "NaN is in no range");
        assert_eq!(ch.filter(2, &Match::Eq(Value::Int(4)), None).unwrap(), [3]);
        assert_eq!(
            ch.filter(1, &Match::Eq(Value::Int(5)), None).unwrap(),
            [0, 3]
        );
        assert!(ch.filter(1, &api, None).is_err(), "a search on numbers");
        assert!(
            ch.filter(0, &fast, Some(&[9])).is_err(),
            "a row past the end"
        );

        assert_eq!(ch.sort(1, false, None).unwrap(), [1, 0, 3, 4, 2]);
        assert_eq!(ch.sort(1, true, None).unwrap(), [4, 0, 3, 1, 2]);
        assert_eq!(ch.sort(0, false, Some(&[3, 0])).unwrap(), [3, 0]);

        let view = ch.take(&[3, 0]).unwrap();
        assert_ne!(view.id(), ch.id());
        assert_eq!(view.len(), 2);
        assert_eq!(view.get(0, 0), Cell::Str("api/del"));
        assert_eq!(view.get(1, 2), Cell::I64(1));
        assert!(ch.take(&[5]).is_err());

        // A view of the whole, its rows shared and its identity its own.
        let shown = view.publish_from(&ch);
        assert_eq!((shown.id(), shown.rev(), shown.len()), (view.id(), 2, 5));
        assert_eq!(shown.get(2, 0), Cell::Str("API/put"));
    }
}
