//! `KuiValue`: the caller-owned payloads that ride on events and clicks.

use super::*;

// ---------------------------------------------------------------------------
// Values

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_null() -> *mut KuiValue {
    Box::into_raw(Box::new(KuiValue(Value::Null)))
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_bool(v: bool) -> *mut KuiValue {
    Box::into_raw(Box::new(KuiValue(Value::Bool(v))))
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_int(v: i64) -> *mut KuiValue {
    Box::into_raw(Box::new(KuiValue(Value::Int(v))))
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_float(v: f64) -> *mut KuiValue {
    Box::into_raw(Box::new(KuiValue(Value::Float(v))))
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_str(s: KuiStr) -> *mut KuiValue {
    guard(std::ptr::null_mut(), || {
        Box::into_raw(Box::new(KuiValue(Value::Str(kstr(s).into_owned()))))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_map() -> *mut KuiValue {
    Box::into_raw(Box::new(KuiValue(Value::Map(Vec::new()))))
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_list() -> *mut KuiValue {
    Box::into_raw(Box::new(KuiValue(Value::List(Vec::new()))))
}

/// Appends to a list value. Consumes `val`; on anything but a list it is
/// dropped and nothing changes.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_list_push(list: *mut KuiValue, val: *mut KuiValue) {
    guard((), || {
        if val.is_null() {
            return;
        }
        let val = unsafe { Box::from_raw(val) };
        let Some(list) = (unsafe { list.as_mut() }) else {
            return;
        };
        if let Value::List(items) = &mut list.0 {
            items.push(val.0);
        }
    });
}

/// Sets `key` on a map value. Consumes `val`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_map_set(map: *mut KuiValue, key: KuiStr, val: *mut KuiValue) {
    guard((), || {
        if val.is_null() {
            return;
        }
        let val = unsafe { Box::from_raw(val) };
        let Some(map) = (unsafe { map.as_mut() }) else {
            return;
        };
        if let Value::Map(entries) = &mut map.0 {
            let key = kstr(key).into_owned();
            if let Some(e) = entries.iter_mut().find(|(k, _)| *k == key) {
                e.1 = val.0;
            } else {
                entries.push((key, val.0));
            }
        }
    });
}

/// Borrowed lookup on a map value; NULL if absent. Valid as long as the map.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_get(v: *const KuiValue, key: KuiStr) -> *const KuiValue {
    guard(std::ptr::null(), || {
        let Some(v) = (unsafe { v.as_ref() }) else {
            return std::ptr::null();
        };
        match v.0.get(&kstr(key)) {
            // Value and KuiValue are layout-identical (single field).
            Some(inner) => (inner as *const Value).cast(),
            None => std::ptr::null(),
        }
    })
}

/// How many entries a list or map holds; 0 for anything else, which a
/// scalar reader tells apart from an empty one.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_len(v: *const KuiValue) -> usize {
    guard(0, || match unsafe { v.as_ref() }.map(|v| &v.0) {
        Some(Value::List(items)) => items.len(),
        Some(Value::Map(entries)) => entries.len(),
        _ => 0,
    })
}

/// Borrowed entry `i` of a list value (`preedit`'s `cursor`, the two ends
/// an `access` request carries); NULL past the end or on anything but a
/// list. Valid as long as the list.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_at(v: *const KuiValue, i: usize) -> *const KuiValue {
    guard(std::ptr::null(), || {
        match unsafe { v.as_ref() }.map(|v| &v.0) {
            // Value and KuiValue are layout-identical (single field).
            Some(Value::List(items)) => items
                .get(i)
                .map_or(std::ptr::null(), |inner| (inner as *const Value).cast()),
            _ => std::ptr::null(),
        }
    })
}

/// A map's `i`th entry, for walking one whose keys you do not know: the
/// key into `*key` and the borrowed value returned, NULL past the end or
/// on anything but a map. Entries keep the order they were set in.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_entry(
    v: *const KuiValue,
    i: usize,
    key: *mut KuiStr,
) -> *const KuiValue {
    guard(std::ptr::null(), || {
        match unsafe { v.as_ref() }.map(|v| &v.0) {
            Some(Value::Map(entries)) => match entries.get(i) {
                Some((k, inner)) => {
                    if !key.is_null() {
                        unsafe {
                            key.write(KuiStr {
                                ptr: k.as_ptr(),
                                len: k.len(),
                            })
                        };
                    }
                    (inner as *const Value).cast()
                }
                None => std::ptr::null(),
            },
            _ => std::ptr::null(),
        }
    })
}

/// The number a payload carries, as a double: a float as it is, an
/// integer widened. The reader for everything geometry-shaped an event
/// carries - a drag's `x`/`dx`, a layout's rect, a resize's `scale` -
/// which kui_value_as_int would truncate. False for anything else.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_as_float(v: *const KuiValue, out: *mut f64) -> bool {
    guard(false, || {
        let (Some(v), Some(out)) = (unsafe { v.as_ref() }, unsafe { out.as_mut() }) else {
            return false;
        };
        match v.0.as_float() {
            Some(f) => {
                *out = f;
                true
            }
            None => false,
        }
    })
}

/// A payload's boolean - a key event's `shift` / `ctrl` / `alt` / `super`
/// / `repeat`, a modifiers event's four. False - and `*out` untouched -
/// for anything that is not one; a bool is never coerced from a number.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_as_bool(v: *const KuiValue, out: *mut bool) -> bool {
    guard(false, || {
        let (Some(v), Some(out)) = (unsafe { v.as_ref() }, unsafe { out.as_mut() }) else {
            return false;
        };
        match v.0.as_bool() {
            Some(b) => {
                *out = b;
                true
            }
            None => false,
        }
    })
}

/// Whether the value is the null one: what a key event's `text` is on a
/// release, and a missing `tag`. A NULL pointer answers true too, so a
/// `kui_value_get` miss reads the same as an explicit null.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_is_null(v: *const KuiValue) -> bool {
    guard(true, || match unsafe { v.as_ref() } {
        Some(v) => matches!(v.0, Value::Null),
        None => true,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_as_int(v: *const KuiValue, out: *mut i64) -> bool {
    guard(false, || {
        let (Some(v), Some(out)) = (unsafe { v.as_ref() }, unsafe { out.as_mut() }) else {
            return false;
        };
        match v.0.as_int() {
            Some(i) => {
                *out = i;
                true
            }
            None => false,
        }
    })
}

/// Borrowed string view; valid as long as the value.
#[unsafe(no_mangle)]
pub extern "C" fn kui_value_as_str(v: *const KuiValue, out: *mut KuiStr) -> bool {
    guard(false, || {
        let (Some(v), Some(out)) = (unsafe { v.as_ref() }, unsafe { out.as_mut() }) else {
            return false;
        };
        match v.0.as_str() {
            Some(s) => {
                *out = KuiStr {
                    ptr: s.as_ptr(),
                    len: s.len(),
                };
                true
            }
            None => false,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_value_free(v: *mut KuiValue) {
    if !v.is_null() {
        drop(unsafe { Box::from_raw(v) });
    }
}
