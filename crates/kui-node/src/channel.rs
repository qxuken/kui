//! `Channel` for Node (ADR 0065): bulk data the core owns and a handle
//! names. `new Channel({ name: [...], ms: Float64Array })` builds one in a
//! single call; it serializes as its handle, so `props: { rows: channel }`
//! hands a hundred thousand rows to a component as one small value, and a
//! Lua or C reader resolves the same handle without a copy. `reduce`
//! buckets a range for a chart in Rust (backlog C95); `append` is the next
//! revision with more rows (backlog C147); `filter`, `sort` and `take`
//! build a view in Rust (backlog C148).

use kui_core::channel::{self, Cell, Column};
use napi::bindgen_prelude::{Either, Float64Array, Int32Array, Object, Uint32Array};
use napi_derive::napi;
// `serde_json::Value` spelled out, never aliased: napi-rs writes the name it
// sees into index.d.ts, and an alias there is a type TypeScript has never
// heard of (`Json`) where `Value` reads as `any`.
use serde_json::Value;

use crate::{Result, err};

/// One revision of a channel, held. Columns are numbers (a `Float64Array`
/// or an array of numbers), booleans or strings, all of one length.
#[napi]
pub struct Channel {
    inner: channel::Channel,
}

fn columns_of(columns: &Object) -> Result<Vec<(String, Column)>> {
    let mut out = Vec::new();
    for name in Object::keys(columns)? {
        let v: Option<Either<Float64Array, Vec<Value>>> = columns.get(&name)?;
        let column = match v {
            None => return Err(err(format!("channel column `{name}` is missing"))),
            Some(Either::A(f)) => Column::F64(f.to_vec()),
            Some(Either::B(list)) => {
                let values: Vec<kui_core::Value> = list.iter().map(crate::value_of).collect();
                channel::column_of(&values)
                    .map_err(|e| err(format!("channel column `{name}`: {e}")))?
            }
        };
        out.push((name, column));
    }
    Ok(out)
}

fn cell_json(c: Cell<'_>) -> Value {
    match c {
        Cell::Null => Value::Null,
        Cell::F64(x) => serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number),
        Cell::I64(x) => Value::from(x),
        Cell::Bool(x) => Value::Bool(x),
        Cell::Str(s) => Value::String(s.to_string()),
    }
}

/// What `channel.reduce` answers (backlog C95): a typed array per field,
/// one entry a bucket. NaN is skipped; a bucket of only NaN is NaN at its
/// first row. Rows are the channel's, 0-based.
#[napi(object)]
pub struct ChannelReduction {
    pub min: Float64Array,
    pub max: Float64Array,
    pub first: Float64Array,
    pub last: Float64Array,
    pub min_row: Uint32Array,
    pub max_row: Uint32Array,
    pub first_row: Uint32Array,
    pub last_row: Uint32Array,
}

/// A channel's intervals on lanes (`channel.lanes`, ADR 0075).
#[napi]
pub struct ChannelLanes {
    inner: channel::Lanes,
}

/// What `lanes.window` answers: a typed array per field, a run per index —
/// its start, end, row (0-based; -1 for a strip of several) and how many
/// intervals it holds.
#[napi(object)]
pub struct ChannelRuns {
    pub s: Float64Array,
    pub e: Float64Array,
    pub row: Int32Array,
    pub n: Uint32Array,
}

#[napi]
impl ChannelLanes {
    /// How many lanes.
    #[napi(getter)]
    pub fn count(&self) -> u32 {
        self.inner.count() as u32
    }

    /// How many intervals.
    #[napi(getter)]
    pub fn length(&self) -> u32 {
        self.inner.len() as u32
    }

    /// The earliest start.
    #[napi(getter)]
    pub fn lo(&self) -> f64 {
        self.inner.extent().0
    }

    /// The latest end.
    #[napi(getter)]
    pub fn hi(&self) -> f64 {
        self.inner.extent().1
    }

    /// Lane `lane`'s runs reaching into `v0 .. v1`, at the detail where
    /// an interval at least `min` long is a run of its own and shorter
    /// ones a gap under it apart merge into a strip: about a run a pixel
    /// with `min` a pixel's width.
    #[napi]
    pub fn window(&self, lane: u32, v0: f64, v1: f64, min: f64) -> ChannelRuns {
        let mut runs = Vec::new();
        self.inner.window(lane as usize, v0, v1, min, &mut runs);
        ChannelRuns {
            s: Float64Array::new(runs.iter().map(|r| r.start).collect()),
            e: Float64Array::new(runs.iter().map(|r| r.end).collect()),
            row: Int32Array::new(
                runs.iter()
                    .map(|r| r.row.map_or(-1, |x| x as i32))
                    .collect(),
            ),
            n: Uint32Array::new(runs.iter().map(|r| r.count).collect()),
        }
    }
}

