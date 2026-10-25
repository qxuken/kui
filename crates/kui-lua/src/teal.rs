//! The view DSL described for Teal, the typed Lua dialect: a declaration
//! file a project saves as `kui.d.tl` and names its global environment
//! (`global_env_def = "kui"` in `tlconfig.lua`), so `tl check` types a
//! view's tables and `tl gen` writes the plain Lua a [`LuaExtension`]
//! loads. Nothing of Teal runs in kui: a typed script is compiled ahead.
//!
//! It is [`luals_meta`]'s file in Teal's words — the same tables, read the
//! same way, so the two cannot disagree about a prop. Teal says some
//! things its own way, and the translation here is where:
//!
//! - a union of string literals is an `enum`;
//! - a union holds one table type unless each says how to tell it from
//!   the rest, so a shape beside another table is a record `where` the
//!   first field it cannot go without is set;
//! - an alias inside a union is spelt out member by member, since Teal
//!   matches a table literal against the union's own members only;
//! - what an app's message extends (`kui.Event`, the `kui.With…` fields)
//!   is an interface, and `kui.Props` one too, for the element records;
//! - `env`, a bare table to LuaLS, is a record, since Teal will not index
//!   `any`: its facts and calls are the schema's rows, typed here.
//!
//! The tests check the file with the Teal compiler itself (v0.24.8, under
//! `tests/tl`), against the parser the way `meta.rs`'s tests check the
//! LuaLS file: every value the parser takes, Teal takes.
//!
//! [`LuaExtension`]: crate::LuaExtension
//! [`luals_meta`]: crate::luals_meta

use std::collections::BTreeMap;
use std::fmt::Write as _;

use kui_core::schema::{self, Cell};

use crate::meta::{
    self, ALIASES, EVENT_DATA, LIST, ROW_HEIGHTS, SHAPES, UNIFORM_LIST, WINDOW_ENTRY,
    WINDOW_EVENTS, WITH_UNIONS,
};

/// The Teal type of each fact `env` carries, by the Lua path
/// `schema::ENV_FIELDS` files it under. A whole number where the reading
/// is a handle, a key or a count, as the event classes type them.
const ENV_FACTS: &[(&str, &str)] = &[
    ("refresh_hz", "number"),
    ("frame_budget_ms", "number"),
    ("focused", "boolean"),
    ("system.appearance", "string"),
    ("system.accent", "integer"),
    ("system.motion", "string"),
    ("system.locale", "string"),
    ("system.assistive", "string"),
    ("window.id", "integer"),
    ("window.custom_chrome", "boolean"),
    ("window.maximized", "boolean"),
    ("window.fullscreen", "boolean"),
    ("window.always_on_top", "boolean"),
    ("window.controls_w", "number"),
    ("window.controls_h", "number"),
    ("window.backdrop", "string"),
    ("audio.device", "string"),
    ("audio.live", "integer"),
    ("viewport_w", "number"),
    ("viewport_h", "number"),
    ("focus", "integer"),
    ("focus_visible", "boolean"),
    ("caret_visible", "boolean"),
    ("now", "number"),
    ("region", "integer"),
];

/// The calls `env` carries, by the name `schema::DOORS`' Lua column gives
/// each: the parameters and what it returns, in Teal (an empty return is
/// none). A key is `KeyRef`, the integer an event carried or the label a
/// `key` prop declared.
const ENV_CALLS: &[(&str, &str, &str)] = &[
    ("edit_text", "key: KeyRef", "string"),
    ("set_edit_text", "key: KeyRef, text: string", ""),
    ("is_focused", "key: KeyRef", "boolean"),
    ("is_hovered", "key: KeyRef", "boolean"),
    ("is_pressed", "key: KeyRef", "boolean"),
    ("is_drop_target", "key: KeyRef", "boolean"),
    ("drop_target", "", "integer"),
    ("label_of", "key: KeyRef", "string"),
    ("request_frame_at", "at: number", ""),
    ("request_frame_for", "why: string", ""),
    ("span", "name: string, f: function(): any...", "any..."),
    ("perf_measure", "name: string, ms: number", ""),
    ("perf_count", "name: string, n: number", ""),
    ("perf_gauge", "name: string, value: number", ""),
    ("perf_mark", "name: string", ""),
    ("perf_measuring", "", "boolean"),
    ("slot_fill", "name: string", "string"),
    ("exit_with", "key: KeyRef, exit: any", ""),
    ("set_focus", "key: KeyRef", ""),
    ("blur", "", ""),
    ("focus_next", "", ""),
    ("focus_prev", "", ""),
    ("focus_region", "key: KeyRef", ""),
    ("announce", "text: string, politeness?: string", ""),
    ("reveal", "key: KeyRef", ""),
    ("scroll_offset", "key: KeyRef", "Point"),
    ("text_hit", "key: KeyRef, x: number, y: number", "TextHit"),
    (
        "open_menu",
        "key: KeyRef, x: number, y: number, items: {any}",
        "boolean",
    ),
    ("close_menu", "", "boolean"),
    ("selection_text", "", "string"),
    ("request_copy", "", "string, boolean"),
    ("answer_selection_range", "text: string", "boolean"),
    ("set_clipboard", "text: string, html?: string", ""),
    ("set_clipboard_secret", "text: string", ""),
    ("request_paste", "", ""),
    ("awaiting_paste", "", "boolean"),
    ("request_files", "opts?: any", "boolean"),
    ("awaiting_files", "", "boolean"),
    ("selection_ends", "", "SelectionEnds"),
    ("cell_selection", "", "CellSelection"),
    ("selection_html", "", "string"),
    ("select_all_in", "key: KeyRef", "boolean"),
    ("clear_selection", "", "boolean"),
    ("caret_rect", "key: KeyRef, byte: number", "Rect"),
    ("scroll_geometry", "key: KeyRef", "ScrollGeometry"),
    ("layout_of", "key: KeyRef", "Rect"),
    ("set_scroll", "key: KeyRef, x: number, y: number", ""),
    (
        "shift_scroll",
        "key: KeyRef, drawn: number, target: number",
        "",
    ),
    (
        "edge_scroll",
        "key: KeyRef, x: number, y: number, rect: Rect",
        "Point",
    ),
    (
        "set_window_size",
        "window: integer, w: number, h: number",
        "",
    ),
    ("set_tokens", "decl: any", ""),
    ("focus_window", "window: integer", ""),
    (
        "measure_text",
        "s: string | number | {any:any}, opts?: Props, max_w?: number",
        "TextMetrics",
    ),
    (
        "caret_in_text",
        "s: string | number | {any:any}, opts: Props, max_w: number, byte: number",
        "Rect",
    ),
    (
        "add_extension",
        "namespace: string, path: string",
        "boolean, string",
    ),
    ("extension_namespaces", "", "{string}"),
];

