//! The docs this runner carries, for its own version (ADR 0071, decisions
//! 1 and 2): props.md, howto.md, Kit's component reference, the book's
//! chapters as text and the ADRs, gzipped into the binary by `build.rs`.
//! `kui docs <query>` searches the first three, `kui docs --adr <n>` prints
//! an ADR, and `--docs` writes them all into `.kui/docs`.

use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;

static BUNDLE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/docs.bin.gz"));

/// The book's root: the workspace's `homepage`. It is published from each
/// release tag to the one address, so it is the latest release's book;
/// there is no copy per version.
pub const BOOK: &str = env!("CARGO_PKG_HOMEPAGE");

/// One carried file: its path under `.kui/docs` and its text.
pub struct Doc {
    pub path: String,
    pub text: String,
}

/// Every carried file, unpacked on first use.
pub fn all() -> &'static [Doc] {
    static DOCS: OnceLock<Vec<Doc>> = OnceLock::new();
    DOCS.get_or_init(|| {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(BUNDLE)
            .read_to_end(&mut raw)
            .expect("the docs bundle built into the runner unpacks");
        let mut docs = Vec::new();
        let mut at = 0;
        let next = |at: &mut usize| {
            let n = u32::from_le_bytes(raw[*at..*at + 4].try_into().unwrap()) as usize;
            let s = String::from_utf8_lossy(&raw[*at + 4..*at + 4 + n]).into_owned();
            *at += 4 + n;
            s
        };
        while at < raw.len() {
            let path = next(&mut at);
            let text = next(&mut at);
            docs.push(Doc { path, text });
        }
        docs
    })
}

fn get(path: &str) -> &'static str {
    all()
        .iter()
        .find(|d| d.path == path)
        .map(|d| d.text.as_str())
        .unwrap_or("")
}

/// Writes every carried file under `dir` (`.kui/docs`), replacing what an
/// earlier version wrote there.
pub fn write(dir: &Path) -> std::io::Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    for d in all() {
        let p = dir.join(&d.path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(p, &d.text)?;
    }
    Ok(())
}

/// The ADRs, by number: `(n, title, text)`.
pub fn adrs() -> Vec<(u32, String, &'static str)> {
    all()
        .iter()
        .filter_map(|d| {
            let name = d.path.strip_prefix("adr/")?;
            let n: u32 = name.get(..4)?.parse().ok()?;
            let title = d
                .text
                .lines()
                .find_map(|l| l.strip_prefix("# "))
                .unwrap_or(name)
                .to_string();
            Some((n, title, d.text.as_str()))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Search

/// Which document an entry is from.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Source {
    Props,
    Howto,
    Kit,
}

/// One searchable entry: a props.md row, a howto question, a Kit section.
pub struct Entry {
    pub source: Source,
    /// The props.md section, the howto part; empty for Kit.
    pub section: String,
    /// The row's name, the question, the component.
    pub title: String,
    /// Names an exact query hits: a prop's spellings, a component's name.
    pub names: Vec<String>,
    /// For a row: its table's header and cells. For a section: its body.
    pub header: Vec<String>,
    pub cells: Vec<String>,
    pub body: String,
}

/// Every searchable entry, parsed once.
pub fn entries() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        let mut out = props_rows(get("props.md"));
        out.extend(sections(get("howto.md"), Source::Howto));
        out.extend(sections(get("kit.md"), Source::Kit));
        out
    })
}