/// What `channel.filter` keeps a row for (backlog C148): `eq`, a cell
/// equal to it; `contains`, a string cell holding it (`caseSensitive` to
/// match its letters' case); `min` / `max`, a number cell in that range,
/// either end left open.
#[napi(object)]
pub struct ChannelMatch {
    pub eq: Option<Value>,
    pub contains: Option<String>,
    pub case_sensitive: Option<bool>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

fn match_of(m: &ChannelMatch) -> Result<channel::Match> {
    if let Some(text) = &m.contains {
        return Ok(channel::Match::Contains {
            text: text.clone(),
            case: m.case_sensitive.unwrap_or(false),
        });
    }
    if let Some(v) = &m.eq {
        return Ok(channel::Match::Eq(crate::value_of(v)));
    }
    if m.min.is_some() || m.max.is_some() {
        return Ok(channel::Match::Range {
            min: m.min,
            max: m.max,
        });
    }
    Err(err(
        "filter: a match is { eq }, { contains, caseSensitive } or { min, max }",
    ))
}

fn rows_of(rows: &Either<Uint32Array, Vec<u32>>) -> Vec<usize> {
    match rows {
        Either::A(a) => a.iter().map(|&r| r as usize).collect(),
        Either::B(v) => v.iter().map(|&r| r as usize).collect(),
    }
}

fn rows_out(rows: Vec<usize>) -> Uint32Array {
    Uint32Array::new(rows.into_iter().map(|r| r as u32).collect())
}

impl Channel {
    fn col(&self, col: Either<String, u32>) -> Result<usize> {
        match col {
            Either::A(name) => self.inner.col(&name).ok_or_else(|| {
                err(format!(
                    "the channel has no column {name:?}; it has {}",
                    self.inner.names().join(", ")
                ))
            }),
            Either::B(i) => Ok(i as usize),
        }
    }
}

#[napi]
impl Channel {
    /// A channel of `columns`: each key a column's name, each value its
    /// rows. Throws for columns of two lengths or of mixed kinds.
    #[napi(constructor)]
    pub fn new(columns: Object) -> Result<Self> {
        let inner = channel::Channel::table(columns_of(&columns)?)
            .map_err(|e| err(format!("Channel: {e}")))?;
        Ok(Channel { inner })
    }

    /// The channel a handle names, if someone still holds it: what an
    /// event's payload or a worker's message carries.
    #[napi(factory)]
    pub fn resolve(handle: Value) -> Result<Self> {
        let v = crate::value_of(&handle);
        channel::Channel::resolve(&v)
            .map(|inner| Channel { inner })
            .ok_or_else(|| err("Channel.resolve: no channel is held under that handle"))
    }

    /// How many rows.
    #[napi(getter)]
    pub fn length(&self) -> u32 {
        self.inner.len() as u32
    }

    /// Its revision: 1, and one more for each `publish`.
    #[napi(getter)]
    pub fn rev(&self) -> f64 {
        self.inner.rev() as f64
    }

    /// The column names, in order.
    #[napi(getter)]
    pub fn names(&self) -> Vec<String> {
        self.inner.names().to_vec()
    }

    /// Row `row`'s value of column `col` (a name, or a 0-based index);
    /// `null` past the end.
    #[napi]
    pub fn get(&self, row: u32, col: Either<String, u32>) -> Result<Value> {
        let c = self.col(col)?;
        Ok(cell_json(self.inner.get(row as usize, c)))
    }

    /// One column whole: a `Float64Array` for numbers, an array otherwise.
    #[napi]
    pub fn column(&self, col: Either<String, u32>) -> Result<Either<Float64Array, Vec<Value>>> {
        let c = self.col(col)?;
        let column = self
            .inner
            .column(c)
            .ok_or_else(|| err(format!("the channel has no column {c}")))?;
        Ok(match column {
            Column::F64(v) => Either::A(Float64Array::new(v.clone())),
            Column::I64(v) => Either::A(Float64Array::new(v.iter().map(|x| *x as f64).collect())),
            other => Either::B((0..other.len()).map(|i| cell_json(other.get(i))).collect()),
        })
    }

