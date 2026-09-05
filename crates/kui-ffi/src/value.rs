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