/// A table row's cells, split at pipes no backslash escapes.
fn split_row(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut prev = '\0';
    for c in inner.chars() {
        if c == '|' && prev != '\\' {
            cells.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
        prev = c;
    }
    cells.push(cur.trim().to_string());
    cells
}

fn is_separator(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("|-") || t.starts_with("| -") || t.starts_with("|:")
}

/// The code spans of a cell that is only code spans: `hoverBg` → hoverBg.
fn code_names(cell: &str) -> Vec<String> {
    cell.split('`')
        .enumerate()
        .filter(|(i, s)| i % 2 == 1 && !s.is_empty())
        .map(|(_, s)| s.to_string())
        .collect()
}

fn props_rows(text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut section = String::new();
    let mut header: Vec<String> = Vec::new();
    let mut prev = "";
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            section = h.trim().to_string();
            header.clear();
        } else if is_separator(line) {
            header = split_row(prev);
        } else if line.starts_with('|') && !header.is_empty() {
            let cells = split_row(line);
            let first = clean(&cells[0]).replace('`', "");
            let mut names: Vec<String> = Vec::new();
            for c in cells.iter().take(3) {
                names.extend(code_names(c));
            }
            if names.is_empty() {
                names.push(first.clone());
            }
            out.push(Entry {
                source: Source::Props,
                section: section.clone(),
                title: first,
                names,
                header: header.clone(),
                cells,
                body: String::new(),
            });
        } else if !line.starts_with('|') {
            header.clear();
        }
        prev = line;
    }
    out
}

/// howto.md's `###` questions under their `##` part; kit.md's `## kit.x`
/// components and `###` types.
fn sections(text: &str, source: Source) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    let mut part = String::new();
    let mut fence = false;
    let mut open: Option<Entry> = None;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        }
        let heading = if fence {
            None
        } else if let Some(h) = line.strip_prefix("### ") {
            Some((3, h.trim()))
        } else {
            line.strip_prefix("## ").map(|h| (2, h.trim()))
        };
        if let Some((level, h)) = heading {
            if let Some(e) = open.take() {
                out.push(e);
            }
            let entry = match (source, level) {
                (Source::Howto, 2) => {
                    part = h.to_string();
                    None
                }
                (Source::Kit, 2) if !h.contains('.') => None,
                _ => Some(h.to_string()),
            };
            open = entry.map(|title| Entry {
                source,
                section: if source == Source::Howto {
                    part.clone()
                } else {
                    String::new()
                },
                // `kit.table` is found as `table` too.
                names: match title.split_once('.') {
                    Some((_, bare)) if source == Source::Kit => {
                        vec![title.clone(), bare.to_string()]
                    }
                    _ => vec![title.clone()],
                },
                title,
                header: Vec::new(),
                cells: Vec::new(),
                body: String::new(),
            });
            continue;
        }
        if let Some(e) = open.as_mut() {
            e.body.push_str(line);
            e.body.push('\n');
        }
    }
    out.extend(open);
    out
}

const STOP: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "can", "do", "does", "for", "from", "how",
    "i", "in", "is", "it", "its", "me", "my", "of", "on", "or", "so", "that", "the", "to", "what",
    "when", "why", "with", "you",
];

/// Lowercased, with `_`, `-` and spaces dropped: `hover_bg`, `hoverBg`
/// and `hover-bg` are one name.
fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '_' | '-' | ' ' | '`'))
        .flat_map(char::to_lowercase)
        .collect()
}

/// One entry and how well it matched.
pub struct Hit {
    pub entry: &'static Entry,
    pub score: i64,
}

/// The entries matching `query`, best first. Every word of the query
/// (but the little ones) has to appear; when no entry has them all, the
/// entries with the most of them.
pub fn search(query: &str) -> Vec<Hit> {
    let lower = query.to_lowercase();
    let all_words: Vec<&str> = lower
        .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-' || c == '$'))
        .filter(|w| !w.is_empty())
        .collect();
    let mut words: Vec<&str> = all_words
        .iter()
        .copied()
        .filter(|w| !STOP.contains(w))
        .collect();
    if words.is_empty() {
        words = all_words;
    }
    let q = norm(query);
    let mut scored: Vec<(usize, Hit)> = Vec::new();
    for e in entries() {
        let mut name = 0;
        for n in &e.names {
            let n = norm(n);
            name = name.max(if n == q {
                1000
            } else if q.len() >= 3 && n.starts_with(&q) {
                300
            } else if q.len() >= 3 && n.contains(&q) {
                150
            } else {
                0
            });
        }
        let title = e.title.to_lowercase();
        let body = if e.cells.is_empty() {
            e.body.to_lowercase()
        } else {
            e.cells.join(" ").to_lowercase()
        };
        let mut matched = 0;
        let mut s = 0;
        for w in &words {
            let in_title = title.contains(w);
            let count = body.matches(w).count() as i64;
            if in_title {
                s += 40;
            }
            if count > 0 {
                s += 10 + count.min(10);
            }
            if in_title || count > 0 {
                matched += 1;
            }
        }
        if name == 0 && matched == 0 {
            continue;
        }
        scored.push((
            matched,
            Hit {
                entry: e,
                score: name + s,
            },
        ));
    }
    let best = scored
        .iter()
        .map(|(m, h)| if h.score >= 1000 { words.len() } else { *m })
        .max()
        .unwrap_or(0);
    let mut hits: Vec<Hit> = scored
        .into_iter()
        .filter(|(m, h)| h.score >= 150 || *m == best)
        .map(|(_, h)| h)
        .collect();
    hits.sort_by_key(|h| std::cmp::Reverse(h.score));
    hits
}

