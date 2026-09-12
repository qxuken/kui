//! Lua extensions for kui. A script defines `view(env, slot)` returning a plain
//! table tree (built with the injected `row`/`column`/`text`/`button`
//! prelude) and optionally `on_event(ev)`, whose return value is its replies
//! to the host; `slot` says which slot the host is filling (`slot.name`,
//! `slot.namespace`, `slot.params`, `slot.key`; ADR 0014), and a `slots`
//! global lists the names it fills. `env` carries host facts
//! (refresh rate, focus, viewport), queries (`env.edit_text(key)`,
//! `env.is_focused(key)`, `env.is_hovered(key)`, `env.is_pressed(key)`,
//! `env.measure_text(s, opts, max_w)`), focus verbs (`env.set_focus(key)`,
//! `env.blur()`, `env.focus_next()`, `env.focus_prev()`),
//! `env.announce(text, politeness)` and scroll calls
//! (`env.reveal(key)`, `env.scroll_offset(key)`, `env.set_scroll(key, x, y)`,
//! `env.scroll_geometry(key)`), text queries (`env.text_hit(key, x, y)`,
//! `env.caret_rect(key, byte)`) and window requests
//! (`env.set_window_size(window, w, h)`, `env.focus_window(window)`); the
//! root table may set `window_title` and `always_on_top`. Because the IR is data all the way down, the binding is
//! just table-to-node conversion — no closures cross the boundary.
//!
//! ## The two `focus` names
//!
//! `env.focused` and `env.focus` are one letter apart and are *not* the
//! same fact, so neither is going away:
//!
//! - `env.focused` (bool) is the **window**'s keyboard focus — whether this
//!   window has the keyboard at all. It is `Env::focused`, and every
//!   binding carries it; Node reads it off its own `env` object.
//! - `env.focus` (integer, nil for none) is the focused **node**'s key.
//!   Node spells this `ctx.focused()` and C `kui_focused`.
//!
//! Lua puts host facts and runtime queries on one table where Node has two
//! objects (`env` and `ctx`), so the name `focused` was already spent on
//! the window fact and the node reading could not have it. That also
//! settles the verbs: `env.focus` is a value, so moving focus is
//! `env.set_focus(key)` rather than the `focus(key)` of the other
//! bindings. `env.blur()`, `env.focus_next()` and `env.focus_prev()` need
//! no such dodge and keep the Node and C spellings.
//!
//! `env.set_focus`, `env.is_focused` and `env.reveal` take the node either
//! way it can be spelled: the integer key an event carried, or the string
//! its `key` field declared — `env.set_focus("note")` — resolved through
//! the frame being built so far and then the last finished one
//! (`Ui::key_of`), so a node no event has come from can be named. Two
//! nodes on one label under different parents resolve to the first in tree
//! order with an `ambiguous-key` warning; a label no node declared is an
//! error naming both spellings.
//!
//! `set_focus` and `blur` take effect at once; `focus_next` / `focus_prev`
//! resolve when the frame finishes, because the Tab ring is made of a
//! finished tree and `view` is still declaring one (`Ui::focus_next`).
//! Either way `env.focus` is a value the host wrote before `view` ran, so
//! it still reads the focus the frame opened with — `env.is_focused(key)`
//! is the query that answers about now.
//!
//! Props come from the shared schema (`kui_core::schema`): every row is
//! reachable from Lua under its snake_case name (`min_width`, `on_click`,
//! `line_height`, ...), so Lua and the Node binding accept the same surface
//! by construction. Only the composites keep Lua-flavored shapes:
//! `pad = 8 | {all,x,y,l,r,t,b}`, `border = {w, color}`, `scroll`/
//! `scroll_x`/`scroll_y`/`clip` booleans, `float = "below" | {anchor, at,
//! self, dx, dy, fit}` (`self_at` still answers to `self`), sizing
//! `{pct = 50} | {grow = 2}`, and `tooltip = "hint"` on a container
//! (hover-gated). Those are *shapes*, not rules: what a name falls back
//! to, what a preset attaches to and what a hint implies are decided in
//! `kui_core::spec`, which this module hands its extracted scalars to.
//!
//! ## A script may host a plugin of its own
//!
//! `env.add_extension(namespace, path)` loads a C extension — a `.so` /
//! `.dylib` / `.dll` exporting the `kui_ext_*` entry points
//! `crates/kui-ffi/include/kui.h` describes, the same plugin a Rust, C or
//! Node host loads — and `fill { name = "ns/slot", params = ... }` is the
//! position it draws in, among the script's own children. The script is
//! a host to it exactly as its host is a host to the script: it declares
//! where, it passes params every frame, and the plugin's replies come
//! back to `on_event` with `from` naming the namespace
//! (`kui_core::slot`). It is C libraries and only that; a script does not
//! load another script, because a host that wants two scripts loads two.
//!
//! Events arrive as their payload table plus `node_key` (the emitting
//! node's key as an integer), which is what `env.edit_text` takes.

use kui_core::schema::{self, Kind, Parsed, PropsOut};
use kui_core::{
    Align, Color, Content, EditOptions, Extension, FloatConfig, Key, PadShorthand, Sizing, Slot,
    Span, Ui, UiEvent, Value, WindowConfig, widgets,
};
use mlua::{Lua, Table};

const PRELUDE: &str = include_str!("prelude.lua");

pub struct LuaExtension {
    lua: Lua,
    name: String,
    /// The script's `slots` global, read once at load: the slot names it
    /// fills (ADR 0014 decision 2). Empty — no global — means `"root"`.
    slots: Vec<String>,
    /// The C extensions this script loaded (`env.add_extension`), in the
    /// order it asked for them. A `RefCell` because the loading happens
    /// inside `view`, where the script's own interpreter holds a shared
    /// borrow of everything else here.
    loaded: std::cell::RefCell<Vec<Loaded>>,
    /// The `tokens = { colors = …, lengths = … }` global the script
    /// declared at load (ADR 0027), declared into the core under this
    /// extension's origin on the first `view` that finds none there;
    /// `env.set_tokens` replaces it from inside a view.
    tokens: Option<kui_core::Tokens>,
}

/// One plugin a script loaded: the namespace it chose, where it came
/// from, and the origin the frame's list gave it.
struct Loaded {
    namespace: String,
    path: std::path::PathBuf,
    origin: kui_core::OriginId,
}

impl LuaExtension {
    pub fn from_source(name: impl Into<String>, source: &str) -> mlua::Result<Self> {
        let lua = Lua::new();
        lua.load(PRELUDE).set_name("kui:prelude").exec()?;
        let name = name.into();
        lua.load(source).set_name(&name).exec()?;
        let slots = match lua.globals().get::<mlua::Value>("slots")? {
            mlua::Value::Nil => Vec::new(),
            mlua::Value::Table(t) => t.sequence_values::<String>().collect::<mlua::Result<_>>()?,
            other => {
                return Err(mlua::Error::runtime(format!(
                    "`slots` must be a list of slot names, not {}",
                    other.type_name()
                )));
            }
        };
        let tokens = match lua.globals().get::<mlua::Value>("tokens")? {
            mlua::Value::Nil => None,
            mlua::Value::Table(t) => Some(parse_tokens(&t)?),
            other => {
                return Err(mlua::Error::runtime(format!(
                    "`tokens` must be a table {{ colors = …, lengths = … }}, not {}",
                    other.type_name()
                )));
            }
        };
        Ok(Self {
            lua,
            name,
            slots,
            loaded: Default::default(),
            tokens,
        })
    }

    /// The namespace a reply's origin names, for a plugin this script
    /// loaded: what `on_event` puts on the event as `from`.
    fn loaded_as(&self, origin: kui_core::OriginId) -> Option<String> {
        self.loaded
            .borrow()
            .iter()
            .find(|l| l.origin == origin)
            .map(|l| l.namespace.clone())
    }

    /// The script's own interpreter. A host reaches for this to seed a
    /// global the script reads — the escape hatch for facts that are not
    /// `env` and not events, which is what the conformance corpus needs to
    /// tell a script which phase of a scene to build.
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    pub fn from_file(path: impl AsRef<std::path::Path>) -> mlua::Result<Self> {
        let path = path.as_ref();
        let source = std::fs::read_to_string(path).map_err(mlua::Error::external)?;
        let name = path
            .file_name()
            .map_or_else(|| "lua".into(), |n| n.to_string_lossy().into_owned());
        Self::from_source(name, &source)
    }
}

impl Extension for LuaExtension {
    fn name(&self) -> &str {
        &self.name
    }

    fn slots(&self) -> &[String] {
        &self.slots
    }

    fn view(&mut self, slot: &Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        let view: mlua::Function = self
            .lua
            .globals()
            .get("view")
            .map_err(|_| "script defines no view()".to_string())?;
        // Which slot this is, as `view`'s second argument rather than a
        // field of `env`: `env`'s value keys are pinned to
        // `schema::ENV_FIELDS` across every binding, and a slot is the
        // host's fact, not the driver's. A script written as `view(env)`
        // never sees it.
        let slot_table = slot_table(&self.lua, slot).map_err(|e| format!("slot: {e}"))?;
        // The table the script was loaded with, declared once per core
        // under this origin — a script that declares from `view` through
        // `env.set_tokens` has already, and is not overwritten.
        let origin = ui.core().origin();
        if let Some(t) = &self.tokens
            && !ui.core().tokens_declared(origin)
        {
            ui.set_tokens(t.clone());
        }
        // The env's query functions borrow the frame for the duration of
        // view(); the returned table outlives the scope, the borrow does
        // not. A RefCell because measurement shapes text (a mutable query)
        // while the rest only read; Lua calls them one at a time.
        let root: Table = {
            let frame = std::cell::RefCell::new(&mut *ui);
            self.lua
                .scope(|scope| {
                    let env = env_table(&self.lua, scope, &frame, &self.loaded)?;
                    view.call((env, slot_table))
                })
                .map_err(|e| format!("view(): {e}"))?
        };
        // The root table may declare host state alongside the tree.
        if let Ok(Some(title)) = root.get::<Option<String>>("window_title") {
            ui.window_title(&title);
        }
        if let Ok(Some(true)) = root.get::<Option<bool>>("always_on_top") {
            ui.always_on_top(true);
        }
        declare_windows(ui, &root).map_err(|e| format!("windows: {e}"))?;
        build_node(ui, &root).map_err(|e| format!("view table: {e}"))
    }

    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
        let Ok(f) = self.lua.globals().get::<mlua::Function>("on_event") else {
            return Vec::new();
        };
        let payload = value_to_lua(&self.lua, &ev.payload).and_then(|p| {
            // Map payloads learn which node emitted them; edit widgets emit
            // {kind="changed"|"submit"} and scripts read the text back with
            // env.edit_text(ev.node_key).
            if let mlua::Value::Table(t) = &p
                && !t.contains_key("node_key")?
            {
                t.set("node_key", ev.key.0 as i64)?;
                // And, when this is a reply from a plugin the script
                // loaded, who is answering: the namespace it chose in
                // `env.add_extension`. Absent for the script's own nodes,
                // which is what tells the two apart. `node_key` on a reply
                // is the key of the node *inside the plugin* whose event it
                // answers, which is the plugin's business and not this
                // script's — `from` is the useful half.
                if let Some(ns) = self.loaded_as(ev.origin) {
                    t.set("from", ns)?;
                }
            }
            Ok(p)
        });
        // What `on_event` returns is the script's replies to the host (ADR
        // 0014 decision 6): nothing, a table (one reply), or a sequence of
        // tables (several) — the list-or-map reading `lua_to_value` already
        // makes.
        match payload.and_then(|p| f.call::<mlua::Value>(p)) {
            Ok(mlua::Value::Nil) => Vec::new(),
            Ok(v) => match lua_to_value(&v) {
                Ok(Value::List(replies)) => replies,
                Ok(reply) => vec![reply],
                Err(e) => {
                    eprintln!("kui-lua: '{}' on_event reply: {e}", self.name);
                    Vec::new()
                }
            },
            Err(e) => {
                eprintln!("kui-lua: '{}' on_event error: {e}", self.name);
                Vec::new()
            }
        }
    }
}

/// `view`'s second argument: `{ name = ..., namespace = ..., params = ...,
/// key = ... }` — the slot in the script's own vocabulary, the namespace
/// the host loaded the script under (what tells one instance from
/// another), `params` absent for a slot declared without any
/// (`Value::Null`), and the slot's key as the integer the events and
/// `env.set_focus` use.
fn slot_table(lua: &Lua, slot: &Slot<'_>) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("name", slot.name)?;
    t.set("namespace", slot.namespace)?;
    t.set("key", slot.key.0 as i64)?;
    if !matches!(slot.params, Value::Null) {
        t.set("params", value_to_lua(lua, slot.params)?)?;
    }
    Ok(t)
}

/// A node named either way a script can: the integer key an event carried,
/// or the string label its `key` field declared, resolved through the
/// frame so far and then the last finished one (`Ui::key_of`). A string no
/// node declared is an error naming both spellings, since nothing else
/// would (backlog F5) — see [`key_query`] for the calls that answer instead.
fn key_arg(ui: &mut Ui<'_>, v: mlua::Value) -> mlua::Result<Key> {
    match v {
        mlua::Value::Integer(i) => Ok(Key(i as u64)),
        mlua::Value::String(s) => {
            let label = s.to_str()?;
            ui.key_of(&label).ok_or_else(|| {
                mlua::Error::runtime(format!(
                    "no node is keyed {:?}: pass the label a `key` field declared in this or the last \
                     frame, or the integer key an event carried",
                    &*label
                ))
            })
        }
        other => Err(mlua::Error::runtime(format!(
            "a node key is an integer or a declared label, not {}",
            other.type_name()
        ))),
    }
}

/// The two spellings for a *query* — `is_hovered`, `scroll_geometry`,
/// `edit_text` and the rest — where a name nothing declared is the answer
/// rather than an error. Every one of them already has a "no such node"
/// reply for a key no layout resolved (false, nil, a zero offset), and a
/// label is the spelling a view uses *before* the node exists: the first
/// frame of a `virtual_column` asks its own container for geometry that is
/// not there yet. The command verbs ([`key_arg`]) keep throwing, where a
/// typo is a bug worth naming (backlog C25). What a key may *be* is the
/// same question for both, so anything that is not an integer or a string
/// is refused here too.
fn key_query(ui: &mut Ui<'_>, v: mlua::Value) -> mlua::Result<Option<Key>> {
    match v {
        mlua::Value::Integer(i) => Ok(Some(Key(i as u64))),
        mlua::Value::String(s) => Ok(ui.key_of(&s.to_str()?)),
        other => Err(mlua::Error::runtime(format!(
            "a node key is an integer or a declared label, not {}",
            other.type_name()
        ))),
    }
}

/// A Lua sequence as a plain-data list — `lua_to_value` reads an empty
/// table as an empty map, and a list of rows is a list even when empty.
fn lua_list_to_value(t: &Table) -> mlua::Result<Value> {
    let mut items = Vec::with_capacity(t.raw_len());
    for item in t.sequence_values::<mlua::Value>() {
        items.push(lua_to_value(&item?)?);
    }
    Ok(Value::List(items))
}

/// The snake spellings a script may have learned first — `select_all`,
/// `look_up` — kept as aliases of the wire names in a row's `role`, the
/// way `direction` is one for `repeat` (ADR 0020, decision 10). The rest
/// of a row is the core's call (`MenuItem::from_value`).
fn alias_menu_role(row: &mut Value) {
    let Value::Map(fields) = row else { return };
    for (k, v) in fields.iter_mut() {
        if k == "role"
            && let Value::Str(name) = v
        {
            match name.as_str() {
                "select_all" => *name = kui_core::MenuRole::SelectAll.name().to_string(),
                "look_up" => *name = kui_core::MenuRole::LookUp.name().to_string(),
                _ => {}
            }
        }
    }
}

/// A menu's rows, from a Lua list of row tables, read by the core's one
/// row reader.
fn menu_items(t: &mlua::Table) -> mlua::Result<Vec<kui_core::MenuItem>> {
    let mut rows = lua_list_to_value(t)?;
    if let Value::List(rows) = &mut rows {
        rows.iter_mut().for_each(alias_menu_role);
    }
    kui_core::MenuItem::list_from_value(&rows).map_err(mlua::Error::runtime)
}

