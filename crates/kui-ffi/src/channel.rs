//! Channels across the C boundary (ADR 0065): a builder a host fills one
//! column at a time, an opaque `KuiChannel` it holds for as long as the
//! data should live, a handle value for its props or messages, and the
//! readers a plugin uses on a handle it was given — by row and column,
//! never a copy of the whole.

use std::cell::RefCell;

use kui_core::channel::{Cell, Channel, Column, Strs};

use super::*;

/// Columns gathered for [`kui_channel_finish`].
pub struct KuiChannelBuilder {
    cols: Vec<(String, Column)>,
}

/// One revision of a channel, held: the data lives while any holder, in C
/// or any other binding, does.
pub struct KuiChannel(pub(crate) Channel);

thread_local! {
    /// Why the last [`kui_channel_finish`] on this thread answered NULL.
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

/// A builder with no columns yet. [`kui_channel_finish`] consumes it.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_new() -> *mut KuiChannelBuilder {
    Box::into_raw(Box::new(KuiChannelBuilder { cols: Vec::new() }))
}

fn add(b: *mut KuiChannelBuilder, name: KuiStr, col: impl FnOnce() -> Column) -> bool {
    guard(false, || {
        let Some(b) = (unsafe { b.as_mut() }) else {
            return false;
        };
        b.cols.push((kstr(name).into_owned(), col()));
        true
    })
}

/// Adds a column of `count` numbers, copied. False on a NULL builder.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_add_f64(
    b: *mut KuiChannelBuilder,
    name: KuiStr,
    values: *const f64,
    count: usize,
) -> bool {
    add(b, name, || {
        Column::F64(if values.is_null() {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(values, count) }.to_vec()
        })
    })
}

/// Adds a column of `count` integers, copied.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_add_i64(
    b: *mut KuiChannelBuilder,
    name: KuiStr,
    values: *const i64,
    count: usize,
) -> bool {
    add(b, name, || {
        Column::I64(if values.is_null() {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(values, count) }.to_vec()
        })
    })
}

/// Adds a column of `count` strings, copied into one buffer.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_add_str(
    b: *mut KuiChannelBuilder,
    name: KuiStr,
    values: *const KuiStr,
    count: usize,
) -> bool {
    add(b, name, || {
        let strs: Strs = if values.is_null() {
            Strs::default()
        } else {
            unsafe { std::slice::from_raw_parts(values, count) }
                .iter()
                .map(|s| kstr(*s))
                .collect()
        };
        Column::Str(strs)
    })
}

/// The channel the builder's columns make, yours until
/// [`kui_channel_free`]; the builder is consumed either way. NULL when the
/// columns are of two lengths or a name is given twice, with the reason
/// readable through [`kui_channel_error`].
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_finish(b: *mut KuiChannelBuilder) -> *mut KuiChannel {
    guard(std::ptr::null_mut(), || {
        if b.is_null() {
            return std::ptr::null_mut();
        }
        let b = unsafe { Box::from_raw(b) };
        match Channel::table(b.cols) {
            Ok(ch) => {
                LAST_ERROR.with_borrow_mut(String::clear);
                Box::into_raw(Box::new(KuiChannel(ch)))
            }
            Err(e) => {
                LAST_ERROR.with_borrow_mut(|s| *s = e.0);
                std::ptr::null_mut()
            }
        }
    })
}

/// Why the last [`kui_channel_finish`] on this thread answered NULL; false
/// (and `out` untouched) when it did not. Borrowed until the next call.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_error(out: *mut KuiStr) -> bool {
    guard(false, || {
        LAST_ERROR.with_borrow(|s| {
            if s.is_empty() || out.is_null() {
                return false;
            }
            unsafe { *out = KuiStr::borrowed(s) };
            true
        })
    })
}

/// The channel a handle names — a props entry, a message field — held
/// anew and yours until [`kui_channel_free`]; NULL when the value is no
/// handle or nobody holds the channel any more.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_resolve(handle: *const KuiValue) -> *mut KuiChannel {
    guard(std::ptr::null_mut(), || {
        let Some(v) = (unsafe { handle.as_ref() }) else {
            return std::ptr::null_mut();
        };
        Channel::resolve(&v.0).map_or(std::ptr::null_mut(), |ch| {
            Box::into_raw(Box::new(KuiChannel(ch)))
        })
    })
}

/// Lets go of a hold; the data goes when the last holder, anywhere, does.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_free(ch: *mut KuiChannel) {
    guard((), || {
        if !ch.is_null() {
            drop(unsafe { Box::from_raw(ch) });
        }
    })
}