/// The ADRs whose title has every word of the query.
pub fn adrs_matching(query: &str) -> Vec<(u32, String)> {
    let lower = query.to_lowercase();
    let words: Vec<&str> = lower
        .split_whitespace()
        .filter(|w| !STOP.contains(w))
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    adrs()
        .into_iter()
        .filter(|(_, t, _)| {
            let t = t.to_lowercase();
            words.iter().all(|w| t.contains(w))
        })
        .map(|(n, t, _)| (n, t))
        .collect()
}

// ---------------------------------------------------------------------------
// Printing

/// Markdown made readable in a terminal: links to their text, escaped
/// pipes plain.
fn clean(s: &str) -> String {
    let s = s
        .replace("\\\\|", "|")
        .replace("\\|", "|")
        .replace("<br>", " ");
    let mut out = String::with_capacity(s.len());
    let mut rest = s.as_str();
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find("](").map(|c| open + c) else {
            break;
        };
        let Some(end) = rest[close..].find(')').map(|e| close + e) else {
            break;
        };
        if rest[open + 1..close].contains('[') {
            out.push_str(&rest[..open + 1]);
            rest = &rest[open + 1..];
            continue;
        }
        out.push_str(&rest[..open]);
        out.push_str(&rest[open + 1..close]);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// `text` wrapped at `width`, every line behind `indent`.
fn wrap(text: &str, indent: &str, width: usize) -> String {
    let mut out = String::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty()
            && indent.len() + line.chars().count() + 1 + word.chars().count() > width
        {
            out.push_str(indent);
            out.push_str(&line);
            out.push('\n');
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push_str(indent);
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// A section's Markdown for a terminal: paragraphs and list items
/// wrapped, a table's rows as one item each, code as it is.
fn render_body(body: &str, width: usize) -> String {
    let mut out = String::new();
    let mut para = String::new();
    let mut fence = false;
    let flush = |para: &mut String, out: &mut String| {
        if !para.trim().is_empty() {
            let text = clean(para.trim());
            if let Some(item) = text.strip_prefix("- ") {
                let w = wrap(item, "    ", width);
                out.push_str("  - ");
                out.push_str(w.trim_start());
            } else {
                out.push_str(&wrap(&text, "  ", width));
            }
        }
        para.clear();
    };
    let mut header_seen = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            flush(&mut para, &mut out);
            fence = !fence;
            continue;
        }
        if fence {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let t = line.trim();
        if t.starts_with('|') {
            flush(&mut para, &mut out);
            if is_separator(t) {
                header_seen = true;
                continue;
            }
            if !header_seen {
                continue;
            }
            let cells: Vec<String> = split_row(t).iter().map(|c| clean(c)).collect();
            let last = cells.len().saturating_sub(1);
            let mut item = cells[0].clone();
            let middle: Vec<&str> = cells[1..last.max(1)]
                .iter()
                .map(String::as_str)
                .filter(|c| !c.is_empty() && *c != "—")
                .collect();
            if !middle.is_empty() && cells[1] != cells[0] {
                item.push_str(&format!(" ({})", middle.join(" · ")));
            } else if middle.len() > 1 {
                item.push_str(&format!(" {}", middle[1..].join(" · ")));
            }
            if last > 0 {
                item.push_str(" — ");
                item.push_str(&cells[last]);
            }
            let w = wrap(&item, "      ", width);
            out.push_str("    ");
            out.push_str(w.trim_start());
            continue;
        }
        header_seen = false;
        if t.is_empty() {
            flush(&mut para, &mut out);
        } else {
            if t.starts_with("- ") {
                flush(&mut para, &mut out);
            }
            para.push_str(t);
            para.push(' ');
        }
    }
    flush(&mut para, &mut out);
    out
}

/// One hit as the terminal prints it.
pub fn render(hit: &Hit, width: usize) -> String {
    let e = hit.entry;
    let mut out = String::new();
    match e.source {
        Source::Props => {
            out.push_str(&format!("{} — props.md › {}\n", e.title, e.section));
            let last = e.cells.len().saturating_sub(1);
            let described = matches!(
                e.header.last().map(String::as_str),
                Some("description" | "notes" | "when" | "meaning")
            );
            let upto = if described { last } else { e.cells.len() };
            let facts: Vec<String> = (1..upto)
                .filter(|&i| !e.cells[i].is_empty())
                .map(|i| {
                    let label = e.header.get(i).map(String::as_str).unwrap_or("");
                    format!("{label} {}", clean(&e.cells[i]).replace('`', ""))
                })
                .collect();
            if !facts.is_empty() {
                out.push_str(&wrap(&facts.join(" · "), "  ", width));
            }
            if described && last > 0 {
                out.push_str(&wrap(&clean(&e.cells[last]), "  ", width));
            }
        }
        Source::Howto => {
            out.push_str(&format!("{} — howto.md › {}\n", e.title, e.section));
            out.push_str(&render_body(&e.body, width));
        }
        Source::Kit => {
            out.push_str(&format!("{} — Kit's component reference\n", e.title));
            out.push_str(&render_body(&e.body, width));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundle_carries_every_document() {
        for p in [
            "props.md",
            "howto.md",
            "kit.md",
            "book/runner.md",
            "adr/0071-docs-that-travel-with-kui.md",
        ] {
            assert!(!get(p).is_empty(), "{p} is not in the bundle");
        }
        assert!(
            adrs()
                .iter()
                .any(|(n, t, _)| *n == 68 && t.contains("reaches a machine"))
        );
        // The book's includes are expanded: the chapter reads whole.
        let runner = get("book/runner.md");
        assert!(
            !runner.contains("{{#include") && runner.contains("function view"),
            "{runner}"
        );
    }

    #[test]
    fn an_exact_name_comes_first_in_either_spelling() {
        for q in ["hoverBg", "hover_bg"] {
            let hits = search(q);
            assert_eq!(hits[0].entry.title, "hoverBg", "{q}");
            assert_eq!(hits[0].entry.source, Source::Props);
        }
        let hits = search("kit.button");
        assert_eq!(hits[0].entry.title, "kit.button");
    }

    #[test]
    fn a_question_finds_its_howto_entry() {
        let hits = search("animate a removal");
        assert!(
            hits.iter()
                .take(5)
                .any(|h| h.entry.source == Source::Howto
                    && h.entry.title.contains("animate a removal")),
            "{:?}",
            hits.iter()
                .take(5)
                .map(|h| &h.entry.title)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_table_row_prints_its_facts_and_description() {
        let hits = search("hoverBg");
        let text = render(&hits[0], 80);
        assert!(
            text.starts_with("hoverBg — props.md › Container props\n"),
            "{text}"
        );
        assert!(
            text.contains("Lua hover_bg") && text.contains("Background while the pointer"),
            "{text}"
        );
        assert!(
            text.lines()
                .all(|l| l.chars().count() <= 80 || !l.contains(' ')),
            "{text}"
        );
    }
}
