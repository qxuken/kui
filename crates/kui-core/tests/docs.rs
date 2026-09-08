//! The shipped docs check themselves: `howto.md` is prose about a moving
//! library, and two field reports in a row found it contradicting the release
//! it shipped in. Nothing here reads the code — these are the two claims a
//! page makes that another file in the same commit can settle.

use std::path::{Path, PathBuf};

fn docs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs")
}

fn read(name: &str) -> String {
    let path = docs_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// An id-shaped token at `bytes[at..]` — `F25`, `C5(b)`, `P8a` — or None.
/// Letters, then digits, then an optional lowercase suffix, then an optional
/// `(x)`, which is how the backlog spells a part of a split entry.
fn id_at(bytes: &[u8], at: usize) -> Option<&str> {
    let mut i = at;
    while bytes.get(i).is_some_and(u8::is_ascii_uppercase) {
        i += 1;
    }
    if i == at {
        return None;
    }
    let digits = i;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    if i == digits {
        return None;
    }
    if bytes.get(i).is_some_and(u8::is_ascii_lowercase) {
        i += 1;
    }
    if bytes.get(i) == Some(&b'(')
        && bytes.get(i + 1).is_some_and(u8::is_ascii_lowercase)
        && bytes.get(i + 2) == Some(&b')')
    {
        i += 3;
    }
    std::str::from_utf8(&bytes[at..i]).ok()
}

/// Every backlog id `howto.md` cites, with the line it cites it on. The
/// spelling the page uses is `backlog F25` / `backlog C5(b)`.
fn cited(page: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in page.lines() {
        let bytes = line.as_bytes();
        for (at, _) in line.match_indices("backlog ") {
            if let Some(id) = id_at(bytes, at + "backlog ".len()) {
                out.push((id.to_string(), line.trim().to_string()));
            }
        }
    }
    out
}

/// Every id the "## Closed — index" section lists as `**ID**`.
fn closed(backlog: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in backlog.lines() {
        if line.starts_with("## ") {
            inside = line.starts_with("## Closed");
            continue;
        }
        if !inside {
            continue;
        }
        let bytes = line.as_bytes();
        for (at, _) in line.match_indices("**") {
            let start = at + 2;
            if let Some(id) = id_at(bytes, start)
                && line[start + id.len()..].starts_with("**")
            {
                out.push(id.to_string());
            }
        }
    }
    out
}

/// A GitHub heading anchor: lowercased, punctuation dropped, spaces hyphened.
fn anchor(heading: &str) -> String {
    let mut out = String::new();
    for c in heading.trim().chars() {
        match c {
            ' ' => out.push('-'),
            '-' | '_' => out.push(c),
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => {}
        }
    }
    out
}

/// A backlog id in a "today" sentence is the drift two field reports hit:
/// the page was written against the tree before the release it shipped in,
/// and cites as an open gap what that release closed. And a `props.md` link
/// that no longer lands should fail here rather than in a reader's browser.
#[test]
fn howto_cites_no_closed_entry_and_every_props_anchor_lands() {
    let page = read("howto.md");
    let closed = closed(&read("BACKLOG.md"));
    assert!(
        closed.len() > 20,
        "the closed index parsed as {} entries — the section moved",
        closed.len()
    );

    let stale: Vec<_> = cited(&page)
        .into_iter()
        .filter(|(id, _)| closed.iter().any(|c| c == id))
        .collect();
    assert!(
        stale.is_empty(),
        "howto.md cites {} closed backlog {} as an open gap; \
         a citation is for work that has not shipped:\n{}",
        stale.len(),
        if stale.len() == 1 { "entry" } else { "entries" },
        stale
            .iter()
            .map(|(id, line)| format!("  {id} — {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let props = read("props.md");
    let headings: Vec<String> = props
        .lines()
        .filter_map(|l| l.strip_prefix('#'))
        .map(|l| anchor(l.trim_start_matches('#')))
        .collect();
    let mut missing = Vec::new();
    for line in page.lines() {
        for (at, _) in line.match_indices("props.md#") {
            let rest = &line[at + "props.md#".len()..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .unwrap_or(rest.len());
            let want = &rest[..end];
            if !headings.iter().any(|h| h == want) {
                missing.push(format!("  props.md#{want} — {}", line.trim()));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "howto.md links anchors props.md has no heading for:\n{}",
        missing.join("\n")
    );
}
