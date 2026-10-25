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
