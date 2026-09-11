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
//! **The second job** (backlog AR2) reads the same set one level deeper —
//! each entry point's parameter and return types, as the source spells
//! them — and writes `abi_rows.rs` into `OUT_DIR`: one `abi_fn!` row per
//! function, which `src/abi_parity.rs` includes. The row used to be typed
//! by hand beside the function and the header, one line of restatement
//! per entry point; it is derived now, and the pin it carries is unchanged
//! — the row coerces the function to the signature it spells, so a
//! misread here is a compile error in the test build, never a wrong
//! prototype in the generated C. A file whose module is `#[cfg]`-gated in
//! lib.rs (`run.rs` behind `runner`) has its rows gated the same way.
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
const EXAMPLE: &str = "../../examples/c/features/slots/panel.rs";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed={EXAMPLE}");
    println!("cargo::rerun-if-changed=src");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let entries = entry_points(&manifest_dir);
    write_abi_rows(&manifest_dir, &entries);
    if !std::path::Path::new(&manifest_dir).join(EXAMPLE).is_file() {
        return;
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let flag = match target_os.as_str() {
        "macos" | "ios" => "-Wl,-export_dynamic".to_string(),
        "windows" => match export_def(&entries) {
            Some(def) => format!("/DEF:{def}"),
            // No list, no flag: the host still builds and only the plugin
            // fails to load, which is what the paragraph above promises.
            None => return,
        },
        _ => "-Wl,--export-dynamic".to_string(),
    };
    println!("cargo::rustc-link-arg-examples={flag}");
}

/// One `pub extern "C" fn kui_*` of the sources: which file, its name,
/// its parameter types and its return type, spelled as the source does.
struct Entry {
    file: String,
    name: String,
    params: Vec<String>,
    ret: Option<String>,
}

/// Every entry point, read from the sources because that is where the set
/// is decided; `dumpbin /exports` on the cdylib agrees with it, which is
/// how it was checked. Every file, whatever feature it sits behind:
/// `run.rs` is only compiled with `runner`, but so is the example the
/// export list is for (`required-features`), and the rows it yields are
/// gated by `write_abi_rows`.
fn entry_points(manifest_dir: &str) -> Vec<Entry> {
    let src = std::path::Path::new(manifest_dir).join("src");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&src)
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "rs"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut out = Vec::new();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        // Line comments go first: one parameter list carries a comment
        // (`kui_cells`), and a comment may quote a signature.
        let text: String = text
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        for marker in ["pub extern \"C\" fn ", "pub unsafe extern \"C\" fn "] {
            for rest in text.split(marker).skip(1) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !name.starts_with("kui_") {
                    continue;
                }
                let after = rest[name.len()..].trim_start();
                let Some(after) = after.strip_prefix('(') else {
                    continue;
                };
                // The parameter list, to its matching paren.
                let mut depth = 1usize;
                let mut end = 0;
                for (i, c) in after.char_indices() {
                    match c {
                        '(' | '<' | '[' => depth += 1,
                        ')' | '>' | ']' => {
                            depth -= 1;
                            if depth == 0 {
                                end = i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let list = &after[..end];
                let tail = after[end + 1..].split('{').next().unwrap_or("");
                let ret = tail
                    .trim()
                    .strip_prefix("->")
                    .map(|r| r.split_whitespace().collect::<Vec<_>>().join(" "));
                let params = split_top(list)
                    .into_iter()
                    .filter_map(|p| {
                        // `name: Type`; the first `:` that is not a `::`.
                        let bytes = p.as_bytes();
                        let colon = (0..bytes.len()).find(|&i| {
                            bytes[i] == b':'
                                && bytes.get(i + 1) != Some(&b':')
                                && (i == 0 || bytes[i - 1] != b':')
                        })?;
                        Some(
                            p[colon + 1..]
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" "),
                        )
                    })
                    .collect();
                out.push(Entry {
                    file: file.clone(),
                    name,
                    params,
                    ret,
                });
            }
        }
    }
    out
}

/// A comma-separated list split at its top level: a `<`, `(` or `[` keeps
/// the commas inside it.
fn split_top(list: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut cur = String::new();
    for c in list.chars() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out.into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The `#[cfg(...)]` a `mod x;` in lib.rs sits behind, by file name.
fn module_cfgs(manifest_dir: &str) -> Vec<(String, String)> {
    let lib = std::path::Path::new(manifest_dir).join("src/lib.rs");
    let text = std::fs::read_to_string(lib).unwrap_or_default();
    let mut out = Vec::new();
    let mut pending: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(cfg) = line
            .strip_prefix("#[cfg(")
            .and_then(|l| l.strip_suffix(")]"))
        {
            pending = Some(cfg.to_string());
            continue;
        }
        if let Some(name) = line.strip_prefix("mod ").and_then(|l| l.strip_suffix(';'))
            && let Some(cfg) = pending.take()
        {
            out.push((format!("{name}.rs"), cfg));
            continue;
        }
        pending = None;
    }
    out
}

/// `OUT_DIR/abi_rows.rs`: one `abi_fn!(o, n, name(types) -> ret);` per
/// entry point, as one block expression so `include!` takes it whole.
fn write_abi_rows(manifest_dir: &str, entries: &[Entry]) {
    let Ok(out_dir) = std::env::var("OUT_DIR") else {
        return;
    };
    let cfgs = module_cfgs(manifest_dir);
    let mut rows = String::from(
        "// Generated by build.rs from the `pub extern \"C\" fn kui_*` signatures in\n\
         // src/ (backlog AR2); `src/abi_parity.rs` includes it. Not committed.\n{\n",
    );
    for e in entries {
        if let Some((_, cfg)) = cfgs.iter().find(|(f, _)| *f == e.file) {
            let _ = writeln!(rows, "    #[cfg({cfg})]");
        }
        let ret = e.ret.as_ref().map_or(String::new(), |r| format!(" -> {r}"));
        let _ = writeln!(
            rows,
            "    abi_fn!(o, n, {}({}){});",
            e.name,
            e.params.join(", "),
            ret
        );
    }
    rows.push_str("}\n");
    let path = std::path::Path::new(&out_dir).join("abi_rows.rs");
    let _ = std::fs::write(path, rows);
}

/// Writes a module-definition file naming every `kui_*` this crate exports,
/// and answers where it went.
fn export_def(entries: &[Entry]) -> Option<String> {
    let mut names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
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