/// The tables those calls answer with, beside the event shapes
/// (`kui.Rect`, `kui.RangeEnd`) they share.
const ENV_SHAPES: &[(&str, &[(&str, &str)])] = &[
    ("Point", &[("x", "number"), ("y", "number")]),
    ("TextHit", &[("byte", "integer"), ("line", "integer")]),
    (
        "TextMetrics",
        &[
            ("width", "number"),
            ("height", "number"),
            ("lines", "integer"),
        ],
    ),
    (
        "ScrollGeometry",
        &[
            ("x", "number"),
            ("y", "number"),
            ("w", "number"),
            ("h", "number"),
            ("content_w", "number"),
            ("content_h", "number"),
            ("offset", "Point"),
            ("max_offset", "Point"),
        ],
    ),
    (
        "SelectionEnds",
        &[("anchor", "RangeEnd"), ("focus", "RangeEnd")],
    ),
    ("CellEnd", &[("line", "integer"), ("col", "integer")]),
    (
        "CellSelection",
        &[
            ("node", "integer"),
            ("anchor", "CellEnd"),
            ("focus", "CellEnd"),
            ("block", "boolean"),
        ],
    ),
];

/// `view`'s second argument, as the crate docs give it.
const SLOT: &[(&str, &str, &str)] = &[
    ("name", "string", "The slot in the script's own vocabulary."),
    (
        "namespace",
        "string",
        "What the host loaded the script under.",
    ),
    (
        "params",
        "{string:any}",
        "The host's table, nil when it passed none; cast what it reads (`slot.params.title as string`).",
    ),
    (
        "key",
        "integer",
        "The integer key events and `set_focus` use.",
    ),
];

/// The `kui.d.tl` file for Teal, as text.
///
/// Save it where `tl` finds modules, name it the global environment, and
/// a `.tl` view is checked against every builder and prop:
///
/// ```no_run
/// std::fs::write("types/kui.d.tl", kui_lua::teal_decls())?;
/// // tlconfig.lua: return { include_dir = { "types" }, global_env_def = "kui" }
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn teal_decls() -> String {
    let mut t = Teal::default();
    let mut body = String::new();
    t.claim_classes();

    for a in ALIASES {
        let name = bare(a.name);
        let ty = t.union(a.ty, name);
        doc(&mut body, a.doc);
        let _ = writeln!(body, "  type {name} = {ty}\n");
    }
    let _ = writeln!(
        body,
        "  -- A key a call takes: the integer an event carried, or the label a `key` prop declared.\n  type KeyRef = integer | string\n"
    );

    // Every prop a node table may carry, and its children at 1, 2, ….
    body.push_str("  -- Every prop a node table may carry; its children at 1, 2, ….\n");
    body.push_str("  interface Props is {Node | string}\n");
    for (name, ty, d) in meta::props_fields() {
        let ty = t.ty(&ty, &camel(name));
        field(&mut body, name, &ty, d);
    }
    body.push_str("  end\n\n");
    body.push_str(
        "  -- A node: what a constructor returns and a view gives back.\n  record Node is Props\n    type: string\n  end\n\n",
    );

    // An element's own props, on a record of its own. One `kui.Props`
    // already types keeps that type: Teal refuses a record that retypes
    // its interface's field, where LuaLS lets the subclass say `any`.
    let props: Vec<&str> = meta::props_fields().into_iter().map(|p| p.0).collect();
    for e in schema::ELEMENTS.iter().filter(|e| !e.lua_own.is_empty()) {
        let _ = writeln!(body, "  record {} is Props", e.name);
        for own in e.lua_own.iter().filter(|o| !props.contains(o)) {
            field(&mut body, own, "any", e.doc);
        }
        body.push_str("  end\n\n");
    }

    // The list builders' options, and the heights `list` slices by.
    body.push_str("  -- What `uniform_list` reads off its options; every other key is the container's.\n  record UniformList is Props\n");
    for (name, _, ty, d) in UNIFORM_LIST {
        let ty = t.ty(ty, &format!("UniformList{}", camel(name)));
        field(&mut body, name, &ty, d);
    }
    body.push_str("  end\n\n");
    body.push_str("  -- What `list` reads off its options; every other key is the container's.\n  record List is Props\n");
    for (name, _, ty, d) in LIST {
        let ty = t.ty(ty, &format!("List{}", camel(name)));
        field(&mut body, name, &ty, d);
    }
    body.push_str("  end\n\n");
    body.push_str("  -- A variable-height list's row heights: measured where known, the mean of those elsewhere (rows are 0-based).\n  record RowHeights\n");
    for (name, params, ret) in ROW_HEIGHTS {
        let sep = if params.is_empty() { "" } else { ", " };
        let ty = t.ty(
            &format!("fun(self: kui.RowHeights{sep}{params}): {ret}"),
            "RowHeights",
        );
        let _ = writeln!(body, "    {name}: {ty}");
    }
    body.push_str("  end\n\n");

    events(&mut t, &mut body);
    env(&mut body);

    let mut out = String::new();
    out.push_str(
        "-- kui-lua's view DSL for Teal, generated from kui-core's schema by\n\
         -- `kui_lua::teal_decls()`. Regenerate it rather than editing it.\n\
         --\n\
         -- Save it as `kui.d.tl` where `tl` finds modules and name it the\n\
         -- global environment in `tlconfig.lua`:\n\
         --\n\
         --   return { include_dir = { \"types\" }, global_env_def = \"kui\" }\n\
         --\n\
         -- `tl check view.tl` types a view against it, and `tl gen view.tl`\n\
         -- writes the `view.lua` a host loads. `view(env: kui.Env, slot:\n\
         -- kui.Slot): kui.Node` and `on_event(ev: kui.Event, at: kui.Envelope)`\n\
         -- are the script's own globals; an app's message is a record that\n\
         -- extends `kui.Event` (and the `kui.With…` field its event fills),\n\
         -- cast from `ev`.\n\n\
         global type kui = record\n",
    );
    for d in &t.decls {
        for line in d.lines() {
            let _ = writeln!(out, "  {line}");
        }
        out.push('\n');
    }
    out.push_str(&body);
    out.push_str("end\n\n");

    // The prelude's builders, each with the comment above it.
    t.outside = true;
    let class_of = meta::class_of();
    for f in meta::prelude_functions() {
        for line in f.doc.lines() {
            let _ = writeln!(out, "-- {line}");
        }
        let (params, ret) = meta::signature(&f, &class_of);
        let params: Vec<String> = params
            .iter()
            .map(|(p, optional, ty)| {
                let opt = if *optional { "?" } else { "" };
                let ty = if p == "env" {
                    "kui.Env".to_string()
                } else {
                    t.ty(ty, &camel(p))
                };
                format!("{p}{opt}: {ty}")
            })
            .collect();
        let ret = t.ty(ret, "Return");
        let _ = writeln!(
            out,
            "global {}: function({}): {ret}\n",
            f.name,
            params.join(", ")
        );
    }
    out.push_str(
        "-- `rows` rows, none measured, each at `estimate` logical px (20 when left out) until measured.\n\
         global row_heights: function(rows: number, estimate?: number): kui.RowHeights\n",
    );
    out
}