/// A new value naming the channel — `{"$channel": id, "rev": rev}` — for
/// props or a message; free it with `kui_value_free` (or hand it to a call
/// that consumes one). It does not hold the channel.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_handle(ch: *const KuiChannel) -> *mut KuiValue {
    guard(std::ptr::null_mut(), || match unsafe { ch.as_ref() } {
        Some(ch) => Box::into_raw(Box::new(KuiValue(ch.0.handle()))),
        None => std::ptr::null_mut(),
    })
}

/// Rows; 0 for NULL.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_len(ch: *const KuiChannel) -> usize {
    guard(0, || unsafe { ch.as_ref() }.map_or(0, |c| c.0.len()))
}

/// Its revision: 1, and one more for each publish.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_rev(ch: *const KuiChannel) -> u64 {
    guard(0, || unsafe { ch.as_ref() }.map_or(0, |c| c.0.rev()))
}

/// The index of the column called `name`, or -1.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_column(ch: *const KuiChannel, name: KuiStr) -> i32 {
    guard(-1, || {
        unsafe { ch.as_ref() }
            .and_then(|c| c.0.col(&kstr(name)))
            .map_or(-1, |i| i as i32)
    })
}

/// Row `row` of column `col` as a number (an integer widened); false for
/// a string or boolean column, or out of range.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_get_f64(
    ch: *const KuiChannel,
    row: usize,
    col: usize,
    out: *mut f64,
) -> bool {
    guard(false, || {
        let (Some(c), false) = (unsafe { ch.as_ref() }, out.is_null()) else {
            return false;
        };
        match c.0.get(row, col).as_f64() {
            Some(x) => {
                unsafe { *out = x };
                true
            }
            None => false,
        }
    })
}

/// Row `row` of column `col` as a string, borrowed for as long as you hold
/// this `KuiChannel`; false for any other column, or out of range.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_get_str(
    ch: *const KuiChannel,
    row: usize,
    col: usize,
    out: *mut KuiStr,
) -> bool {
    guard(false, || {
        let (Some(c), false) = (unsafe { ch.as_ref() }, out.is_null()) else {
            return false;
        };
        match c.0.get(row, col) {
            Cell::Str(s) => {
                unsafe { *out = KuiStr::borrowed(s) };
                true
            }
            _ => false,
        }
    })
}

/// Doubles a bucket of [`kui_channel_reduce`] takes: the least, greatest,
/// first and last value, then the row each was read from.
pub const KUI_CHANNEL_BUCKET: usize = 8;

/// Column `col` over rows `from..to` in at most `buckets` buckets
/// (backlog C95), `KUI_CHANNEL_BUCKET` doubles each into `out`, which has
/// room for `buckets` of them; `*count` says how many were written, 0 for
/// an empty range. A flat array and not a struct, so the door adds no
/// layout to the ABI. False for NULL, no column `col`, or a column that is
/// not numbers, the reason in `kui_channel_error`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_reduce(
    ch: *const KuiChannel,
    col: usize,
    from: usize,
    to: usize,
    buckets: usize,
    out: *mut f64,
    count: *mut usize,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ch.as_ref() }) else {
            return false;
        };
        if count.is_null() || (out.is_null() && buckets > 0) {
            return false;
        }
        match c.0.reduce(col, from, to, buckets) {
            Ok(r) => {
                for (i, b) in r.iter().enumerate() {
                    let at = unsafe { out.add(i * KUI_CHANNEL_BUCKET) };
                    let row = [
                        b.min,
                        b.max,
                        b.first,
                        b.last,
                        b.min_row as f64,
                        b.max_row as f64,
                        b.first_row as f64,
                        b.last_row as f64,
                    ];
                    unsafe { std::ptr::copy_nonoverlapping(row.as_ptr(), at, KUI_CHANNEL_BUCKET) };
                }
                unsafe { *count = r.len() };
                true
            }
            Err(e) => {
                LAST_ERROR.with(|l| *l.borrow_mut() = e.0);
                false
            }
        }
    })
}

fn fail<T>(e: kui_core::channel::ChannelError, none: T) -> T {
    LAST_ERROR.with_borrow_mut(|s| *s = e.0);
    none
}

