//! The view DSL described for lua-language-server (backlog F81): a
//! `---@meta` file of the prelude's constructors and the props a node
//! table takes, generated from kui-core's schema so it cannot fall
//! behind it — the Lua half of what `npm run gen` does for
//! `index.d.ts`. A host writes it somewhere and puts that directory on
//! the server's `workspace.library`; a script then completes `row {`
//! with its props, and a misspelt one is a diagnostic where it is
//! typed rather than a warning at runtime.
//!
//! What the schema cannot say — the aliases' shapes, the composites'
//! types, which constructor takes nil — is written here by hand, and
//! the tests hold each of those against the parser and the prelude
//! rather than against this file (RG33: the first cut typed `key` as
//! `integer|string`, `{ percent = n }` as a sizing and `repeat` as a
//! field, and every one of them failed at runtime).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use kui_core::schema::{self, Kind};

use crate::PRELUDE;

/// A type the file names once: its name, its doc, the LuaLS type it
/// stands for, and the strings its `string` member means — a bare
/// `string` says less than the parser takes, so the tests sample these
/// instead of any string.
struct Alias {
    name: &'static str,
    doc: &'static str,
    ty: &'static str,
    #[cfg_attr(not(test), allow(dead_code))]
    strings: &'static [&'static str],
}

/// `parse_color`, `length_of`, `parse_sizing` and the `Kind::Min` /
/// `Kind::Max` arm of `parse_value`, in LuaLS's words.
const ALIASES: &[Alias] = &[
    Alias {
        name: "kui.Color",
        doc: "A colour: `0xRRGGBBAA`, `\"#hex\"`, or a `\"$token\"` the host declared.",
        ty: "integer|string",
        strings: &["\"#336699\"", "\"#336699cc\"", "\"$accent\""],
    },
    Alias {
        name: "kui.Length",
        doc: "A length in logical px, or a `\"$token\"`.",
        ty: "number|string",
        strings: &["\"$gap\""],
    },
    Alias {
        name: "kui.SizeArg",
        doc: "A size expression's part: px, a spelling (`\"80%\"`, `\"min(…)\"`), `{ pct = n }` or `{ px = n }`.",
        ty: "number|string|{ pct: number }|{ px: number }",
        strings: &["\"80%\""],
    },
    Alias {
        name: "kui.Size",
        doc: "A size expression (backlog F109), resolved against the parent's content box: px, `\"Npx\"`, `\"N%\"`, `\"min(…)\"`, `\"max(…)\"`, `\"clamp(MIN, TARGET, MAX)\"`, nested — or the same as data, which is never parsed: `{ pct = n }`, `{ px = n }`, `{ min = { … } }`, `{ max = { … } }`, `{ clamp = { MIN, TARGET, MAX } }` (a part a table again, or a spelling). The process keeps 65 536 distinct expressions and never lets one go: past that a new one leaves its prop at its default, with a `size-expressions-full` warning — declare one per layout, not one per frame (backlog RG93).",
        ty: "kui.SizeArg|{ min: kui.SizeArg[] }|{ max: kui.SizeArg[] }|{ clamp: kui.SizeArg[] }",
        strings: &["\"clamp(400px, 80%, 1000px)\""],
    },
    Alias {
        name: "kui.Sizing",
        doc: "A sizing: px, `\"fit\"`, `\"grow\"`, `\"N%\"`, `{ grow = n }`, `{ pct = n }` (n percent), a size expression (`kui.Size`), or a `\"$length\"`.",
        ty: "\"fit\"|\"grow\"|{ grow: number }|kui.Size",
        strings: &["\"50%\"", "\"$gap\"", "\"clamp(400px, 80%, 1000px)\""],
    },
    Alias {
        name: "kui.Min",
        doc: "A lower clamp: px, `\"fit\"`, a size expression (`kui.Size`), or a `\"$length\"`.",
        ty: "\"fit\"|kui.Size",
        strings: &["\"$gap\"", "\"50%\""],
    },
    Alias {
        name: "kui.Max",
        doc: "An upper clamp: px, a size expression (`kui.Size`), or a `\"$length\"`.",
        ty: "kui.Size",
        strings: &["\"$gap\"", "\"min(720px, 100%)\""],
    },
];