/// The event classes: the message `on_event` hears and the envelope
/// beside it, the shapes the data names, each event's data record and the
/// interface naming its field, and the window's own groups (ADR 0047,
/// ADR 0048).
fn events(t: &mut Teal, body: &mut String) {
    let kinds: Vec<String> = schema::EVENTS
        .iter()
        .map(|e| format!("\"{}\"", e.kind))
        .collect();
    let kind = t.union(&kinds.join("|"), "EventKind");
    let _ = writeln!(
        body,
        "  -- An event `on_event` hears: the message its handler declared, the event's data in the\n  \
         -- field named for the event (`drag`, `change`, …), and the envelope beside them. An app's\n  \
         -- message extends it — `local record Pan is kui.Event, kui.WithDrag` — and is cast from `ev`.\n  \
         interface Event\n    \
         -- What happened: the event kind, beside the message rather than in it.\n    \
         event: {kind}\n    \
         -- The emitting node's key, what `env.edit_text` and the rest take.\n    \
         node_key: integer\n    \
         -- The window it came from, as `env.window.id` reads; a `window` event's data, which has the name.\n    \
         window: integer | WindowEvent\n    \
         -- The full name of the slot the node was filled into.\n    \
         slot: string\n    \
         -- The namespace of the plugin a reply came from.\n    \
         from: string\n    \
         -- The message's own `kind`: the core never writes one.\n    \
         kind: string\n    \
         -- A message that is not a table, kept under `tag`.\n    \
         tag: any\n  \
         end\n\n  \
         -- `on_event`'s second argument: the envelope alone, whatever fields the message has.\n  \
         record Envelope\n    \
         event: {kind}\n    \
         node_key: integer\n    \
         window: integer\n    \
         slot: string\n    \
         from: string\n  \
         end\n"
    );
    for (class, fields) in SHAPES {
        let name = bare(class);
        let _ = writeln!(body, "  record {name}");
        for (f, _, ty) in *fields {
            let ty = t.ty(ty, &format!("{name}{}", camel(f)));
            let _ = writeln!(body, "    {f}: {ty}");
        }
        body.push_str("  end\n\n");
    }
    for (class, f, with, fields) in EVENT_DATA {
        let name = bare(class);
        let _ = writeln!(body, "  -- A `{f}` event's data.\n  record {name}");
        for (field, _, ty) in *fields {
            let ty = t.ty(ty, &format!("{name}{}", camel(field)));
            let _ = writeln!(body, "    {}: {ty}", key(field));
        }
        let _ = writeln!(
            body,
            "  end\n\n  -- A message that carries a `{f}` event: its `{f}` field alone.\n  \
             interface {}\n    {f}: {name}\n  end\n",
            bare(with)
        );
    }
    for (name, d, members) in WITH_UNIONS {
        let members: Vec<&str> = members.iter().map(|m| bare(m)).collect();
        doc(body, d);
        let _ = writeln!(
            body,
            "  interface {} is {}\n  end\n",
            bare(name),
            members.join(", ")
        );
    }
    body.push_str(
        "  -- The messages the window's own events arrive as (ADR 0048): `window_events` on the root table.\n  \
         -- A key left out is an event the window does not hear.\n  \
         record WindowEvents\n",
    );
    for (k, with, d) in WINDOW_EVENTS {
        let carries = if with.is_empty() {
            String::new()
        } else {
            format!(" It arrives with `{with}`.")
        };
        field(body, k, "any", &format!("{d}{carries}"));
    }
    body.push_str(
        "  end\n\n  \
         -- A `windows` entry as a table: the window's name and config, and the messages its opening and\n  \
         -- closing (`kui.WithWindow`) and a popup's dismissal (`kui.WithDismiss`) arrive as (ADR 0048).\n  \
         record WindowEntry\n",
    );
    for (f, _, ty) in WINDOW_ENTRY {
        let ty = t.ty(ty, &format!("WindowEntry{}", camel(f)));
        let _ = writeln!(body, "    {f}: {ty}");
    }
    body.push_str("  end\n\n");
}

