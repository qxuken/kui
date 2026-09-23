//! The view DSL described for lua-language-server (backlog F81): a
//! `---@meta` file of the prelude's constructors and the props a node
//! table takes, generated from kui-core's schema so it cannot fall
//! behind it — the Lua half of what `npm run gen` does for
//! `index.d.ts`. A host writes it somewhere and puts that directory on
//! the server's `workspace.library`; a script then completes `row {`
//! with its props, and a misspelt one is a diagnostic where it is
//! typed rather than a warning at runtime.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use kui_core::schema::{self, Kind};

use crate::PRELUDE;

/// The meta file's text.
pub fn luals_meta() -> String {
    let mut out = String::new();
    out.push_str(
        "---@meta kui\n\
         -- kui-lua's view DSL for lua-language-server, generated from\n\
         -- kui-core's schema by `kui_lua::luals_meta()`. Regenerate it\n\
         -- rather than editing it.\n\n",
    );
    out.push_str(
        "---A colour: `0xRRGGBBAA`, `\"#hex\"`, or a `\"$token\"` the host declared.\n\
         ---@alias kui.Color integer|string\n\
         ---A length in logical px, or a `\"$token\"`.\n\
         ---@alias kui.Length number|string\n\
         ---A sizing: px, `\"fit\"`, `\"grow\"`, `\"N%\"`, `{ grow = n }`, `{ percent = n }`, or a `\"$length\"`.\n\
         ---@alias kui.Sizing number|\"fit\"|\"grow\"|string|{ grow: number }|{ percent: number }\n\
         ---A lower clamp: px, `\"fit\"`, or a `\"$length\"`.\n\
         ---@alias kui.Min number|\"fit\"|string\n\n",
    );

    // Every prop a node table takes, the schema's and the composites'.
    out.push_str(
        "---Every prop a node table may carry; its children at 1, 2, ….\n---@class kui.Props\n",
    );
    for p in schema::PROPS {
        field(&mut out, p.snake_name(), &type_of(&p.kind), p.doc);
    }
    for c in schema::CUSTOM {
        for name in c.lua_names {
            let ty = match *name {
                "size" => "number".to_string(),
                "pad" => "kui.Length|{ all?: kui.Length, x?: kui.Length, y?: kui.Length, l?: kui.Length, r?: kui.Length, t?: kui.Length, b?: kui.Length }".into(),
                "border" => "{ w?: kui.Length, color?: kui.Color }".into(),
                "key" | "index" | "row_count" => "integer|string".into(),
                "tooltip" | "window_title" => "string".into(),
                "clip" | "scroll" | "scroll_x" | "scroll_y" | "key_focus" | "always_on_top"
                | "secure_input" => "boolean".into(),
                _ => "any".into(),
            };
            field(&mut out, name, &ty, c.doc);
        }
    }
    out.push_str("---@field [integer] kui.Node|string\n\n");
    out.push_str("---A node: what a constructor returns and a view gives back.\n---@class kui.Node: kui.Props\n---@field type string\n\n");

    // An element's own props, on a class of its own.
    let mut class_of: BTreeMap<&str, String> = BTreeMap::new();
    for e in schema::ELEMENTS {
        let class = format!("kui.{}", e.name);
        if e.lua_own.is_empty() {
            for ctor in constructors(e.lua) {
                class_of.insert(ctor, "kui.Props".into());
            }
            continue;
        }
        let _ = writeln!(out, "---@class {class}: kui.Props");
        for own in e.lua_own {
            field(&mut out, own, "any", e.doc);
        }
        out.push('\n');
        for ctor in constructors(e.lua) {
            class_of.insert(ctor, class.clone());
        }
    }

    // The events a view's `on_event` hears.
    out.push_str(
        "---An event a view hears: its payload's fields beside `kind`.\n---@class kui.Event\n",
    );
    let kinds: Vec<String> = schema::EVENTS
        .iter()
        .map(|e| format!("\"{}\"", e.kind))
        .collect();
    let _ = writeln!(out, "---@field kind {}", kinds.join("|"));
    out.push_str("---@field node_key? integer\n---@field [string] any\n\n");

    // The prelude's constructors, each with the comment above it.
    for (name, params, doc) in prelude_functions() {
        for line in doc.lines() {
            let _ = writeln!(out, "---{line}");
        }
        let element = class_of.get(name.as_str()).cloned();
        for p in &params {
            let (opt, ty) = match p.as_str() {
                "t" | "opts" => ("?", element.as_deref().unwrap_or("kui.Props").to_string()),
                "s" => ("", "string|table".into()),
                "env" => ("", "table".into()),
                "row" => ("", "fun(i: integer): kui.Node".into()),
                _ => ("", "any".into()),
            };
            let _ = writeln!(out, "---@param {p}{opt} {ty}");
        }
        let _ = writeln!(
            out,
            "---@return kui.Node\nfunction {name}({}) end\n",
            params.join(", ")
        );
    }
    out
}

