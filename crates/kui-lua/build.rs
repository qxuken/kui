//! One job: let `lua_panel` load the C panel. The Lua host example is a
//! Rust executable with `kui-ffi` linked in for `CExtension`, and a plugin
//! on the unixes leaves every `kui_*` undefined to take them from the
//! executable that loaded it — which an executable does not offer unless it
//! is built to (`kui-ffi/build.rs` is the long version). Without the flag
//! rustc's dead-stripping drops the 187 unreferenced entry points and the
//! script's third pane reads `symbol not found '_kui_button'`; the file said
//! that was expected on the unixes from the day it was written, and it was
//! only ever a missing link arg. Windows is not here: the shape that loads
//! into any host there is `panel.dll`, which imports `kui_ffi.dll` by name,
//! so the host has nothing to export and only has to ship the DLL.
//!
//! Examples only, and only when the example's source is there: `cargo
//! package` leaves a source outside the crate directory out of the tarball
//! and strips the `[[example]]`, and cargo rejects `rustc-link-arg-examples`
//! from a crate with no example target — which is the publish verify build.

/// Where `[[example]] lua_panel` in Cargo.toml points, relative to this crate.
const EXAMPLE: &str = "../../examples/lua/features/slots/panel.rs";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed={EXAMPLE}");
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    if !std::path::Path::new(&manifest_dir).join(EXAMPLE).is_file() {
        return;
    }
    let flag = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") | Ok("ios") => "-Wl,-export_dynamic",
        Ok("windows") => return,
        _ => "-Wl,--export-dynamic",
    };
    println!("cargo::rustc-link-arg-examples={flag}");
}
