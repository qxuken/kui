//! `RowHeights` for JavaScript: the core's own
//! `widgets::RowHeights`, held by the app the way a Rust app holds one, and
//! the slicing `widgets::list` does, in the three steps `index.js`'s
//! `list()` drives around the app's `measure` callback. The arithmetic —
//! the split prefix sums, the moving estimate and the anchors — is the
//! core's and nobody else's; what crosses is a reading in,
//! the rows to measure out, their heights in, and the plan out.

use kui_core::widgets::{ListReading, ListSlice, RowHeights as Heights};
use kui_core::{Rect, ScrollGeometry, Size, Vec2};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::{Value as Json, json};

/// The heights a `list()` slices by: a measured number per row where one is
/// known, the mean of those for every other row, and the prefix sums over
/// both. The app owns it and hands the same one back every frame; rebuild it
/// (or `clear()` it) when the rows change under the same indices, and
/// `setLen` it when rows are appended.
#[napi]
pub struct RowHeights {
    inner: Heights,
    /// The frame's slicing between `sliceBegin` and `sliceFinish`, and the
    /// rows the last step asked to have measured, in that order.
    slice: Option<ListSlice>,
    pending: Vec<usize>,
}

fn row(i: u32) -> usize {
    i as usize
}

#[napi]
impl RowHeights {
    /// `rows` rows, none measured, each standing at `estimate` logical px
    /// until it is. The estimate only has to be the right order of
    /// magnitude: it decides how wrong the scrollbar is before the list has
    /// been scrolled through, and nothing else.
    #[napi(constructor)]
    pub fn new(rows: u32, estimate: f64) -> Self {
        RowHeights {
            inner: Heights::new(row(rows), estimate as f32),
            slice: None,
            pending: Vec::new(),
        }
    }

    /// How many rows.
    #[napi(getter)]
    pub fn length(&self) -> u32 {
        self.inner.len() as u32
    }

    /// Grows or shrinks to `rows`, keeping what is still in range — rows
    /// appended to a log keep every height already measured.
    #[napi]
    pub fn set_len(&mut self, rows: u32) {
        self.inner.set_len(row(rows));
    }

    /// Forgets every measurement, keeping the length and the estimate —
    /// for a list whose rows changed under the same indices.
    #[napi]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Records row `i`'s height; the estimate for the others is the mean of
    /// the rows recorded this way.
    #[napi]
    pub fn set(&mut self, i: u32, h: f64) {
        self.inner.set(row(i), h as f32);
    }

    /// Row `i`'s height as measured, or null for one at the estimate.
    #[napi]
    pub fn measured(&self, i: u32) -> Option<f64> {
        self.inner.measured(row(i)).map(f64::from)
    }

    /// Row `i`'s height: measured, or the estimate.
    #[napi]
    pub fn get(&self, i: u32) -> f64 {
        f64::from(self.inner.get(row(i)))
    }

    /// What an unmeasured row stands at.
    #[napi]
    pub fn estimate(&self) -> f64 {
        f64::from(self.inner.estimate())
    }

    /// The whole list's height, measured and estimated together.
    #[napi]
    pub fn total(&self) -> f64 {
        f64::from(self.inner.total())
    }

    /// The top of row `i` in content coordinates. `ctx.setScroll(key, 0,
    /// heights.offsetOf(i))` puts row `i` at the top — exactly for a
    /// measured row, within a frame or two for one at the estimate.
    #[napi]
    pub fn offset_of(&mut self, i: u32) -> f64 {
        f64::from(self.inner.offset_of(row(i)))
    }

    /// The row content-coordinate `y` lands in.
    #[napi]
    pub fn row_at(&mut self, y: f64) -> u32 {
        self.inner.row_at(y as f32) as u32
    }