/// Host facts handed to `view(env)`, the reading `schema::ENV_FIELDS`
/// documents and `the_env_table_is_the_documented_env_shape` pins to it key
/// for key: `refresh_hz` (nil if unknown),
/// `frame_budget_ms`, `focused` (the *window*'s keyboard focus, a bool),
/// `system` (what the user set in the OS: `appearance`, `motion` and
/// `assistive` as strings, always there because "unknown" is one of their
/// readings, and `accent` (0xRRGGBBAA) / `locale` (a BCP-47 tag) only when
/// the host can tell), `focus` (the focused *node*'s key), `focus_visible`, `region`
/// (the `focus_region` node in effect, nil for the main ring), `theme`
/// (the palette derived from `system`: one 0xRRGGBBAA number per role in
/// `schema::THEME_ROLES`, plus `appearance` and `disabled_opacity`),
/// `viewport_w`/`viewport_h` (logical px), `window` chrome facts, the
/// queries `edit_text(key)`, `is_focused(key)`, `is_hovered(key)`,
/// `is_pressed(key)`, `scroll_offset(key)`, `scroll_geometry(key)`,
/// `text_hit(key, x, y)` and `caret_rect(key, byte)` (each takes either
/// spelling — the integer key an event carried or the label a `key` field
/// declared — and answers nil/false/zero for a name no frame declared, see
/// `key_query`; the verbs take the same two and refuse an undeclared name,
/// see `key_arg`), the editor verb `set_edit_text(key_or_label, text)` (whose
/// label spelling reaches an editor this view is about to declare),
/// `measure_text(s, opts, max_w)` (see `measure_from_lua`), the
/// focus verbs `set_focus(key)` / `blur()` / `focus_next()` / `focus_prev()` / `focus_region(key)`
/// and the scroll calls `reveal(key)` / `scroll_offset(key)` / `set_scroll(key, x, y)` /
/// `scroll_geometry(key)`, the text queries `text_hit(key, x, y)` /
/// `caret_rect(key, byte)`, the selection calls `selection_text()` /
/// `selection_html()` (the same words with the formatting they declared) /
/// `request_copy()` + `answer_selection_range(text)` (a copy that reaches
/// rows a virtual list never built is asked of the app) /
/// `select_all_in(key)` / `clear_selection()` (ADR 0017 — one selection
/// per window, a `selectable` scope's or the focused editor's), the menu
/// verbs `open_menu(key, x, y, items)` / `close_menu()` (whose chosen row
/// comes back as a `menu` event on that node), the
/// window requests `set_window_size(window,
/// w, h)` / `focus_window(window)`, and the two calls of a script that
/// hosts a plugin of its own: `add_extension(namespace, path)` and
/// `extension_namespaces()`.
fn env_table<'scope, 'env: 'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, 'env>,
    ui: &'env std::cell::RefCell<&'env mut Ui<'_>>,
    loaded: &'env std::cell::RefCell<Vec<Loaded>>,
) -> mlua::Result<Table> {
    let (facts, theme, metrics) = {
        let ui = ui.borrow();
        (ui.env_facts(), ui.theme(), ui.metrics())
    };
    let t = lua.create_table()?;
    // The tokens a script here sees this frame (ADR 0027): its own table
    // over the host's, colours resolved for the appearance as
    // `0xRRGGBBAA`, lengths in px — `env.tokens.colors.peach`. Roles are
    // not listed; they are `env.theme` and `env.metrics`. Rebuilt by
    // `env.set_tokens`, so a script reads back what it just declared.
    t.set("tokens", tokens_table(lua, &ui.borrow())?)?;
    // The facts, one row of `schema::ENV_FIELDS` at a time, read by the
    // row's own getter and filed under its snake path (`system.appearance`
    // is `env.system.appearance`). Lua's rule for a fact the host cannot
    // tell is `refresh_hz`'s: no key rather than a nil-shaped one, so a
    // `Null` reading is left out. Two facts, one letter apart, are both
    // here: `focused` is the *window*'s keyboard, `focus` the focused
    // *node*'s key — Lua cannot converge on Node's `focused()` for the
    // latter because the former has been `focused` since env existed.
    //
    // The one deliberate divergence the table names: `window.native_controls`
    // is a rect elsewhere and two numbers here, the keep-out extent of the
    // OS-drawn controls (macOS traffic lights) at the window origin.
    for row in kui_core::schema::ENV_FIELDS {
        let value = (row.get)(&facts);
        if value == Value::Null {
            continue;
        }
        if row.name == "window.native_controls" {
            let win = t
                .get::<Option<Table>>("window")?
                .unwrap_or(lua.create_table()?);
            let f = |k: &str| value.get(k).and_then(Value::as_float).unwrap_or(0.0) as f32;
            win.set("controls_w", f("x") + f("w"))?;
            win.set("controls_h", f("y") + f("h"))?;
            t.set("window", win)?;
            continue;
        }
        for path in row.lua {
            let lua_value = value_to_lua(lua, &value)?;
            match path.split_once('.') {
                None => t.set(*path, lua_value)?,
                Some((head, leaf)) => {
                    let sub = t.get::<Option<Table>>(head)?.unwrap_or(lua.create_table()?);
                    sub.set(leaf, lua_value)?;
                    t.set(head, sub)?;
                }
            }
        }
    }
    // The palette the core derived from `system`, as roles rather than
    // values (ADR 0019). Every key is a 0xRRGGBBAA number — the same
    // spelling a `color` prop takes — so `bg = env.theme.surface` needs no
    // conversion; `appearance` says which base it came from and
    // `disabled_opacity` is a multiplier, not a colour. The roles are
    // generated from `schema::THEME_ROLES`, so a script and a Rust view
    // read the same list under the same names. Read-only: a Lua script is
    // a guest in someone else's frame, and the theme is the host's to set.
    let th = lua.create_table()?;
    for role in kui_core::schema::THEME_ROLES {
        th.set(role.name, (role.get)(&theme).to_hex())?;
    }
    th.set("appearance", theme.appearance.name())?;
    th.set("disabled_opacity", theme.disabled_opacity)?;
    t.set("theme", th)?;
    // The sizes the stock widgets are built from (backlog T2), generated
    // from `schema::METRIC_ROLES` the same way: `radius = env.metrics.radius`
    // makes a script's control agree with the host's button. Read-only for
    // the same reason the theme is.
    let mt = lua.create_table()?;
    for role in kui_core::schema::METRIC_ROLES {
        mt.set(role.name, (role.get)(&metrics))?;
    }
    t.set("metrics", mt)?;
    t.set(
        "edit_text",
        scope.create_function(move |_, key: i64| Ok(ui.borrow().edit_text(Key(key as u64))))?,
    )?;
    // Replaces an editor's text, caret at the end. Named by the label its
    // `key` field declares as well as by the integer key, and the label is
    // the spelling an `update` that opens the field can use: the key comes
    // from an event the editor has not fired yet (backlog F32). A label no
    // frame has declared is held for the next frame that declares it —
    // seeding a new editor over `initial`, replacing a retained one's draft
    // — and dropped with an `edit-text-without-editor` warning if that
    // frame declares nothing under it.
    t.set(
        "set_edit_text",
        scope.create_function(move |_, (key, text): (mlua::Value, String)| {
            let mut ui = ui.borrow_mut();
            match key {
                mlua::Value::String(label) => {
                    ui.set_edit_text_by_label(&label.to_str()?, &text);
                }
                other => {
                    let key = key_arg(&mut ui, other)?;
                    ui.set_edit_text(key, &text);
                }
            }
            Ok(())
        })?,
    )?;
    // Takes a label too (`key_arg`): a view styles the row it declares by
    // the name it gives it, without an event having told it the key.
    t.set(
        "is_focused",
        scope.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut();
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(false);
            };
            Ok(ui.is_focused(key))
        })?,
    )?;
    t.set(
        "is_hovered",
        scope.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut();
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(false);
            };
            Ok(ui.is_hovered(key))
        })?,
    )?;
    // Held down: the press started on this node and the pointer is still
    // over it (or it captured a drag). Goes with `is_hovered` — a script
    // that draws its own button styles the pressed state from this.
    t.set(
        "is_pressed",
        scope.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut();
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(false);
            };
            Ok(ui.is_pressed(key))
        })?,
    )?;
    // Moving focus from the script, the imperative half of `key_focus`.
    // `set_focus`, not `focus`: `env.focus` is already the reading above
    // and alpha.5 shipped it, so the verb takes the longer name rather
    // than change what a name means under a script that already runs.
    // `blur` / `focus_next` / `focus_prev` match Node and C exactly.
    // The key is an integer or a declared label (`key_arg`): "focus the
    // editor I just created" is `env.set_focus("editor")`, with no event
    // from it needed first.
    t.set(
        "set_focus",
        scope.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut();
            let key = key_arg(&mut ui, key)?;
            ui.focus(key);
            Ok(())
        })?,
    )?;
    t.set(
        "blur",
        scope.create_function(move |_, ()| {
            ui.borrow_mut().blur();
            Ok(())
        })?,
    )?;
    // What Tab and Shift-Tab do: the next / previous focusable node in
    // tree order, wrapping. A script that binds Tab in an `on_key` sink
    // calls these to hand the keyboard on. Like `reveal`, the step
    // resolves when *this* frame finishes — the ring is made of a
    // finished tree, and the script is still declaring one — so a view
    // can step onto a row it is declaring right now.
    t.set(
        "focus_next",
        scope.create_function(move |_, ()| {
            ui.borrow_mut().focus_next();
            Ok(())
        })?,
    )?;
    t.set(
        "focus_prev",
        scope.create_function(move |_, ()| {
            ui.borrow_mut().focus_prev();
            Ok(())
        })?,
    )?;
    // Enters a focus region — `env.focus_region("dock")`, the label or the
    // integer key of a node declared `focus_region = true` — or the main
    // ring for nil (`docs/adr/0022-focus-regions.md`). Resolves when this
    // frame finishes, like `focus_next`, so a script may name the region
    // it is declaring right now — call it after the region's node.
    t.set(
        "focus_region",
        scope.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut();
            let key = match key {
                mlua::Value::Nil => None,
                v => Some(key_arg(&mut ui, v)?),
            };
            ui.focus_region(key);
            Ok(())
        })?,
    )?;
    // `env.announce(text, politeness)` says something once, with no node
    // behind it (`docs/adr/0008-live-regions-and-announcements.md`).
    // `politeness` is "polite" (the default) or "assertive"; "off" and an
    // empty text are no-ops. A region whose message is on screen is the
    // `live` prop instead.
    //
    // `env` exists only inside `view`, and a view runs every frame, so a
    // call here needs a guard the script clears — `on_event` sets a field,
    // `view` announces it and clears it. The core reports the unguarded
    // case as `announcement-repeated`.
    t.set(
        "announce",
        scope.create_function(move |_, (text, live): (String, Option<String>)| {
            let live = live.unwrap_or_else(|| "polite".to_string());
            let Some(i) = schema::LIVE.iter().position(|v| *v == live) else {
                return Err(mlua::Error::runtime(format!(
                    "bad politeness {live:?} (one of {})",
                    schema::LIVE.join(" | ")
                )));
            };
            ui.borrow_mut()
                .announce(&text, kui_core::Live::from_index(i));
            Ok(())
        })?,
    )?;
    // Scrolling from the script: `env.reveal(key)` scrolls whatever
    // contains a node so it shows, and `env.scroll_offset` /
    // `env.set_scroll` read and write a container's retained offset.
    // `view` runs while the frame is being built, so a reveal resolves
    // against *this* frame's layout when it finishes — which is what lets
    // a script reveal a row it is declaring right now. A key the frame
    // does not declare, or one with nothing scrollable above it, is a
    // no-op; the request is not kept for a later frame.
    t.set(
        "reveal",
        scope.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut();
            let key = key_arg(&mut ui, key)?;
            ui.reveal(key);
            Ok(())
        })?,
    )?;
    // `{x, y}` as the last layout clamped it (positive = content moved up
    // / left); zeroes for a node that never scrolled. Stash it and hand it
    // back to `set_scroll` to restore a position.
    t.set(
        "scroll_offset",
        scope.create_function(move |lua, key: mlua::Value| {
            let mut ui = ui.borrow_mut();
            let off = match key_query(&mut ui, key)? {
                Some(key) => ui.scroll_offset(key),
                None => kui_core::Vec2::ZERO,
            };
            let r = lua.create_table()?;
            r.set("x", off.x)?;
            r.set("y", off.y)?;
            Ok(r)
        })?,
    )?;
    // Everything the last layout resolved for a container: its box
    // `{x, y, w, h}`, its content `{content_w, content_h}` and the clamped
    // `{offset = {x, y}}`; nil for a key no layout has resolved as one.
    // This is what lets a script draw a long list affordably — the core
    // builds every child the view declares, so a script that knows `h` and
    // `offset.y` declares the rows that fit plus two spacers holding the
    // space of the rest. It describes the frame before this one, so a
    // resize slices one frame late; declare a row or two extra at each end.
    // Where a point (the `x`, `y` a click or drag event carried) lands in
    // the text a keyed node drew: `{ byte, line }` — the byte offset into
    // that text, across the node's runs in order, and the visual line — or
    // nil for a key that drew no text. Answered from the frame that
    // finished, which is the layout the pointer was over.
    t.set(
        "text_hit",
        scope.create_function(move |lua, (key, x, y): (mlua::Value, f32, f32)| {
            let mut ui = ui.borrow_mut();
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(mlua::Value::Nil);
            };
            let Some(h) = ui.text_hit(key, kui_core::Vec2::new(x, y)) else {
                return Ok(mlua::Value::Nil);
            };
            value_to_lua(lua, &h.to_value())
        })?,
    )?;
    // Opens a context menu at (x, y) over a keyed node, its items a list
    // of tables: `{ label=, role=, enabled=, checked=, id=, accel= }`, all
    // but `label` optional, `role` one of "custom" (the default),
    // "separator", "cut", "copy", "paste", "selectAll", "lookUp" — the
    // spelling the `menu` event reports back ("select_all" / "look_up"
    // are taken too). Choosing a row posts `{kind="menu", role, item}` on
    // the node and closes the menu (docs/adr/0017-selection-as-a-scope.md).
    t.set(
        "open_menu",
        scope.create_function(
            move |_, (key, x, y, items): (mlua::Value, f32, f32, mlua::Table)| {
                let mut ui = ui.borrow_mut();
                let Some(target) = key_query(&mut ui, key)? else {
                    return Ok(false);
                };
                let items = menu_items(&items)?;
                ui.open_menu(kui_core::Menu::new(
                    target,
                    kui_core::Vec2::new(x, y),
                    items,
                ));
                Ok(true)
            },
        )?,
    )?;
    // Closes whatever menu is open; true when there was one.
    t.set(
        "close_menu",
        scope.create_function(move |_, ()| {
            let mut ui = ui.borrow_mut();
            Ok(ui.close_menu())
        })?,
    )?;
    // The window's selected text: what a `selectable` scope holds, or the
    // focused `edit`'s selection — one per window, so there is no choice
    // to make. Nil with no selection, `""` when one exists and covers
    // nothing (docs/adr/0017-selection-as-a-scope.md).
    t.set(
        "selection_text",
        scope.create_function(move |_, ()| {
            let ui = ui.borrow();
            Ok(ui.selection_text())
        })?,
    )?;
    // Asks for the selection as text: returns the text, or nil and true
    // when the app was asked instead — a selection that reaches rows a
    // virtual list never built posts `{kind="selectionrange", from={index,
    // byte}, to={index, byte}}` on the scope, and the app answers with
    // `answer_selection_range` (docs/adr/0017-selection-as-a-scope.md).
    t.set(
        "request_copy",
        scope.create_function(move |_, ()| {
            let mut ui = ui.borrow_mut();
            Ok(match ui.request_copy() {
                kui_core::CopyRequest::Ready(text) => (Some(text), false),
                kui_core::CopyRequest::Asked => (None, true),
                kui_core::CopyRequest::Nothing => (None, false),
            })
        })?,
    )?;
    // Answers a `selectionrange` ask with the text for the range it named,
    // whole. False when nothing asked.
    t.set(
        "answer_selection_range",
        scope.create_function(move |_, text: String| {
            let mut ui = ui.borrow_mut();
            Ok(ui.answer_selection_range(&text))
        })?,
    )?;
    // The selection as HTML: the formatting the text declared (bold,
    // italic, a span's own colour) and not the node's colour, which is
    // the theme's. Nil with no text selection. A second clipboard flavour
    // beside the plain text, never instead of it.
    t.set(
        "selection_html",
        scope.create_function(move |_, ()| {
            let ui = ui.borrow();
            Ok(ui.selection_html())
        })?,
    )?;
    // Selects every run inside the scope a keyed node declared, first
    // byte to last — Select All, scoped. False for a node that drew no
    // text or is not a scope. Runs the frame built but never drew are
    // part of it.
    t.set(
        "select_all_in",
        scope.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut();
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(false);
            };
            Ok(ui.select_all_in(key))
        })?,
    )?;
    // Drops the window's selection, whichever it is; true when there was
    // one to drop.
    t.set(
        "clear_selection",
        scope.create_function(move |_, ()| {
            let mut ui = ui.borrow_mut();
            Ok(ui.clear_selection())
        })?,
    )?;
    // The caret rect for a byte offset in that text: `{ x, y, w, h }`,
    // logical viewport px, zero wide, one line tall; nil for a key that
    // drew no text. A byte past the text is the end.
    t.set(
        "caret_rect",
        scope.create_function(move |lua, (key, byte): (mlua::Value, usize)| {
            let mut ui = ui.borrow_mut();
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(mlua::Value::Nil);
            };
            let Some(r) = ui.caret_rect(key, byte) else {
                return Ok(mlua::Value::Nil);
            };
            value_to_lua(lua, &r.to_value())
        })?,
    )?;
    t.set(
        "scroll_geometry",
        scope.create_function(move |lua, key: mlua::Value| {
            let mut ui = ui.borrow_mut();
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(mlua::Value::Nil);
            };
            let Some(g) = ui.scroll_geometry(key) else {
                return Ok(mlua::Value::Nil);
            };
            // The core's shape, whose keys are already Lua's spelling.
            value_to_lua(lua, &g.to_value())
        })?,
    )?;
    // The rect the last frame laid an `on_layout` node out at — the
    // `layout` event's numbers without the event (backlog C26 step 2).
    t.set(
        "layout_of",
        scope.create_function(move |lua, key: mlua::Value| {
            let mut ui = ui.borrow_mut();
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(mlua::Value::Nil);
            };
            let Some(r) = ui.layout_of(key) else {
                return Ok(mlua::Value::Nil);
            };
            value_to_lua(lua, &r.to_value())
        })?,
    )?;
    // The wheel's move by hand; the next layout clamps it, so 0,0 is "jump
    // to the top" and a huge y is "jump to the end".
    t.set(
        "set_scroll",
        scope.create_function(move |_, (key, x, y): (mlua::Value, f32, f32)| {
            let mut ui = ui.borrow_mut();
            let key = key_arg(&mut ui, key)?;
            ui.set_scroll(key, kui_core::Vec2::new(x, y));
            Ok(())
        })?,
    )?;
    // Window requests from the script: `env.set_window_size(window, w, h)`
    // and `env.focus_window(window)` queue commands the driver applies on
    // its next pump — after this frame, since `view` runs inside one — and
    // a headless driver never drains. Requests, not declarations: the user
    // owns a window's size once it exists (ADR 0004 decision 5).
    // `env.window.id` is the window the script is drawing, and the only
    // one there is until step 3.
    t.set(
        "set_window_size",
        scope.create_function(move |_, (window, w, h): (i64, f32, f32)| {
            ui.borrow_mut()
                .set_window_size(kui_core::WindowId(window as u32), kui_core::Size::new(w, h));
            Ok(())
        })?,
    )?;
    // Declare this script's tokens from inside a view (ADR 0027): the
    // same shape as the `tokens` global, replacing this origin's table
    // whole, in effect for the nodes the same view opens after the call.
    // A script whose lengths follow a tier declares on each change.
    let env = t.clone();
    t.set(
        "set_tokens",
        scope.create_function(move |lua, decl: Table| {
            let tokens = parse_tokens(&decl)?;
            ui.borrow_mut().set_tokens(tokens);
            env.set("tokens", tokens_table(lua, &ui.borrow())?)
        })?,
    )?;
    t.set(
        "focus_window",
        scope.create_function(move |_, window: i64| {
            ui.borrow_mut()
                .focus_window(kui_core::WindowId(window as u32));
            Ok(())
        })?,
    )?;
    t.set(
        "measure_text",
        scope.create_function(
            move |lua, (s, opts, max_w): (mlua::Value, Option<Table>, Option<f32>)| {
                let mut guard = ui.borrow_mut();
                let m = measure_from_lua(&mut guard, &s, opts.as_ref(), max_w)?;
                value_to_lua(lua, &m.to_value())
            },
        )?,
    )?;
    // `env.add_extension(namespace, path)` → true, or nil and a message:
    // the script hosting an extension of its own (ADR 0014, and
    // `kui_core::slot`). The mechanism is C shared libraries and only
    // that — a `.so` / `.dylib` / `.dll` exporting the seven `kui_ext_*`
    // entry points `crates/kui-ffi/include/kui.h` describes — which is
    // the same plugin a Rust, C or Node host loads, and is why a script
    // can place one at all: the contract between a host and an extension
    // is C, so the script is just another host.
    //
    // Called from `view`, because that is where a script knows what it
    // wants, and idempotent by (namespace, path) so the honest spelling
    // is to call it every frame. The namespace joins the frame's *one*
    // map, so it can collide with the host's; a taken one is the error.
    // Nothing here is a new capability: `Lua::new` has `package`, so a
    // script could already `package.loadlib` anything on the disk. What
    // this adds is a plugin that draws in the script's own tree.
    t.set(
        "add_extension",
        scope.create_function(move |_, (namespace, path): (String, String)| {
            let path = std::path::PathBuf::from(path);
            if let Some(prev) = loaded.borrow().iter().find(|l| l.namespace == namespace) {
                return Ok(if prev.path == path {
                    // The every-frame call, already answered.
                    (Some(true), None)
                } else {
                    (
                        None,
                        Some(format!(
                            "`{namespace}` is already {} in this script; a second plugin needs a \
                             second namespace",
                            prev.path.display()
                        )),
                    )
                });
            }
            // SAFETY: no more so than the host loading it would be. The
            // library's code runs in this process on this frame; a script
            // naming one is trusting it as the host trusts the script.
            let ext = match unsafe { kui_ffi::CExtension::open(&path) } {
                Ok(ext) => ext,
                Err(e) => return Ok((None, Some(e))),
            };
            let origin = match ui.borrow_mut().add_extension(&namespace, Box::new(ext)) {
                Ok(origin) => origin,
                Err(e) => return Ok((None, Some(e))),
            };
            loaded.borrow_mut().push(Loaded {
                namespace,
                path,
                origin,
            });
            Ok((Some(true), None))
        })?,
    )?;
    // The namespaces this script loaded, in the order it asked for them —
    // what it can name in a `fill`, and what a reply's `from` will say.
    t.set(
        "extension_namespaces",
        scope.create_function(move |lua, ()| {
            lua.create_sequence_from(loaded.borrow().iter().map(|l| l.namespace.clone()))
        })?,
    )?;
    Ok(t)
}