/// The words the Lua the prelude runs in reserves (5.5's `global` is
/// not among them: the vendored build keeps it a name): a prop under
/// one of these cannot be written `name = …` in a table constructor, so
/// the file gives only the spelling `schema::LUA_ALIASES` has for it.
const LUA_KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// What `uniform_list` reads off its `opts` beyond a container's
/// props: each name, whether the prelude refuses to go on without it,
/// its type and doc.
const UNIFORM_LIST: &[(&str, bool, &str, &str)] = &[
    (
        "key",
        true,
        "string",
        "The container's key, which its scroll geometry is read back by.",
    ),
    (
        "row_h",
        true,
        "number",
        "One row's height: the whole stride, spacing included.",
    ),
    (
        "rows",
        false,
        "number",
        "How many rows the list has; none when left out.",
    ),
    (
        "overscan",
        false,
        "number",
        "Rows built past each end of the viewport; two when left out.",
    ),
    (
        "row_props",
        false,
        "fun(i: integer): kui.Props",
        "Each row's own node's props — its click, stripe, hover; its `index` and `height` stay the list's.",
    ),
];

/// What `list` reads off its `opts` beyond a container's props, as
/// [`UNIFORM_LIST`] is for `uniform_list`.
const LIST: &[(&str, bool, &str, &str)] = &[
    (
        "key",
        true,
        "string",
        "The container's key, which its scroll geometry is read back by.",
    ),
    (
        "heights",
        true,
        "kui.RowHeights",
        "The heights the list slices by, made once with `row_heights` and kept.",
    ),
    (
        "overscan",
        false,
        "number",
        "Rows built past each end of the viewport; two when left out.",
    ),
];

/// `row_heights`' methods: what a script calls on the heights it keeps.
/// The three `slice_*` steps are `list`'s and left out.
const ROW_HEIGHTS: &[(&str, &str, &str)] = &[
    ("len", "", "integer"),
    ("set_len", "rows: integer", "nil"),
    ("clear", "", "nil"),
    ("set", "i: integer, h: number", "nil"),
    ("measured", "i: integer", "number?"),
    ("get", "i: integer", "number"),
    ("estimate", "", "number"),
    ("total", "", "number"),
    ("offset_of", "i: integer", "number"),
    ("row_at", "y: number", "integer"),
];