    /// `list()`'s first step: the reading, and back the rows to measure
    /// (`{ width, rows }`). Not for an app to call; `list()` is the loop.
    #[napi(
        ts_args_type = "reading: ListReading",
        ts_return_type = "{ width: number, rows: number[] }"
    )]
    pub fn slice_begin(&mut self, reading: Json) -> Result<Json> {
        let reading = parse_reading(&reading)?;
        let slice = self.inner.slice(reading);
        self.pending = slice.unmeasured(&self.inner);
        let width = slice.width();
        self.slice = Some(slice);
        Ok(json!({ "width": width, "rows": self.pending }))
    }

    /// `list()`'s middle step: the heights of the rows the last step asked
    /// for, in its order, and back the next rows to measure — empty once
    /// the window has settled.
    #[napi]
    pub fn slice_measured(&mut self, heights: Vec<f64>) -> Result<Vec<u32>> {
        let Some(slice) = self.slice.as_mut() else {
            return Err(Error::from_reason(
                "kui: RowHeights.sliceMeasured before sliceBegin — list() drives these",
            ));
        };
        if heights.len() != self.pending.len() {
            return Err(Error::from_reason(format!(
                "kui: list() measured {} rows where {} were asked for",
                heights.len(),
                self.pending.len()
            )));
        }
        for (&i, &h) in self.pending.iter().zip(&heights) {
            self.inner.set(i, h as f32);
        }
        self.pending = if slice.reslice(&mut self.inner) {
            slice.unmeasured(&self.inner)
        } else {
            Vec::new()
        };
        Ok(self.pending.iter().map(|&i| i as u32).collect())
    }

    /// `list()`'s last step: the rows to build (`first`..`last`) with each
    /// one's height, the spacers, and the scroll correction for
    /// `ctx.shiftScroll` (null when nothing moved).
    #[napi(ts_return_type = "ListPlan")]
    pub fn slice_finish(&mut self) -> Result<Json> {
        let Some(slice) = self.slice.take() else {
            return Err(Error::from_reason(
                "kui: RowHeights.sliceFinish before sliceBegin — list() drives these",
            ));
        };
        self.pending.clear();
        let plan = slice.finish(&mut self.inner);
        let heights: Vec<f32> = plan.range.clone().map(|i| self.inner.get(i)).collect();
        Ok(json!({
            "first": plan.range.start,
            "last": plan.range.end,
            "heights": heights,
            "lead": plan.lead,
            "tail": plan.tail,
            "shift": plan.shift.map(|(drawn, target)| json!({ "drawn": drawn, "target": target })),
            "firstFrame": plan.first_frame,
        }))
    }
}

fn num(v: &Json, name: &str) -> Result<f32> {
    v.get(name)
        .and_then(Json::as_f64)
        .map(|n| n as f32)
        .ok_or_else(|| Error::from_reason(format!("kui: a list reading needs a number `{name}`")))
}

/// `{ geometry, scrollY, viewportW, viewportH, padT, padX, overscan }`,
/// `geometry` being what `ctx.scrollGeometry(key)` answered (null before
/// the first layout).
fn parse_reading(v: &Json) -> Result<ListReading> {
    let geometry = match v.get("geometry") {
        None | Some(Json::Null) => None,
        Some(g) => {
            let off = |k: &str| -> Result<Vec2> {
                let o = g.get(k).ok_or_else(|| {
                    Error::from_reason(format!("kui: a list reading's geometry needs `{k}`"))
                })?;
                Ok(Vec2::new(num(o, "x")?, num(o, "y")?))
            };
            Some(ScrollGeometry {
                rect: Rect::new(num(g, "x")?, num(g, "y")?, num(g, "w")?, num(g, "h")?),
                content: Size::new(num(g, "contentW")?, num(g, "contentH")?),
                offset: off("offset")?,
                max_offset: off("maxOffset")?,
            })
        }
    };
    Ok(ListReading {
        geometry,
        scroll_y: num(v, "scrollY")?,
        viewport: Size::new(num(v, "viewportW")?, num(v, "viewportH")?),
        pad_t: num(v, "padT")?,
        pad_x: num(v, "padX")?,
        overscan: v.get("overscan").and_then(Json::as_u64).unwrap_or(2) as usize,
    })
}