/// `view`'s arguments: `env` with its facts (`schema::ENV_FIELDS`), the
/// palette (`THEME_ROLES`), the metrics (`METRIC_ROLES`), the tokens and
/// the calls (`DOORS`' Lua column), and `slot`.
fn env(body: &mut String) {
    for (name, fields) in ENV_SHAPES {
        let _ = writeln!(body, "  record {name}");
        for (f, ty) in *fields {
            let _ = writeln!(body, "    {f}: {ty}");
        }
        body.push_str("  end\n\n");
    }

    // The facts filed under a head (`window.id`), each head a record.
    let mut flat = String::new();
    let mut heads: BTreeMap<&str, String> = BTreeMap::new();
    for row in schema::ENV_FIELDS {
        for path in row.lua {
            let ty = env_fact(path);
            match path.split_once('.') {
                None => field(&mut flat, path, ty, row.doc),
                Some((head, leaf)) => {
                    field(heads.entry(head).or_default(), leaf, ty, row.doc);
                }
            }
        }
    }
    for (head, fields) in &heads {
        let _ = writeln!(body, "  record Env{}\n{fields}  end\n", camel(head));
    }

    body.push_str("  -- `env.theme`: the palette the core derived from `system`, one `0xRRGGBBAA` per role.\n  record Theme\n");
    for role in schema::THEME_ROLES {
        field(body, role.name, "integer", role.doc);
    }
    body.push_str(
        "    -- The base the palette came from.\n    appearance: string\n    \
         -- A multiplier, not a colour: what a disabled control's opacity is.\n    \
         disabled_opacity: number\n  end\n\n",
    );
    body.push_str("  -- `env.metrics`: the sizes the stock widgets are built from, in logical px.\n  record Metrics\n");
    for role in schema::METRIC_ROLES {
        field(body, role.name, "number", role.doc);
    }
    body.push_str("  end\n\n");
    body.push_str(
        "  -- `env.tokens`: the named tokens in force, the script's own over the host's.\n  \
         record Tokens\n    colors: {string:integer}\n    lengths: {string:number}\n    \
         families: {string:string}\n  end\n\n",
    );

    body.push_str(
        "  -- `view`'s first argument: the host's facts for this frame, and the calls that answer about it.\n  record Env\n",
    );
    body.push_str(&flat);
    for head in heads.keys() {
        let _ = writeln!(body, "    {head}: Env{}", camel(head));
    }
    body.push_str("    theme: Theme\n    metrics: Metrics\n    tokens: Tokens\n");
    for d in schema::DOORS {
        let Cell::Is(name) = d.lua else { continue };
        let (params, ret) = env_call(name);
        let ret = if ret.is_empty() {
            String::new()
        } else {
            format!(": {ret}")
        };
        field(body, name, &format!("function({params}){ret}"), d.doc);
    }
    body.push_str("  end\n\n");

    body.push_str(
        "  -- `view`'s second argument: which slot the host is filling.\n  record Slot\n",
    );
    for (f, ty, d) in SLOT {
        field(body, f, ty, d);
    }
    body.push_str("  end\n");
}

/// A fact's type, by its Lua path; the tests hold the table to the schema.
fn env_fact(path: &str) -> &'static str {
    ENV_FACTS
        .iter()
        .find(|(p, _)| *p == path)
        .map(|(_, ty)| *ty)
        .unwrap_or_else(|| panic!("env.{path} has no Teal type in ENV_FACTS"))
}

/// A call's parameters and returns, by its Lua name.
fn env_call(name: &str) -> (&'static str, &'static str) {
    ENV_CALLS
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|(_, p, r)| (*p, *r))
        .unwrap_or_else(|| panic!("env.{name} has no Teal signature in ENV_CALLS"))
}

/// The translation's state: the nested types it named on the way, which
/// the namespace record declares ahead of the classes.
#[derive(Default)]
struct Teal {
    /// Each named type's declaration, in the order it was named.
    decls: Vec<String>,
    /// Every name the namespace holds, and what it was named for: a
    /// second definition under a name is a clash the tests meet.
    names: BTreeMap<String, String>,
    /// The name a definition already has, so a shape or a set of words
    /// met twice is one type.
    by_def: BTreeMap<String, String>,
    /// Writing outside the namespace record, where its types are `kui.X`.
    outside: bool,
}

impl Teal {
    /// Takes the names the classes are written under, so a named type
    /// cannot land on one.
    fn claim_classes(&mut self) {
        let mut classes: Vec<String> = [
            "Props",
            "Node",
            "UniformList",
            "List",
            "RowHeights",
            "Event",
            "Envelope",
            "WindowEvents",
            "WindowEntry",
            "KeyRef",
            "Theme",
            "Metrics",
            "Tokens",
            "Env",
            "Slot",
        ]
        .map(String::from)
        .to_vec();
        classes.extend(ALIASES.iter().map(|a| bare(a.name).to_string()));
        classes.extend(SHAPES.iter().map(|s| bare(s.0).to_string()));
        classes.extend(EVENT_DATA.iter().map(|d| bare(d.0).to_string()));
        classes.extend(EVENT_DATA.iter().map(|d| bare(d.2).to_string()));
        classes.extend(WITH_UNIONS.iter().map(|w| bare(w.0).to_string()));
        classes.extend(ENV_SHAPES.iter().map(|s| s.0.to_string()));
        classes.extend(
            schema::ELEMENTS
                .iter()
                .filter(|e| !e.lua_own.is_empty())
                .map(|e| e.name.to_string()),
        );
        for c in classes {
            let prev = self.names.insert(c.clone(), format!("class {c}"));
            assert!(prev.is_none(), "two classes are named {c}");
        }
    }

    /// `name` as a type is written where the text goes.
    fn qual(&self, name: &str) -> String {
        if self.outside {
            format!("kui.{name}")
        } else {
            name.to_string()
        }
    }

    /// Names `def` `want` — or answers the name it already has — and
    /// declares it with `body`, which is handed the name and writes inside
    /// the namespace.
    fn declare(
        &mut self,
        want: String,
        def: String,
        body: impl FnOnce(&mut Self, &str) -> String,
    ) -> String {
        if let Some(name) = self.by_def.get(&def) {
            return self.qual(name);
        }
        if let Some(prev) = self.names.insert(want.clone(), def.clone()) {
            panic!("{want} is named for both `{prev}` and `{def}`");
        }
        self.by_def.insert(def, want.clone());
        let outside = std::mem::replace(&mut self.outside, false);
        let decl = body(self, &want);
        self.outside = outside;
        self.decls.push(decl);
        self.qual(&want)
    }

    /// A LuaLS type in Teal's words. One alias alone keeps its name;
    /// anything else is [`Teal::union`].
    fn ty(&mut self, luals: &str, ctx: &str) -> String {
        if let [m] = meta::members(luals)[..]
            && let Some(a) = alias(m.strip_suffix('?').unwrap_or(m))
        {
            return self.qual(bare(a.name));
        }
        self.union(luals, ctx)
    }