/// The next revision of `ch`'s identity: its rows, then the builder's
/// (backlog C147), each of `ch`'s columns named in the builder, of its
/// kind (integers widen into a number column) and all of one length. With
/// `cap` above 0, the oldest rows go past it. The builder is consumed
/// either way; `ch` is untouched and still yours. NULL on a mismatch, the
/// reason in [`kui_channel_error`].
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_append(
    ch: *const KuiChannel,
    rows: *mut KuiChannelBuilder,
    cap: usize,
) -> *mut KuiChannel {
    guard(std::ptr::null_mut(), || {
        if rows.is_null() {
            return std::ptr::null_mut();
        }
        let b = unsafe { Box::from_raw(rows) };
        let Some(c) = (unsafe { ch.as_ref() }) else {
            return std::ptr::null_mut();
        };
        match c.0.append(b.cols, (cap > 0).then_some(cap)) {
            Ok(next) => {
                LAST_ERROR.with_borrow_mut(String::clear);
                Box::into_raw(Box::new(KuiChannel(next)))
            }
            Err(e) => fail(e, std::ptr::null_mut()),
        }
    })
}

/// `within`'s rows, or none for NULL.
fn rows_in<'a>(within: *const usize, n: usize) -> Option<&'a [usize]> {
    (!within.is_null()).then(|| unsafe { std::slice::from_raw_parts(within, n) })
}

/// Writes `rows` into `out` and their count into `count`.
fn rows_out(rows: Vec<usize>, out: *mut usize, count: *mut usize) -> bool {
    unsafe {
        std::ptr::copy_nonoverlapping(rows.as_ptr(), out, rows.len());
        *count = rows.len();
    }
    true
}

fn query(
    ch: *const KuiChannel,
    out: *mut usize,
    count: *mut usize,
    run: impl FnOnce(&Channel) -> Result<Vec<usize>, kui_core::channel::ChannelError>,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ch.as_ref() }) else {
            return false;
        };
        if out.is_null() || count.is_null() {
            return false;
        }
        match run(&c.0) {
            Ok(rows) => rows_out(rows, out, count),
            Err(e) => fail(e, false),
        }
    })
}

/// The rows — all, or the `n_within` of `within` in its order — whose
/// number in column `col` is at least `min` and at most `max` (backlog
/// C148): `min == max` for one value, `-INFINITY` or `INFINITY` for an
/// open end; NaN is in no range. `out` has room for the channel's length,
/// or `n_within`; `*count` is how many were written. False for NULL, no
/// column `col`, a column that is not numbers, or a row past the end, the
/// reason in [`kui_channel_error`].
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_filter_f64(
    ch: *const KuiChannel,
    col: usize,
    min: f64,
    max: f64,
    within: *const usize,
    n_within: usize,
    out: *mut usize,
    count: *mut usize,
) -> bool {
    let m = if min == max {
        kui_core::channel::Match::Eq(kui_core::Value::Float(min))
    } else {
        kui_core::channel::Match::Range {
            min: min.is_finite().then_some(min),
            max: max.is_finite().then_some(max),
        }
    };
    query(ch, out, count, |c| {
        c.filter(col, &m, rows_in(within, n_within))
    })
}

/// A string filter's way: the cell is `text`.
pub const KUI_CHANNEL_EQUAL: i32 = 0;
/// The cell holds `text`, its letters' case as given.
pub const KUI_CHANNEL_CONTAINS: i32 = 1;
/// The cell holds `text`, whatever its letters' case.
pub const KUI_CHANNEL_CONTAINS_ANY_CASE: i32 = 2;

/// [`kui_channel_filter_f64`] for a string column: the rows whose cell is
/// `text` (`KUI_CHANNEL_EQUAL`) or holds it (`KUI_CHANNEL_CONTAINS`,
/// `KUI_CHANNEL_CONTAINS_ANY_CASE`).
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_filter_str(
    ch: *const KuiChannel,
    col: usize,
    text: KuiStr,
    how: i32,
    within: *const usize,
    n_within: usize,
    out: *mut usize,
    count: *mut usize,
) -> bool {
    let text = kstr(text).into_owned();
    let m = match how {
        KUI_CHANNEL_CONTAINS => kui_core::channel::Match::Contains { text, case: true },
        KUI_CHANNEL_CONTAINS_ANY_CASE => kui_core::channel::Match::Contains { text, case: false },
        _ => kui_core::channel::Match::Eq(kui_core::Value::Str(text)),
    };
    query(ch, out, count, |c| {
        c.filter(col, &m, rows_in(within, n_within))
    })
}

/// The rows — all, or `within`'s — ordered by column `col`, least first or
/// greatest with `descending`; ties keep their order, NaN goes last
/// (backlog C148). `out` and `*count` as [`kui_channel_filter_f64`]'s.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_sort(
    ch: *const KuiChannel,
    col: usize,
    descending: bool,
    within: *const usize,
    n_within: usize,
    out: *mut usize,
    count: *mut usize,
) -> bool {
    query(ch, out, count, |c| {
        c.sort(col, descending, rows_in(within, n_within))
    })
}