    /// Column `col` over rows `from` up to `to` in `buckets` buckets of
    /// near one size — one a row when the range has fewer — each one's
    /// least, greatest, first and last value and the rows they were read
    /// from: a chart's pixel columns of a hundred thousand rows without the
    /// rows crossing into JavaScript. `to` past the end stops at it; an
    /// empty range answers empty arrays. Throws for a column that is not
    /// numbers.
    #[napi]
    pub fn reduce(
        &self,
        col: Either<String, u32>,
        from: u32,
        to: u32,
        buckets: u32,
    ) -> Result<ChannelReduction> {
        let c = self.col(col)?;
        let r = self
            .inner
            .reduce(c, from as usize, to as usize, buckets as usize)
            .map_err(|e| err(format!("reduce: {e}")))?;
        let f = |g: fn(&channel::Bucket) -> f64| Float64Array::new(r.iter().map(g).collect());
        let u = |g: fn(&channel::Bucket) -> usize| {
            Uint32Array::new(r.iter().map(|b| g(b) as u32).collect())
        };
        Ok(ChannelReduction {
            min: f(|b| b.min),
            max: f(|b| b.max),
            first: f(|b| b.first),
            last: f(|b| b.last),
            min_row: u(|b| b.min_row),
            max_row: u(|b| b.max_row),
            first_row: u(|b| b.first_row),
            last_row: u(|b| b.last_row),
        })
    }

    /// The next revision under the same identity, holding `columns`: a
    /// handle to this channel names that revision from now on.
    #[napi]
    pub fn publish(&self, columns: Object) -> Result<Channel> {
        let inner = self
            .inner
            .publish(columns_of(&columns)?)
            .map_err(|e| err(format!("publish: {e}")))?;
        Ok(Channel { inner })
    }

    /// The next revision under the same identity: this one's rows and
    /// `columns`' after them, each column of this channel named, of its
    /// kind and all of one length (backlog C147). With `cap`, the oldest
    /// rows go past it. This revision's rows are shared with the next, not
    /// copied, so a growing log costs its new lines.
    #[napi]
    pub fn append(&self, columns: Object, cap: Option<u32>) -> Result<Channel> {
        let inner = self
            .inner
            .append(columns_of(&columns)?, cap.map(|c| c as usize))
            .map_err(|e| err(format!("append: {e}")))?;
        Ok(Channel { inner })
    }

    /// The rows — all, or those of `rows` in its order — whose cell in
    /// column `col` matches, 0-based (backlog C148). Throws for a match of
    /// the wrong kind for the column, or a row past the end.
    #[napi]
    pub fn filter(
        &self,
        col: Either<String, u32>,
        matching: ChannelMatch,
        rows: Option<Either<Uint32Array, Vec<u32>>>,
    ) -> Result<Uint32Array> {
        let c = self.col(col)?;
        let within = rows.as_ref().map(rows_of);
        self.inner
            .filter(c, &match_of(&matching)?, within.as_deref())
            .map(rows_out)
            .map_err(|e| err(format!("filter: {e}")))
    }

    /// The rows — all, or `rows` — ordered by column `col`, least first or
    /// greatest with `descending`; ties keep their order, NaN goes last
    /// (backlog C148).
    #[napi]
    pub fn sort(
        &self,
        col: Either<String, u32>,
        descending: Option<bool>,
        rows: Option<Either<Uint32Array, Vec<u32>>>,
    ) -> Result<Uint32Array> {
        let c = self.col(col)?;
        let within = rows.as_ref().map(rows_of);
        self.inner
            .sort(c, descending.unwrap_or(false), within.as_deref())
            .map(rows_out)
            .map_err(|e| err(format!("sort: {e}")))
    }

    /// A new channel of `rows`, in that order (backlog C148): a filtered
    /// or sorted view, made in Rust.
    #[napi]
    pub fn take(&self, rows: Either<Uint32Array, Vec<u32>>) -> Result<Channel> {
        let inner = self
            .inner
            .take(&rows_of(&rows))
            .map_err(|e| err(format!("take: {e}")))?;
        Ok(Channel { inner })
    }

    /// The intervals `start .. start + len` (`len` left out: points) on
    /// the lanes column `lane` names (one lane without it), built once in
    /// Rust for `ChannelLanes.window` (ADR 0075): what a waterfall draws a
    /// million spans as, a window at a level of detail.
    #[napi]
    pub fn lanes(
        &self,
        start: Either<String, u32>,
        len: Option<Either<String, u32>>,
        lane: Option<Either<String, u32>>,
    ) -> Result<ChannelLanes> {
        let s = self.col(start)?;
        let l = len.map(|c| self.col(c)).transpose()?;
        let d = lane.map(|c| self.col(c)).transpose()?;
        let inner = self
            .inner
            .lanes(s, l, d)
            .map_err(|e| err(format!("lanes: {e}")))?;
        Ok(ChannelLanes { inner })
    }

    /// The handle: `{ $channel, rev }`, a value that rides in props, in
    /// messages and to a worker.
    #[napi]
    pub fn handle(&self) -> Value {
        serde_json::json!({ "$channel": self.inner.id(), "rev": self.inner.rev() })
    }

    /// What `JSON.stringify` writes: the handle, so a channel in a
    /// component's props crosses as one.
    #[napi(js_name = "toJSON")]
    pub fn to_json(&self) -> Value {
        self.handle()
    }
}
