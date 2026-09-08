//! One job, in service of loading a C *extension* into a Rust host (see
//! `src/ext.rs`): the plugin leaves the whole `kui_*` API undefined, so the
//! host executable is what has to provide it at load time — and an
//! executable does not offer its symbols to a plugin unless it is built to.
//!
//! Being *in* the executable is not the problem: rustc links every object of
//! this crate's rlib, so all 135 `#[no_mangle]` functions are there whether
//! the host calls them or not. Being *reachable from the plugin* is, and
//! each platform asks for it differently.
//!
//! **ELF and Mach-O.** GNU ld gives an executable a dynamic symbol table
//! holding only what it imports, and without `--export-dynamic` the plugin's
//! load fails with `undefined symbol: kui_open`. Apple's linker exports an
//! executable's globals already, so there the flag pins behaviour the
//! default happens to give.
//!
//! **Windows.** A DLL cannot have an unresolved import: the plugin names the
//! module each `kui_*` comes from in its own import table, and it gets that
//! name from an import library at link time. So the host has to *export* the
//! symbols, which makes link.exe write the `c_panel.lib` beside the exe that
//! `examples/c/build.ps1` then links the plugin against. `/DEF:` is how they
//! are named — one `/EXPORT:` link arg each would do the same, but 135 of
//! them do not survive rustc's quoting, and a response file is quoted too.
//! (rustc's own spelling of this is `-Z export-executable-symbols`; it is
//! unstable, and this file goes the day it is not.)
//!
//! The list is read out of this crate's own sources rather than kept here:
//! the exports are exactly the `pub extern "C" fn kui_*` set, and a list
//! that had to be edited by hand would be a list that went stale the first
//! time a function was added. `cargo build` reruns this when `src/` changes.
//!
//! One consequence worth knowing, and it is Windows' rather than kui's: an
//! import library names the module it imports from, so a plugin linked
//! against `c_panel.lib` loads into `c_panel.exe` and not into some other
//! host. A host outside this crate exports its own `kui_*` the same way and
//! ships its own import library for its plugins to link against; on ELF the
//! same plugin binary would have loaded into either.
//!
//! Examples only — nothing about the cdylib or a staticlib consumer wants
//! it. A host outside this crate passes its own equivalent.
//!
//! Nothing here fails loudly if it stops working: a host whose symbols are
//! not exported still builds, and the plugin only fails to load. The CI step
//! that runs `--example c_panel -- --headless` is the check.
//!
//! The example's source sits outside this crate's directory, so `cargo
//! package` leaves it out of the tarball and strips the `[[example]]` from
//! the packaged manifest. Cargo then rejects `rustc-link-arg-examples` from a
//! crate with no example target, which is exactly what `cargo publish`'s
//! verify build is. So the flag is only emitted when the source is there.

use std::fmt::Write as _;

/// Where `[[example]] c_panel` in Cargo.toml points, relative to this crate.
const EXAMPLE: &str = "../../examples/c/panel.rs";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed={EXAMPLE}");
    println!("cargo::rerun-if-changed=src");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    if !std::path::Path::new(&manifest_dir).join(EXAMPLE).is_file() {
        return;
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let flag = match target_os.as_str() {
        "macos" | "ios" => "-Wl,-export_dynamic".to_string(),
        "windows" => match export_def(&manifest_dir) {
            Some(def) => format!("/DEF:{def}"),
            // No list, no flag: the host still builds and only the plugin
            // fails to load, which is what the paragraph above promises.
            None => return,
        },
        _ => "-Wl,--export-dynamic".to_string(),
    };
    println!("cargo::rustc-link-arg-examples={flag}");
}

/// Writes a module-definition file naming every `kui_*` this crate exports,
/// and answers where it went. The set is read from the sources because that
/// is where it is decided; `dumpbin /exports` on the cdylib agrees with it,
/// which is how it was checked.
fn export_def(manifest_dir: &str) -> Option<String> {
    let src = std::path::Path::new(manifest_dir).join("src");
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&src).ok()? {
        let path = entry.ok()?.path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        // Every file, whatever feature it sits behind: `run.rs` is only
        // compiled with `runner`, but so is the example this flag is for
        // (`required-features`), so a build without the feature has no
        // host to link and nothing reads the list.
        let text = std::fs::read_to_string(&path).ok()?;
        for rest in text.split("pub extern \"C\" fn ").skip(1) {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if name.starts_with("kui_") {
                names.push(name);
            }
        }
    }
    if names.is_empty() {
        return None;
    }
    names.sort_unstable();
    names.dedup();

    let mut def = String::from(
        "; Generated by build.rs - the host's kui_* exports, so that\n\
                                ; link.exe writes the import library a C plugin links against.\n\
                                EXPORTS\n",
    );
    for name in &names {
        let _ = writeln!(def, "    {name}");
    }
    let out = std::path::Path::new(&std::env::var("OUT_DIR").ok()?).join("kui-host-exports.def");
    std::fs::write(&out, def).ok()?;
    // Forward slashes: the flag travels as a string through cargo and
    // rustc before link.exe sees it, and nothing on that path owes a
    // backslash any respect.
    Some(
        out.to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/"),
    )
}
