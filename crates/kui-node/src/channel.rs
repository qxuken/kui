//! `Channel` for Node (ADR 0065): bulk data the core owns and a handle
//! names. `new Channel({ name: [...], ms: Float64Array })` builds one in a
//! single call; it serializes as its handle, so `props: { rows: channel }`
//! hands a hundred thousand rows to a component as one small value, and a
//! Lua or C reader resolves the same handle without a copy.

use kui_core::channel::{self, Cell, Column};
use napi::bindgen_prelude::{Either, Float64Array, Object};
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