/// `env.measure_text(s, opts, max_w)` → `{ width, height, lines }` (logical
/// px): what layout would give a text node with that content and style,
/// wrapped to `max_w` when given. `s` is a string, a span list (the same
/// shape `text({...})` takes) or a whole `text(...)` node table, whose own
/// props are then the style; `opts` is a style table (`size`, `font`,
/// `wrap`, `max_lines`, `ellipsis`, ...). The metrics do not scale
/// linearly: `measured × zoom` is not `measure(size × zoom)`, because
/// shaping rounds per size, so anything that zooms measures at the size it
/// draws.
/// `env.tokens`: the frame's resolved tokens, own over host, as two
/// tables of name → value.
fn tokens_table(lua: &Lua, ui: &Ui<'_>) -> mlua::Result<Table> {
    let look = ui.tokens();
    let tokens = lua.create_table()?;
    let colors = lua.create_table()?;
    for (name, c) in look.colors() {
        colors.set(name, c.to_hex())?;
    }
    let lengths = lua.create_table()?;
    for (name, v) in look.lengths() {
        lengths.set(name, v)?;
    }
    tokens.set("colors", colors)?;
    tokens.set("lengths", lengths)?;
    Ok(tokens)
}

fn measure_from_lua(
    ui: &mut Ui<'_>,
    s: &mlua::Value,
    opts: Option<&Table>,
    max_w: Option<f32>,
) -> mlua::Result<kui_core::TextMetrics> {
    let style = match opts {
        Some(t) => with_refs(ui, |refs| parse_props(t, false, refs))?.style,
        None => kui_core::TextStyle::default(),
    };
    let measure_spans = |ui: &mut Ui<'_>, spans: &Table, style: &kui_core::TextStyle| {
        let parts = with_refs(ui, |refs| collect_spans(spans, refs))?;
        let spans: Vec<Span<'_>> = parts.iter().map(span_of).collect();
        Ok(ui.measure_rich_text(&spans, style, max_w))
    };
    match s {
        mlua::Value::String(s) => Ok(ui.measure_text(&s.to_str()?, &style, max_w)),
        mlua::Value::Table(t) => {
            if t.get::<Option<String>>("type")?.as_deref() == Some("text") {
                let style = with_refs(ui, |refs| parse_props(t, false, refs))?.style;
                match t.get::<Option<Table>>("spans")? {
                    Some(spans) => measure_spans(ui, &spans, &style),
                    None => {
                        let value: String = t.get("value")?;
                        Ok(ui.measure_text(&value, &style, max_w))
                    }
                }
            } else {
                measure_spans(ui, t, &style)
            }
        }
        other => Err(bad(format!(
            "measure_text: expected a string, a span list or a text node, got {}",
            other.type_name()
        ))),
    }
}

// ---------------------------------------------------------------------------
// Table tree -> IR

fn bad(msg: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::runtime(msg.to_string())
}

fn build_children(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    for child in t.sequence_values::<Table>() {
        build_node(ui, &child?)?;
    }
    Ok(())
}

/// Builds `t`'s children inside a widget's content closure, carrying the
/// first error out (widget closures can't return one).
fn with_children(
    ui: &mut Ui<'_>,
    t: &Table,
    widget: impl FnOnce(&mut Ui<'_>, &mut dyn FnMut(&mut Ui<'_>)),
) -> mlua::Result<()> {
    let mut result = Ok(());
    widget(ui, &mut |ui| result = build_children(ui, t));
    result
}

/// The `ELEMENTS` row a Lua node type is: the two container constructors are
/// one element, `input` is the chrome around an `edit`, and the widget
/// functions spell their names with underscores.
fn element_of(ty: &str) -> &str {
    match ty {
        "row" | "column" => "box",
        "input" => "edit",
        "window_buttons" => "windowButtons",
        "menu_bar" => "menuBar",
        "latency_graph" | "latency_hud" => "latencyGraph",
        other => other,
    }
}

fn menu_bar_of(t: &Table) -> mlua::Result<kui_core::MenuBar> {
    let Some(list) = t.get::<Option<Table>>("menu")? else {
        return Ok(kui_core::MenuBar::default());
    };
    let mut menus = lua_list_to_value(&list)?;
    if let Value::List(menus) = &mut menus {
        for menu in menus.iter_mut() {
            let Value::Map(fields) = menu else { continue };
            for (k, items) in fields.iter_mut() {
                if k != "items" {
                    continue;
                }
                // An empty Lua table read as a map is an empty row list.
                if matches!(items, Value::Map(m) if m.is_empty()) {
                    *items = Value::List(Vec::new());
                }
                if let Value::List(rows) = items {
                    rows.iter_mut().for_each(alias_menu_role);
                }
            }
        }
    }
    kui_core::MenuBar::from_value(&menus).map_err(mlua::Error::runtime)
}

fn declare_windows(ui: &mut Ui<'_>, root: &Table) -> mlua::Result<()> {
    let Some(list) = root.get::<Option<Table>>("windows")? else {
        return Ok(());
    };
    // Plain data with a fixed shape, read by the core (`WindowConfig::
    // from_value`): a name, or `{name, kind?, width?, height?, activates?,
    // anchor?}`.
    for entry in list.sequence_values::<mlua::Value>() {
        let v = lua_to_value(&entry?)?;
        let (name, cfg) = WindowConfig::from_value(&v).map_err(mlua::Error::runtime)?;
        ui.window(&name, cfg);
    }
    Ok(())
}

/// Warns about every key in the node table that no table claims — the
/// binding is about to drop it (see `diag::UNKNOWN_PROP`). Only string keys:
/// children sit at the integer ones.
fn check_props(ui: &mut Ui<'_>, t: &Table, element: &str) -> mlua::Result<()> {
    if !ui.core().diagnostics() {
        return Ok(());
    }
    for pair in t.pairs::<mlua::Value, mlua::Value>() {
        let (k, _) = pair?;
        let mlua::Value::String(k) = k else { continue };
        let name = k.to_str()?;
        // `type` is the prelude's element tag, not a prop.
        if name.as_ref() == "type" || schema::known_prop(element, &name, schema::Spelling::Snake) {
            continue;
        }
        let w = kui_core::diag::unknown_prop(element, &name, schema::Spelling::Snake);
        ui.core().warn(w);
    }
    Ok(())
}

/// `fill { name = "todos/panel", params = {...} }`: a position an
/// extension fills, in place (ADR 0014). Not a node and so not a schema
/// element — it draws nothing itself and takes none of the props a box
/// takes, which is why it is checked here rather than by `check_props`.
/// `name` is the full `namespace/slot`: the namespace this script loaded
/// the plugin under (`env.add_extension`) and the slot in the plugin's own
/// vocabulary. `params` is whatever the plugin should read this frame —
/// plain data, declared every frame, retained by nobody, exactly like an
/// `on_click` payload.
fn build_fill(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    let name: String = t
        .get::<Option<String>>("name")?
        .filter(|n| !n.is_empty())
        .ok_or_else(|| bad("fill needs a name (\"namespace/slot\")"))?;
    if !name.contains(kui_core::NAMESPACE_SEPARATOR) {
        return Err(bad(format!(
            "bad slot name {name:?} (a full \"namespace/slot\")"
        )));
    }
    for pair in t.pairs::<mlua::Value, mlua::Value>() {
        let (k, _) = pair?;
        let mlua::Value::String(k) = k else { continue };
        let k = k.to_str()?;
        if !matches!(k.as_ref(), "type" | "name" | "params") {
            return Err(bad(format!(
                "fill takes name and params, not {:?} — it is a position, not a box",
                k.as_ref()
            )));
        }
    }
    let params = match t.get::<mlua::Value>("params")? {
        mlua::Value::Nil => Value::Null,
        v => lua_to_value(&v)?,
    };
    ui.slot_with(&name, &params);
    Ok(())
}

fn build_node(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    let ty: String = t.get("type")?;
    if ty == "fill" {
        return build_fill(ui, t);
    }
    check_props(ui, t, element_of(&ty))?;
    match ty.as_str() {
        "row" | "column" => {
            let p = with_refs(ui, |refs| parse_props(t, ty == "row", refs))?;
            ui.core().open_from(p, Content::Box);
            build_children(ui, t)?;
            ui.close();
            Ok(())
        }
        "text" => {
            let style = with_refs(ui, |refs| parse_props(t, false, refs))?.style;
            if let Some(spans) = t.get::<Option<Table>>("spans")? {
                let parts = with_refs(ui, |refs| collect_spans(&spans, refs))?;
                let spans: Vec<Span<'_>> = parts.iter().map(span_of).collect();
                ui.rich_text(&spans, style);
            } else {
                let value: String = t.get("value")?;
                ui.text(&value, style);
            }
            Ok(())
        }
        "image" => {
            // Handle from the host (kui_image_add / Resources::add_image),
            // passed to scripts as a plain integer. `sampling` and `fit`
            // are the rows ADR 0025 gives the element, by name.
            let id: i64 = t.get("id")?;
            let spec = with_refs(ui, |refs| parse_props(t, false, refs))?.spec;
            let named = |row: &str, names: &[&str]| -> mlua::Result<usize> {
                match t.get::<Option<String>>(row)? {
                    None => Ok(0),
                    Some(s) => names
                        .iter()
                        .position(|n| *n == s)
                        .ok_or_else(|| bad(format!("{row} must be one of {}", names.join(", ")))),
                }
            };
            let sampling_names: Vec<&str> =
                kui_core::Sampling::ALL.iter().map(|s| s.name()).collect();
            let fit_names: Vec<&str> = kui_core::ImageFit::ALL.iter().map(|f| f.name()).collect();
            let opts = kui_core::ImageOpts {
                sampling: kui_core::Sampling::ALL[named("sampling", &sampling_names)?],
                fit: kui_core::ImageFit::ALL[named("fit", &fit_names)?],
            };
            ui.image_with(kui_core::ImageId::from_ffi(id as u64), opts, spec);
            Ok(())
        }
        "polygon" => {
            // `points`, each a `{x, y}` pair; the fill is the `bg` row, read
            // by parse_props like any node's (ADR 0025, decision 6).
            let p = with_refs(ui, |refs| parse_props(t, false, refs))?;
            let Some(list) = t.get::<Option<Table>>("points")? else {
                return Err(bad("polygon needs points"));
            };
            let points: Vec<kui_core::Vec2> = list
                .sequence_values::<mlua::Value>()
                .map(|v| {
                    let mlua::Value::Table(pt) = v? else {
                        return Err(bad("a polygon point is a {x, y} table"));
                    };
                    Ok(kui_core::Vec2::new(pt.get(1)?, pt.get(2)?))
                })
                .collect::<mlua::Result<_>>()?;
            ui.core().open_from(p, Content::Polygon(&points));
            Ok(())
        }
        "fragment" => {
            // Handle from the host (kui_fragment_add / Core::add_fragment),
            // passed to scripts as a plain integer, like an image's — and
            // `image`, the image handle the function samples (backlog V1),
            // absent or 0 for none.
            let id: i64 = t.get("id")?;
            let image: Option<i64> = t.get("image")?;
            let params: Vec<f32> = match t.get::<Option<Table>>("params")? {
                Some(list) => list.sequence_values::<f32>().collect::<mlua::Result<_>>()?,
                None => Vec::new(),
            };
            let p = with_refs(ui, |refs| parse_props(t, false, refs))?;
            let frag = kui_core::FragmentRef {
                id: kui_core::FragmentId::from_ffi(id as u64),
                image: image
                    .filter(|i| *i != 0)
                    .map(|i| kui_core::ImageId::from_ffi(i as u64)),
            };
            ui.core().open_from(p, Content::Fragment(frag, &params));
            build_children(ui, t)?;
            ui.close();
            Ok(())
        }
        "line" => {
            // `from`/`to` or `points`, each point a `{x, y}` pair. `width`
            // parses as a sizing row too, harmlessly: the core overrides a
            // line's sizing with its own box. `color` is the text-colour
            // row, read off the parsed style, so it defaults to the
            // foreground like a text node's.
            let p = with_refs(ui, |refs| parse_props(t, false, refs))?;
            let point = |v: mlua::Value| -> mlua::Result<kui_core::Vec2> {
                let mlua::Value::Table(pt) = v else {
                    return Err(bad("a line point is a {x, y} table"));
                };
                Ok(kui_core::Vec2::new(pt.get(1)?, pt.get(2)?))
            };
            let points: Vec<kui_core::Vec2> = match t.get::<Option<Table>>("points")? {
                Some(list) => list
                    .sequence_values::<mlua::Value>()
                    .map(|v| point(v?))
                    .collect::<mlua::Result<_>>()?,
                None => {
                    let (Some(from), Some(to)) = (
                        t.get::<Option<mlua::Value>>("from")?,
                        t.get::<Option<mlua::Value>>("to")?,
                    ) else {
                        return Err(bad("line needs from and to, or points"));
                    };
                    vec![point(from)?, point(to)?]
                }
            };
            let width = t.get::<Option<f32>>("width")?.unwrap_or(1.0);
            // No `color` is the theme's foreground, as for a text run.
            let stroke_color = p.style.color.unwrap_or(ui.theme().fg);
            let mut stroke = kui_core::Stroke::new(width, stroke_color);
            stroke.curve = t.get::<Option<bool>>("curve")?.unwrap_or(false);
            ui.core().open_from(p, Content::Line(&points, stroke));
            Ok(())
        }
        "cells" => {
            // A string per row in `lines`, cells past a row's end blank;
            // `runs` of {row, col, len, fg, bg, flags} colour and attribute
            // spans over them (0 keeps the default). The style rows size
            // the cells; the node rows are the node's.
            let p = with_refs(ui, |refs| parse_props(t, false, refs))?;
            let rows: usize = t.get::<Option<usize>>("rows")?.unwrap_or(0);
            let cols: usize = t.get::<Option<usize>>("cols")?.unwrap_or(0);
            if rows == 0 || cols == 0 {
                return Err(bad("cells needs rows and cols"));
            }
            let default_fg = p.style.color.unwrap_or(ui.theme().fg).to_hex();
            let mut cells = vec![kui_core::Cell::new(' ', default_fg, 0); rows * cols];
            if let Some(lines) = t.get::<Option<Table>>("lines")? {
                for (r, line) in lines.sequence_values::<String>().enumerate() {
                    if r >= rows {
                        break;
                    }
                    for (c, ch) in line?.chars().take(cols).enumerate() {
                        cells[r * cols + c].ch = ch;
                    }
                }
            }
            if let Some(runs) = t.get::<Option<Table>>("runs")? {
                for run in runs.sequence_values::<Table>() {
                    let run = run?;
                    let row: usize = run.get(1)?;
                    let col: usize = run.get(2)?;
                    let len: usize = run.get(3)?;
                    let fg: u32 = run.get::<Option<u32>>(4)?.unwrap_or(0);
                    let bg: u32 = run.get::<Option<u32>>(5)?.unwrap_or(0);
                    let flags: u8 = run.get::<Option<u8>>(6)?.unwrap_or(0);
                    if row >= rows {
                        continue;
                    }
                    for c in col..(col + len).min(cols) {
                        let cell = &mut cells[row * cols + c];
                        if fg != 0 {
                            cell.fg = fg;
                        }
                        if bg != 0 {
                            cell.bg = bg;
                        }
                        cell.flags |= flags;
                    }
                }
            }
            let cursor = match t.get::<Option<Table>>("cursor_at")? {
                Some(cur) => {
                    let shape = t
                        .get::<Option<String>>("cursor_shape")?
                        .and_then(|s| kui_core::CellCursor::from_name(&s))
                        .unwrap_or(kui_core::CellCursor::Block);
                    let color = match t.get::<mlua::Value>("cursor_color")? {
                        mlua::Value::Nil => None,
                        v => with_refs(ui, |refs| parse_color(&v, refs))?,
                    }
                    .unwrap_or(Color::rgb8(0xff, 0xff, 0xff));
                    Some((cur.get::<usize>(1)?, cur.get::<usize>(2)?, shape, color))
                }
                None => None,
            };
            // The absolute line row 0 is; 0 when the app says nothing.
            let origin_line = t.get::<Option<u64>>("origin_line")?.unwrap_or(0);
            let grid = kui_core::CellGrid {
                rows,
                cols,
                cells: &cells,
                style: p.style,
                cursor,
                origin_line,
            };
            ui.core().open_from(p, Content::Cells(&grid));
            Ok(())
        }
        "audio" => {
            // Handle from the host (kui_sound_add / Core::add_sound), passed
            // to scripts as a plain integer, like images.
            let src: i64 = t.get("src")?;
            let mut spec = kui_core::AudioSpec::new(kui_core::SoundId::from_ffi(src as u64));
            if let Some(v) = t.get::<Option<f32>>("volume")? {
                spec = spec.volume(v);
            }
            if t.get::<Option<bool>>("loop")?.unwrap_or(false) {
                spec = spec.looped();
            }
            spec = spec.paused(t.get::<Option<bool>>("paused")?.unwrap_or(false));
            if t.get::<Option<bool>>("finish")?.unwrap_or(false) {
                spec = spec.finish();
            }
            if let Some(tag) = t.get::<Option<mlua::Value>>("tag")? {
                spec.tag = Some(lua_to_value(&tag)?);
            }
            match t.get::<Option<String>>("key")? {
                Some(k) => ui.audio_keyed(&k, spec),
                None => ui.audio(spec),
            };
            Ok(())
        }
        "input" => {
            let label: String = t.get("label")?;
            let initial: String = t.get::<Option<String>>("initial")?.unwrap_or_default();
            widgets::text_input(ui, &label, &initial);
            Ok(())
        }
        "edit" => {
            let p = with_refs(ui, |refs| parse_props(t, false, refs))?;
            let label = match p.key.clone().or(t.get::<Option<String>>("label")?) {
                Some(l) => l,
                None => return Err(bad("edit needs a key (state is retained by key)")),
            };
            let initial: String = t.get::<Option<String>>("initial")?.unwrap_or_default();
            let opts = EditOptions {
                style: p.style,
                multiline: t.get::<Option<bool>>("multiline")?.unwrap_or(false),
                autofocus: t.get::<Option<bool>>("autofocus")?.unwrap_or(false),
                wrap: p.wrap,
                ..Default::default()
            };
            ui.text_edit(&label, &initial, &opts, p.spec);
            Ok(())
        }
        "titlebar" => {
            if t.raw_len() > 0 {
                with_children(ui, t, |ui, body| widgets::titlebar_with(ui, body))
            } else {
                let title: String = t.get::<Option<String>>("title")?.unwrap_or_default();
                widgets::titlebar(ui, &title);
                Ok(())
            }
        }
        "window_buttons" => {
            widgets::window_buttons(ui);
            Ok(())
        }
        "menu_bar" => {
            widgets::menu_bar(ui, menu_bar_of(t)?);
            Ok(())
        }
        "tooltip" => {
            if t.raw_len() > 0 {
                with_children(ui, t, |ui, body| widgets::tooltip_with(ui, body))
            } else {
                let value: String = t.get("value")?;
                widgets::tooltip(ui, &value);
                Ok(())
            }
        }
        "latency_graph" => {
            widgets::latency_graph(ui);
            Ok(())
        }
        "latency_hud" => {
            let (mut x, mut y) = (Align::End, Align::End);
            if let Some(at) = t.get::<Option<Table>>("at")? {
                x = parse_align(&at.get::<String>(1)?)?;
                y = parse_align(&at.get::<String>(2)?)?;
            }
            widgets::latency_hud_at(ui, x, y);
            Ok(())
        }
        "button" => {
            // `label` is the accessible name, and the text too unless
            // `text` says otherwise — the one string a script always gave
            // its button is the row a reader hears first. The other rows
            // the stock button admits (`schema::BUTTON_ROWS_LUA`) are read
            // by name over `widgets::button_spec`, as the JSX encoder and
            // `kui_button_with` read them: the look stays the widget's.
            // The tooltip goes first so an explicit `description` wins
            // over the shorthand, as it does in C — a table has no order
            // to make "the later one" mean anything.
            let label: String = t.get("label")?;
            let text: String = t
                .get::<Option<String>>("text")?
                .unwrap_or_else(|| label.clone());
            let key: String = t
                .get::<Option<String>>("key")?
                .unwrap_or_else(|| label.clone());
            let payload = match t.get::<Option<mlua::Value>>("on_click")? {
                Some(v) => lua_to_value(&v)?,
                None => Value::Null,
            };
            let mut out = PropsOut::new();
            out.spec = widgets::button_spec(&ui.metrics())
                .on_click(payload)
                .label(label.as_str());
            if let Some(hint) = t.get::<Option<String>>("tooltip")? {
                out.apply_tooltip(&hint);
            }
            with_refs(ui, |refs| {
                for name in ["description", "disabled", "accent"] {
                    let v = t.get::<mlua::Value>(name)?;
                    if v.is_nil() {
                        continue;
                    }
                    let def = schema::by_snake_name(name).expect("a button row");
                    if let Some(parsed) =
                        parse_value(&def.kind, &v, refs).map_err(|e| bad(format!("{name}: {e}")))?
                    {
                        schema::apply(def, parsed, &mut out).map_err(bad)?;
                    }
                }
                Ok(())
            })?;
            widgets::button_with(ui, &key, &text, out.spec, out.tooltip.as_deref());
            Ok(())
        }
        other => Err(mlua::Error::runtime(format!("unknown node type '{other}'"))),
    }
}