/// The meta file's text.
pub fn luals_meta() -> String {
    let mut out = String::new();
    out.push_str(
        "---@meta kui\n\
         -- kui-lua's view DSL for lua-language-server, generated from\n\
         -- kui-core's schema by `kui_lua::luals_meta()`. Regenerate it\n\
         -- rather than editing it.\n\n",
    );
    for a in ALIASES {
        let _ = writeln!(out, "---{}\n---@alias {} {}", a.doc, a.name, a.ty);
    }
    out.push('\n');

    // Every prop a node table takes, the schema's and the composites'.
    out.push_str(
        "---Every prop a node table may carry; its children at 1, 2, ….\n---@class kui.Props\n",
    );
    for (name, ty, doc) in props_fields() {
        field(&mut out, name, &ty, doc);
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

    // `uniform_list`'s options: a container's props and its own.
    out.push_str("---What `uniform_list` reads off its options; every other key is the container's.\n---@class kui.UniformList: kui.Props\n");
    for (name, required, ty, doc) in UNIFORM_LIST {
        let opt = if *required { "" } else { "?" };
        let _ = writeln!(out, "---@field {name}{opt} {ty} {doc}");
    }
    out.push('\n');

    // `list`'s options, and the heights it slices by.
    out.push_str("---What `list` reads off its options; every other key is the container's.\n---@class kui.List: kui.Props\n");
    for (name, required, ty, doc) in LIST {
        let opt = if *required { "" } else { "?" };
        let _ = writeln!(out, "---@field {name}{opt} {ty} {doc}");
    }
    out.push_str(
        "\n---A variable-height list's row heights: measured where known, the mean of those elsewhere (rows are 0-based).\n---@class kui.RowHeights\n",
    );
    for (name, params, ret) in ROW_HEIGHTS {
        let _ = writeln!(
            out,
            "---@field {name} fun(self: kui.RowHeights{}{params}): {ret}",
            if params.is_empty() { "" } else { ", " }
        );
    }
    out.push_str(
        "\n---`rows` rows, none measured, each at `estimate` logical px (20 when left out) until measured.\n---@param rows integer\n---@param estimate? number\n---@return kui.RowHeights\nfunction row_heights(rows, estimate) end\n\n",
    );

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

    // The prelude's constructors, each with the comment above it. A
    // table parameter is optional only where the body defaults or tests
    // it before indexing it: `button(nil)` is an error in the prelude, so
    // it is one where it is typed too.
    for f in prelude_functions() {
        for line in f.doc.lines() {
            let _ = writeln!(out, "---{line}");
        }
        let element = class_of.get(f.name.as_str()).cloned();
        for p in &f.params {
            let (opt, ty) = match p.as_str() {
                "opts" if f.name == "uniform_list" => ("", "kui.UniformList".into()),
                "opts" if f.name == "list" => ("", "kui.List".into()),
                "measure" => ("", "fun(i: integer, width: number): number".into()),
                "t" | "opts" => (
                    if tolerates_nil(&f.body, p) { "?" } else { "" },
                    element.as_deref().unwrap_or("kui.Props").to_string(),
                ),
                // Spans, or anything `tostring` makes the text of.
                "s" => ("", "string|number|table".into()),
                "env" => ("", "table".into()),
                "row" => ("", "fun(i: integer): kui.Node".into()),
                "key" => ("", "string|integer".into()),
                "i" => ("", "integer".into()),
                "row_h" => ("", "number".into()),
                _ => ("", "any".into()),
            };
            let _ = writeln!(out, "---@param {p}{opt} {ty}");
        }
        // Most of the prelude builds a node; the two list readings answer.
        let ret = match f.name.as_str() {
            "reveal_row" => "boolean",
            "rows_in_view" => "integer",
            _ => "kui.Node",
        };
        let _ = writeln!(
            out,
            "---@return {ret}\nfunction {}({}) end\n",
            f.name,
            f.params.join(", ")
        );
    }
    out
}

/// Every field of `kui.Props`: its Lua name, type and doc. A schema
/// row under each name a Lua table can spell it by, a composite under
/// each of its Lua names.
fn props_fields() -> Vec<(&'static str, String, &'static str)> {
    let mut out = Vec::new();
    for p in schema::PROPS {
        for name in lua_names(p.snake_name()) {
            out.push((name, type_of(&p.kind), p.doc));
        }
    }
    for c in schema::CUSTOM {
        for name in c.lua_names {
            out.push((*name, composite_type(name), c.doc));
        }
    }
    out
}

/// The names a Lua table writes the schema row `snake` under: the
/// aliases `schema::LUA_ALIASES` gives it, and its own name unless that
/// is a keyword no table constructor can hold.
fn lua_names(snake: &'static str) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Vec::new();
    if !LUA_KEYWORDS.contains(&snake) {
        names.push(snake);
    }
    names.extend(
        schema::LUA_ALIASES
            .iter()
            .filter(|(_, real)| *real == snake)
            .map(|(alias, _)| *alias),
    );
    names
}

/// A composite's type, by what `parse_props` (or, for the window's
/// rows, the root read in `lib.rs`) takes under that name. `any` for a
/// name this does not know, which the tests refuse.
fn composite_type(name: &str) -> String {
    match name {
        "size" => "kui.Length".into(),
        "pad" => "kui.Length|{ all?: kui.Length, x?: kui.Length, y?: kui.Length, l?: kui.Length, r?: kui.Length, t?: kui.Length, b?: kui.Length }".into(),
        "border" => "{ w?: kui.Length, color?: kui.Color }".into(),
        "float" => {
            let presets: Vec<String> = kui_core::FLOAT_PRESETS
                .iter()
                .map(|p| format!("\"{p}\""))
                .collect();
            format!("{}|table", presets.join("|"))
        }
        "key" | "tooltip" | "window_title" => "string".into(),
        "index" | "row_count" => "number".into(),
        "windows" => "table".into(),
        "option_as_alt" => {
            let names: Vec<String> = kui_core::OptionAsAlt::ALL
                .iter()
                .map(|v| format!("\"{}\"", v.name()))
                .collect();
            names.join("|")
        }
        "clip" | "scroll" | "scroll_x" | "scroll_y" | "key_focus" | "always_on_top"
        | "secure_input" => "boolean".into(),
        _ => "any".into(),
    }
}

/// A field line: its name, type and the doc flattened to one line.
fn field(out: &mut String, name: &str, ty: &str, doc: &str) {
    let doc = doc.split_whitespace().collect::<Vec<_>>().join(" ");
    let _ = writeln!(out, "---@field {name}? {ty} {doc}");
}

/// A schema row's type, by the `parse_value` arm its kind takes.
fn type_of(kind: &Kind) -> String {
    match kind {
        Kind::F32 => "kui.Length".into(),
        Kind::Color => "kui.Color".into(),
        Kind::Flag => "boolean".into(),
        Kind::Enum(names) => names
            .iter()
            .map(|n| format!("\"{n}\""))
            .collect::<Vec<_>>()
            .join("|"),
        Kind::Sizing => "kui.Sizing".into(),
        Kind::Min => "kui.Min".into(),
        Kind::Max => "kui.Max".into(),
        Kind::Msg | Kind::Tag => "any".into(),
        Kind::Str => "string".into(),
        // The stock three, for an editor to offer, or any installed name.
        Kind::Family => "\"sans\"|\"serif\"|\"mono\"|string".into(),
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

/// A global function the prelude defines.
struct PreludeFn {
    name: String,
    params: Vec<String>,
    /// The comment block right above it.
    doc: String,
    /// Its lines up to the `end` that closes it at the margin.
    body: String,
}

/// Whether a prelude function tolerates `p` being nil: it defaults it
/// (`t = t or {}`) or tests it (`if opts then`) before indexing it.
fn tolerates_nil(body: &str, p: &str) -> bool {
    body.contains(&format!("{p} = {p} or ")) || body.contains(&format!("if {p} then"))
}

/// Every global function the prelude defines, in its order.
fn prelude_functions() -> Vec<PreludeFn> {
    let mut out: Vec<PreludeFn> = Vec::new();
    let mut doc: Vec<&str> = Vec::new();
    let mut open = false;
    for line in PRELUDE.lines() {
        if open {
            if line == "end" {
                open = false;
            } else if let Some(f) = out.last_mut() {
                f.body.push_str(line);
                f.body.push('\n');
            }
            continue;
        }
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
            out.push(PreludeFn {
                name: name.to_string(),
                params,
                doc: doc.join("\n"),
                body: String::new(),
            });
            open = true;
        }
        doc.clear();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::{Lua, Table};

    /// Every prop the schema has and every constructor the prelude
    /// defines is in the file, each constructor typed by its element.
    #[test]
    fn the_meta_covers_the_schema_and_the_prelude() {
        let meta = luals_meta();
        assert!(meta.starts_with("---@meta kui\n"));
        for p in schema::PROPS {
            let names = lua_names(p.snake_name());
            assert!(!names.is_empty(), "{} has no Lua spelling", p.snake_name());
            for name in names {
                assert!(
                    meta.contains(&format!("---@field {name}? ")),
                    "{name} missing"
                );
            }
        }
        // `repeat` is a keyword: the row is there as `direction` only.
        assert!(meta.contains("---@field direction? "));
        assert!(!meta.contains("---@field repeat? "));
        for f in prelude_functions() {
            assert!(
                meta.contains(&format!("function {}(", f.name)),
                "{} missing",
                f.name
            );
        }
        assert!(meta.contains("---@param t? kui.Props\n---@return kui.Node\nfunction row(t) end"));
        assert!(meta.contains("---@param t kui.edit\n"), "edit's own props");
        assert!(meta.contains("---@param opts kui.UniformList\n"));
        assert!(meta.contains("---@field initial? any"));
        assert_eq!(constructors("`row { }`, `column { }`"), ["row", "column"]);
        assert_eq!(constructors("`text(\"s\", {…})`"), ["text"]);
        assert!(constructors("`size`").is_empty());
    }

    /// A fresh Lua with the prelude loaded.
    fn prelude_lua() -> Lua {
        let lua = Lua::new();
        lua.load(PRELUDE).exec().unwrap();
        lua
    }

    /// `props` through the parser a view's tables go through, over a
    /// bare core: no tokens declared, so a `$name` misses (a warning, a
    /// default) rather than failing the parse.
    fn parses(lua: &Lua, props: &str) -> Result<(), String> {
        let t: Table = lua
            .load(format!("return {{ {props} }}"))
            .eval()
            .map_err(|e| format!("does not compile: {e}"))?;
        // One core per thread, leaked: a core is slow to make and the
        // lookup only borrows it.
        thread_local! {
            static CORE: &'static kui_core::Core = Box::leak(Box::new(kui_core::Core::new()));
        }
        let core: &'static kui_core::Core = CORE.with(|c| *c);
        let mut refs = crate::Refs::new(core.token_lookup());
        crate::parse_props(&t, false, &mut refs)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// The composites the root table carries and `parse_props` never
    /// reads: the parser takes anything under these, so neither
    /// direction below says anything about them.
    const ROOT_ONLY: &[&str] = &[
        "window_title",
        "always_on_top",
        "secure_input",
        "option_as_alt",
        "windows",
    ];

    /// `ty`'s union members, split at the top level only.
    fn members(ty: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let (mut depth, mut start) = (0, 0);
        for (i, c) in ty.char_indices() {
            match c {
                '{' | '(' => depth += 1,
                '}' | ')' => depth -= 1,
                '|' if depth == 0 => {
                    out.push(ty[start..i].trim());
                    start = i + 1;
                }
                _ => {}
            }
        }
        out.push(ty[start..].trim());
        out
    }

    fn alias(name: &str) -> Option<&'static Alias> {
        ALIASES.iter().find(|a| a.name == name)
    }

    /// A `{ a: T, b?: U }` shape's fields: name, whether required, type.
    fn shape(member: &str) -> Vec<(&str, bool, &str)> {
        let inner = member.trim_start_matches('{').trim_end_matches('}');
        let mut out = Vec::new();
        let (mut depth, mut start) = (0, 0);
        let mut fields = Vec::new();
        for (i, c) in inner.char_indices() {
            match c {
                '{' | '(' => depth += 1,
                '}' | ')' => depth -= 1,
                ',' if depth == 0 => {
                    fields.push(&inner[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
        fields.push(&inner[start..]);
        for f in fields {
            let Some((name, ty)) = f.split_once(':') else {
                continue;
            };
            let name = name.trim();
            let (name, required) = match name.strip_suffix('?') {
                Some(n) => (n, false),
                None => (name, true),
            };
            out.push((name, required, ty.trim()));
        }
        out
    }

    /// A Lua value of every member of `ty` the parser can be asked
    /// about: literals and shapes from the type itself, `string` as the
    /// enclosing alias means it. `any`, `table` and functions give none.
    fn samples(ty: &str, strings: &[&str]) -> Vec<String> {
        let mut out = Vec::new();
        for m in members(ty) {
            match m {
                "number" => out.push("12.5".into()),
                "integer" => out.push("3".into()),
                "boolean" => out.push("true".into()),
                "string" if strings.is_empty() => out.push("\"x\"".into()),
                "string" => out.extend(strings.iter().map(|s| s.to_string())),
                "any" | "table" => {}
                m if m.starts_with('"') => out.push(m.into()),
                m if m.starts_with('{') => {
                    let fields: Vec<String> = shape(m)
                        .into_iter()
                        .map(|(name, _, ty)| {
                            let v = samples(ty, &[]).into_iter().next().unwrap();
                            format!("{name} = {v}")
                        })
                        .collect();
                    out.push(format!("{{ {} }}", fields.join(", ")));
                }
                // A list: three of its first member, which is what every
                // list a size takes (`clamp`'s three, `min`'s any) admits.
                m if m.ends_with("[]") => {
                    let one = samples(&m[..m.len() - 2], &[]).into_iter().next().unwrap();
                    out.push(format!("{{ {one}, {one}, {one} }}"));
                }
                m if m.starts_with("kui.") => {
                    let a = alias(m).unwrap_or_else(|| panic!("{m} is no alias"));
                    out.extend(samples(a.ty, a.strings));
                }
                m => panic!("no sample for {m}"),
            }
        }
        out
    }

    /// Every value the file says a prop takes, the parser takes: each
    /// member of each field's type, sampled, under the field's name —
    /// which the table constructor compiling also holds to being a name
    /// Lua can write.
    #[test]
    fn every_annotated_type_parses() {
        let lua = Lua::new();
        for c in schema::CUSTOM {
            for name in c.lua_names {
                assert_ne!(
                    composite_type(name),
                    "any",
                    "{name}: a composite the meta does not type"
                );
            }
        }
        let mut failed = Vec::new();
        for (name, ty, _) in props_fields() {
            if ROOT_ONLY.contains(&name) {
                continue;
            }
            for v in samples(&ty, &[]) {
                if let Err(e) = parses(&lua, &format!("{name} = {v}")) {
                    failed.push(format!("{name} = {v} ({ty}): {e}"));
                }
            }
        }
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }

    /// A probe value, and what a type has to have to admit it.
    enum Probe {
        Int,
        Float,
        Bool,
        Str(&'static str),
        Table(&'static [&'static str]),
    }

    const PROBES: &[(&str, Probe)] = &[
        ("3", Probe::Int),
        ("12.5", Probe::Float),
        ("false", Probe::Bool),
        ("\"x\"", Probe::Str("x")),
        ("\"$gap\"", Probe::Str("$gap")),
        ("\"#336699\"", Probe::Str("#336699")),
        ("\"50%\"", Probe::Str("50%")),
        ("\"fit\"", Probe::Str("fit")),
        ("\"grow\"", Probe::Str("grow")),
        ("\"below\"", Probe::Str("below")),
        ("{}", Probe::Table(&[])),
        ("{ grow = 2 }", Probe::Table(&["grow"])),
        ("{ pct = 50 }", Probe::Table(&["pct"])),
        ("{ percent = 50 }", Probe::Table(&["percent"])),
    ];

    /// Whether `ty` admits `p`. `integer` admits a float: the parser
    /// truncates one where a colour or a handle goes, and the annotation
    /// says which the value means rather than every number it survives.
    fn admits(ty: &str, p: &Probe) -> bool {
        members(ty).into_iter().any(|m| match (m, p) {
            ("any", _) => true,
            ("table", Probe::Table(_)) => true,
            ("number" | "integer", Probe::Int | Probe::Float) => true,
            ("boolean", Probe::Bool) => true,
            ("string", Probe::Str(_)) => true,
            (m, Probe::Str(s)) if m.starts_with('"') => m == format!("\"{s}\""),
            (m, Probe::Table(keys)) if m.starts_with('{') => shape(m)
                .iter()
                .all(|(name, required, _)| !required || keys.contains(name)),
            (m, p) if m.starts_with("kui.") => alias(m).is_some_and(|a| admits(a.ty, p)),
            _ => false,
        })
    }

    /// Every value the parser takes for a prop, the file admits: a probe
    /// set of numbers, strings and table shapes under each field, and a
    /// probe the parser accepts that the field's type refuses is a type
    /// narrower than the truth (`size` typed `number` while a `"$token"`
    /// parses). Flags are left out — anything but `true` is off, so the
    /// parser takes every value and the annotation says what turns one
    /// on — as are the root's composites, which it never reads.
    #[test]
    fn every_parsed_value_is_admitted() {
        let lua = Lua::new();
        let mut failed = Vec::new();
        for (name, ty, _) in props_fields() {
            if ty == "boolean" || ROOT_ONLY.contains(&name) {
                continue;
            }
            for (src, p) in PROBES {
                if parses(&lua, &format!("{name} = {src}")).is_ok() && !admits(&ty, p) {
                    failed.push(format!("{name} = {src} parses; {ty} refuses it"));
                }
            }
        }
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }

    /// Each word `LUA_KEYWORDS` lists is one the Lua the prelude runs in
    /// refuses as a table key, and an ordinary name is not.
    #[test]
    fn the_keywords_are_lua_s() {
        let lua = Lua::new();
        for k in LUA_KEYWORDS {
            assert!(
                lua.load(format!("return {{ {k} = 1 }}")).exec().is_err(),
                "{k} compiles as a key"
            );
        }
        assert!(lua.load("return { direction = 1 }").exec().is_ok());
    }

    /// A table parameter the file marks optional is one the prelude
    /// really takes nil for, and one it marks required is one nil fails:
    /// every prelude function with a `t` or `opts`, called with it nil.
    #[test]
    fn a_param_is_optional_where_the_prelude_takes_nil() {
        let lua = prelude_lua();
        let meta = luals_meta();
        let mut checked = 0;
        for f in prelude_functions() {
            for p in f.params.iter().filter(|p| *p == "t" || *p == "opts") {
                let args: Vec<&str> = f
                    .params
                    .iter()
                    .map(|q| match q.as_str() {
                        q if q == p => "nil",
                        "s" => "\"x\"",
                        "env" => "{ scroll_geometry = function() end, viewport_h = 100 }",
                        "row" => "function(i) return text(tostring(i)) end",
                        _ => "nil",
                    })
                    .collect();
                let src = format!("return {}({})", f.name, args.join(", "));
                let takes_nil = lua.load(&src).exec().is_ok();
                let optional = section(&meta, &f.name).contains(&format!("---@param {p}? "));
                assert_eq!(
                    optional,
                    takes_nil,
                    "{}: `{p}` marked {} but the prelude {} nil",
                    f.name,
                    if optional { "optional" } else { "required" },
                    if takes_nil { "takes" } else { "refuses" }
                );
                checked += 1;
            }
        }
        // Both answers are exercised: `row` defaults `t`, `button` indexes it.
        assert!(section(&meta, "row").contains("---@param t? "));
        assert!(section(&meta, "button").contains("---@param t kui.button"));
        assert!(checked > 20, "{checked} parameters");
        // `s` is the text or its spans, and a number is text: `text(i)`
        // in a row builder is the common case.
        assert!(lua.load("return text(3), tooltip(3)").exec().is_ok());
        assert!(section(&meta, "text").contains("---@param s string|number|table\n"));
    }

    /// The lines the file gives `name`: from the blank line before its
    /// doc to its `function` line.
    fn section<'a>(meta: &'a str, name: &str) -> &'a str {
        let end = meta
            .find(&format!("\nfunction {name}("))
            .unwrap_or_else(|| panic!("{name} missing"));
        let start = meta[..end].rfind("\n\n").map_or(0, |i| i + 2);
        &meta[start..end]
    }

    /// `kui.UniformList` is what `uniform_list` reads: every field
    /// is an `opts.` read in its body, every read is a field or a prop,
    /// and a required field is one the prelude refuses to go on without.
    #[test]
    fn uniform_list_s_class_is_what_it_reads() {
        let f = prelude_functions()
            .into_iter()
            .find(|f| f.name == "uniform_list")
            .unwrap();
        let reads: Vec<&str> = f
            .body
            .split("opts.")
            .skip(1)
            .map(|r| {
                let end = r
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .unwrap_or(r.len());
                &r[..end]
            })
            .collect();
        let props: Vec<&str> = props_fields().into_iter().map(|(n, _, _)| n).collect();
        for (name, _, _, _) in UNIFORM_LIST {
            assert!(reads.contains(name), "{name} is never read");
        }
        for r in &reads {
            assert!(
                props.contains(r) || UNIFORM_LIST.iter().any(|(n, ..)| n == r),
                "opts.{r} is read and typed nowhere"
            );
        }
        let lua = prelude_lua();
        let env = "{ scroll_geometry = function() end, viewport_h = 100 }";
        for (name, required, _, _) in UNIFORM_LIST {
            let opts: Vec<String> = UNIFORM_LIST
                .iter()
                .filter(|(n, ..)| n != name)
                .map(|(n, _, ty, _)| {
                    let v = if *ty == "string" {
                        "\"log\""
                    } else if ty.starts_with("fun") {
                        "function() return {} end"
                    } else {
                        "4"
                    };
                    format!("{n} = {v}")
                })
                .collect();
            let src = format!(
                "return uniform_list({env}, {{ {} }}, function(i) return text(i) end)",
                opts.join(", ")
            );
            assert_eq!(
                lua.load(&src).exec().is_err(),
                *required,
                "uniform_list without {name}"
            );
        }
        // Its `pad` is the prop's every shape: it read `pad_t` and
        // `pad_y`, which no Lua table carries, and did arithmetic on a
        // table `pad`.
        for pad in ["8", "{ t = 8 }", "{ y = 8 }", "{ all = 8 }", "\"$gap\""] {
            let src = format!(
                "return uniform_list({env}, {{ key = \"log\", row_h = 4, rows = 9, pad = {pad} }}, \
                 function(i) return text(i) end)"
            );
            assert!(lua.load(&src).exec().is_ok(), "pad = {pad}");
        }
    }

    /// The same checks catch what RG33 found in the first cut.
    #[test]
    fn the_checks_catch_the_first_cut_s_types() {
        let lua = Lua::new();
        // `{ percent = n }` as a sizing, `key` as a number, `index` as a string.
        assert!(parses(&lua, "width = { percent = 50 }").is_err());
        assert!(parses(&lua, "key = 3").is_err());
        assert!(parses(&lua, "index = \"x\"").is_err());
        // `size` as a bare number while a token parses.
        assert!(parses(&lua, "size = \"$gap\"").is_ok());
        assert!(!admits("number", &Probe::Str("$gap")));
        // `repeat` as a field name.
        assert!(parses(&lua, "repeat = \"alternate\"").is_err());
        assert!(parses(&lua, "direction = \"alternate\"").is_ok());
    }
}
