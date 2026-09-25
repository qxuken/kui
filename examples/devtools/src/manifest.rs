//! The smoke rosters, read from what enrols an example rather than from
//! a list somebody keeps (ADR 0021, decision 4): a windowed example is an
//! `[[example]]` of `kui-native`, a headless one is named in its crate's
//! `[package.metadata.kui] headless = [...]`, and a Node example is in
//! `examples/node/package.json`'s `kui.windowed` / `kui.headless`. The
//! `smoke` binary runs what these answer and the pins in `lib.rs` check
//! every mirror of them against the same reading, so the two cannot
//! drift (backlog AR4).
//!
//! Read by line: a manifest is small, and this needs no parser to stay
//! honest about.

use std::path::{Path, PathBuf};

/// The workspace root: this crate is `examples/devtools`.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The crates whose manifests can enrol an example.
pub const CRATES: &[&str] = &["kui-native", "kui-core", "kui-ffi", "kui-lua"];

/// The `[[example]]` names and the `headless = [...]` list of one
/// manifest.
pub fn manifest(path: &Path) -> (Vec<String>, Vec<String>) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut examples = Vec::new();
    let mut headless = Vec::new();
    let mut in_example = false;
    let mut in_metadata = false;
    let mut in_headless = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_example = line == "[[example]]";
            in_metadata = line == "[package.metadata.kui]";
            continue;
        }
        if in_example && let Some(rest) = line.strip_prefix("name = \"") {
            examples.push(rest.trim_end_matches('"').to_string());
        }
        if in_metadata && line.starts_with("headless = [") {
            in_headless = !line.ends_with(']');
            for name in line["headless = [".len()..]
                .trim_end_matches(']')
                .split(',')
            {
                let name = name.trim().trim_matches('"');
                if !name.is_empty() {
                    headless.push(name.to_string());
                }
            }
            continue;
        }
        if in_headless {
            if line.starts_with(']') {
                in_headless = false;
            } else {
                let name = line.trim_end_matches(',').trim_matches('"');
                if !name.is_empty() {
                    headless.push(name.to_string());
                }
            }
        }
    }
    (examples, headless)
}

/// One crate's manifest.
pub fn crate_manifest(krate: &str) -> (Vec<String>, Vec<String>) {
    manifest(&root().join("crates").join(krate).join("Cargo.toml"))
}

/// The windowed round: every `[[example]]` of `kui-native`, sorted. All of them
/// open a window; the other crates' examples are the C and Lua panel
/// hosts (their rounds are `cbuild`'s) and kui-core's corpus
/// dump, which opens none.
pub fn windowed() -> Vec<String> {
    let (mut examples, _) = crate_manifest("kui-native");
    examples.sort();
    examples
}

/// The headless round: `(crate, example)` for every name in a
/// `headless = [...]` list, in manifest order.
pub fn headless() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for krate in CRATES {
        let (_, names) = crate_manifest(krate);
        out.extend(names.into_iter().map(|n| (krate.to_string(), n)));
    }
    out
}

/// One of `examples/node/package.json`'s `kui` rosters (`"windowed"` or
/// `"headless"`): the quoted `<dir>/<name>` entries of its array.
pub fn node_roster(which: &str) -> Vec<String> {
    let pkg = std::fs::read_to_string(root().join("examples/node/package.json"))
        .expect("examples/node/package.json");
    let needle = format!("\"{which}\": [");
    let Some(start) = pkg.find(&needle) else {
        return Vec::new();
    };
    let rest = &pkg[start + needle.len()..];
    let end = rest.find(']').unwrap_or(rest.len());
    rest[..end]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}
