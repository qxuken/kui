//! `row_heights(rows, estimate)` for Lua: the core's own
//! `widgets::RowHeights` as userdata the script keeps between views, as a
//! Rust app keeps one, and the slicing `widgets::list` does in the three
//! steps the prelude's `list` drives around the script's `measure`. The
//! arithmetic — the split prefix sums, the moving estimate and the
//! anchors — is the core's; what crosses is a reading in, the
//! rows to measure out, their heights in, and the plan out.

use kui_core::widgets::{ListReading, ListSlice, RowHeights};
use kui_core::{Rect, ScrollGeometry, Size, Vec2};
use mlua::{Lua, Table, UserData, UserDataMethods};

/// The userdata: the heights, and the frame's slicing between
/// `slice_begin` and `slice_finish` with the rows the last step asked for.
pub(crate) struct LuaRowHeights {
    inner: RowHeights,
    slice: Option<ListSlice>,
    pending: Vec<usize>,
}

/// A row index from Lua: 0-based, as `uniform_list`'s and `index` are.
fn row(i: i64) -> usize {
    i.max(0) as usize
}

impl UserData for LuaRowHeights {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method("len", |_, this, ()| Ok(this.inner.len()));
        m.add_method_mut("set_len", |_, this, rows: i64| {
            this.inner.set_len(row(rows));
            Ok(())
        });
        m.add_method_mut("clear", |_, this, ()| {
            this.inner.clear();
            Ok(())
        });
        m.add_method_mut("set", |_, this, (i, h): (i64, f32)| {
            this.inner.set(row(i), h);
            Ok(())
        });
        m.add_method("measured", |_, this, i: i64| {
            Ok(this.inner.measured(row(i)))
        });
        m.add_method("get", |_, this, i: i64| Ok(this.inner.get(row(i))));
        m.add_method("estimate", |_, this, ()| Ok(this.inner.estimate()));
        m.add_method("total", |_, this, ()| Ok(this.inner.total()));
        m.add_method_mut("offset_of", |_, this, i: i64| {
            Ok(this.inner.offset_of(row(i)))
        });
        m.add_method_mut("row_at", |_, this, y: f32| Ok(this.inner.row_at(y)));

        // The three steps `list` drives. Not for a script to call.
        m.add_method_mut("slice_begin", |lua, this, reading: Table| {
            let slice = this.inner.slice(parse_reading(&reading)?);
            this.pending = slice.unmeasured(&this.inner);
            let width = slice.width();
            this.slice = Some(slice);
            Ok((width, rows_table(lua, &this.pending)?))
        });
        m.add_method_mut("slice_measured", |lua, this, heights: Vec<f32>| {
            let Some(slice) = this.slice.as_mut() else {
                return Err(mlua::Error::runtime(
                    "slice_measured before slice_begin -- list() drives these",
                ));
            };
            if heights.len() != this.pending.len() {
                return Err(mlua::Error::runtime(format!(
                    "list() measured {} rows where {} were asked for",
                    heights.len(),
                    this.pending.len()
                )));
            }
            for (&i, &h) in this.pending.iter().zip(&heights) {
                this.inner.set(i, h);
            }
            this.pending = if slice.reslice(&mut this.inner) {
                slice.unmeasured(&this.inner)
            } else {
                Vec::new()
            };
            rows_table(lua, &this.pending)
        });
        m.add_method_mut("slice_finish", |lua, this, ()| {
            let Some(slice) = this.slice.take() else {
                return Err(mlua::Error::runtime(
                    "slice_finish before slice_begin -- list() drives these",
                ));
            };
            this.pending.clear();
            let plan = slice.finish(&mut this.inner);
            let t = lua.create_table()?;
            t.set("first", plan.range.start)?;
            t.set("last", plan.range.end)?;
            let heights = lua.create_table()?;
            for (n, i) in plan.range.clone().enumerate() {
                heights.set(n + 1, this.inner.get(i))?;
            }
            t.set("heights", heights)?;
            t.set("lead", plan.lead)?;
            t.set("tail", plan.tail)?;
            if let Some((drawn, target)) = plan.shift {
                let s = lua.create_table()?;
                s.set("drawn", drawn)?;
                s.set("target", target)?;
                t.set("shift", s)?;
            }
            t.set("first_frame", plan.first_frame)?;
            Ok(t)
        });
    }
}

fn rows_table(lua: &Lua, rows: &[usize]) -> mlua::Result<Table> {
    lua.create_sequence_from(rows.iter().copied())
}

fn num(t: &Table, name: &str) -> mlua::Result<f32> {
    t.get::<Option<f32>>(name)?
        .ok_or_else(|| mlua::Error::runtime(format!("a list reading needs a number `{name}`")))
}

fn vec2(t: &Table, name: &str) -> mlua::Result<Vec2> {
    let o: Table = t.get(name)?;
    Ok(Vec2::new(num(&o, "x")?, num(&o, "y")?))
}

/// `{ geometry, scroll_y, viewport_w, viewport_h, pad_t, pad_x, overscan }`,
/// `geometry` being what `env.scroll_geometry(key)` answered (nil before
/// the first layout).
fn parse_reading(t: &Table) -> mlua::Result<ListReading> {
    let geometry = match t.get::<Option<Table>>("geometry")? {
        None => None,
        Some(g) => Some(ScrollGeometry {
            rect: Rect::new(num(&g, "x")?, num(&g, "y")?, num(&g, "w")?, num(&g, "h")?),
            content: Size::new(num(&g, "content_w")?, num(&g, "content_h")?),
            offset: vec2(&g, "offset")?,
            max_offset: vec2(&g, "max_offset")?,
        }),
    };
    Ok(ListReading {
        geometry,
        scroll_y: num(t, "scroll_y")?,
        viewport: Size::new(num(t, "viewport_w")?, num(t, "viewport_h")?),
        pad_t: num(t, "pad_t")?,
        pad_x: num(t, "pad_x")?,
        overscan: t.get::<Option<i64>>("overscan")?.unwrap_or(2).max(0) as usize,
    })
}

/// `row_heights(rows, estimate)`: the global a script makes its heights
/// with, once, and keeps.
pub(crate) fn register(lua: &Lua) -> mlua::Result<()> {
    lua.globals().set(
        "row_heights",
        lua.create_function(|_, (rows, estimate): (i64, Option<f32>)| {
            Ok(LuaRowHeights {
                inner: RowHeights::new(row(rows), estimate.unwrap_or(20.0)),
                slice: None,
                pending: Vec::new(),
            })
        })?,
    )
}