    /// A LuaLS union in Teal's words, its aliases spelt out: the string
    /// literals one `enum` (`ctx` itself when they are all there is,
    /// `ctx` + `Word` beside other members), a shape a record, and
    /// `string` or `number` taking in the literals or the integers they
    /// already admit.
    fn union(&mut self, luals: &str, ctx: &str) -> String {
        let mut words = Vec::new();
        let mut members = Vec::new();
        flatten(luals, ctx, &mut words, &mut members);
        let tables = members
            .iter()
            .filter(|(m, _)| {
                m.starts_with('{')
                    || m.ends_with("[]")
                    || *m == "table"
                    || (m.starts_with("kui.") && alias(m).is_none())
            })
            .count();
        let mut parts: Vec<String> = Vec::new();
        for (m, ctx) in members {
            let t = if m.starts_with('{') {
                self.shape(m, &ctx, tables > 1)
            } else {
                self.member(m, &ctx)
            };
            if !parts.contains(&t) {
                parts.push(t);
            }
        }
        if parts.iter().any(|p| p == "any") {
            return "any".into();
        }
        if parts.iter().any(|p| p == "string") {
            words.clear();
        }
        if parts.iter().any(|p| p == "number") {
            parts.retain(|p| p != "integer");
        }
        if !words.is_empty() {
            let want = if parts.is_empty() {
                ctx.to_string()
            } else {
                format!("{ctx}Word")
            };
            let mut sorted = words.clone();
            sorted.sort();
            let e = self.declare(want, format!("enum {}", sorted.join(" ")), |_, name| {
                format!("enum {name}\n  {}\nend", words.join(" "))
            });
            parts.insert(0, e);
        }
        parts.join(" | ")
    }

    /// One member that is neither a literal nor a shape.
    fn member(&mut self, m: &str, ctx: &str) -> String {
        match m {
            "number" | "integer" | "string" | "boolean" | "any" => m.into(),
            "table" => "{any:any}".into(),
            m if m.ends_with("[]") => {
                let inner = &m[..m.len() - 2];
                let inner = inner
                    .strip_prefix('(')
                    .and_then(|i| i.strip_suffix(')'))
                    .unwrap_or(inner);
                format!("{{{}}}", self.ty(inner, ctx))
            }
            m if m.starts_with("fun(") => self.function(m, ctx),
            m if m.starts_with("kui.") => self.qual(bare(m)),
            m => panic!("no Teal type for `{m}`"),
        }
    }

    /// A `{ a: T, b?: U }` shape as a record: named `ctx` where it is the
    /// union's one table, and `ctx` + its first required field `where`
    /// that field is set where the union has others to tell it from.
    fn shape(&mut self, m: &str, ctx: &str, discriminated: bool) -> String {
        let fields = meta::shape(m);
        let first = fields.iter().find(|f| f.1).map(|f| f.0);
        let (want, guard) = match (discriminated, first) {
            (false, _) => (ctx.to_string(), None),
            (true, Some(f)) => (format!("{ctx}{}", camel(f)), Some(f)),
            (true, None) => {
                panic!("{ctx}: `{m}` has no field to tell it from the union's other tables")
            }
        };
        let def = m.split_whitespace().collect::<Vec<_>>().join(" ");
        self.declare(want, def, |t, name| {
            let mut out = format!("record {name}");
            if let Some(f) = guard {
                let _ = write!(out, " where self.{f} ~= nil");
            }
            out.push('\n');
            for (f, _, ty) in &fields {
                let ty = t.ty(ty, &format!("{name}{}", camel(f)));
                let _ = writeln!(out, "  {f}: {ty}");
            }
            out.push_str("end");
            out
        })
    }

    /// `fun(a: T, b?: U): R` as `function(a: T, b?: U): R`; a `nil` return
    /// is none, and `R?` is `R`, every Teal value being nilable.
    fn function(&mut self, m: &str, ctx: &str) -> String {
        let inner = &m["fun(".len()..];
        let mut depth = 1;
        let close = inner
            .char_indices()
            .find(|&(_, c)| {
                match c {
                    '(' | '{' => depth += 1,
                    ')' | '}' => depth -= 1,
                    _ => {}
                }
                depth == 0
            })
            .map(|(i, _)| i)
            .unwrap_or_else(|| panic!("`{m}` does not close"));
        let params: Vec<String> = split_top(&inner[..close], ',')
            .into_iter()
            .filter(|p| !p.is_empty())
            .map(|p| {
                let (name, ty) = p
                    .split_once(':')
                    .unwrap_or_else(|| panic!("`{p}` in `{m}`"));
                let name = name.trim();
                let ty = self.ty(
                    ty.trim(),
                    &format!("{ctx}{}", camel(name.trim_end_matches('?'))),
                );
                format!("{name}: {ty}")
            })
            .collect();
        let ret = inner[close + 1..].trim().trim_start_matches(':').trim();
        let ret = ret.strip_suffix('?').unwrap_or(ret);
        if ret.is_empty() || ret == "nil" {
            format!("function({})", params.join(", "))
        } else {
            let ret = self.ty(ret, ctx);
            format!("function({}): {ret}", params.join(", "))
        }
    }
}

/// `luals`'s members with its aliases spelt out, each alias's own under
/// its name (so a shape is named for the alias that has it, wherever it
/// is spelt out): the string literals into `words`, the rest beside the
/// context each is named in.
fn flatten<'a>(
    luals: &'a str,
    ctx: &str,
    words: &mut Vec<String>,
    members: &mut Vec<(&'a str, String)>,
) {
    for m in meta::members(luals) {
        let m = m.strip_suffix('?').unwrap_or(m);
        if m.starts_with('"') {
            if !words.iter().any(|w| w == m) {
                words.push(m.to_string());
            }
        } else if let Some(a) = alias(m) {
            flatten(a.ty, bare(a.name), words, members);
        } else {
            members.push((m, ctx.to_string()));
        }
    }
}

fn alias(name: &str) -> Option<&'static meta::Alias> {
    ALIASES.iter().find(|a| a.name == name)
}

/// `kui.Name` without its namespace.
fn bare(name: &str) -> &str {
    name.strip_prefix("kui.").unwrap_or(name)
}

