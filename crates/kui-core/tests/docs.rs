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

/// The page with every run of whitespace collapsed to one space. A hard
/// wrap is not a boundary in prose, and the line-by-line reading this
/// replaces was one word of rewrap away from missing every citation on the
/// page: `…take it (backlog\nF25 is that gap)` cited F25 and parsed as
/// nothing.
fn flattened(page: &str) -> String {
    let mut out = String::with_capacity(page.len());
    let mut gap = false;
    for ch in page.chars() {
        if ch.is_whitespace() {
            gap = true;
        } else {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.push(ch);
        }
    }
    out
}

/// Enough of the sentence around `at` to find it by eye in the page.
fn context(flat: &str, at: usize) -> String {
    let start = flat[..at].rfind(". ").map_or(0, |i| i + 2);
    let end = flat[at..]
        .find(". ")
        .map_or(flat.len(), |i| at + i + 1)
        .min(start + 240);
    flat[start..end.max(at)].trim().to_string()
}

/// Every backlog id `howto.md` cites, with the sentence it cites it in.
/// The spellings the page has used: `backlog F25`, `backlog entry F27`,
/// `backlog F27/F30`, `backlog C5(b)` — and any of them wrapped anywhere,
/// which is why this reads the flattened page rather than its lines.
fn cited(page: &str) -> Vec<(String, String)> {
    let flat = flattened(page);
    let lower = flat.to_ascii_lowercase();
    let bytes = flat.as_bytes();
    let mut out = Vec::new();
    for (at, _) in lower.match_indices("backlog ") {
        let mut i = at + "backlog ".len();
        for word in ["entry ", "entries ", "item ", "items "] {
            if lower[i..].starts_with(word) {
                i += word.len();
            }
        }
        // A run of ids: `F27/F30`, `F25 and F26`, `F25, F26`.
        while let Some(id) = id_at(bytes, i) {
            out.push((id.to_string(), context(&flat, at)));
            i += id.len();
            let rest = &lower[i..];
            i += match () {
                _ if rest.starts_with('/') || rest.starts_with('-') => 1,
                _ if rest.starts_with(", ") => 2,
                _ if rest.starts_with(" and ") => 5,
                _ => break,
            };
        }
    }
    out
}

/// The extractor is the half of this file that can fail open — a page that
/// cites nothing and a parser that finds nothing look the same from the
/// assertion. So it is pinned here, on the spellings the page has actually
/// used and on the wrap that defeated the first version of it.
#[test]
fn a_citation_is_found_however_the_page_wraps() {
    let found = |s: &str| -> Vec<String> { cited(s).into_iter().map(|(id, _)| id).collect() };
    assert_eq!(
        found("the first moment it is there (backlog F25 is that gap)"),
        ["F25"]
    );
    assert_eq!(
        found("the first moment it is\nthere (backlog F25 is that gap)"),
        ["F25"]
    );
    assert_eq!(
        found("the first moment it is there (backlog\nF25 is that gap)"),
        ["F25"]
    );
    assert_eq!(
        found("silence there means \"wrong tag\" (backlog F27 is the fix)"),
        ["F27"]
    );
    assert_eq!(found("Backlog entry F30 wants that as a promise"), ["F30"]);
    assert_eq!(found("backlog F27/F30 both want it"), ["F27", "F30"]);
    assert_eq!(found("backlog F25 and F26 are one round"), ["F25", "F26"]);
    assert_eq!(found("wait for backlog C5(b) to land"), ["C5(b)"]);
    assert!(found("the word backlog on its own cites nothing").is_empty());
    assert!(
        !cited("(backlog F25 is that gap)")[0].1.is_empty(),
        "a finding reports the sentence it was found in"
    );
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
fn howto_cites_no_closed_entry_and_every_anchor_lands() {
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

    // Both pages this one links into by anchor. `CHANGELOG.md` is here for
    // a drift the release round produces rather than the author: the open
    // section is `## <ver> (unreleased)` while it is being written and
    // `scripts/set-version.sh` dates the heading at the bump, so every
    // `#010-alphaN-unreleased` link stops landing between the last commit
    // and the tag — the F33 class again, through a link this time. The
    // script rewrites those links now; this is what says so if it ever
    // stops.
    let mut missing = Vec::new();
    for doc in ["props.md", "CHANGELOG.md"] {
        let target = if doc == "CHANGELOG.md" {
            read("../CHANGELOG.md")
        } else {
            read(doc)
        };
        let headings: Vec<String> = target
            .lines()
            .filter_map(|l| l.strip_prefix('#'))
            .map(|l| anchor(l.trim_start_matches('#')))
            .collect();
        let needle = format!("{doc}#");
        for line in page.lines() {
            for (at, _) in line.match_indices(&needle) {
                let rest = &line[at + needle.len()..];
                let end = rest
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                    .unwrap_or(rest.len());
                let want = &rest[..end];
                if !headings.iter().any(|h| h == want) {
                    missing.push(format!("  {doc}#{want} — {}", line.trim()));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "howto.md links anchors the target page has no heading for:\n{}",
        missing.join("\n")
    );
}