/// A field line: its name, type and the doc flattened to one line.
fn field(out: &mut String, name: &str, ty: &str, doc: &str) {
    let doc = doc.split_whitespace().collect::<Vec<_>>().join(" ");
    let _ = writeln!(out, "---@field {name}? {ty} {doc}");
}

fn type_of(kind: &Kind) -> String {
    match kind {
        Kind::F32 => "number|string".into(),
        Kind::Color => "kui.Color".into(),
        Kind::Flag => "boolean".into(),
        Kind::Enum(names) => names
            .iter()
            .map(|n| format!("\"{n}\""))
            .collect::<Vec<_>>()
            .join("|"),
        Kind::Sizing => "kui.Sizing".into(),
        Kind::Min => "kui.Min".into(),
        Kind::Msg | Kind::Tag => "any".into(),
        Kind::Str => "string".into(),
        Kind::Resource => "integer".into(),
        Kind::Keyframes | Kind::Enter => "table".into(),
    }
}

/// The constructors an element's `lua` column names: the identifier
/// opening each backticked form, `row { }` → `row`.
fn constructors(lua: &str) -> Vec<&str> {
    lua.split('`')
        .skip(1)
        .step_by(2)
        .filter_map(|form| {
            let end = form.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
            let rest = form[end..].trim_start();
            (end > 0 && (rest.starts_with('{') || rest.starts_with('('))).then(|| &form[..end])
        })
        .collect()
}

/// Every global function the prelude defines: its name, its parameters
/// and the comment block right above it.
fn prelude_functions() -> Vec<(String, Vec<String>, String)> {
    let mut out = Vec::new();
    let mut doc: Vec<&str> = Vec::new();
    for line in PRELUDE.lines() {
        if let Some(c) = line.strip_prefix("--") {
            doc.push(c.strip_prefix(' ').unwrap_or(c));
            continue;
        }
        if let Some(sig) = line.strip_prefix("function ")
            && let Some((name, rest)) = sig.split_once('(')
            && let Some((params, _)) = rest.split_once(')')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            let params = params
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            out.push((name.to_string(), params, doc.join("\n")));
        }
        doc.clear();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every prop the schema has and every constructor the prelude
    /// defines is in the file, each constructor typed by its element.
    #[test]
    fn the_meta_covers_the_schema_and_the_prelude() {
        let meta = luals_meta();
        assert!(meta.starts_with("---@meta kui\n"));
        for p in schema::PROPS {
            assert!(
                meta.contains(&format!("---@field {}? ", p.snake_name())),
                "{} missing",
                p.snake_name()
            );
        }
        for (name, _, _) in prelude_functions() {
            assert!(
                meta.contains(&format!("function {name}(")),
                "{name} missing"
            );
        }
        assert!(meta.contains("---@param t? kui.Props\n---@return kui.Node\nfunction row(t) end"));
        assert!(meta.contains("---@param t? kui.edit\n"), "edit's own props");
        assert!(meta.contains("---@field initial? any"));
        assert_eq!(constructors("`row { }`, `column { }`"), ["row", "column"]);
        assert_eq!(constructors("`text(\"s\", {…})`"), ["text"]);
        assert!(constructors("`size`").is_empty());
    }
}
