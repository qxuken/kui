//! `Channel` for Node (ADR 0065): bulk data the core owns and a handle
//! names. `new Channel({ name: [...], ms: Float64Array })` builds one in a
//! single call; it serializes as its handle, so `props: { rows: channel }`
//! hands a hundred thousand rows to a component as one small value, and a
//! Lua or C reader resolves the same handle without a copy. `reduce`
//! buckets a range for a chart in Rust (backlog C95).

use kui_core::channel::{self, Cell, Column};
use napi::bindgen_prelude::{Either, Float64Array, Object, Uint32Array};
use napi_derive::napi;
use serde_json::Value as Json;

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
        let v: Option<Either<Float64Array, Vec<Json>>> = columns.get(&name)?;
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

fn cell_json(c: Cell<'_>) -> Json {
    match c {
        Cell::Null => Json::Null,
        Cell::F64(x) => serde_json::Number::from_f64(x).map_or(Json::Null, Json::Number),
        Cell::I64(x) => Json::from(x),
        Cell::Bool(x) => Json::Bool(x),
        Cell::Str(s) => Json::String(s.to_string()),
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
    pub fn resolve(handle: Json) -> Result<Self> {
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
    pub fn get(&self, row: u32, col: Either<String, u32>) -> Result<Json> {
        let c = self.col(col)?;
        Ok(cell_json(self.inner.get(row as usize, c)))
    }

    /// One column whole: a `Float64Array` for numbers, an array otherwise.
    #[napi]
    pub fn column(&self, col: Either<String, u32>) -> Result<Either<Float64Array, Vec<Json>>> {
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

    /// The handle: `{ $channel, rev }`, a value that rides in props, in
    /// messages and to a worker.
    #[napi]
    pub fn handle(&self) -> Json {
        serde_json::json!({ "$channel": self.inner.id(), "rev": self.inner.rev() })
    }

    /// What `JSON.stringify` writes: the handle, so a channel in a
    /// component's props crosses as one.
    #[napi(js_name = "toJSON")]
    pub fn to_json(&self) -> Json {
        self.handle()
    }
}
