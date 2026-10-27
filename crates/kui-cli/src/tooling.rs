//! What `kui new` and `kui types` write into an app's folder for its
//! editor and the coding agent beside it, and what keeps it current (ADR
//! 0071, decisions 3 to 6): `.kui/types` always, `.kui/docs` with
//! `--docs`, the agent block with `--agents`, the choice remembered as
//! `tooling` in the manifest, and `.kui/version` naming the runner that
//! wrote them, so `kui run` and `kui check` rewrite them after an upgrade.

use std::path::{Path, PathBuf};

use crate::compat::VERSION;
use crate::project::{self, Tooling};

/// `.kui/version`: the runner that wrote `.kui/`.
const STAMP: &str = "version";

/// The agent block's markers. kui rewrites what is between them and
/// nothing else of the file.
const BEGIN: &str = "<!-- kui:begin";
const END: &str = "<!-- kui:end -->";

/// `.kui/types`, `.kui/.gitignore` and the stamp.
pub fn write_types(dir: &Path) -> Result<PathBuf, String> {
    let kui = dir.join(".kui");
    let types = kui.join("types");
    crate::teal::write_types(&types).map_err(|e| format!("{}: {e}", types.display()))?;
    std::fs::write(kui.join(".gitignore"), "*\n").map_err(|e| format!("{}: {e}", kui.display()))?;
    stamp_now(dir)?;
    Ok(types)
}

/// `.kui/docs`: every document the runner carries.
pub fn write_docs(dir: &Path) -> Result<PathBuf, String> {
    let docs = dir.join(".kui").join("docs");
    crate::docs::write(&docs).map_err(|e| format!("{}: {e}", docs.display()))?;
    stamp_now(dir)?;
    Ok(docs)
}

fn stamp_now(dir: &Path) -> Result<(), String> {
    let p = dir.join(".kui").join(STAMP);
    std::fs::write(&p, format!("{VERSION}\n")).map_err(|e| format!("{}: {e}", p.display()))
}

/// The version that wrote `.kui/`, if one did.
pub fn stamp(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join(".kui").join(STAMP))
        .ok()
        .map(|s| s.trim().to_string())
}

/// `kui run` and `kui check`: when `.kui/` was written by another runner,
/// `.kui/types` again — if the app has them, or opted into more — and
/// `.kui/docs` and the agent block when its manifest opted into them, in
/// one line on stderr. Nothing the app did not opt into is created. A
/// failure is said and the run goes on.
pub fn refresh(p: &project::Project) {
    let dir = &p.dir;
    let has_types = dir.join(".kui").join("types").is_dir();
    if !has_types && !p.tooling.any() {
        return;
    }
    let was = stamp(dir);
    if was.as_deref() == Some(VERSION) {
        return;
    }
    let mut done = Vec::new();
    let mut run = || -> Result<(), String> {
        write_types(dir)?;
        done.push(".kui/types".to_string());
        if p.tooling.any() {
            write_docs(dir)?;
            done.push(".kui/docs".to_string());
        }
        if p.tooling.agents {
            let file = write_agent_block(dir)?;
            done.push(format!(
                "the agent block in {}",
                file.file_name().unwrap_or_default().to_string_lossy()
            ));
        }
        Ok(())
    };
    let result = run();
    let from = was
        .map(|v| format!("kui {v}"))
        .unwrap_or_else(|| "an earlier kui".into());
    match result {
        Ok(()) => eprintln!(
            "kui: rewrote {} for kui {VERSION} (written by {from})",
            join(&done)
        ),
        Err(e) => eprintln!("kui: refreshing .kui for kui {VERSION}: {e}"),
    }
}

