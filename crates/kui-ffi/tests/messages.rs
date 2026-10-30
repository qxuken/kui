//! Message constructors (F283): the former inline helpers and the direct
//! entry points produce the same maps, without the temporary boxed values.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ffi::{CStr, CString};

use kui_ffi::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Counts {
    alloc: usize,
    free: usize,
    realloc: usize,
}

thread_local! {
    // Thread-local so other tests and libtest itself cannot affect a count.
    static COUNTS: Cell<Option<Counts>> = const { Cell::new(None) };
}

struct Counting;

fn count(f: impl FnOnce(&mut Counts)) {
    let _ = COUNTS.try_with(|cell| {
        if let Some(mut counts) = cell.get() {
            f(&mut counts);
            cell.set(Some(counts));
        }
    });
}

// Forward every allocation unchanged; only count while this thread opts in.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(|c| c.alloc += 1);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(|c| c.alloc += 1);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        count(|c| c.free += 1);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(|c| c.realloc += 1);
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn measure<T>(f: impl FnOnce() -> T) -> (T, Counts) {
    struct Stop;
    impl Drop for Stop {
        fn drop(&mut self) {
            COUNTS.set(None);
        }
    }
    COUNTS.set(Some(Counts::default()));
    let stop = Stop;
    let result = f();
    let counts = COUNTS.get().unwrap();
    drop(stop);
    (result, counts)
}

struct Message(*mut KuiValue);

impl Drop for Message {
    fn drop(&mut self) {
        kui_value_free(self.0);
    }
}

fn ks(s: &str) -> KuiStr {
    KuiStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

fn cs(s: &CStr) -> KuiStr {
    KuiStr {
        ptr: s.to_bytes().as_ptr(),
        len: s.to_bytes().len(),
    }
}

fn old_msg(kind: &CStr) -> Message {
    let m = kui_value_map();
    kui_value_map_set(m, ks("kind"), kui_value_str(cs(kind)));
    Message(m)
}

fn old_msg_int(kind: &CStr, field: &CStr, n: i64) -> Message {
    let m = old_msg(kind);
    kui_value_map_set(m.0, cs(field), kui_value_int(n));
    m
}

#[derive(Debug, PartialEq)]
enum Scalar {
    Str(String),
    Int(i64),
}

fn string(s: KuiStr) -> String {
    // Every view here is borrowed from a live map whose strings are UTF-8.
    unsafe { std::str::from_utf8(std::slice::from_raw_parts(s.ptr, s.len)) }
        .unwrap()
        .to_owned()
}

fn entries(m: &Message) -> Vec<(String, Scalar)> {
    assert!(!m.0.is_null());
    (0..kui_value_len(m.0))
        .map(|i| {
            let mut key: KuiStr = unsafe { std::mem::zeroed() };
            let value = kui_value_entry(m.0, i, &mut key);
            let mut text: KuiStr = unsafe { std::mem::zeroed() };
            let mut n = 0;
            let scalar = if kui_value_as_str(value, &mut text) {
                Scalar::Str(string(text))
            } else {
                assert!(kui_value_as_int(value, &mut n));
                Scalar::Int(n)
            };
            (string(key), scalar)
        })
        .collect()
}

#[test]
fn direct_messages_match_the_inline_helpers_including_replacement_and_order() {
    // ASCII, Unicode, empty strings, malformed UTF-8 and a literal NUL
    // before more bytes: conversion and C-string semantics stay the same.
    let kinds: &[&CStr] = &[
        c"row",
        c"récolter",
        c"",
        c"bad\xff",
        CStr::from_bytes_until_nul(b"a\0ignored\0").unwrap(),
    ];
    let fields: &[&CStr] = &[c"line", c"行", c"", c"kind", c"bad\xfe"];
    for kind in kinds {
        assert_eq!(
            entries(&Message(kui_msg(kind.as_ptr()))),
            entries(&old_msg(kind))
        );
        for field in fields {
            for n in [i64::MIN, -1, 0, 42, i64::MAX] {
                let m = Message(kui_msg_int(kind.as_ptr(), field.as_ptr(), n));
                assert_eq!(entries(&m), entries(&old_msg_int(kind, field, n)));
                // Callers can still extend and replace fields afterwards.
                kui_value_map_set(m.0, ks("more"), kui_value_int(7));
                kui_value_map_set(m.0, ks("more"), kui_value_int(8));
                assert_eq!(entries(&m).last(), Some(&("more".into(), Scalar::Int(8))));
            }
        }
    }
}

#[test]
fn messages_own_their_strings_and_do_not_borrow_the_callers_buffers() {
    let kind = CString::new("pick").unwrap();
    let field = CString::new("row").unwrap();
    let m = Message(kui_msg_int(kind.as_ptr(), field.as_ptr(), 7));
    drop(kind);
    drop(field);
    assert_eq!(
        entries(&m),
        vec![
            ("kind".into(), Scalar::Str("pick".into())),
            ("row".into(), Scalar::Int(7)),
        ]
    );
}

#[test]
fn null_names_are_rejected_without_allocating() {
    let ((), counts) = measure(|| {
        assert!(kui_msg(std::ptr::null()).is_null());
        assert!(kui_msg_int(std::ptr::null(), c"row".as_ptr(), 7).is_null());
        assert!(kui_msg_int(c"pick".as_ptr(), std::ptr::null(), 7).is_null());
    });
    assert_eq!(counts, Counts::default());
}

#[test]
fn constructing_messages_eliminates_temporary_boxes_and_frees() {
    let (old, old_counts) = measure(|| old_msg_int(c"pick", c"row", 7));
    let (new, new_counts) = measure(|| Message(kui_msg_int(c"pick".as_ptr(), c"row".as_ptr(), 7)));
    assert_eq!(entries(&old), entries(&new));
    assert_eq!(
        old_counts,
        Counts {
            alloc: 7,
            free: 2,
            realloc: 0
        }
    );
    assert_eq!(
        new_counts,
        Counts {
            alloc: 5,
            free: 0,
            realloc: 0
        }
    );
    let (old, old_counts) = measure(|| old_msg(c"pick"));
    let (new, new_counts) = measure(|| Message(kui_msg(c"pick".as_ptr())));
    assert_eq!(entries(&old), entries(&new));
    assert_eq!(
        old_counts,
        Counts {
            alloc: 5,
            free: 1,
            realloc: 0
        }
    );
    assert_eq!(
        new_counts,
        Counts {
            alloc: 4,
            free: 0,
            realloc: 0
        }
    );
}