/// `snake_case` as `CamelCase`.
fn camel(snake: &str) -> String {
    snake
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// `s` split at `sep` outside brackets, each piece trimmed.
fn split_top(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (i, c) in s.char_indices() {
        match c {
            '{' | '(' => depth += 1,
            '}' | ')' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out
}

/// A record field Teal can write: a Lua keyword is no Teal name either,
/// and the event classes have one (`repeat`), so it is quoted.
fn key(name: &str) -> String {
    if meta::LUA_KEYWORDS.contains(&name) {
        format!("[\"{name}\"]")
    } else {
        name.to_string()
    }
}

/// A doc as one comment line, at the namespace's indent.
fn doc(out: &mut String, doc: &str) {
    let doc = doc.split_whitespace().collect::<Vec<_>>().join(" ");
    let _ = writeln!(out, "  -- {doc}");
}

/// A record field with its doc above it.
fn field(out: &mut String, name: &str, ty: &str, doc: &str) {
    let doc = doc.split_whitespace().collect::<Vec<_>>().join(" ");
    let _ = writeln!(out, "    -- {doc}\n    {}: {ty}", key(name));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::tests::{PROBES, Probe, ROOT_ONLY, parses, samples};
    use kui_core::Extension as _;
    use mlua::{Lua, Table};
    use std::path::PathBuf;
    use std::sync::OnceLock;

    /// Teal's compiler at the release the file is held to (`tests/tl`).
    /// It checks under 5.5 but writes for 5.4 at most, which 5.5 runs.
    const TL: &str = include_str!("../tests/tl/tl.lua");

    /// Where the file is written, once a process, for the compiler's
    /// module search to find as `kui`.
    fn decls_dir() -> &'static PathBuf {
        static DIR: OnceLock<PathBuf> = OnceLock::new();
        DIR.get_or_init(|| {
            let dir = std::env::temp_dir().join(format!("kui-lua-teal-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("kui.d.tl"), teal_decls()).unwrap();
            dir
        })
    }

    /// The compiler in a fresh Lua, finding modules beside the file.
    fn tl() -> (Lua, Table) {
        let lua = Lua::new();
        let tl: Table = lua.load(TL).set_name("@tl.lua").eval().unwrap();
        tl.set("path", format!("{}/?.lua", decls_dir().display()))
            .unwrap();
        (lua, tl)
    }

    /// `src` checked as `name`, with `kui` the global environment where
    /// `with_kui` (what `global_env_def = "kui"` loads): each syntax and
    /// type error as its line and message.
    fn check_as(src: &str, name: &str, with_kui: bool) -> Vec<(usize, String)> {
        let (lua, tl) = tl();
        lua.load(
            r#"
            local tl, src, name, with_kui = ...
            local env = assert(tl.new_env({
              defaults = { gen_target = "5.4", gen_compat = "off" },
              predefined_modules = with_kui and { "kui" } or nil,
            }))
            local res = tl.check_string(src, env, name)
            local out = {}
            for _, list in ipairs({ res.syntax_errors, res.type_errors }) do
              for _, e in ipairs(list or {}) do out[#out + 1] = e.y .. "\t" .. e.msg end
            end
            return out
            "#,
        )
        .call::<Vec<String>>((tl, src, name, with_kui))
        .unwrap()
        .into_iter()
        .map(|e| {
            let (y, msg) = e.split_once('\t').unwrap();
            (y.parse().unwrap(), msg.to_string())
        })
        .collect()
    }

    fn check(src: &str) -> Vec<(usize, String)> {
        check_as(src, "view.tl", true)
    }

    /// The file is Teal the compiler takes whole, and it declares every
    /// prop the schema has and every builder the prelude defines.
    #[test]
    fn the_file_is_teal_the_compiler_takes() {
        let decls = teal_decls();
        let errors = check_as(&decls, "kui.d.tl", false);
        assert!(errors.is_empty(), "{errors:?}");
        let props = &decls[decls.find("  interface Props is").unwrap()..];
        let props = &props[..props.find("\n  end\n").unwrap()];
        for (name, ..) in meta::props_fields() {
            assert!(props.contains(&format!("\n    {name}: ")), "{name} missing");
        }
        for f in meta::prelude_functions() {
            assert!(
                decls.contains(&format!("\nglobal {}: function(", f.name)),
                "{} missing",
                f.name
            );
        }
        // The prelude's nil-taking table is optional in Teal too, and an
        // element with props of its own types its table.
        assert!(decls.contains("\nglobal row: function(t?: kui.Props): kui.Node\n"));
        assert!(decls.contains("\nglobal edit: function(t: kui.edit): kui.Node\n"));
        assert!(
            decls.contains("\nglobal uniform_list: function(env: kui.Env, opts: kui.UniformList, ")
        );
    }

    /// Whether `ty` takes whole numbers only: `integer` without `number`
    /// among its members, aliases spelt out.
    fn whole_only(ty: &str) -> bool {
        let mut words = Vec::new();
        let mut members = Vec::new();
        flatten(ty, "", &mut words, &mut members);
        members.iter().any(|(m, _)| *m == "integer") && !members.iter().any(|(m, _)| *m == "number")
    }

    /// Whether `ty` reads a table with `keys`: it takes any table, or a
    /// shape among its members names every one of them. A table no shape
    /// names is one the parser walks past (`pad = { pct = 50 }` pads
    /// nothing), and refusing it is what the file is for.
    fn reads_keys(ty: &str, keys: &[&str]) -> bool {
        let mut words = Vec::new();
        let mut members = Vec::new();
        flatten(ty, "", &mut words, &mut members);
        members.iter().any(|(m, _)| match *m {
            "table" | "any" => true,
            m if m.starts_with('{') => {
                let fields = meta::shape(m);
                keys.iter().all(|k| fields.iter().any(|f| f.0 == *k))
            }
            m => m.starts_with("kui."),
        })
    }

    /// Every value the parser takes for a prop, Teal takes under the
    /// prop's name in a `row { }`: each member of the LuaLS type sampled,
    /// and each probe the parser accepts. Left out are what the LuaLS
    /// tests leave out — a flag, which the parser takes anything for, and
    /// the root's composites, which it never reads — a fraction where the
    /// type is whole (a colour, a handle: the parser truncates or refuses
    /// one, and the type says which number the value means), and a table
    /// whose keys the parser walks past.
    #[test]
    fn every_value_the_parser_takes_teal_takes() {
        let lua = Lua::new();
        let mut lines = Vec::new();
        for (name, ty, _) in meta::props_fields() {
            if ROOT_ONLY.contains(&name) {
                continue;
            }
            for v in samples(&ty, &[]) {
                lines.push(format!("{name} = {v}"));
            }
            if ty == "boolean" {
                continue;
            }
            for (src, p) in PROBES {
                match p {
                    Probe::Float if whole_only(&ty) => continue,
                    Probe::Table(keys) if !reads_keys(&ty, keys) => continue,
                    _ => {}
                }
                if parses(&lua, &format!("{name} = {src}")).is_ok() {
                    lines.push(format!("{name} = {src}"));
                }
            }
        }
        let src: String = lines
            .iter()
            .map(|l| format!("local _ = row {{ {l} }}\n"))
            .collect();
        let failed: Vec<String> = check(&src)
            .into_iter()
            .map(|(y, msg)| format!("{}: {msg}", lines[y - 1]))
            .collect();
        assert!(lines.len() > 500, "{} lines", lines.len());
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }

    /// What kui refuses, or only warns about, Teal refuses where it is
    /// typed: a misspelt prop, a wrong-typed value, a word no enum has, a
    /// misspelt env read, a missing table, an event phase that does not
    /// exist.
    #[test]
    fn teal_refuses_what_kui_refuses() {
        let lua = Lua::new();
        // A key no table claims is a warning the host can read, not a
        // refusal; the rest the parser refuses.
        assert!(parses(&lua, "widht = 300").is_ok());
        let refused = [
            "bg = true",
            "key = 3",
            "index = \"x\"",
            "float = \"sideways\"",
            "direction = \"sideways\"",
        ];
        for p in refused {
            assert!(parses(&lua, p).is_err(), "{p} parses");
        }
        let mut src: String = std::iter::once("widht = 300")
            .chain(refused)
            .map(|p| format!("local _ = row {{ {p} }}\n"))
            .collect();
        let calls = [
            "local _ = env.theme.surfce",
            "local _ = env.is_focused()",
            "local _ = button()",
            "local _ = (ev as Pan).drag.phase == \"begin\"",
        ];
        src.push_str(
            "local record Pan is kui.Event, kui.WithDrag end\n\
             global function view(env: kui.Env, ev: kui.Event): kui.Node\n",
        );
        let first_call = src.lines().count() + 1;
        for c in calls {
            let _ = writeln!(src, "  {c}");
        }
        src.push_str("  return row()\nend\n");
        let errors = check(&src);
        let lines = refused.len() + 1;
        for y in (1..=lines).chain(first_call..first_call + calls.len()) {
            assert!(
                errors.iter().any(|(at, _)| *at == y),
                "line {y} `{}` checks: {errors:?}",
                src.lines().nth(y - 1).unwrap()
            );
        }
        assert_eq!(errors.len(), lines + calls.len(), "{errors:?}");
        // What Teal cannot say: a literal against a union of `where`
        // records is told apart at run time only, so a shape no member
        // has gets past it. When it does not, this goes in the list.
        assert!(parses(&lua, "width = { percent = 50 }").is_err());
        assert!(check("local _ = row { width = { percent = 50 } }\n").is_empty());
    }

    /// The `kui.Env` record is the env a view is handed: its facts are
    /// `schema::ENV_FIELDS`' Lua paths and its calls `schema::DOORS`' Lua
    /// column, each table here naming exactly those, and a real env's
    /// values are of the types the file gives them — under an env with
    /// every optional fact present — as the tables its calls answer with
    /// have only the fields the file gives theirs.
    #[test]
    fn the_env_record_is_the_env() {
        use kui_core::{Core, Key, Rect, Size, Slot};
        let mut paths: Vec<&str> = schema::ENV_FIELDS
            .iter()
            .flat_map(|f| f.lua.iter().copied())
            .collect();
        let mut ours: Vec<&str> = ENV_FACTS.iter().map(|f| f.0).collect();
        paths.sort();
        ours.sort();
        assert_eq!(ours, paths, "ENV_FACTS and schema::ENV_FIELDS' Lua paths");
        let mut doors: Vec<&str> = schema::DOORS
            .iter()
            .filter_map(|d| match d.lua {
                Cell::Is(name) => Some(name),
                _ => None,
            })
            .collect();
        let mut calls: Vec<&str> = ENV_CALLS.iter().map(|c| c.0).collect();
        doors.sort();
        calls.sort();
        assert_eq!(calls, doors, "ENV_CALLS and schema::DOORS' Lua column");

        let mut ext = crate::LuaExtension::from_source(
            "env",
            r#"
                local function kind(v)
                  return type(v) == "number" and math.type(v) or type(v)
                end
                function view(env)
                  facts, answers = {}, {}
                  for k, v in pairs(env) do
                    if type(v) == "table" then
                      for wk, wv in pairs(v) do facts[k .. "." .. wk] = kind(wv) end
                    elseif type(v) ~= "function" then facts[k] = kind(v) end
                  end
                  local asked = {
                    scroll_offset = env.scroll_offset("list"),
                    scroll_geometry = env.scroll_geometry("list"),
                    layout_of = env.layout_of("a"),
                    measure_text = env.measure_text("hi"),
                    text_hit = env.text_hit("t", 1, 1),
                    caret_rect = env.caret_rect("t", 0),
                    caret_in_text = env.caret_in_text("hi", {}, 100, 0),
                  }
                  for call, r in pairs(asked) do
                    for k, v in pairs(r) do
                      answers[#answers + 1] = call .. "." .. k
                      if type(v) == "table" then
                        for sk in pairs(v) do answers[#answers + 1] = call .. "." .. k .. "." .. sk end
                      end
                    end
                  end
                  return column { key = "root",
                    row { key = "a", focusable = true, width = 50, height = 20 },
                    text("hello", { key = "t" }),
                    column { key = "list", height = 40, scroll = true,
                      row { height = 200 } },
                    column { key = "dock", focus_region = true,
                      row { key = "d", focusable = true, width = 50, height = 20 } },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        core.env.refresh_hz = Some(60.0);
        core.env.system.accent = Some(kui_core::Color::hex(0x3b82f6ff));
        core.env.system.locale = kui_core::Locale::new("en-US");
        core.env.window.native_controls = Some(Rect::new(0.0, 0.0, 78.0, 28.0));
        let frame = |core: &mut Core, ext: &mut crate::LuaExtension| {
            let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
            ext.view(&Slot::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        core.set_focus(Some(Key::ROOT.str("root").str("dock").str("d")));
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);

        let globals = ext.lua().globals();
        let facts: BTreeMap<String, String> = globals.get("facts").unwrap();
        let admits = |ty: &str, kind: &str| match ty {
            "number" => kind == "integer" || kind == "float",
            ty => ty == kind,
        };
        for (path, ty) in ENV_FACTS {
            let kind = facts
                .get(*path)
                .unwrap_or_else(|| panic!("env.{path} is not there"));
            assert!(admits(ty, kind), "env.{path} is a {kind}, typed {ty}");
        }
        for role in schema::THEME_ROLES {
            assert_eq!(facts[&format!("theme.{}", role.name)], "integer");
        }
        for role in schema::METRIC_ROLES {
            assert!(admits("number", &facts[&format!("metrics.{}", role.name)]));
        }

        // A call's answer has the fields of the record it returns, and a
        // field that is a record has that record's.
        let shape = |name: &str| -> Vec<(String, String)> {
            if let Some((_, fields)) = ENV_SHAPES.iter().find(|s| s.0 == name) {
                return fields
                    .iter()
                    .map(|(f, t)| (f.to_string(), t.to_string()))
                    .collect();
            }
            let (_, fields) = SHAPES
                .iter()
                .find(|s| bare(s.0) == name)
                .unwrap_or_else(|| panic!("{name} is no shape"));
            fields
                .iter()
                .map(|(f, _, t)| (f.to_string(), bare(t).to_string()))
                .collect()
        };
        let answers: Vec<String> = globals.get("answers").unwrap();
        for a in &answers {
            let mut parts = a.split('.');
            let call = parts.next().unwrap();
            let mut ty = env_call(call).1.to_string();
            for p in parts {
                let fields = shape(&ty);
                ty = fields
                    .iter()
                    .find(|(f, _)| f == p)
                    .unwrap_or_else(|| panic!("env.{call}: `{a}` is no field of {ty}"))
                    .1
                    .clone();
            }
        }
        // `layout_of` answers a `Rect` as `caret_in_text` does, so the four
        // that answer under this frame cover every shape but the
        // selections', which need a selection to answer at all.
        for call in [
            "scroll_offset",
            "scroll_geometry",
            "measure_text",
            "caret_in_text",
        ] {
            assert!(
                answers.iter().any(|a| a.starts_with(&format!("{call}."))),
                "env.{call} answered nothing: {answers:?}"
            );
        }
    }

    /// A view in Teal — the crate docs' panel, typed, with a message
    /// record, an event's data and the list builders — checks clean, and
    /// the Lua `tl gen` writes for it runs as a script.
    #[test]
    fn a_teal_view_checks_and_its_lua_runs() {
        let src = r#"
global slots = { "root" }

local todos = { "ship the layout solver", "wire up wgpu" }
local done: {integer:boolean} = {}
local heights = row_heights(100, 24)

local record Toggle is kui.Event
  index: integer
end
local record Pan is kui.Event, kui.WithDrag
end

global function view(env: kui.Env, slot: kui.Slot): kui.Node
  local t = env.theme
  local params = slot.params or {}
  local items: {kui.Node} = {}
  for i, todo in ipairs(todos) do
    items[#items + 1] = row {
      gap = 8, pad = { t = 4, b = 4 },
      on_click = { kind = "toggle", index = i },
      text((done[i] and "[x] " or "[ ] ") .. todo, { size = 14, color = t.fg }),
    }
  end
  local g = env.scroll_geometry("log")
  local at = g and g.offset.y or 0
  return column {
    width = 300, height = "grow", pad = 16, gap = 10, min_width = "50%",
    bg = t.surface, radius = env.metrics.radius, border = { w = 1, color = t.border },
    text((params.title or "todos") as string, { size = 12, color = t.muted }),
    text({ "a ", { "b", bold = true } }),
    edit { key = "filter", label = "filter todos", initial = "", width = "grow" },
    column { height = { grow = 1 }, width = { pct = 50 }, scroll = true, table.unpack(items) },
    uniform_list(env, { key = "log", row_h = 20, rows = 50, height = 100 },
      function(i: integer): kui.Node return text(i) end),
    list(env, { key = "var", heights = heights, height = 100 },
      function(_i: integer, _w: number): number return 24 end,
      function(i: integer): kui.Node return text(i) end),
    button { label = "add", on_click = { kind = "add" } },
    row { width = "clamp(400px, 80%, 1000px)", height = { min = { 10, "50%", { px = 3 } } },
          on_drag = { kind = "pan" } },
    text(at),
  }
end

global function on_event(ev: kui.Event, at: kui.Envelope): {string:any}
  if ev.kind == "toggle" then
    local m = ev as Toggle
    done[m.index] = not done[m.index]
    return { index = m.index, done = done[m.index] }
  elseif ev.kind == "pan" and at.event == "drag" then
    local p = ev as Pan
    if p.drag.phase == "start" then heights:set(0, p.drag.x) end
  elseif ev.kind == "add" then
    todos[#todos + 1] = "todo #" .. (#todos + 1)
  end
end
"#;
        let errors = check(src);
        assert!(errors.is_empty(), "{errors:?}");

        let (lua, tl) = tl();
        let code: String = lua
            .load(
                r#"
                local tl, src = ...
                local env = assert(tl.new_env({
                  defaults = { gen_target = "5.4", gen_compat = "off" },
                  predefined_modules = { "kui" },
                }))
                return assert(tl.gen(src, env))
                "#,
            )
            .call((tl, src))
            .unwrap();
        let mut ext = crate::LuaExtension::from_source("view", &code).unwrap();
        let mut core = kui_core::Core::new();
        for _ in 0..2 {
            let mut ui = core.frame(kui_core::Size::new(800.0, 600.0), 1.0);
            ext.view(&kui_core::Slot::root(), &mut ui).unwrap();
            ui.finish();
        }
        assert!(core.output().0.quads.len() > 3);
    }
}
