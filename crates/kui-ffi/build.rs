//! One job, in service of loading a C *extension* into a Rust host (see
//! `src/ext.rs`): the plugin is a shared library with the whole `kui_*` API
//! left undefined, so the host executable is what has to provide it at
//! `dlopen` time — and an executable exports nothing to the dynamic loader
//! unless it is linked to.
//!
//! Being *in* the executable is not the problem: rustc links every object of
//! this crate's rlib, so all 100 `#[no_mangle]` functions are there whether
//! the host calls them or not. Being *visible* is. GNU ld gives an executable
//! a dynamic symbol table holding only what it imports, and without
//! `--export-dynamic` the plugin's load fails with `undefined symbol:
//! kui_open`. Apple's linker exports an executable's globals already, so
//! there the flag pins behaviour the default happens to give.
//!
//! Examples only — nothing about the cdylib or a staticlib consumer wants
//! it. A host outside this crate passes its own equivalent.
//!
//! Nothing here fails loudly if it stops working: a host whose symbols are
//! not exported still builds, and the plugin only fails to load. The CI step
//! that runs `--example c_panel -- --headless` is the check.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    let flag = match std::env::var("CARGO_CFG_TARGET_OS")
        .unwrap_or_default()
        .as_str()
    {
        "macos" | "ios" => Some("-Wl,-export_dynamic"),
        // Windows resolves a plugin's imports through an import library
        // rather than the executable, so there is nothing to ask for.
        "windows" => None,
        _ => Some("-Wl,--export-dynamic"),
    };
    if let Some(flag) = flag {
        println!("cargo::rustc-link-arg-examples={flag}");
    }
}