/// `a`, `a and b`, `a, b and c`.
fn join(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

// ---------------------------------------------------------------------------
// The manifest

/// Writes `want` into the app's manifest as `tooling`, with what it
/// already says: the `tooling` table replaced in place if there is one,
/// else a line at the top of the table it returns — a manifest of
/// `tooling` alone when the folder has none. The text around it is kept
/// as written; read back, the manifest must say what was meant, or it is
/// put back and that is the error. `None` when it already said it.
pub fn remember(dir: &Path, want: Tooling) -> Result<Option<PathBuf>, String> {
    let have = project::tooling_of(dir)?;
    let merged = have.union(want);
    if merged == have {
        return Ok(None);
    }
    let Some(path) = project::manifest(dir) else {
        let path = dir.join("kui.config.lua");
        std::fs::write(
            &path,
            format!("return {{\n  tooling = {},\n}}\n", merged.lua()),
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
        return Ok(Some(path));
    };
    let before = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let by_hand = || {
        format!(
            "{}: kui could not place `tooling = {}` in it; add it to the table it returns",
            path.display(),
            merged.lua()
        )
    };
    let after = with_tooling(&before, merged).ok_or_else(by_hand)?;
    std::fs::write(&path, &after).map_err(|e| format!("{}: {e}", path.display()))?;
    match project::tooling_of(dir) {
        Ok(t) if t == merged => Ok(Some(path)),
        _ => {
            let _ = std::fs::write(&path, &before);
            Err(by_hand())
        }
    }
}

/// `text` with its `tooling = { … }` replaced by `t`'s, or a line for it
/// opening the table after `return`.
fn with_tooling(text: &str, t: Tooling) -> Option<String> {
    let table = t.lua();
    if let Some(start) = find_word(text, "tooling") {
        let after = &text[start + "tooling".len()..];
        let eq = after.len() - after.trim_start().len();
        let rest = after.trim_start().strip_prefix('=')?;
        let ws = rest.len() - rest.trim_start().len();
        let open = start + "tooling".len() + eq + 1 + ws;
        if !text[open..].starts_with('{') {
            return None;
        }
        let mut depth = 0;
        for (i, c) in text[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let end = open + i + 1;
                        return Some(format!("{}{table}{}", &text[..open], &text[end..]));
                    }
                }
                _ => {}
            }
        }
        return None;
    }
    let ret = find_word(text, "return")?;
    let after = &text[ret + "return".len()..];
    let brace = ret + "return".len() + (after.len() - after.trim_start().len());
    if !text[brace..].starts_with('{') {
        return None;
    }
    let body = brace + 1;
    let line_end = text[body..].find('\n').map(|i| body + i);
    match line_end {
        // `return {` ends its line: a line of its own, indented as the next.
        Some(nl) if text[body..nl].trim().is_empty() => {
            let next = text[nl + 1..]
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("  ");
            let indent: String = next.chars().take_while(|c| c.is_whitespace()).collect();
            let indent = if indent.is_empty() {
                "  ".to_string()
            } else {
                indent
            };
            Some(format!(
                "{}\n{indent}tooling = {table},{}",
                &text[..nl],
                &text[nl..]
            ))
        }
        // `return { name = "x" }` on one line.
        _ => Some(format!(
            "{} tooling = {table},{}",
            &text[..body],
            &text[body..]
        )),
    }
}

/// Where `word` first stands as a word outside a comment line.
fn find_word(text: &str, word: &str) -> Option<usize> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let code = line.split("--").next().unwrap_or("");
        let mut from = 0;
        while let Some(i) = code[from..].find(word) {
            let at = from + i;
            let before = code[..at].chars().next_back();
            let after = code[at + word.len()..].chars().next();
            let edge = |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
            if edge(before) && edge(after) {
                return Some(offset + at);
            }
            from = at + word.len();
        }
        offset += line.len();
    }
    None
}

// ---------------------------------------------------------------------------
// The agent block