struct SpanPart {
    text: String,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    color: Option<Color>,
    bg: Option<Color>,
}

fn span_of(p: &SpanPart) -> Span<'_> {
    let mut s = Span::new(&p.text);
    if p.bold {
        s = s.bold();
    }
    if p.italic {
        s = s.italic();
    }
    if p.underline {
        s = s.underline();
    }
    if p.strikethrough {
        s = s.strikethrough();
    }
    if let Some(c) = p.color {
        s = s.color(c);
    }
    if let Some(c) = p.bg {
        s = s.bg(c);
    }
    s
}

/// `{ "plain", { "styled", bold = true, italic = true, color = 0x.. }, ... }`
fn collect_spans(spans: &Table, refs: &mut Refs<'_>) -> mlua::Result<Vec<SpanPart>> {
    let mut out = Vec::new();
    for item in spans.sequence_values::<mlua::Value>() {
        match item? {
            mlua::Value::String(s) => out.push(SpanPart {
                text: s.to_str()?.to_string(),
                bold: false,
                italic: false,
                underline: false,
                strikethrough: false,
                color: None,
                bg: None,
            }),
            mlua::Value::Table(t) => {
                let text: String = t
                    .get::<Option<String>>(1)?
                    .ok_or_else(|| bad("span table needs its text at [1]"))?;
                let color = match t.get::<mlua::Value>("color")? {
                    mlua::Value::Nil => None,
                    v => parse_color(&v, refs)?,
                };
                let bg = match t.get::<mlua::Value>("bg")? {
                    mlua::Value::Nil => None,
                    v => parse_color(&v, refs)?,
                };
                out.push(SpanPart {
                    text,
                    bold: t.get::<Option<bool>>("bold")?.unwrap_or(false),
                    italic: t.get::<Option<bool>>("italic")?.unwrap_or(false),
                    underline: t.get::<Option<bool>>("underline")?.unwrap_or(false),
                    strikethrough: t.get::<Option<bool>>("strikethrough")?.unwrap_or(false),
                    color,
                    bg,
                });
            }
            other => {
                return Err(bad(format!(
                    "span must be a string or table, got {}",
                    other.type_name()
                )));
            }
        }
    }
    Ok(out)
}

/// `{ colors = { name = colour | { light =, dark = } }, lengths = { name =
/// px } }` as a [`kui_core::Tokens`] (ADR 0027). Names are sorted, since a
/// Lua table's iteration order is not one a declaration can promise and
/// the index is only what `env.tokens` lists them in.
pub fn parse_tokens(t: &Table) -> mlua::Result<kui_core::Tokens> {
    let mut out = kui_core::Tokens::new();
    for pair in t.pairs::<String, mlua::Value>() {
        let (k, _) = pair?;
        if k != "colors" && k != "lengths" {
            return Err(bad(format!("tokens: unknown key {k:?} (colors, lengths)")));
        }
    }
    if let Some(colors) = t.get::<Option<Table>>("colors")? {
        let mut entries: Vec<(String, mlua::Value)> = colors
            .pairs::<String, mlua::Value>()
            .collect::<mlua::Result<_>>()?;
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, v) in entries {
            out = match &v {
                mlua::Value::Table(halves) => {
                    let half = |k: &str| -> mlua::Result<Color> {
                        match halves.get::<mlua::Value>(k)? {
                            mlua::Value::Nil => Err(bad(format!(
                                "tokens.colors.{name}: needs both light and dark"
                            ))),
                            v => parse_color_value(&v),
                        }
                    };
                    out.color_themed(&name, half("light")?, half("dark")?)
                }
                v => out.color(
                    &name,
                    parse_color_value(v).map_err(|e| bad(format!("tokens.colors.{name}: {e}")))?,
                ),
            };
        }
    }
    if let Some(lengths) = t.get::<Option<Table>>("lengths")? {
        let mut entries: Vec<(String, mlua::Value)> = lengths
            .pairs::<String, mlua::Value>()
            .collect::<mlua::Result<_>>()?;
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, v) in entries {
            let px = number(&v)
                .ok_or_else(|| bad(format!("tokens.lengths.{name}: a length is a number")))?;
            out = out.length(&name, px);
        }
    }
    Ok(out)
}

/// What a `$name` in a prop resolves through while a table is parsed
/// (ADR 0027): the core's lookup for the running origin — its own table
/// over the host's, the roles in front — and the names that did not
/// resolve, raised as `unknown-token` once the borrow is handed back
/// ([`with_refs`]). A slot whose name resolves to nothing is left out, so
/// it keeps its default — the theme's foreground for a text's `color`, a
/// fit width, the default text size — the way Node's encoder drops the
/// prop; not an explicit transparent or zero, which would hide the text
/// a typo was on.
pub struct Refs<'a> {
    look: kui_core::TokenLookup<'a>,
    errors: Vec<kui_core::TokenError>,
}

impl<'a> Refs<'a> {
    pub fn new(look: kui_core::TokenLookup<'a>) -> Self {
        Self {
            look,
            errors: Vec::new(),
        }
    }

    fn color(&mut self, name: &str) -> Option<Color> {
        match self.look.color(name) {
            Ok(c) => Some(c),
            Err(e) => {
                self.errors.push(e);
                None
            }
        }
    }

    fn length(&mut self, name: &str) -> Option<f32> {
        match self.look.length(name) {
            Ok(v) => Some(v),
            Err(e) => {
                self.errors.push(e);
                None
            }
        }
    }
}

/// Runs `f` with a [`Refs`] over the frame's lookup, then raises what did
/// not resolve. The lookup borrows the core for `f`'s duration and nothing
/// longer, so the caller can open the node it parsed right after.
fn with_refs<R>(
    ui: &mut Ui<'_>,
    f: impl FnOnce(&mut Refs<'_>) -> mlua::Result<R>,
) -> mlua::Result<R> {
    let (r, errors) = {
        let mut refs = Refs::new(ui.core().token_lookup());
        let r = f(&mut refs);
        (r, refs.errors)
    };
    for e in errors {
        ui.core().warn_unknown_token(&e);
    }
    r
}

/// A `$name` if `v` is one.
fn reference(v: &mlua::Value) -> mlua::Result<Option<String>> {
    if let mlua::Value::String(s) = v
        && let Some(name) = kui_core::tokens::reference(&s.to_str()?)
    {
        return Ok(Some(name.to_string()));
    }
    Ok(None)
}

/// A number, or a `$name` length token.
fn length_of(v: &mlua::Value, refs: &mut Refs<'_>) -> mlua::Result<Option<f32>> {
    if let Some(name) = reference(v)? {
        return Ok(refs.length(&name));
    }
    number(v)
        .map(Some)
        .ok_or_else(|| bad("expected a number or a \"$token\""))
}

/// Table → props. Constructor-order specials first (`dir` from the node
/// type, `size` before any style prop), then the Lua-shaped composites,
/// then every schema row by its snake_case name. Unknown keys (`type`,
/// `value`, `label`, children) fall through. `refs` is what a `$name`
/// resolves through ([`with_refs`]).
pub fn parse_props(t: &Table, is_row: bool, refs: &mut Refs<'_>) -> mlua::Result<PropsOut> {
    let mut out = PropsOut::new();
    if is_row {
        out.spec = kui_core::NodeSpec::row();
    }
    match t.get::<mlua::Value>("size")? {
        mlua::Value::Nil => {}
        v => {
            if let Some(size) = length_of(&v, refs)? {
                out.style = kui_core::TextStyle::new(size);
            }
        }
    }
    // `radius` sets all four corners, so it must land before any
    // `radius_tl`-style override — table iteration order is undefined.
    match t.get::<mlua::Value>("radius")? {
        mlua::Value::Nil => {}
        v => {
            if let Some(r) = length_of(&v, refs)? {
                out.with_spec(|s| s.radius(r));
            }
        }
    }
    // Overflow bits accumulate across the walk (`clip` and `scroll` are
    // separate keys) and are applied once, so nothing here has to know that
    // scrolling clips too.
    let mut overflow = 0;
    for pair in t.pairs::<mlua::Value, mlua::Value>() {
        let (k, v) = pair?;
        let mlua::Value::String(k) = k else { continue };
        let k = k.to_str()?;
        match k.as_ref() {
            "size" | "radius" => {}
            "pad" => {
                let pad = parse_pad(&v, refs)?;
                out.apply_pad(pad);
            }
            "border" => {
                let b = match v {
                    mlua::Value::Table(b) => b,
                    _ => return Err(bad("border must be a table {w=, color=}")),
                };
                let w = length_of(&b.get::<mlua::Value>("w")?, refs)?.unwrap_or(0.0);
                let c = parse_color(&b.get::<mlua::Value>("color")?, refs)?
                    .unwrap_or(Color::TRANSPARENT);
                out.with_spec(|s| s.border(w, c));
            }
            "clip" => overflow |= bit(&v, kui_core::OVERFLOW_CLIP),
            "scroll_x" => overflow |= bit(&v, kui_core::OVERFLOW_SCROLL_X),
            "scroll" | "scroll_y" => overflow |= bit(&v, kui_core::OVERFLOW_SCROLL_Y),
            "float" => {
                let cfg = parse_float(&v)?;
                out.with_spec(|s| s.float(cfg));
            }
            "key_focus" => out.key_focus = truthy(&v),
            "key" => {
                let mlua::Value::String(s) = &v else {
                    return Err(bad("key must be a string"));
                };
                out.key = Some(s.to_str()?.to_string());
            }
            "index" => {
                let Some(i) = v.as_number().or_else(|| v.as_integer().map(|i| i as f64)) else {
                    return Err(bad("index must be a number (the row's data index)"));
                };
                out.index = Some(i.max(0.0) as u64);
            }
            "tooltip" => {
                let mlua::Value::String(s) = &v else {
                    return Err(bad("tooltip must be a string"));
                };
                out.apply_tooltip(s.to_str()?.as_ref());
            }
            name => {
                // `repeat` is a Lua keyword, so that row also answers to
                // CSS's own name for it (`schema::LUA_ALIASES`, which the
                // unknown-prop check reads too).
                let name = schema::lua_alias(name).unwrap_or(name);
                let Some(def) = schema::by_snake_name(name) else {
                    continue;
                };
                if let Some(parsed) =
                    parse_value(&def.kind, &v, refs).map_err(|e| bad(format!("{name}: {e}")))?
                {
                    schema::apply(def, parsed, &mut out).map_err(bad)?;
                }
            }
        }
    }
    out.with_spec(|s| s.overflow_bits(overflow));
    Ok(out)
}

fn truthy(v: &mlua::Value) -> bool {
    matches!(v, mlua::Value::Boolean(true))
}

/// `bit` when the flag is on, for ORing an overflow mask together.
fn bit(v: &mlua::Value, bit: u32) -> u32 {
    if truthy(v) { bit } else { 0 }
}

fn number(v: &mlua::Value) -> Option<f32> {
    match v {
        mlua::Value::Number(n) => Some(*n as f32),
        mlua::Value::Integer(n) => Some(*n as f32),
        _ => None,
    }
}

/// One schema value from Lua, by kind. `None` = absent (a false flag).
fn parse_value(kind: &Kind, v: &mlua::Value, refs: &mut Refs<'_>) -> mlua::Result<Option<Parsed>> {
    Ok(Some(match kind {
        Kind::F32 => match length_of(v, refs)? {
            Some(px) => Parsed::F32(px),
            None => return Ok(None),
        },
        Kind::Color => match parse_color(v, refs)? {
            Some(c) => Parsed::Color(c),
            None => return Ok(None),
        },
        Kind::Flag => {
            if truthy(v) {
                Parsed::Flag
            } else {
                return Ok(None);
            }
        }
        Kind::Enum(names) => {
            let mlua::Value::String(s) = v else {
                return Err(bad(format!("expected one of {names:?}")));
            };
            Parsed::Enum(schema::enum_index(names, &s.to_str()?).map_err(bad)?)
        }
        Kind::Sizing => match parse_sizing(v, refs)? {
            Some(s) => Parsed::Sizing(s),
            None => return Ok(None),
        },
        Kind::Min => Parsed::Min(match v {
            mlua::Value::String(s) => schema::min_str(&s.to_str()?).map_err(bad)?,
            v => kui_core::Min::px(number(v).ok_or_else(|| bad("expected a number or \"fit\""))?),
        }),
        Kind::Msg | Kind::Tag => Parsed::Msg(lua_to_value(v)?),
        Kind::Str => {
            let mlua::Value::String(s) = v else {
                return Err(bad("expected a string"));
            };
            Parsed::Str(s.to_str()?.to_string())
        }
        Kind::Resource => match v {
            mlua::Value::Integer(n) => Parsed::Resource(*n as u64),
            mlua::Value::Number(n) => Parsed::Resource(*n as u64),
            _ => return Err(bad("expected a resource handle (integer)")),
        },
        Kind::Keyframes => {
            Parsed::Keyframes(kui_core::keyframes::parse(&lua_to_value(v)?).map_err(bad)?)
        }
        Kind::Enter => Parsed::Enter(kui_core::enter::parse(&lua_to_value(v)?).map_err(bad)?),
    }))
}

/// `0xRRGGBBAA` integers or `"#hex"` strings — a value, never a reference:
/// what a token declaration holds.
fn parse_color_value(v: &mlua::Value) -> mlua::Result<Color> {
    match v {
        mlua::Value::Integer(n) => Ok(schema::color_num(*n as u32)),
        mlua::Value::Number(n) => Ok(schema::color_num(*n as u32)),
        mlua::Value::String(s) => schema::color_hex_str(&s.to_str()?).map_err(bad),
        _ => Err(bad("color must be a 0xRRGGBBAA integer or \"#hex\" string")),
    }
}

/// A colour prop: a value, or a `"$name"` token reference.
fn parse_color(v: &mlua::Value, refs: &mut Refs<'_>) -> mlua::Result<Option<Color>> {
    if let Some(name) = reference(v)? {
        return Ok(refs.color(&name));
    }
    parse_color_value(v)
        .map(Some)
        .map_err(|_| bad("color must be a 0xRRGGBBAA integer, a \"#hex\" string or a \"$token\""))
}

fn parse_sizing(v: &mlua::Value, refs: &mut Refs<'_>) -> mlua::Result<Option<Sizing>> {
    if let Some(name) = reference(v)? {
        return Ok(refs.length(&name).map(Sizing::Fixed));
    }
    Ok(Some(match v {
        mlua::Value::Number(n) => Sizing::Fixed(*n as f32),
        mlua::Value::Integer(n) => Sizing::Fixed(*n as f32),
        mlua::Value::String(s) => schema::sizing_str(&s.to_str()?).map_err(bad)?,
        mlua::Value::Table(t) => {
            if let Some(p) = t.get::<Option<f32>>("pct")? {
                Sizing::Percent(p / 100.0)
            } else if let Some(f) = t.get::<Option<f32>>("grow")? {
                Sizing::Grow(f)
            } else {
                return Err(bad("sizing table needs pct or grow"));
            }
        }
        _ => return Err(bad("invalid sizing value")),
    }))
}

/// The `pad` prop as declared: a number is the all-round shorthand, a table
/// names any of the family (`x`, `y`, `l`, `r`, `t`, `b`). What a missing
/// edge falls back to is [`PadShorthand::resolve`]'s call.
fn parse_pad(v: &mlua::Value, refs: &mut Refs<'_>) -> mlua::Result<PadShorthand> {
    match v {
        mlua::Value::Table(t) => {
            let mut edge = |k: &str| -> mlua::Result<Option<f32>> {
                match t.get::<mlua::Value>(k)? {
                    mlua::Value::Nil => Ok(None),
                    v => length_of(&v, refs),
                }
            };
            Ok(PadShorthand {
                all: edge("all")?,
                x: edge("x")?,
                y: edge("y")?,
                l: edge("l")?,
                r: edge("r")?,
                t: edge("t")?,
                b: edge("b")?,
            })
        }
        v => Ok(PadShorthand {
            all: length_of(v, refs).map_err(|_| bad("invalid padding value"))?,
            ..PadShorthand::default()
        }),
    }
}

fn parse_align(s: &str) -> mlua::Result<Align> {
    schema::enum_index(schema::ALIGNS, s)
        .map(schema::align_idx)
        .map_err(bad)
}

/// A preset name, wherever Lua spells one: `float = "below"` and a float
/// table's `anchor`. The names and what each attaches to are core's.
fn float_preset(name: &str) -> mlua::Result<FloatConfig> {
    FloatConfig::preset(name).ok_or_else(|| {
        bad(format!(
            "bad float preset '{name}' (one of {})",
            kui_core::FLOAT_PRESETS.join(" | ")
        ))
    })
}

/// An `{ x, y }` attach point under `key`, or `None` when it is absent.
fn parse_attach(f: &Table, key: &str) -> mlua::Result<Option<(Align, Align)>> {
    let Some(at) = f.get::<Option<Table>>(key)? else {
        return Ok(None);
    };
    Ok(Some((
        parse_align(&at.get::<String>(1)?)?,
        parse_align(&at.get::<String>(2)?)?,
    )))
}

fn parse_float(v: &mlua::Value) -> mlua::Result<FloatConfig> {
    let f = match v {
        mlua::Value::String(s) => return float_preset(&s.to_str()?),
        mlua::Value::Table(f) => f,
        _ => return Err(bad("float must be a preset string or a table")),
    };
    // `self` is the name the other bindings use; `self_at` stays accepted
    // because Lua shipped with it.
    let self_at = match parse_attach(f, "self")? {
        Some(at) => Some(at),
        None => parse_attach(f, "self_at")?,
    };
    Ok(FloatConfig::build(
        match f.get::<Option<String>>("anchor")? {
            Some(name) => float_preset(&name)?,
            None => FloatConfig::parent(),
        },
        parse_attach(f, "at")?,
        self_at,
        f.get::<Option<f32>>("dx")?,
        f.get::<Option<f32>>("dy")?,
        f.get::<Option<bool>>("fit")?.unwrap_or(false),
    ))
}

// ---------------------------------------------------------------------------
// Value <-> Lua

pub fn lua_to_value(v: &mlua::Value) -> mlua::Result<Value> {
    Ok(match v {
        mlua::Value::Nil => Value::Null,
        mlua::Value::Boolean(b) => Value::Bool(*b),
        mlua::Value::Integer(i) => Value::Int(*i),
        mlua::Value::Number(n) => Value::Float(*n),
        mlua::Value::String(s) => Value::Str(s.to_str()?.to_string()),
        mlua::Value::Table(t) => {
            let len = t.raw_len();
            if len > 0 {
                let mut list = Vec::with_capacity(len);
                for item in t.sequence_values::<mlua::Value>() {
                    list.push(lua_to_value(&item?)?);
                }
                Value::List(list)
            } else {
                let mut map = Vec::new();
                for pair in t.pairs::<String, mlua::Value>() {
                    let (k, v) = pair?;
                    map.push((k, lua_to_value(&v)?));
                }
                Value::Map(map)
            }
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "cannot convert {} to event payload",
                other.type_name()
            )));
        }
    })
}

