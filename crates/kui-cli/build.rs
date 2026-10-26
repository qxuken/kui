//! The docs the runner carries (ADR 0071, decision 2): props.md, howto.md,
//! Kit's component reference, the book's chapters as text and the ADRs,
//! gzipped into one bundle `src/docs.rs` includes. Each entry is its path
//! in the bundle and its text, both behind a little-endian u32 length.

use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let mut files: Vec<(String, PathBuf)> = vec![
        ("props.md".into(), root.join("docs/props.md")),
        ("howto.md".into(), root.join("docs/howto.md")),
        ("kit.md".into(), root.join("addons/kit/bindings/reference.md")),
    ];
    for (dir, under) in [("docs/book/src", "book"), ("docs/adr", "adr")] {
        let dir = root.join(dir);
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "md"))
            .collect();
        names.sort();
        for p in names {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            files.push((format!("{under}/{name}"), p));
        }
    }
    let mut raw = Vec::new();
    for (name, path) in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        let mut text = read(path);
        if name.starts_with("book/") {
            text = expand_includes(&text, path.parent().unwrap());
        }
        // howto.md links the book's contents where the repository keeps
        // them; the bundle keeps the chapters under `book/`.
        let text = if name == "howto.md" { text.replace("](book/src/", "](book/") } else { text };
        for part in [name.as_bytes(), text.as_bytes()] {
            raw.extend_from_slice(&(part.len() as u32).to_le_bytes());
            raw.extend_from_slice(part);
        }
    }
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    gz.write_all(&raw).unwrap();
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("docs.bin.gz");
    std::fs::write(&out, gz.finish().unwrap()).unwrap();
}

/// A chapter's mdBook `{{#include file:anchor}}` and `{{#rustdoc_include
/// …}}` replaced by what they include — the lines between `ANCHOR: name`
/// and `ANCHOR_END: name`, or the whole file — so the text reads whole
/// without the examples beside it.
fn expand_includes(text: &str, dir: &Path) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("{{#") {
        out.push_str(&rest[..at]);
        let Some(end) = rest[at..].find("}}") else { break };
        let directive = &rest[at + 3..at + end];
        rest = &rest[at + end + 2..];
        let Some(arg) = directive
            .strip_prefix("include ")
            .or_else(|| directive.strip_prefix("rustdoc_include "))
        else {
            out.push_str("{{#");
            out.push_str(directive);
            out.push_str("}}");
            continue;
        };
        let (file, anchor) = match arg.trim().split_once(':') {
            Some((f, a)) => (f, Some(a)),
            None => (arg.trim(), None),
        };
        let path = dir.join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        let source = read(&path);
        let lines: Vec<&str> = match anchor {
            None => source.lines().collect(),
            Some(a) => {
                let start = format!("ANCHOR: {a}");
                let end = format!("ANCHOR_END: {a}");
                source
                    .lines()
                    .skip_while(|l| !l.contains(&start))
                    .skip(1)
                    .take_while(|l| !l.contains(&end))
                    .collect()
            }
        };
        let kept: Vec<&str> = lines.into_iter().filter(|l| !l.contains("ANCHOR")).collect();
        out.push_str(&kept.join("\n"));
    }
    out.push_str(rest);
    out
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}