/// The block itself, for this runner.
pub fn agent_block() -> String {
    format!(
        "{BEGIN} — written by kui {VERSION}; kui rewrites what is between these markers and nothing else -->
## kui

This app runs on the kui {VERSION} runner: Lua 5.5 (or Teal) with Kit, no build step.

- **Types**: `.kui/types` — LuaLS (`kui.lua`, `kui_runner.lua`, `kit/ui.lua`) and Teal (`kui.d.tl`, `kit/ui.d.tl`).
- **Docs, at this version**: `.kui/docs` — `props.md` (every prop, element and event), `howto.md` (tasks), `kit.md` (Kit's components), `book/` and `adr/`. `kui docs <query>` searches them from a terminal.
- **Commands**: `kui run --watch` runs the app and reloads it on save, keeping `model`; `kui check` type-checks the `.tl` files; `kui run test --headless` runs the test script in `test/` with no window.

House rules:

- Lua 5.5: `init()` returns the model, `update(msg)` changes the global `model`, `view(env)` returns the whole tree, built again each frame.
- Kit's components first (`local kit = require(\"kit.ui\")`, listed in `kit.md`); draw a control from boxes only when Kit has none.
- Events are data: an `on_*` prop is the message its event arrives as in `update`, never a callback.
- A call that waits (`kui.http`, `kui.sleep`, `kui.process`) made from `update` goes in `kui.spawn`, and hands its result back with `kui.dispatch`.
{END}"
    )
}

/// Writes the agent block: between its markers where `AGENTS.md` or
/// `CLAUDE.md` has them, else appended to the one of them the app has,
/// else as a new `AGENTS.md`. The file it went into.
pub fn write_agent_block(dir: &Path) -> Result<PathBuf, String> {
    let block = agent_block();
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let files = [dir.join("AGENTS.md"), dir.join("CLAUDE.md")];
    let target = files
        .iter()
        .find(|p| read(p).is_some_and(|t| t.contains(BEGIN)))
        .or_else(|| files.iter().find(|p| p.exists()))
        .cloned()
        .unwrap_or_else(|| files[0].clone());
    let before = read(&target).unwrap_or_default();
    let after = match before.find(BEGIN) {
        Some(start) => {
            let Some(end) = before[start..].find(END).map(|i| start + i + END.len()) else {
                return Err(format!(
                    "{}: kui's block opens and does not close; put `{END}` back after it, or remove both",
                    target.display()
                ));
            };
            format!("{}{block}{}", &before[..start], &before[end..])
        }
        None if before.is_empty() => format!("{block}\n"),
        None => {
            let sep = if before.ends_with("\n\n") {
                ""
            } else if before.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            format!("{before}{sep}{block}\n")
        }
    };
    if after != before {
        std::fs::write(&target, after).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOTH: Tooling = Tooling {
        docs: true,
        agents: true,
    };
    const DOCS: Tooling = Tooling {
        docs: true,
        agents: false,
    };

    #[test]
    fn tooling_opens_the_returned_table_indented_as_its_first_line() {
        let text = "-- my app\nreturn {\n    name = \"x\",\n}\n";
        assert_eq!(
            with_tooling(text, DOCS).unwrap(),
            "-- my app\nreturn {\n    tooling = { docs = true },\n    name = \"x\",\n}\n"
        );
        assert_eq!(
            with_tooling("return { name = \"x\" }\n", DOCS).unwrap(),
            "return { tooling = { docs = true }, name = \"x\" }\n"
        );
    }

    #[test]
    fn a_tooling_table_is_replaced_in_place() {
        let text = "return {\n  name = \"x\",\n  tooling = { docs = true }, -- mine\n  size = { w = 1, h = 2 },\n}\n";
        assert_eq!(
            with_tooling(text, BOTH).unwrap(),
            "return {\n  name = \"x\",\n  tooling = { docs = true, agents = true }, -- mine\n  size = { w = 1, h = 2 },\n}\n"
        );
    }

    #[test]
    fn a_word_in_a_comment_or_inside_a_name_is_not_the_key() {
        assert_eq!(
            find_word(
                "-- return tooling\nlocal my_tooling = 1\nreturn {}",
                "tooling"
            ),
            None
        );
        assert_eq!(find_word("-- return\nreturn {}", "return"), Some(10));
        assert!(
            with_tooling("local t = {}\nreturn t\n", DOCS).is_none(),
            "no table after return: by hand"
        );
    }
}