pub fn value_to_lua(lua: &Lua, v: &Value) -> mlua::Result<mlua::Value> {
    Ok(match v {
        Value::Null => mlua::Value::Nil,
        Value::Bool(b) => mlua::Value::Boolean(*b),
        Value::Int(i) => mlua::Value::Integer(*i),
        Value::Float(f) => mlua::Value::Number(*f),
        Value::Str(s) => mlua::Value::String(lua.create_string(s)?),
        Value::List(items) => {
            let t = lua.create_table_with_capacity(items.len(), 0)?;
            for (i, item) in items.iter().enumerate() {
                t.set(i + 1, value_to_lua(lua, item)?)?;
            }
            mlua::Value::Table(t)
        }
        Value::Map(entries) => {
            let t = lua.create_table_with_capacity(0, entries.len())?;
            for (k, v) in entries {
                t.set(k.as_str(), value_to_lua(lua, v)?)?;
            }
            mlua::Value::Table(t)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kui_core::{
        Core, Edges, FontFamily, InputEvent, NodeSpec, OriginId, Rect, Size, TextStyle, Vec2,
        WindowButton, WindowId,
    };

    #[test]
    fn value_round_trips_through_lua() {
        let lua = Lua::new();
        let original = Value::map([
            ("kind", "inc".into()),
            ("by", Value::Int(2)),
            (
                "weights",
                Value::List(vec![Value::Float(0.5), Value::Float(1.5)]),
            ),
            ("enabled", Value::Bool(true)),
        ]);
        let lua_v = value_to_lua(&lua, &original).unwrap();
        let back = lua_to_value(&lua_v).unwrap();
        assert_eq!(back.get("kind").and_then(Value::as_str), Some("inc"));
        assert_eq!(back.get("by").and_then(Value::as_int), Some(2));
        assert_eq!(back.get("enabled").and_then(Value::as_bool), Some(true));
        match back.get("weights") {
            Some(Value::List(items)) => assert_eq!(items.len(), 2),
            other => panic!("expected list, got {other:?}"),
        }
    }

    /// A `Refs` over a bare core: no tokens declared, the roles resolve.
    /// Leaked on purpose — the lookup borrows the core, and a test parses
    /// one table and asserts, so a core per call is the simplest shape.
    fn test_refs() -> Refs<'static> {
        let core: &'static Core = Box::leak(Box::new(Core::new()));
        Refs::new(core.token_lookup())
    }

    fn eval_table(lua: &Lua, src: &str) -> Table {
        lua.load(src).eval().unwrap()
    }

    /// The whole schema surface from a Lua table equals the Rust builder.
    #[test]
    fn schema_props_match_the_rust_builder() {
        let lua = Lua::new();
        let t = eval_table(
            &lua,
            r##"{
                width = "grow", height = {pct = 50},
                min_width = 10, max_width = 500, min_height = 5, max_height = 300,
                pad = {l = 1, r = 2, t = 3, b = 4}, gap = 8,
                main_align = "center", cross_align = "end", center = false,
                bg = "#14161e", radius = 6, border = {w = 1, color = 0x2a2d3aff},
                clip = true, scroll = true, scroll_x = true,
                float = {anchor = "viewport", at = {"end", "end"}, self_at = {"end", "end"},
                         dx = -8, dy = -8, fit = true},
                hoverable = true, window = "close",
                on_click = {kind = "hit"}, on_drag = "d", on_key = 7, key_up = true,
                modal = "dlg", on_context_menu = {kind = "menu"},
                initial_focus = true,
                key = "panel", key_focus = true,
            }"##,
        );
        let p = parse_props(&t, true, &mut test_refs()).unwrap();
        let expected = NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Percent(0.5))
            .min_width(10.0)
            .max_width(500.0)
            .min_height(5.0)
            .max_height(300.0)
            .padding(Edges {
                l: 1.0,
                r: 2.0,
                t: 3.0,
                b: 4.0,
            })
            .gap(8.0)
            .main_align(Align::Center)
            .cross_align(Align::End)
            .bg(Color::hex(0x14161eff))
            .radius(6.0)
            .border(1.0, Color::hex(0x2a2d3aff))
            .clip()
            .scroll_y()
            .scroll_x()
            .float(
                FloatConfig::viewport()
                    .at(Align::End, Align::End)
                    .self_at(Align::End, Align::End)
                    .offset(-8.0, -8.0)
                    .fit(),
            )
            .hoverable()
            .window_button(WindowButton::Close)
            .on_click(Value::map([("kind", "hit".into())]))
            .on_drag("d")
            .on_key(Value::Int(7))
            .key_up()
            .modal("dlg".into())
            .initial_focus()
            .on_context_menu(Value::map([("kind", "menu".into())]));
        assert_eq!(p.spec, expected);
        assert_eq!(p.key.as_deref(), Some("panel"));
        assert!(p.key_focus);
    }

    /// A min is a number or `"fit"`, per axis, and nothing else: the
    /// sizing words a min cannot be are refused by name.
    #[test]
    fn a_min_is_a_number_or_fit() {
        let lua = Lua::new();
        let t = eval_table(&lua, r#"{ min_width = "fit", min_height = 3 }"#);
        let p = parse_props(&t, false, &mut test_refs()).unwrap();
        let expected = NodeSpec::column()
            .min_width(kui_core::Min::FIT)
            .min_height(3.0);
        assert_eq!(p.spec, expected);
        let t = eval_table(&lua, r#"{ min_width = "grow" }"#);
        let err = parse_props(&t, false, &mut test_refs())
            .unwrap_err()
            .to_string();
        assert!(err.contains("bad min"), "{err}");
    }

    /// The shapes the core decides on, spelled the Lua way: the pad family
    /// beyond `l/r/t/b`, `self` as the other bindings name it, and a preset
    /// as a base with one override — the untouched `dy` keeps below's gap.
    #[test]
    fn the_lua_composites_resolve_the_way_the_core_says() {
        let lua = Lua::new();
        let t = eval_table(
            &lua,
            r#"{ pad = { all = 4, x = 10, b = 1 },
                 float = { anchor = "below", dx = 6 } }"#,
        );
        let p = parse_props(&t, false, &mut test_refs()).unwrap();
        assert_eq!(
            p.spec.layout.padding,
            Edges {
                l: 10.0,
                r: 10.0,
                t: 4.0,
                b: 1.0,
            }
        );
        assert_eq!(
            p.spec.layout.float,
            Some(FloatConfig::build(
                FloatConfig::below(),
                None,
                None,
                Some(6.0),
                None,
                false
            ))
        );

        // `self` and the `self_at` Lua shipped with name the same point.
        let by_self = eval_table(&lua, r#"{ float = { self = {"end", "start"} } }"#);
        let by_self_at = eval_table(&lua, r#"{ float = { self_at = {"end", "start"} } }"#);
        assert_eq!(
            parse_props(&by_self, false, &mut test_refs())
                .unwrap()
                .spec
                .layout
                .float,
            parse_props(&by_self_at, false, &mut test_refs())
                .unwrap()
                .spec
                .layout
                .float
        );

        // An unknown preset names the ones that exist instead of silently
        // floating against the parent.
        let bad_preset = eval_table(&lua, r#"{ float = "beneath" }"#);
        let e = parse_props(&bad_preset, false, &mut test_refs())
            .unwrap_err()
            .to_string();
        assert!(e.contains("beneath") && e.contains("below"), "{e}");
    }

    #[test]
    fn text_style_props_match_the_rust_builder() {
        let lua = Lua::new();
        let t = eval_table(
            &lua,
            r#"{ size = 20, line_height = 30, color = 0x73d98cff, family = "mono" }"#,
        );
        let style = parse_props(&t, false, &mut test_refs()).unwrap().style;
        assert_eq!(
            style,
            TextStyle::new(20.0)
                .line_height(30.0)
                .color(Color::hex(0x73d98cff))
                .family(FontFamily::Mono)
        );
        let t = eval_table(&lua, r#"{ wrap = "none", max_lines = 2, ellipsis = true }"#);
        assert_eq!(
            parse_props(&t, false, &mut test_refs()).unwrap().style,
            TextStyle::default().nowrap().max_lines(2).ellipsis()
        );
        // `size` is applied first regardless of table iteration order, so a
        // color set alongside it survives the TextStyle::new reset.
        let t = eval_table(&lua, r##"{ color = "#fff", size = 12 }"##);
        assert_eq!(
            parse_props(&t, false, &mut test_refs()).unwrap().style,
            TextStyle::new(12.0).color(Color::hex(0xffffffff))
        );
    }

    #[test]
    fn bad_values_name_the_prop() {
        let lua = Lua::new();
        let t = eval_table(&lua, r#"{ main_align = "middle" }"#);
        let e = parse_props(&t, false, &mut test_refs())
            .unwrap_err()
            .to_string();
        assert!(e.contains("main_align"), "{e}");
        assert!(e.contains("middle"), "{e}");
    }

    fn frame(core: &mut Core, ext: &mut LuaExtension) -> usize {
        let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Slot::root(), &mut ui).unwrap();
        ui.finish();
        core.output().0.quads.len()
    }

    #[test]
    fn script_view_builds_ir_nodes() {
        let mut ext = LuaExtension::from_source(
            "test",
            r#"
                count = 41
                function view()
                  return column { gap = 8, pad = 16, bg = 0x10121aff,
                    text("count: " .. count, { size = 20 }),
                    button { label = "bump", on_click = { kind = "bump" } },
                  }
                end
                function on_event(ev)
                  if ev.kind == "bump" then count = count + 1 end
                end
            "#,
        )
        .unwrap();

        let mut core = Core::new();
        let quads = frame(&mut core, &mut ext);
        // Panel bg + button bg + glyphs for two strings.
        assert!(quads > 10, "expected panel/button/glyph quads, got {quads}");

        // Events round-trip into Lua state.
        ext.on_event(&UiEvent {
            origin: OriginId(1),
            key: Key::ROOT,
            payload: Value::map([("kind", "bump".into())]),
            window: WindowId::MAIN,
        });
        let count: i64 = ext.lua.globals().get("count").unwrap();
        assert_eq!(count, 42);
    }

    /// The root table's `windows` list is `Ui::window` per entry: a name
    /// alone takes the defaults, a table its own size, and the commands
    /// the declared set produces come out of the core for the host to
    /// drain — this binding opens nothing itself.
    #[test]
    fn the_root_table_declares_windows() {
        use kui_core::{WindowCommand, WindowConfig};
        let mut ext = LuaExtension::from_source(
            "windows",
            r#"
                function view(env)
                  return column {
                    windows = { { name = "palette", width = 400, height = 300,
                                  activates = false }, "tools" },
                    text("main"),
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let cmds = core.take_window_commands();
        assert_eq!(cmds.len(), 2, "{cmds:?}");
        assert_eq!(
            cmds[0],
            WindowCommand::Open {
                id: WindowId(1),
                owner: WindowId::MAIN,
                origin: OriginId(1),
                config: WindowConfig {
                    size: Size::new(400.0, 300.0),
                    activates: false,
                    ..WindowConfig::default()
                },
            }
        );
        assert_eq!(
            cmds[1],
            WindowCommand::Open {
                id: WindowId(2),
                owner: WindowId::MAIN,
                origin: OriginId(1),
                config: WindowConfig::default(),
            }
        );
        // Declared again: nothing new, and no unknown-prop line for the key.
        frame(&mut core, &mut ext);
        assert!(core.take_window_commands().is_empty());
        assert!(core.take_warnings().is_empty());
    }

    /// `kind = "popup"` is ADR 0004 decision 9's menu surface: the anchor
    /// rides through untouched, and it does not activate unless asked —
    /// a popup that takes OS focus blurs the field that opened it.
    #[test]
    fn a_windows_entry_declares_a_popup() {
        use kui_core::{Rect, WindowCommand, WindowConfig, WindowKind};
        let mut ext = LuaExtension::from_source(
            "windows-popup",
            r#"
                function view(env)
                  return column {
                    windows = { { name = "menu", kind = "popup",
                                  width = 160, height = 320,
                                  anchor = { x = 12, y = 40, w = 160, h = 24 } } },
                    text("main"),
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        assert_eq!(
            core.take_window_commands(),
            vec![WindowCommand::Open {
                id: WindowId(1),
                owner: WindowId::MAIN,
                origin: OriginId(1),
                config: WindowConfig {
                    kind: WindowKind::Popup,
                    size: Size::new(160.0, 320.0),
                    activates: false,
                    anchor: Rect::new(12.0, 40.0, 160.0, 24.0),
                },
            }]
        );
    }

    /// A kind kui does not have is refused where it is written rather than
    /// dropped: opening a normal window for it would read as the popup
    /// having worked. (C cannot do this — an integer field has no room to
    /// refuse in — so it warns `unknown-window-kind` a frame later.)
    #[test]
    fn a_windows_entry_cannot_name_an_unknown_kind() {
        let mut ext = LuaExtension::from_source(
            "windows-kind",
            r#"
                function view(env)
                  return column {
                    windows = { { name = "palette", kind = "sheet" } },
                    text("main"),
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
        ui.set_origin(OriginId(1));
        let err = ext.view(&Slot::root(), &mut ui).unwrap_err();
        assert!(err.contains("sheet"), "{err}");
        assert!(err.contains("popup"), "{err}");
    }

    /// Every node type the prelude offers lowers without error and draws.
    #[test]
    fn every_node_type_lowers() {
        let mut ext = LuaExtension::from_source(
            "all",
            r##"
                function view(env)
                  return column { gap = 4, window_title = "all nodes", always_on_top = true,
                    titlebar { text("custom title"), window_buttons() },
                    titlebar { title = "plain title" },
                    text({ "same IR as ", { "Rust", bold = true, color = "#73d98c" },
                           { " — flatter", italic = true } }, { size = 13 }),
                    edit { key = "note", initial = "hello", size = 14, width = 200,
                           multiline = true },
                    input { label = "name", initial = "" },
                    row { tooltip = "hover hint", pad = 4, text("badge") },
                    row { hoverable = true, text("legend"), tooltip("always shown") },
                    row { text("rich tip"), tooltip { text("a"), text("b") } },
                    latency_graph(),
                    latency_hud { at = { "start", "end" } },
                    button { label = "ok", on_click = "ok" },
                  }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        let quads = frame(&mut core, &mut ext);
        assert!(quads > 60, "got {quads} quads");
        assert_eq!(core.window_title(), Some("all nodes"));
        assert!(core.always_on_top());
        // And every key above is one some table claims: this scene is the
        // allow-list's fixture, so a new element prop that nobody adds to
        // `ELEMENTS.lua_own` fails here instead of warning at a user.
        let unknown: Vec<String> = core
            .take_warnings()
            .into_iter()
            .filter(|w| w.code == kui_core::diag::UNKNOWN_PROP)
            .map(|w| w.message)
            .collect();
        assert!(unknown.is_empty(), "{unknown:#?}");
    }

    /// A key no table claims is thrown on the floor by the binding — so it
    /// says so, once, in the spelling Lua actually takes.
    #[test]
    fn unknown_props_warn_once_in_lua_spelling() {
        let mut ext = LuaExtension::from_source(
            "typos",
            r#"
                function view(env)
                  return column { pad = 8,
                    row { hoverBg = 0x333333ff, width = 10, height = 10 },
                    row { hoverBg = 0x333333ff, width = 10, height = 10 },
                    row { colour = 0x333333ff, width = 10, height = 10 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let mut warned: Vec<String> = core
            .take_warnings()
            .into_iter()
            .filter(|w| w.code == kui_core::diag::UNKNOWN_PROP)
            .map(|w| w.message)
            .collect();
        warned.sort();
        assert_eq!(warned.len(), 2, "one per name, not per node: {warned:#?}");
        assert!(
            warned[1].contains("`hoverBg` is not a prop of box")
                && warned[1].contains("did you mean `hover_bg`?"),
            "{warned:#?}"
        );
        // Nothing near `colour`, so no guess is offered.
        assert!(warned[0].contains("`colour`") && !warned[0].contains("did you mean"));
        // The second frame is silent: (code, key) dedup, as for every check.
        frame(&mut core, &mut ext);
        assert!(core.take_warnings().is_empty());
    }

    /// `direction` is the Lua spelling of the `repeat` row (a Lua keyword),
    /// and the check reads the same alias table the parser remaps through.
    #[test]
    fn the_repeat_alias_does_not_warn() {
        let mut ext = LuaExtension::from_source(
            "alias",
            r#"
                function view(env)
                  return column {
                    row { width = 10, height = 10, keyframes = { { bg = 0x000000ff } },
                          transition = 100, direction = "alternate" },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        assert!(core.take_warnings().is_empty());
    }

    /// The check is behind the same gate as every other diagnostic.
    #[test]
    fn unknown_props_stay_quiet_with_diagnostics_off() {
        let mut ext = LuaExtension::from_source(
            "quiet",
            r#"
                function view(env)
                  return column { hoverBg = 0x333333ff }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_diagnostics(false);
        frame(&mut core, &mut ext);
        assert!(core.take_warnings().is_empty());
    }

    /// `tooltip = "hint"` on a container makes it hoverable and floats the
    /// hint only while the cursor is over it.
    #[test]
    fn tooltip_prop_is_hover_gated() {
        let mut ext = LuaExtension::from_source(
            "tip",
            r#"
                function view(env)
                  return column { pad = 10,
                    row { width = 100, height = 40, bg = 0x333333ff, tooltip = "a long hint" },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let idle = frame(&mut core, &mut ext);
        core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 30.0)));
        let hovered = frame(&mut core, &mut ext);
        assert!(
            hovered > idle + 5,
            "hover should add the tooltip's quads ({idle} -> {hovered})"
        );
        core.handle_input(InputEvent::CursorLeft);
        assert_eq!(frame(&mut core, &mut ext), idle);
    }

    /// `role` / `label` / `checked` / `selected` / `expanded` / `value_*`
    /// The stock button reads the access rows and nothing else
    /// (`schema::BUTTON_ROWS_LUA`): `label` is the name and the text
    /// unless `text` says otherwise, `tooltip` is the description, and a
    /// row it would drop is warned about with the rows it does read.
    #[test]
    fn the_stock_button_admits_the_access_rows() {
        let mut ext = LuaExtension::from_source(
            "button",
            r#"
                function view(env)
                  return column { pad = 10, gap = 4,
                    button { label = "go", on_click = "go", description = "Starts the run" },
                    button { label = "Stop the run", text = "stop", on_click = "stop",
                             disabled = true, tooltip = "Nothing is running" },
                    button { label = "x", on_click = "x", radius = 12, hoverBg = 0x333333ff },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let tree = core.access_tree().clone();
        let named = |n: &str| {
            tree.nodes
                .iter()
                .find(|node| node.name.as_deref() == Some(n))
        };
        let go = named("go").expect("the button, named by its label");
        assert_eq!(go.role, kui_core::Role::Button);
        assert_eq!(go.description.as_deref(), Some("Starts the run"));
        assert!(!go.disabled);
        let stop = named("Stop the run").expect("named past its text");
        assert_eq!(stop.description.as_deref(), Some("Nothing is running"));
        assert!(stop.disabled);
        assert_eq!(tree.nodes.len(), 4, "window and three buttons");
        let ws = core.take_warnings();
        assert_eq!(ws.len(), 2, "{ws:?}");
        let radius = ws
            .iter()
            .find(|w| w.message.contains("`radius`"))
            .expect("a row the button does not read");
        assert!(
            radius
                .message
                .contains("is a prop, but not one button reads")
        );
        assert!(radius.message.contains("`description`"));
        // The camel spelling is a misspelling here, and the fix offered is
        // never a row the button would drop.
        let hover = ws
            .iter()
            .find(|w| w.message.contains("hoverBg"))
            .expect("a misspelling");
        assert!(hover.message.contains("is not a prop of button"));
        assert!(!hover.message.contains("did you mean"));
    }

    /// The one paint row the stock button takes: `accent` is a question
    /// put to the OS, not a colour, so a script that declares it gets the
    /// stock blue on a host that was never told what the accent is and
    /// the OS colour on one that was.
    #[test]
    fn an_accent_button_takes_the_colour_the_host_pushed() {
        let mut ext = LuaExtension::from_source(
            "accent",
            r#"
                function view(env)
                  return column { pad = 10,
                    button { label = "go", on_click = "go", accent = true },
                  }
                end
            "#,
        )
        .unwrap();
        let mut bg = |core: &mut Core| {
            frame(core, &mut ext);
            core.output().0.quads[0].color
        };
        let mut core = Core::new();
        assert_eq!(
            bg(&mut core),
            kui_core::Color::rgb8(0x3b, 0x5b, 0xd4),
            "no accent pushed, the stock button"
        );
        assert!(
            core.take_warnings().is_empty(),
            "and a row the button reads"
        );

        let accent = kui_core::Color::hex(0x007affff);
        core.env.system.accent = Some(accent);
        assert_eq!(bg(&mut core), accent);
    }

    /// are schema rows, so a script declares semantics like any other
    /// prop; the access tree shows them (and numbers a tab list itself),
    /// and an assistive request on a script's button emits its message.
    #[test]
    fn semantics_reach_the_access_tree() {
        let mut ext = LuaExtension::from_source(
            "a11y",
            r#"
                function view(env)
                  return column { pad = 10,
                    row { key = "save", on_click = "save", label = "Save", width = 20, height = 20 },
                    row { key = "check", role = "checkbox", checked = true, text("Remember") },
                    row { key = "vol", role = "slider", label = "Volume",
                          value_now = 3, value_min = 0, value_max = 10 },
                    row { key = "art", role = "none", on_click = "art", text("Art") },
                    row { key = "tip", tooltip = "more here", on_click = "t", text("Tip") },
                    row { key = "tabs", role = "tabList",
                      row { key = "t0", role = "tab", text("General") },
                      row { key = "t1", role = "tab", selected = true, text("Network") },
                    },
                    row { key = "adv", on_click = "adv", expanded = "collapsed",
                          label = "Advanced" },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let tree = core.access_tree().clone();
        let named = |n: &str| {
            tree.nodes
                .iter()
                .find(|node| node.name.as_deref() == Some(n))
                .cloned()
        };
        let save = named("Save").expect("labelled button");
        assert_eq!(save.role, kui_core::Role::Button);
        assert_eq!(save.origin, OriginId(1));
        let check = named("Remember").expect("checkbox named by its text");
        assert_eq!(check.role, kui_core::Role::Checkbox);
        assert_eq!(check.checked, Some(true));
        let vol = named("Volume").expect("slider");
        assert_eq!(
            (vol.number, vol.min, vol.max),
            (Some(3.0), Some(0.0), Some(10.0))
        );
        assert!(named("Art").is_none(), "role none hides a would-be button");
        let tip = named("Tip").expect("button");
        assert_eq!(tip.description.as_deref(), Some("more here"));
        // Every tab reports the state; the ordinals are the core's, not
        // the script's.
        let (t0, t1) = (named("General").unwrap(), named("Network").unwrap());
        assert_eq!((t0.selected, t1.selected), (Some(false), Some(true)));
        assert_eq!((t0.pos_in_set, t1.pos_in_set), (Some(0), Some(1)));
        let tabs = tree
            .nodes
            .iter()
            .find(|n| n.role == kui_core::Role::TabList)
            .expect("tab list");
        assert_eq!(tabs.set_size, Some(2));
        // An enum row, so a shut disclosure can say it is shut.
        let adv = named("Advanced").expect("disclosure");
        assert_eq!(adv.expanded, Some(false));

        let evs = core.handle_input(InputEvent::Access(kui_core::AccessRequest {
            key: save.key,
            action: kui_core::AccessAction::Click,
            value: None,
            anchor: None,
            focus: None,
        }));
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].origin, OriginId(1));
        assert_eq!(evs[0].payload.as_str(), Some("save"));
    }

    /// A script that draws its own editor: `role = "multilineTextInput"`
    /// on the sink, `role = "line"` rows with `caret` / `selection_anchor`
    /// byte offsets, and a selection request coming back as a table.
    #[test]
    fn a_custom_editor_in_lua_reaches_the_access_tree() {
        let mut ext = LuaExtension::from_source(
            "ed",
            r#"
                function view(env)
                  return column { key = "ed", role = "multilineTextInput", label = "Doc",
                    on_key = "keys",
                    row { role = "line", selection_anchor = 1, text("ab"), text("cd") },
                    row { role = "line", caret = 2, text("ef") },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let tree = core.access_tree().clone();
        let ed = tree
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("Doc"))
            .expect("the sink is the editor")
            .clone();
        assert_eq!(ed.role, kui_core::Role::MultilineTextInput);
        assert_eq!(ed.value.as_deref(), Some("abcd\nef"));
        assert_eq!(ed.runs.len(), 3);
        assert_eq!(ed.runs[1].text, "cd\n");
        assert_eq!(ed.caret, Some(7));
        assert_eq!(ed.selection, Some((1, 7)));
        let (a, f) = (ed.anchor.unwrap(), ed.focus.unwrap());
        assert_eq!((a.run, a.character), (ed.runs[0].key, 1));
        assert_eq!((f.run, f.character), (ed.runs[2].key, 2));

        let evs = core.handle_input(InputEvent::Access(
            kui_core::AccessRequest::new(ed.key, kui_core::AccessAction::SetTextSelection)
                .with_selection(
                    kui_core::TextPos {
                        run: ed.runs[1].key,
                        character: 1,
                    },
                    kui_core::TextPos {
                        run: ed.runs[2].key,
                        character: 0,
                    },
                ),
        ));
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].origin, OriginId(1));
        let p = &evs[0].payload;
        assert_eq!(
            p.get("action").and_then(Value::as_str),
            Some("setTextSelection")
        );
        let at = |k: &str, f: &str| p.get(k).and_then(|v| v.get(f)).and_then(Value::as_int);
        assert_eq!(
            (at("anchor", "line"), at("anchor", "offset")),
            (Some(0), Some(3))
        );
        assert_eq!(
            (at("focus", "line"), at("focus", "offset")),
            (Some(1), Some(0))
        );
        assert_eq!(p.get("tag").and_then(Value::as_str), Some("keys"));
    }

    /// Editors: autofocus, typing produces a "changed" event carrying the
    /// node key, and `env.edit_text(key)` reads the buffer back next frame.
    #[test]
    fn edit_text_round_trips_through_events() {
        let mut ext = LuaExtension::from_source(
            "edit",
            r#"
                pending = nil
                seen = nil
                function view(env)
                  if pending then seen = env.edit_text(pending) end
                  return column {
                    edit { key = "note", initial = "hi", autofocus = true, width = 200 },
                  }
                end
                function on_event(ev)
                  if ev.kind == "changed" then pending = ev.node_key end
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let events = core.handle_input(InputEvent::Text("!".into()));
        assert_eq!(
            events.len(),
            1,
            "typing into the autofocused editor emits one event"
        );
        assert_eq!(
            events[0].payload.get("kind").and_then(Value::as_str),
            Some("changed")
        );
        for ev in &events {
            ext.on_event(ev);
        }
        frame(&mut core, &mut ext);
        // A single-line field opens with the caret after its seed (F20).
        let seen: Option<String> = ext.lua.globals().get("seen").unwrap();
        assert_eq!(seen.as_deref(), Some("hi!"));
    }

    /// `env.set_edit_text` by the label the view declares: the spelling a
    /// script that is *opening* the editor can use, since the key comes
    /// from an event the editor has not fired (backlog F32). The frame
    /// that declares the field takes the held text over its `initial`.
    #[test]
    fn set_edit_text_by_label_seeds_the_editor_the_next_frame_declares() {
        let mut ext = LuaExtension::from_source(
            "edit",
            r#"
                frames = 0
                function view(env)
                  frames = frames + 1
                  if frames == 1 then return column {} end
                  if frames == 2 then
                    env.set_edit_text("note", "from the model")
                  end
                  return column {
                    edit { key = "note", initial = "ignored", width = 200 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        // A frame with no editor in it, then the one that opens the field:
        // `env` lives inside `view`, so the call is made from the frame
        // that declares the editor and its tree is what claims the text.
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let key = core.key_of("note").expect("the view declared the editor");
        assert_eq!(core.edit_text(key).as_deref(), Some("from the model"));
        let codes: Vec<&str> = core.take_warnings().iter().map(|w| w.code).collect();
        assert!(!codes.contains(&"edit-text-without-editor"), "{codes:?}");
        // And it is the seed, not a per-frame reset: the next frame's
        // typing is kept.
        core.set_focus(Some(key));
        core.handle_input(InputEvent::Text("!".into()));
        frame(&mut core, &mut ext);
        assert_eq!(core.edit_text(key).as_deref(), Some("from the model!"));
    }

    /// A script that owns its keyboard and asks for releases (`key_up`)
    /// sees both halves of a key on one `{kind="key"}` payload: a held key
    /// is `phase="down"` then `"up"`, and focus leaving while it is held
    /// delivers the `up` anyway.
    #[test]
    fn a_lua_key_sink_hears_press_and_release() {
        use kui_core::{KeyCode, KeyMods, KeyPress};
        let mut ext = LuaExtension::from_source(
            "game",
            r#"
                log = {}
                function view(env)
                  return column { key = "world", on_key = "keys", key_up = true,
                    key_focus = true, width = 400, height = 300 }
                end
                function on_event(ev)
                  if ev.kind == "key" then
                    log[#log + 1] = ev.phase .. ":" .. ev.code ..
                      "@" .. ev.physical ..
                      ":" .. tostring(ev.text) .. ":" .. tostring(ev.tag)
                  end
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let feed = |core: &mut Core, ext: &mut LuaExtension, ev| {
            for e in core.handle_input(ev) {
                ext.on_event(&e);
            }
        };
        let w = || KeyPress::new(KeyCode::Char('w'), KeyMods::default()).with_text("w");
        feed(&mut core, &mut ext, InputEvent::KeyDown(w()));
        feed(&mut core, &mut ext, InputEvent::KeyUp(w()));
        // The same key on a Russian layout, as a driver reports it: the
        // layout says "ц", the position says W. A script matching on
        // `ev.code` keeps working, and `ev.physical` is there for one that
        // would rather bind the position.
        let ru = || {
            KeyPress::from_layout(KeyCode::Char('ц'), KeyCode::Char('w'), KeyMods::default())
                .with_text("ц")
        };
        feed(&mut core, &mut ext, InputEvent::KeyDown(ru()));
        feed(&mut core, &mut ext, InputEvent::KeyUp(ru()));
        // Pressed again, then focus dropped while it is still down.
        feed(&mut core, &mut ext, InputEvent::KeyDown(w()));
        core.set_focus(None);
        for e in core.take_pending_events() {
            ext.on_event(&e);
        }
        let log: Vec<String> = ext.lua.globals().get("log").unwrap();
        assert_eq!(
            log,
            [
                "down:w@w:w:keys",
                // A release inserts nothing, so `text` is nil in Lua.
                "up:w@w:nil:keys",
                // The layout key never reaches `code`; the text it inserts
                // is still the layout's own.
                "down:w@w:ц:keys",
                "up:w@w:nil:keys",
                "down:w@w:w:keys",
                "up:w@w:nil:keys",
            ]
        );
    }

    /// `audio { }` nodes are retained playbacks: declared → play, declared
    /// again → nothing, gone → stop. The host hands the sound id to the
    /// script as an integer, like images.
    #[test]
    fn audio_nodes_drive_playback_commands() {
        use kui_core::AudioCommand;
        let mut ext = LuaExtension::from_source(
            "audio",
            r#"
                playing = true
                function view(env)
                  local items = {}
                  if playing then
                    items[1] = audio { src = SOUND, loop = true, volume = 0.5,
                                       key = "music", tag = { kind = "music" } }
                  end
                  return column { table.unpack(items) }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let sound = core.add_sound(vec![0; 8]);
        ext.lua
            .globals()
            .set("SOUND", sound.to_ffi() as i64)
            .unwrap();
        frame(&mut core, &mut ext);
        let cmds = core.take_audio_commands();
        assert!(
            matches!(
                cmds.as_slice(),
                [AudioCommand::Play { sound: s, looped: true, volume, .. }]
                    if *s == sound && *volume == 0.5
            ),
            "{cmds:?}"
        );
        frame(&mut core, &mut ext);
        assert!(
            core.take_audio_commands().is_empty(),
            "re-declaring is silent"
        );
        ext.lua.globals().set("playing", false).unwrap();
        frame(&mut core, &mut ext);
        assert!(matches!(
            core.take_audio_commands().as_slice(),
            [AudioCommand::Stop { .. }]
        ));
    }

    /// `env.measure_text` answers what layout gives the same text, in every
    /// input shape, and `on_layout` rects arrive in `on_event` with the
    /// node key like any other event.
    #[test]
    fn measure_and_layout_events_reach_scripts() {
        let mut ext = LuaExtension::from_source(
            "measure",
            r#"
                seen = nil
                function view(env)
                  local plain = env.measure_text("hello world", { size = 14 })
                  local node = env.measure_text(text("hello world", { size = 14 }))
                  local rich = env.measure_text({ "hello ", { "world", bold = true } }, { size = 14 })
                  local narrow = env.measure_text("hello world", { size = 14 }, plain.width / 2)
                  assert(plain.width > 0 and plain.lines == 1)
                  assert(node.width == plain.width and node.height == plain.height)
                  assert(rich.width > 0)
                  assert(narrow.lines > 1 and narrow.width <= plain.width / 2 + 0.5)
                  return column {
                    row { key = "panel", width = plain.width, height = 20, on_layout = "panel" },
                  }
                end
                function on_event(ev)
                  if ev.kind == "layout" then seen = ev end
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let evs = core.take_pending_events();
        assert_eq!(evs.len(), 1, "one layout event on first sight");
        for ev in &evs {
            ext.on_event(ev);
        }
        let seen: Table = ext.lua.globals().get("seen").unwrap();
        assert_eq!(seen.get::<String>("tag").unwrap(), "panel");
        assert_eq!(seen.get::<f64>("h").unwrap(), 20.0);
        assert!(seen.get::<f64>("w").unwrap() > 0.0);
        assert_eq!(seen.get::<i64>("node_key").unwrap(), evs[0].key.0 as i64);
        // Unchanged next frame: silence.
        frame(&mut core, &mut ext);
        assert!(core.take_pending_events().is_empty());
    }

    /// Window requests from a script queue like a reveal — against the
    /// frame being built, drained by the driver after it — in call order,
    /// and once; a headless core keeps them and nothing else changes.
    #[test]
    fn scripts_queue_window_size_and_focus_requests() {
        let mut ext = LuaExtension::from_source(
            "win",
            r#"
                function view(env)
                  env.set_window_size(env.window.id, 640, 480)
                  env.focus_window(env.window.id)
                  return column { width = "grow", height = "grow" }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Slot::root(), &mut ui).unwrap();
        ui.finish();
        assert_eq!(
            core.take_window_commands(),
            vec![
                kui_core::WindowCommand::SetSize {
                    window: WindowId::MAIN,
                    size: Size::new(640.0, 480.0),
                },
                kui_core::WindowCommand::Focus(WindowId::MAIN),
            ]
        );
        assert!(core.take_window_commands().is_empty());
        assert_eq!(core.viewport(), Size::new(400.0, 200.0));
    }

    /// Scrolling from a script: `env.reveal` scrolls a row into view against
    /// the frame the script is building, and `env.set_scroll` /
    /// `env.scroll_offset` write and read the retained offset. Keys are the
    /// integers events carry, so a script reveals the row it got an
    /// `on_click` from.
    #[test]
    fn scripts_reveal_and_move_scroll_offsets() {
        let mut ext = LuaExtension::from_source(
            "scroll",
            r#"
                rows, want, jump, seen = 20, nil, nil, nil
                function view(env)
                  if want then env.reveal(want) end
                  if jump then env.set_scroll(jump[1], jump[2], jump[3]) end
                  want, jump = nil, nil
                  local list = { key = "list", width = "grow", height = "grow",
                                 scroll_y = true }
                  for i = 0, rows - 1 do
                    list[#list + 1] = row { key = "row" .. i, width = "grow",
                                            height = 30, bg = 0x282840ff }
                  end
                  seen = env.scroll_offset(list_key)
                  return column(list)
                end
            "#,
        )
        .unwrap();
        // 20 rows of 30 in a 200-tall window: 400 of overflow.
        let list = Key::ROOT.str("list");
        let row = |i: usize| list.str(&format!("row{i}"));
        ext.lua.globals().set("list_key", list.0 as i64).unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Slot::root(), &mut ui).unwrap();
            ui.finish();
        };

        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list), Vec2::ZERO);

        // A reveal made while the frame is being built resolves against that
        // same frame — the script does not have to wait a frame to see it.
        ext.lua.globals().set("want", row(15).0 as i64).unwrap();
        frame(&mut core, &mut ext);
        let after = core.scroll_offset(list);
        assert!(after.y > 0.0, "reveal moved nothing: {after:?}");
        assert!(after.y <= 15.0 * 30.0, "scrolled past row 15: {after:?}");
        // Spent: the next frame does not drift.
        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list), after);

        // The script reads the offset back (as of the last layout).
        let seen: Table = ext.lua.globals().get("seen").unwrap();
        assert_eq!(seen.get::<f32>("y").unwrap(), after.y);
        assert_eq!(seen.get::<f32>("x").unwrap(), 0.0);

        // set_scroll jumps, and the layout clamps: "to the end", then home.
        let jump = |ext: &LuaExtension, y: f64| {
            let t = ext.lua.create_table().unwrap();
            t.set(1, list.0 as i64).unwrap();
            t.set(2, 0.0).unwrap();
            t.set(3, y).unwrap();
            ext.lua.globals().set("jump", t).unwrap();
        };
        jump(&ext, 1e9);
        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list).y, 400.0);
        jump(&ext, -1e9);
        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list), Vec2::ZERO);

        // A key the frame does not declare is a no-op, and is not kept.
        jump(&ext, 120.0);
        frame(&mut core, &mut ext);
        ext.lua
            .globals()
            .set("want", Key::ROOT.str("ghost").0 as i64)
            .unwrap();
        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list).y, 120.0);
        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list).y, 120.0);
    }

    /// `env.is_pressed(key)` completes the interaction trio next to
    /// `is_hovered` / `is_focused`: held down means the press landed on
    /// this node and the pointer is still on it.
    #[test]
    fn scripts_read_the_pressed_state() {
        let mut ext = LuaExtension::from_source(
            "press",
            r#"
                function view(env)
                  pressed = env.is_pressed(btn_key)
                  hovered = env.is_hovered(btn_key)
                  return column { key = "root", pad = 10,
                    row { key = "btn", width = 100, height = 40,
                          bg = 0x333333ff, on_click = "hit" },
                  }
                end
            "#,
        )
        .unwrap();
        let btn = Key::ROOT.str("root").str("btn");
        ext.lua.globals().set("btn_key", btn.0 as i64).unwrap();
        let mut core = Core::new();
        let pressed = |ext: &LuaExtension| ext.lua.globals().get::<bool>("pressed").unwrap();
        let hovered = |ext: &LuaExtension| ext.lua.globals().get::<bool>("hovered").unwrap();

        frame(&mut core, &mut ext);
        assert!(!pressed(&ext), "idle");

        // Hover alone is not a press.
        core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 30.0)));
        frame(&mut core, &mut ext);
        assert!(hovered(&ext) && !pressed(&ext), "hover is not press");

        core.handle_input(InputEvent::MouseDown {
            button: kui_core::MouseButton::Primary,
            clicks: 1,
        });
        frame(&mut core, &mut ext);
        assert!(pressed(&ext), "held down");

        // The press is stuck to the node it started on, so wandering off a
        // node with no drag releases the pressed look while the button is
        // still down; the release clears it either way.
        core.handle_input(InputEvent::MouseUp {
            button: kui_core::MouseButton::Primary,
        });
        frame(&mut core, &mut ext);
        assert!(!pressed(&ext), "released");
    }

    /// The focus verbs: `env.set_focus(key)` moves focus now,
    /// `env.focus_next` / `env.focus_prev` walk the Tab ring, `env.blur`
    /// drops it — and `env.focus` reads back the node's key, which is a
    /// different fact from `env.focused`, the window's.
    /// The table `view(env)` gets is the documented env reading and
    /// nothing else: its value keys are `schema::ENV_FIELDS`'s Lua
    /// spellings, key for key, under an env with every optional fact
    /// present. The queries and verbs beside them are Lua's own surface —
    /// Node and C spell them as calls on the context — and are pinned here
    /// too, so adding one is a deliberate two-place change.
    #[test]
    fn the_env_table_is_the_documented_env_shape() {
        let mut ext = LuaExtension::from_source(
            "env",
            r#"
                function view(env)
                  values, calls = {}, {}
                  for k, v in pairs(env) do
                    if type(v) == "function" then calls[#calls + 1] = k
                    elseif type(v) == "table" then
                      for wk in pairs(v) do values[#values + 1] = k .. "." .. wk end
                    else values[#values + 1] = k end
                  end
                  return column { key = "root",
                    row { key = "a", focusable = true, width = 50, height = 20 },
                    column { key = "dock", focus_region = true,
                      row { key = "d", focusable = true, width = 50, height = 20 },
                    },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        // Every key that only appears when set: a rate, an accent, a
        // locale, a controls rect, and a focused node (which the ring has
        // to see a frame first) — one inside a region, so the region in
        // effect is a reading too.
        core.env.refresh_hz = Some(60.0);
        core.env.system.accent = Some(kui_core::Color::hex(0x3b82f6ff));
        core.env.system.locale = kui_core::Locale::new("en-US");
        core.env.window.native_controls = Some(Rect::new(0.0, 0.0, 78.0, 28.0));
        frame(&mut core, &mut ext);
        core.set_focus(Some(Key::ROOT.str("root").str("dock").str("d")));
        frame(&mut core, &mut ext);

        let sorted = |name: &str| -> Vec<String> {
            let mut v: Vec<String> = ext.lua.globals().get(name).unwrap();
            v.sort();
            v
        };
        let mut documented: Vec<String> = kui_core::schema::ENV_FIELDS
            .iter()
            .flat_map(|f| f.lua.iter().map(|k| (*k).to_string()))
            .collect();
        // The palette rides in the same reading and is pinned the same
        // way, to `schema::THEME_ROLES` rather than to `ENV_FIELDS` — it
        // is derived from `system` and not reported by the host, so it is
        // not an `Env` field (ADR 0019). The two flat keys beside the
        // roles are the base it came from and the disabled multiplier.
        documented.extend(
            kui_core::schema::THEME_ROLES
                .iter()
                .map(|r| format!("theme.{}", r.name)),
        );
        documented.push("theme.appearance".into());
        documented.push("theme.disabled_opacity".into());
        // And the metrics beside it (backlog T2), pinned to
        // `schema::METRIC_ROLES` the same way.
        documented.extend(
            kui_core::schema::METRIC_ROLES
                .iter()
                .map(|r| format!("metrics.{}", r.name)),
        );
        // And the tokens (ADR 0027): two tables, the app's own names
        // under them, so the keys pinned here are the two halves.
        documented.push("tokens.colors".into());
        documented.push("tokens.lengths".into());
        documented.sort();
        assert_eq!(
            sorted("values"),
            documented,
            "env's value keys and schema::ENV_FIELDS + THEME_ROLES + METRIC_ROLES + tokens disagree"
        );
        assert_eq!(
            sorted("calls"),
            [
                "add_extension",
                "announce",
                "answer_selection_range",
                "blur",
                "caret_rect",
                "clear_selection",
                "close_menu",
                "edit_text",
                "extension_namespaces",
                "focus_next",
                "focus_prev",
                "focus_region",
                "focus_window",
                "is_focused",
                "is_hovered",
                "is_pressed",
                "layout_of",
                "measure_text",
                "open_menu",
                "request_copy",
                "reveal",
                "scroll_geometry",
                "scroll_offset",
                "select_all_in",
                "selection_html",
                "selection_text",
                "set_edit_text",
                "set_focus",
                "set_scroll",
                "set_tokens",
                "set_window_size",
                "text_hit",
            ],
            "env's queries and verbs changed; update env_table's doc too"
        );
    }

    /// Tokens (ADR 0027): the `tokens` global is declared under the
    /// script's origin, a `$name` resolves in a colour, a length, a sizing,
    /// a pad edge, a border and a span, a themed colour follows the
    /// appearance, and `env.tokens` reads the frame's values back.
    #[test]
    fn a_script_declares_tokens_and_references_them_by_name() {
        let mut ext = LuaExtension::from_source(
            "tokens",
            r##"
                tokens = {
                  colors = { peach = "#ffcc99", ink = { light = "#111111", dark = "#eeeeee" } },
                  lengths = { side_w = 132, gap = 6 },
                }
                function view(env)
                  seen = env.tokens
                  return column { pad = "$gap", gap = "$gap",
                    row { key = "a", width = "$side_w", height = 20, bg = "$peach",
                          border = { w = "$gap", color = "$ink" }, radius = "$radius" },
                    row { key = "b", width = 20, height = "$side_w", bg = "$surface",
                          pad = { l = "$gap", r = 2 } },
                    text({ "x", { "y", color = "$peach", bg = "$ink" } }, { size = "$gap", color = "$peach" }),
                  }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        assert!(core.take_warnings().is_empty());
        let peach = Color::hex(0xffcc99ff);
        let paper = Color::hex(0xeeeeeeff);
        let radius = core.metrics().radius;
        let surface = core.theme().surface;
        let dl = core.output().0;
        let a = dl
            .quads
            .iter()
            .find(|q| q.color == peach && q.kind == kui_core::QuadKind::Solid)
            .expect("the peach box");
        assert_eq!(a.rect.w, 132.0, "width from a length token");
        assert_eq!(a.border_w, 6.0);
        assert_eq!(
            a.border_color, paper,
            "the dark half on an unknown appearance"
        );
        assert_eq!(a.radius[0], radius, "$radius is the metric");
        let b = dl
            .quads
            .iter()
            .find(|q| q.color == surface && q.rect.h == 132.0)
            .expect("the $surface box, 132 tall");
        assert_eq!(b.rect.w, 20.0);
        let seen: Table = ext.lua.globals().get("seen").unwrap();
        let colors: Table = seen.get("colors").unwrap();
        let lengths: Table = seen.get("lengths").unwrap();
        assert_eq!(colors.get::<u32>("peach").unwrap(), 0xffcc99ff);
        assert_eq!(colors.get::<u32>("ink").unwrap(), 0xeeeeeeff);
        assert_eq!(lengths.get::<f32>("side_w").unwrap(), 132.0);
        // The light half, without the script changing.
        core.set_system(kui_core::SystemEnv {
            appearance: kui_core::Appearance::Light,
            ..Default::default()
        });
        frame(&mut core, &mut ext);
        let a = core
            .output()
            .0
            .quads
            .iter()
            .find(|q| q.color == peach && q.kind == kui_core::QuadKind::Solid)
            .unwrap();
        assert_eq!(a.border_color, Color::hex(0x111111ff));
    }

    /// A script's table is its own: its `$peach` is the host's until it
    /// declares one, its `grey` never reaches the host, and
    /// `env.set_tokens` from inside a view replaces the script's table for
    /// the nodes after the call.
    #[test]
    fn a_scripts_tokens_sit_over_the_hosts() {
        let mut ext = LuaExtension::from_source(
            "guest",
            r##"
                tokens = { colors = { grey = "#808080" } }
                function view(env)
                  first = { peach = env.tokens.colors.peach, grey = env.tokens.colors.grey }
                  if redeclare then
                    env.set_tokens { colors = { peach = "#ffaa77" }, lengths = { w = 40 } }
                  end
                  second = { peach = env.tokens.colors.peach, grey = env.tokens.colors.grey }
                  return column { row { key = "a", width = redeclare and "$w" or 10, height = 10, bg = "$peach" } }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_tokens(kui_core::Tokens::new().color("peach", Color::hex(0xffcc99ff)));
        frame(&mut core, &mut ext);
        let first: Table = ext.lua.globals().get("first").unwrap();
        assert_eq!(
            first.get::<u32>("peach").unwrap(),
            0xffcc99ff,
            "the host's peach"
        );
        assert_eq!(
            first.get::<u32>("grey").unwrap(),
            0x808080ff,
            "its own grey"
        );
        assert!(
            core.token_lookup().color("grey").is_err(),
            "the guest's grey is not the host's"
        );
        assert!(core.take_warnings().is_empty());

        ext.lua.globals().set("redeclare", true).unwrap();
        frame(&mut core, &mut ext);
        let second: Table = ext.lua.globals().get("second").unwrap();
        assert_eq!(
            second.get::<u32>("peach").unwrap(),
            0xffaa77ff,
            "its own peach now"
        );
        assert!(
            second.get::<Option<u32>>("grey").unwrap().is_none(),
            "replaced whole"
        );
        let a = core
            .output()
            .0
            .quads
            .iter()
            .find(|q| q.color == Color::hex(0xffaa77ff))
            .expect("painted the script's peach");
        assert_eq!(a.rect.w, 40.0);
        assert_eq!(
            core.token_lookup().color("peach"),
            Ok(Color::hex(0xffcc99ff)),
            "the host's table did not move"
        );
    }

    /// A `$name` nothing declared warns once and leaves the slot at its
    /// default; a declared role name warns at the declaration.
    #[test]
    fn a_missing_token_warns_and_paints_nothing() {
        let mut ext = LuaExtension::from_source(
            "missing",
            r##"
                tokens = { colors = { surface = "#ff0000", peach = "#ffcc99" }, lengths = { gap = 6 } }
                function view(env)
                  return column {
                    row { key = "a", width = "$gap", height = 10, bg = "$peech", pad = "$peach" },
                    row { key = "b", width = "$peach", height = 10, bg = "$gap" },
                    text("still here", { color = "$peech", size = "$gap" }),
                  }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let ws = core.take_warnings();
        let mut codes: Vec<&str> = ws.iter().map(|w| w.code).collect();
        codes.sort();
        assert_eq!(
            codes,
            [
                kui_core::diag::RESERVED_TOKEN,
                kui_core::diag::UNKNOWN_TOKEN,
                kui_core::diag::UNKNOWN_TOKEN,
                kui_core::diag::UNKNOWN_TOKEN,
            ],
            "surface refused once; peech, peach-as-length and gap-as-colour once each: {ws:?}"
        );
        assert!(
            ws.iter().any(|w| w
                .message
                .contains("`$gap` is a length token, and this slot takes a color")),
            "{ws:?}"
        );
        // No solid quad was painted: both bgs were left out. The text with
        // the mistyped colour is still there, in the theme's foreground and
        // at the declared size — a typo hides nothing.
        let fg = core.theme().fg;
        let dl = core.output().0;
        assert!(
            dl.quads
                .iter()
                .all(|q| q.kind != kui_core::QuadKind::Solid || q.color.a == 0.0)
        );
        let glyphs: Vec<_> = dl
            .quads
            .iter()
            .filter(|q| {
                matches!(
                    q.kind,
                    kui_core::QuadKind::GlyphMask | kui_core::QuadKind::GlyphSubpixel
                )
            })
            .collect();
        assert!(!glyphs.is_empty(), "the text painted");
        assert!(glyphs.iter().all(|q| q.color == fg), "in the foreground");
        assert!(glyphs[0].rect.h > 3.0, "at the declared 6 px size, not 0");
    }

    #[test]
    fn scripts_move_focus_with_the_verbs() {
        let mut ext = LuaExtension::from_source(
            "focus",
            r#"
                function view(env)
                  if cmd == "set" then env.set_focus(target)
                  elseif cmd == "blur" then env.blur()
                  elseif cmd == "next" then env.focus_next()
                  elseif cmd == "prev" then env.focus_prev() end
                  cmd = nil
                  -- `env.focus` is a value the host filled in before view()
                  -- ran, so it lags a verb called above by a frame;
                  -- `env.is_focused` is a call and answers about now.
                  seen_focus = env.focus
                  seen_live = env.is_focused(target or 0)
                  seen_window = env.focused
                  return column { key = "root", pad = 10,
                    row { key = "a", focusable = true, width = 50, height = 20 },
                    row { key = "b", focusable = true, width = 50, height = 20 },
                    row { key = "c", focusable = true, width = 50, height = 20 },
                  }
                end
            "#,
        )
        .unwrap();
        let root = Key::ROOT.str("root");
        let (a, b, c) = (root.str("a"), root.str("b"), root.str("c"));
        let mut core = Core::new();
        let cmd = |ext: &LuaExtension, c: &str| ext.lua.globals().set("cmd", c).unwrap();
        let seen = |ext: &LuaExtension| ext.lua.globals().get::<Option<i64>>("seen_focus").unwrap();
        let live = |ext: &LuaExtension| ext.lua.globals().get::<bool>("seen_live").unwrap();

        // The ring is the last finished frame's, so build one first.
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), None);
        assert_eq!(seen(&ext), None, "nil for no focus, not 0");

        // Straight to a node by key, the imperative form of `key_focus`.
        ext.lua.globals().set("target", b.0 as i64).unwrap();
        cmd(&ext, "set");
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), Some(b));
        // `env.focus` is a snapshot the host wrote before view() ran, so it
        // still holds what focus was when the frame opened; `env.is_focused`
        // is a query into the live frame and sees the move at once.
        assert_eq!(seen(&ext), None, "the value lags a same-frame verb");
        assert!(live(&ext), "the query does not");
        frame(&mut core, &mut ext);
        assert_eq!(
            seen(&ext),
            Some(b.0 as i64),
            "and the next frame carries it"
        );

        // Tab and Shift-Tab, wrapping.
        cmd(&ext, "next");
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), Some(c));
        cmd(&ext, "next");
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), Some(a), "wraps");
        cmd(&ext, "prev");
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), Some(c), "wraps back");

        cmd(&ext, "blur");
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), None);
        frame(&mut core, &mut ext);
        assert_eq!(seen(&ext), None);

        // By label: the node's own `key` string, with no event from it
        // first (F5). `c` was never clicked, tabbed to or reported.
        ext.lua.globals().set("target", "c").unwrap();
        cmd(&ext, "set");
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), Some(c), "set_focus(\"c\") resolves the label");
        assert!(live(&ext), "and is_focused(\"c\") answers about it");
        assert!(core.take_warnings().is_empty());
        // A label nothing declares is an error that names both spellings.
        ext.lua.globals().set("target", "nope").unwrap();
        cmd(&ext, "set");
        let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
        let e = ext.view(&Slot::root(), &mut ui).unwrap_err().to_string();
        ui.finish();
        assert!(
            e.contains("no node is keyed \"nope\"") && e.contains("integer key"),
            "{e}"
        );

        // `env.focused` is the window's focus, not the node's: it stays a
        // bool through all of the above, and follows the host env instead.
        assert!(ext.lua.globals().get::<bool>("seen_window").unwrap());
    }

    /// Two nodes on one label under different parents: the first in tree
    /// order is the one focused, and the frame says so once.
    #[test]
    fn a_shared_label_resolves_to_the_first_and_warns() {
        let mut ext = LuaExtension::from_source(
            "dup",
            r#"
                function view(env)
                  if go then env.set_focus("item"); go = nil end
                  return column { key = "root",
                    row { row { key = "item", focusable = true, width = 50, height = 20 } },
                    row { row { key = "item", focusable = true, width = 50, height = 20 } },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        ext.lua.globals().set("go", true).unwrap();
        frame(&mut core, &mut ext);
        assert_eq!(
            core.focus(),
            Some(Key::ROOT.str("root").index(0).str("item")),
            "the first, under the auto-keyed first row"
        );
        let ws = core.take_warnings();
        assert_eq!(ws.len(), 1, "{ws:?}");
        assert_eq!(ws[0].code, kui_core::diag::AMBIGUOUS_KEY);
        assert!(ws[0].message.contains("\"item\""), "{}", ws[0].message);
    }

    /// A script opens a context menu with `env.open_menu` and hears the
    /// chosen row as a `menu` event on the node it named (ADR 0017).
    #[test]
    fn scripts_open_a_menu_and_hear_the_row() {
        let mut ext = LuaExtension::from_source(
            "menu",
            r#"
                frames = 0
                function view(env)
                  frames = frames + 1
                  if frames == 2 then
                    opened = env.open_menu("card", 40, 30, {
                      { role = "copy" },
                      { role = "separator" },
                      -- The wire spelling the event reports, and the
                      -- snake one a script may have learned first.
                      { role = "selectAll" },
                      { role = "look_up" },
                      { label = "Wrap", checked = true },
                      { label = "Inspect", id = "inspect" },
                    })
                  end
                  return column {
                    column { key = "card", selectable = true, text("one", { size = 14 }) },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Slot::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        assert!(ext.lua.globals().get::<bool>("opened").unwrap());
        let roles: Vec<_> = core
            .menu()
            .expect("open")
            .items
            .iter()
            .map(|i| (i.role, i.checked))
            .collect();
        assert_eq!(
            roles,
            [
                (kui_core::MenuRole::Copy, false),
                (kui_core::MenuRole::Separator, false),
                (kui_core::MenuRole::SelectAll, false),
                (kui_core::MenuRole::LookUp, false),
                (kui_core::MenuRole::Custom, true),
                (kui_core::MenuRole::Custom, false),
            ]
        );
        frame(&mut core, &mut ext);
        // The row the tree reports is the row the pointer can press, and
        // the checked one reads as checked there.
        let tree = core.access_tree();
        assert_eq!(
            tree.nodes
                .iter()
                .find(|n| n.name.as_deref() == Some("Wrap"))
                .expect("the checked row")
                .checked,
            Some(true)
        );
        let row = tree
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("Inspect"))
            .expect("the menu drew")
            .rect;
        let at = kui_core::Vec2::new(row.x + row.w / 2.0, row.y + row.h / 2.0);
        core.handle_input(InputEvent::CursorMoved(at));
        core.handle_input(InputEvent::mouse_down(1));
        let events = core.handle_input(InputEvent::mouse_up());
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(
            events[0].payload.get("item").and_then(Value::as_str),
            Some("inspect")
        );
        assert_eq!(
            events[0].origin,
            OriginId(1),
            "posted with the origin that asked for the menu"
        );
    }

    /// A `selectable` container scopes one selection over the runs inside
    /// it, and a script reads it back by the scope's label (ADR 0017).
    #[test]
    fn scripts_select_and_read_a_scope() {
        let mut ext = LuaExtension::from_source(
            "sel",
            r#"
                frames = 0
                function view(env)
                  frames = frames + 1
                  if frames > 1 then
                    before = env.selection_text()
                    took = env.select_all_in("card")
                    text_out = env.selection_text()
                    not_a_scope = env.select_all_in("plain")
                  end
                  return column {
                    column { key = "card", selectable = true,
                      text("one", { size = 14 }),
                      text("two", { size = 14 }) },
                    row { key = "plain", width = 10, height = 10 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Slot::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let g = ext.lua.globals();
        assert!(matches!(
            g.get::<mlua::Value>("before").unwrap(),
            mlua::Value::Nil
        ));
        assert!(g.get::<bool>("took").unwrap());
        assert_eq!(g.get::<String>("text_out").unwrap(), "one\ntwo");
        assert!(
            !g.get::<bool>("not_a_scope").unwrap(),
            "a node that drew no text is not a scope"
        );
    }

    /// A script turns a click into a caret with `env.text_hit` and a caret
    /// into a rect with `env.caret_rect` (backlog C18), both by the `line`
    /// row's label and across the runs inside it — answered, mid-build,
    /// from the frame that finished.
    #[test]
    fn scripts_map_points_to_bytes_on_a_line_of_runs() {
        let mut ext = LuaExtension::from_source(
            "hit",
            r#"
                frames = 0
                function view(env)
                  local mono = { size = 14, family = "mono" }
                  local w = env.measure_text("M", mono, 0).width
                  frames = frames + 1
                  -- A label nothing has declared yet is an error by name
                  -- (F5), so the first frame declares and the next asks.
                  if frames > 1 then
                    hit = env.text_hit("line", 7.2 * w, 5)
                    seam = env.caret_rect("line", 4)
                    far = env.text_hit("line", 390, 5)
                    none = env.caret_rect("plain", 0)
                  end
                  cell = w
                  return column {
                    row { key = "line",
                      text("let ", mono),
                      row { bg = 0x3b5bd455, text("value", mono) },
                      text(" = 1;", mono) },
                    row { key = "plain", width = 10, height = 10 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Slot::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        let hit: mlua::Value = ext.lua.globals().get("hit").unwrap();
        assert!(
            matches!(hit, mlua::Value::Nil),
            "nothing drawn before the first frame"
        );
        frame(&mut core, &mut ext);
        let hit: mlua::Table = ext.lua.globals().get("hit").unwrap();
        assert_eq!(
            hit.get::<usize>("byte").unwrap(),
            7,
            "the fourth cell of \"value\""
        );
        assert_eq!(hit.get::<u32>("line").unwrap(), 0);
        let cell: f32 = ext.lua.globals().get("cell").unwrap();
        let seam: mlua::Table = ext.lua.globals().get("seam").unwrap();
        let x: f32 = seam.get("x").unwrap();
        assert!((x - 4.0 * cell).abs() < 0.75, "{x} vs {}", 4.0 * cell);
        assert_eq!(seam.get::<f32>("w").unwrap(), 0.0);
        let far: mlua::Table = ext.lua.globals().get("far").unwrap();
        // 14 is also what pins the prelude's `text` copying its options:
        // the three runs share one `mono` table, and before the copy they
        // were one table holding the last string, so the line was three
        // times " = 1;" and 15 long.
        assert_eq!(
            far.get::<usize>("byte").unwrap(),
            14,
            "the end, across the runs"
        );
        let none: mlua::Value = ext.lua.globals().get("none").unwrap();
        assert!(matches!(none, mlua::Value::Nil), "a node that drew no text");
    }

    /// `cells { lines=, runs= }` is a terminal's screen as one node
    /// (backlog C20): the rows draw, a run colours its span, and the
    /// access tree reads the screen back.
    #[test]
    fn scripts_draw_a_screen_of_cells() {
        let mut ext = LuaExtension::from_source(
            "term",
            r#"
                function view(env)
                  return column { cells { key = "term", rows = 2, cols = 11, size = 14, family = "mono",
                    lines = { "hello world", "  bye" },
                    runs = { { 1, 2, 3, 0xff0000ff, 0x0000ffff, 1 } },
                    cursor_at = { 1, 4 }, cursor_shape = "underline" } }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Slot::root(), &mut ui).unwrap();
        ui.finish();
        let (dl, _) = core.output();
        let glyphs = dl
            .quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::GlyphMask)
            .count();
        assert_eq!(glyphs, 13, "hello world + bye");
        let tree = core.access_tree();
        let term = tree
            .nodes
            .iter()
            .find(|n| n.role == kui_core::Role::Terminal)
            .expect("a terminal node");
        assert_eq!(term.value.as_deref(), Some("hello world\n  bye"));
    }

    /// A `selectable` grid selects in cells, and the click count picks the
    /// grain — one a cell, two the word, three the whole row (ADR 0017,
    /// decision 4). Lua declares the scope and reads the result back
    /// through `env.selection_text`; the gesture itself is the core's, so
    /// what this pins is that a Lua-declared grid is a scope at all.
    #[test]
    fn a_lua_grid_selects_by_cell_word_and_row() {
        let mut ext = LuaExtension::from_source(
            "term",
            r#"
                function view(env)
                  said = env.selection_text()
                  return column { cells { key = "term", rows = 2, cols = 12,
                    size = 14, family = "mono", line_height = 20,
                    origin_line = 900, selectable = true,
                    lines = { "hello world", "bye there" } } }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Slot::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        let said = |ext: &LuaExtension| ext.lua.globals().get::<Option<String>>("said").unwrap();

        // The middle of (row, col), from the grid's own metrics.
        let w = core
            .measure_text(
                "M",
                &kui_core::TextStyle::new(14.0)
                    .family(kui_core::FontFamily::Mono)
                    .line_height(20.0),
                None,
            )
            .width;
        let at = |r: usize, c: usize| Vec2::new(w * (c as f32 + 0.5), 20.0 * (r as f32 + 0.5));
        let click = |core: &mut Core, p: Vec2, clicks: u8| {
            core.handle_input(InputEvent::CursorMoved(p));
            core.handle_input(InputEvent::MouseDown {
                button: kui_core::MouseButton::Primary,
                clicks,
            });
            core.handle_input(InputEvent::MouseUp {
                button: kui_core::MouseButton::Primary,
            });
        };

        click(&mut core, at(0, 8), 2);
        frame(&mut core, &mut ext);
        assert_eq!(said(&ext).as_deref(), Some("world"), "a double click");

        click(&mut core, at(1, 1), 3);
        frame(&mut core, &mut ext);
        assert_eq!(said(&ext).as_deref(), Some("bye there"), "a triple click");
    }

    /// A span's `underline`, `strikethrough` and `bg` reach the core
    /// (backlog C22): the frame carries the solid quads beside the glyphs.
    #[test]
    fn spans_carry_their_decorations() {
        let mut ext = LuaExtension::from_source(
            "deco",
            r#"
                function view(env)
                  return row { text({ "let ", { "value", bg = 0x3b5bd455, underline = true },
                                      " = 1;" }, { size = 14, family = "mono" }) }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Slot::root(), &mut ui).unwrap();
        ui.finish();
        let (dl, _) = core.output();
        let solids = dl
            .quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::Solid)
            .count();
        assert_eq!(solids, 2, "a background and an underline");
    }

    /// `features = "liga=0"` reaches the shaper through the same schema
    /// row every binding reads (backlog C23): a text with it and one
    /// without are shaped twice.
    #[test]
    fn features_are_a_text_option() {
        let mut ext = LuaExtension::from_source(
            "features",
            r#"
                function view(env)
                  return row { text("fi ->", { size = 16 }),
                               text("fi ->", { size = 16, features = "liga=0 calt=0" }) }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Slot::root(), &mut ui).unwrap();
        ui.finish();
        assert_eq!(core.text_cache_len(), 2);
    }

    /// `text(s, opts)` copies its options. It used to write `type` and
    /// `value` into the table it was handed and return it, so a script
    /// that hoisted a style — `local mono = { size = 14 }` — and passed it
    /// to three texts built one table three times, showing the last string
    /// thrice. Found by the text-hit test above (backlog C18).
    #[test]
    fn a_style_table_shared_by_three_texts_is_three_texts() {
        let mut ext = LuaExtension::from_source(
            "shared",
            r#"
                local mono = { size = 14, family = "mono" }
                function view(env)
                  return row { text("a", mono), text("bb", mono), text("ccc", mono) }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Slot::root(), &mut ui).unwrap();
        ui.finish();
        let names: Vec<String> = core
            .access_tree()
            .nodes
            .iter()
            .filter_map(|n| n.value.clone().or_else(|| n.name.clone()))
            .collect();
        let joined = names.join("|");
        assert!(
            joined.contains("a") && joined.contains("bb") && joined.contains("ccc"),
            "three different texts, got {joined:?}"
        );
        assert_eq!(
            core.text_cache_len(),
            3,
            "three strings shaped, not one three times"
        );
    }

    /// A script virtualizes a 10k-row list with nothing but
    /// `env.scroll_geometry`: it declares the rows crossing the window and
    /// two spacers holding the space of the rest, so the frame costs a
    /// screenful and the scrollbar still spans the whole list.
    #[test]
    fn scripts_slice_a_long_list_from_the_geometry() {
        let mut ext = LuaExtension::from_source(
            "virtual",
            r#"
                ROWS, ROW_H, built = 10000, 30, 0
                function view(env)
                  local g = env.scroll_geometry(list_key)
                  -- No layout yet: fall back to the window for one frame.
                  local h = g and g.h or 200
                  local top = g and g.offset.y or 0
                  local first = math.min(ROWS, math.max(0, math.floor(top / ROW_H)))
                  local last = math.min(ROWS, math.ceil((top + h) / ROW_H))
                  local list = { key = "list", width = "grow", height = "grow",
                                 scroll_y = true }
                  if first > 0 then
                    list[#list + 1] = row { key = "lead", width = "grow",
                                            height = first * ROW_H }
                  end
                  for i = first, last - 1 do
                    list[#list + 1] = row { key = "row" .. i, width = "grow",
                                            height = ROW_H, bg = 0x282840ff }
                  end
                  if last < ROWS then
                    list[#list + 1] = row { key = "tail", width = "grow",
                                            height = (ROWS - last) * ROW_H }
                  end
                  built = last - first
                  return column(list)
                end
            "#,
        )
        .unwrap();
        let list = Key::ROOT.str("list");
        ext.lua.globals().set("list_key", list.0 as i64).unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Slot::root(), &mut ui).unwrap();
            ui.finish();
        };

        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let built: i64 = ext.lua.globals().get("built").unwrap();
        assert_eq!(built, 7, "200 / 30 rounded up");

        // The spacers make it the same list: full travel, and "jump to the
        // end" lands on the last row even though it was never built.
        let g = core.scroll_geometry(list).expect("laid out");
        assert_eq!(g.content.h, 10_000.0 * 30.0);
        core.set_scroll(list, Vec2::new(0.0, 1e9));
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list).y, 10_000.0 * 30.0 - 200.0);
        let built: i64 = ext.lua.globals().get("built").unwrap();
        assert_eq!(built, 7, "still a screenful at the far end");
    }

    /// The same list as one call: `virtual_column` from the prelude owns the
    /// slicing, the two spacers and the row keys, and the script says what a
    /// row looks like. It names its container by label, which is why the
    /// queries had to answer for a name no frame has declared yet — the
    /// first frame asks before the container exists (backlog C25).
    #[test]
    fn the_prelude_virtualizes_a_long_list_in_one_call() {
        let mut ext = LuaExtension::from_source(
            "virtual",
            r#"
                ROWS, ROW_H, first_built, built = 10000, 30, -1, 0
                function view(env)
                  built, first_built = 0, -1
                  return virtual_column(env,
                    { key = "list", rows = ROWS, row_h = ROW_H,
                      width = "grow", height = "grow" },
                    function(i)
                      built = built + 1
                      if first_built < 0 then first_built = i end
                      return column { fill = true, bg = 0x282840ff,
                                      on_click = { kind = "pick", row = i } }
                    end)
                end
            "#,
        )
        .unwrap();
        let list = Key::ROOT.str("list");
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Slot::root(), &mut ui).unwrap();
            ui.finish();
        };

        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let built: i64 = ext.lua.globals().get("built").unwrap();
        assert_eq!(built, 9, "200 / 30 rounded up, plus two rows of overscan");

        // The spacers make it the whole list, and a row keeps the key its
        // data index gives it however far the range has slid.
        let g = core.scroll_geometry(list).expect("laid out");
        assert_eq!(g.content.h, 10_000.0 * 30.0);
        core.set_scroll(list, Vec2::new(0.0, 300.0 * 30.0));
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let first: i64 = ext.lua.globals().get("first_built").unwrap();
        assert_eq!(first, 298, "two rows of overscan above row 300");
        let built: i64 = ext.lua.globals().get("built").unwrap();
        assert_eq!(built, 11, "a screenful with overscan on both sides now");
        // The row's own node carries the data index, which is the key a list
        // that built all ten thousand would have given it.
        assert!(
            core.access_tree()
                .nodes
                .iter()
                .any(|n| n.key == list.index(300).index(0)),
            "row 300 is not keyed by its data index"
        );
    }

    /// The same clamp as the JSX widget's: a list that shrank while scrolled
    /// slices past its own new end, and an unclamped `first` builds a lead
    /// spacer taller than the whole list with no rows in it.
    #[test]
    fn a_virtual_column_whose_list_shrank_lands_in_one_frame() {
        let mut ext = LuaExtension::from_source(
            "virtual",
            r#"
                ROWS, first_built = 200, -1
                function view(env)
                  first_built = -1
                  return virtual_column(env,
                    { key = "list", rows = ROWS, row_h = 20,
                      width = "grow", height = "grow" },
                    function(i)
                      if first_built < 0 then first_built = i end
                      return column { fill = true, bg = 0x282840ff }
                    end)
                end
            "#,
        )
        .unwrap();
        let list = Key::ROOT.str("list");
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Slot::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        core.set_scroll(list, Vec2::new(0.0, 3_000.0));
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let deep: i64 = ext.lua.globals().get("first_built").unwrap();
        assert!(
            deep > 100,
            "expected to be deep in the list, built from {deep}"
        );

        ext.lua.globals().set("ROWS", 10).unwrap();
        frame(&mut core, &mut ext);
        let g = core.scroll_geometry(list).expect("laid out");
        assert_eq!(
            g.content.h,
            10.0 * 20.0,
            "the content is the list it has now"
        );
        frame(&mut core, &mut ext);
        let first: i64 = ext.lua.globals().get("first_built").unwrap();
        assert_eq!(first, 0, "and every row of it is built");
        assert_eq!(core.scroll_offset(list).y, 0.0);
    }

    /// A query answers for a label nothing declared, where a command says it
    /// is a name nothing answers to. The first frame of any view that slices
    /// by geometry asks before its container exists.
    #[test]
    fn a_query_answers_for_an_undeclared_label_and_a_command_refuses() {
        let mut ext = LuaExtension::from_source(
            "queries",
            r#"
                function view(env)
                  geom = env.scroll_geometry("nothing")
                  off = env.scroll_offset("nothing")
                  hovered = env.is_hovered("nothing")
                  pressed = env.is_pressed("nothing")
                  focused = env.is_focused("nothing")
                  hit = env.text_hit("nothing", 1, 1)
                  caret = env.caret_rect("nothing", 0)
                  refused = not pcall(function() env.set_scroll("nothing", 0, 0) end)
                  refused_focus = not pcall(function() env.set_focus("nothing") end)
                  return column { width = "grow", height = "grow" }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        ext.view(&Slot::root(), &mut ui).unwrap();
        ui.finish();
        let g = ext.lua.globals();
        assert_eq!(g.get::<mlua::Value>("geom").unwrap(), mlua::Value::Nil);
        assert_eq!(g.get::<mlua::Value>("hit").unwrap(), mlua::Value::Nil);
        assert_eq!(g.get::<mlua::Value>("caret").unwrap(), mlua::Value::Nil);
        assert!(!g.get::<bool>("hovered").unwrap());
        assert!(!g.get::<bool>("pressed").unwrap());
        assert!(!g.get::<bool>("focused").unwrap());
        let off: Table = g.get("off").unwrap();
        assert_eq!(off.get::<f32>("y").unwrap(), 0.0);
        assert!(
            g.get::<bool>("refused").unwrap(),
            "set_scroll named nothing"
        );
        assert!(g.get::<bool>("refused_focus").unwrap());
    }

    #[test]
    fn bad_script_reports_error_not_panic() {
        let mut ext = LuaExtension::from_source("bad", "function view() return 5 end").unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        assert!(ext.view(&Slot::root(), &mut ui).is_err());
    }
}
