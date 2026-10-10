//! The bodies of the fuzz targets under `fuzz_targets/`, one module each.
//!
//! A target binary is one line, `fuzz_target!(|data| kui_fuzz::path::run(data))`,
//! so what it checks lives here, where `tests/regressions.rs` can call it
//! on stable with the inputs a fuzzer once crashed on (see [`TARGETS`]).
//! Each `run` takes the raw bytes libFuzzer hands over and panics on a
//! finding: a panic in the code under test, or an `assert!` here on a
//! property the code promises in its docs.

pub mod decode;
pub mod parsers;
pub mod path;
pub mod scenes;
pub mod values;

/// What a target runs on one input.
pub type Run = fn(&[u8]);

/// Every target by name: the binary's name, the directory its regressions
/// are kept in, and the function both run.
pub const TARGETS: &[(&str, Run)] = &[
    ("parsers", parsers::run),
    ("values", values::run),
    ("path", path::run),
    ("scenes", scenes::run),
    ("decode", decode::run),
];