/// A new channel of the `n` rows `rows`, in that order, yours until
/// [`kui_channel_free`] (backlog C148). NULL for a row past the end, the
/// reason in [`kui_channel_error`].
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_take(
    ch: *const KuiChannel,
    rows: *const usize,
    n: usize,
) -> *mut KuiChannel {
    guard(std::ptr::null_mut(), || {
        let Some(c) = (unsafe { ch.as_ref() }) else {
            return std::ptr::null_mut();
        };
        match c.0.take(rows_in(rows, n).unwrap_or(&[])) {
            Ok(view) => Box::into_raw(Box::new(KuiChannel(view))),
            Err(e) => fail(e, std::ptr::null_mut()),
        }
    })
}

/// A channel's intervals on lanes (ADR 0075), yours until
/// [`kui_channel_lanes_free`].
pub struct KuiChannelLanes(kui_core::channel::Lanes);

/// Doubles a run of [`kui_channel_lanes_window`] takes: its start, its
/// end, its row (-1 for a strip of several) and how many intervals it
/// holds.
pub const KUI_CHANNEL_RUN: usize = 4;

/// The intervals of column `start` to `start + len` (`len` -1: points) on
/// the lanes column `lane` names (-1: one lane), built once for
/// [`kui_channel_lanes_window`]. NULL for NULL or a column that is not
/// numbers, the reason in [`kui_channel_error`].
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_lanes(
    ch: *const KuiChannel,
    start: usize,
    len: i32,
    lane: i32,
) -> *mut KuiChannelLanes {
    guard(std::ptr::null_mut(), || {
        let Some(c) = (unsafe { ch.as_ref() }) else {
            return std::ptr::null_mut();
        };
        let opt = |i: i32| (i >= 0).then_some(i as usize);
        match c.0.lanes(start, opt(len), opt(lane)) {
            Ok(l) => Box::into_raw(Box::new(KuiChannelLanes(l))),
            Err(e) => fail(e, std::ptr::null_mut()),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_lanes_free(l: *mut KuiChannelLanes) {
    guard((), || {
        if !l.is_null() {
            drop(unsafe { Box::from_raw(l) });
        }
    })
}

/// How many lanes; 0 for NULL.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_lanes_count(l: *const KuiChannelLanes) -> usize {
    guard(0, || unsafe { l.as_ref() }.map_or(0, |l| l.0.count()))
}

/// The earliest start and the latest end; false for NULL.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_lanes_extent(
    l: *const KuiChannelLanes,
    lo: *mut f64,
    hi: *mut f64,
) -> bool {
    guard(false, || {
        let Some(l) = (unsafe { l.as_ref() }) else {
            return false;
        };
        let (a, b) = l.0.extent();
        unsafe {
            if let Some(lo) = lo.as_mut() {
                *lo = a;
            }
            if let Some(hi) = hi.as_mut() {
                *hi = b;
            }
        }
        true
    })
}

/// Lane `lane`'s runs reaching into `[v0, v1]` at the detail where an
/// interval at least `min` long is a run of its own (a pixel's width: a
/// run a pixel), `KUI_CHANNEL_RUN` doubles each into `out`, which has room
/// for `room` runs; `*count` is how many were written, the first `room`
/// of them. False for NULL.
#[unsafe(no_mangle)]
pub extern "C" fn kui_channel_lanes_window(
    l: *const KuiChannelLanes,
    lane: usize,
    v0: f64,
    v1: f64,
    min: f64,
    out: *mut f64,
    room: usize,
    count: *mut usize,
) -> bool {
    guard(false, || {
        let Some(l) = (unsafe { l.as_ref() }) else {
            return false;
        };
        if count.is_null() || (out.is_null() && room > 0) {
            return false;
        }
        let mut runs = Vec::new();
        l.0.window(lane, v0, v1, min, &mut runs);
        let n = runs.len().min(room);
        for (i, r) in runs.iter().take(n).enumerate() {
            let row = [
                r.start,
                r.end,
                r.row.map_or(-1.0, |x| x as f64),
                r.count as f64,
            ];
            unsafe {
                std::ptr::copy_nonoverlapping(
                    row.as_ptr(),
                    out.add(i * KUI_CHANNEL_RUN),
                    KUI_CHANNEL_RUN,
                )
            };
        }
        unsafe { *count = n };
        true
    })
}
