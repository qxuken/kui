//! Lua extensions for kui. A script defines `view(env)` returning a plain
//! table tree (built with the injected `row`/`column`/`text`/`button`
//! prelude) and optionally `on_event(ev)`. `env` carries host facts
//! (refresh rate, focus, viewport), queries (`env.edit_text(key)`,
//! `env.is_focused(key)`, `env.is_hovered(key)`, `env.is_pressed(key)`,
//! `env.measure_text(s, opts, max_w)`), focus verbs (`env.set_focus(key)`,
//! `env.blur()`, `env.focus_next()`, `env.focus_prev()`),
//! `env.announce(text, politeness)` and scroll calls
//! (`env.reveal(key)`, `env.scroll_offset(key)`, `env.set_scroll(key, x, y)`,
//! `env.scroll_geometry(key)`) and window requests
//! (`env.set_window_size(window, w, h)`, `env.focus_window(window)`); the
//! root table may set `window_title`. Because the IR is data all the way down, the binding is
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
//! Events arrive as their payload table plus `node_key` (the emitting
//! node's key as an integer), which is what `env.edit_text` takes.

use kui_core::schema::{self, Kind, Parsed, PropsOut};
use kui_core::{
    Align, Color, EditOptions, Extension, FloatConfig, Key, PadShorthand, Sizing, Span, Ui,
    UiEvent, Value, WindowConfig, WindowKind, widgets,
};
use mlua::{Lua, Table};

const PRELUDE: &str = include_str!("prelude.lua");

pub struct LuaExtension {
    lua: Lua,
    name: String,
}

impl LuaExtension {
    pub fn from_source(name: impl Into<String>, source: &str) -> mlua::Result<Self> {
        let lua = Lua::new();
        lua.load(PRELUDE).set_name("kui:prelude").exec()?;
        let name = name.into();
        lua.load(source).set_name(&name).exec()?;
        Ok(Self { lua, name })
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

    fn view(&mut self, ui: &mut Ui<'_>) -> Result<(), String> {
        let view: mlua::Function = self
            .lua
            .globals()
            .get("view")
            .map_err(|_| "script defines no view()".to_string())?;
        // The env's query functions borrow the frame for the duration of
        // view(); the returned table outlives the scope, the borrow does
        // not. A RefCell because measurement shapes text (a mutable query)
        // while the rest only read; Lua calls them one at a time.
        let root: Table = {
            let frame = std::cell::RefCell::new(&mut *ui);
            self.lua
                .scope(|scope| {
                    let env = env_table(&self.lua, scope, &frame)?;
                    view.call(env)
                })
                .map_err(|e| format!("view(): {e}"))?
        };
        // The root table may declare host state alongside the tree.
        if let Ok(Some(title)) = root.get::<Option<String>>("window_title") {
            ui.window_title(&title);
        }
        declare_windows(ui, &root).map_err(|e| format!("windows: {e}"))?;
        build_node(ui, &root).map_err(|e| format!("view table: {e}"))
    }

    fn on_event(&mut self, ev: &UiEvent) {
        let Ok(f) = self.lua.globals().get::<mlua::Function>("on_event") else {
            return;
        };
        let payload = value_to_lua(&self.lua, &ev.payload).and_then(|p| {
            // Map payloads learn which node emitted them; edit widgets emit
            // {kind="changed"|"submit"} and scripts read the text back with
            // env.edit_text(ev.node_key).
            if let mlua::Value::Table(t) = &p
                && !t.contains_key("node_key")?
            {
                t.set("node_key", ev.key.0 as i64)?;
            }
            Ok(p)
        });
        if let Err(e) = payload.and_then(|p| f.call::<()>(p)) {
            eprintln!("kui-lua: '{}' on_event error: {e}", self.name);
        }
    }
}

/// Host facts handed to `view(env)`, the reading `schema::ENV_FIELDS`
/// documents and `the_env_table_is_the_documented_env_shape` pins to it key
/// for key: `refresh_hz` (nil if unknown),
/// `frame_budget_ms`, `focused` (the *window*'s keyboard focus, a bool),
/// `focus` (the focused *node*'s key), `focus_visible`,
/// `viewport_w`/`viewport_h` (logical px), `window` chrome facts, the
/// queries `edit_text(key)`, `is_focused(key)`, `is_hovered(key)`,
/// `is_pressed(key)` (keys are the integers events carry),
/// `measure_text(s, opts, max_w)` (see `measure_from_lua`), the focus verbs
/// `set_focus(key)` / `blur()` / `focus_next()` / `focus_prev()` and the
/// scroll calls `reveal(key)` / `scroll_offset(key)` / `set_scroll(key, x, y)` /
/// `scroll_geometry(key)` and the window requests `set_window_size(window,
/// w, h)` / `focus_window(window)`.
fn env_table<'scope, 'env: 'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, 'env>,
    ui: &'env std::cell::RefCell<&'env mut Ui<'_>>,
) -> mlua::Result<Table> {
    let (env, vp, focus, focus_visible) = {
        let ui = ui.borrow();
        (ui.env(), ui.viewport(), ui.focused(), ui.focus_visible())
    };
    let t = lua.create_table()?;
    if let Some(hz) = env.refresh_hz {
        t.set("refresh_hz", hz)?;
    }
    t.set("frame_budget_ms", env.frame_budget_ms())?;
    // Two different facts, one letter apart. `focused` is the *window*'s:
    // does this window have the keyboard at all. `focus` is the focused
    // *node*'s key (an integer, as events carry them; nil for none) — what
    // Node spells `ctx.focused()` and C `kui_focused`. Lua cannot converge
    // on that name because `focused` is the window fact here, and has been
    // since env existed; see the module doc.
    t.set("focused", env.focused)?;
    if let Some(k) = focus {
        t.set("focus", k.0 as i64)?;
    }
    // Whether focus shows — it got there by Tab or assistive technology
    // rather than a click.
    t.set("focus_visible", focus_visible)?;
    t.set("viewport_w", vp.w)?;
    t.set("viewport_h", vp.h)?;
    let win = env.window;
    let wt = lua.create_table()?;
    // Which window this frame is drawing; 0 until a second one is declared.
    wt.set("id", win.id.0)?;
    wt.set("custom_chrome", win.custom_chrome)?;
    wt.set("maximized", win.maximized)?;
    wt.set("fullscreen", win.fullscreen)?;
    if let Some(r) = win.native_controls {
        // Keep-out extent of OS-drawn controls (macOS traffic lights).
        wt.set("controls_w", r.x + r.w)?;
        wt.set("controls_h", r.y + r.h)?;
    }
    t.set("window", wt)?;
    t.set(
        "edit_text",
        scope.create_function(move |_, key: i64| Ok(ui.borrow().edit_text(Key(key as u64))))?,
    )?;
    t.set(
        "is_focused",
        scope.create_function(move |_, key: i64| Ok(ui.borrow().is_focused(Key(key as u64))))?,
    )?;
    t.set(
        "is_hovered",
        scope.create_function(move |_, key: i64| Ok(ui.borrow().is_hovered(Key(key as u64))))?,
    )?;
    // Held down: the press started on this node and the pointer is still
    // over it (or it captured a drag). Goes with `is_hovered` — a script
    // that draws its own button styles the pressed state from this.
    t.set(
        "is_pressed",
        scope.create_function(move |_, key: i64| Ok(ui.borrow().is_pressed(Key(key as u64))))?,
    )?;
    // Moving focus from the script, the imperative half of `key_focus`.
    // `set_focus`, not `focus`: `env.focus` is already the reading above
    // and alpha.5 shipped it, so the verb takes the longer name rather
    // than change what a name means under a script that already runs.
    // `blur` / `focus_next` / `focus_prev` match Node and C exactly.
    t.set(
        "set_focus",
        scope.create_function(move |_, key: i64| {
            ui.borrow_mut().focus(Key(key as u64));
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
        scope.create_function(move |_, key: i64| {
            ui.borrow_mut().reveal(Key(key as u64));
            Ok(())
        })?,
    )?;
    // `{x, y}` as the last layout clamped it (positive = content moved up
    // / left); zeroes for a node that never scrolled. Stash it and hand it
    // back to `set_scroll` to restore a position.
    t.set(
        "scroll_offset",
        scope.create_function(move |lua, key: i64| {
            let off = ui.borrow().scroll_offset(Key(key as u64));
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
    t.set(
        "scroll_geometry",
        scope.create_function(move |lua, key: i64| {
            let Some(g) = ui.borrow().scroll_geometry(Key(key as u64)) else {
                return Ok(mlua::Value::Nil);
            };
            let r = lua.create_table()?;
            r.set("x", g.rect.x)?;
            r.set("y", g.rect.y)?;
            r.set("w", g.rect.w)?;
            r.set("h", g.rect.h)?;
            r.set("content_w", g.content.w)?;
            r.set("content_h", g.content.h)?;
            let off = lua.create_table()?;
            off.set("x", g.offset.x)?;
            off.set("y", g.offset.y)?;
            r.set("offset", off)?;
            let max = lua.create_table()?;
            max.set("x", g.max_offset.x)?;
            max.set("y", g.max_offset.y)?;
            r.set("max_offset", max)?;
            Ok(mlua::Value::Table(r))
        })?,
    )?;
    // The wheel's move by hand; the next layout clamps it, so 0,0 is "jump
    // to the top" and a huge y is "jump to the end".
    t.set(
        "set_scroll",
        scope.create_function(move |_, (key, x, y): (i64, f32, f32)| {
            ui.borrow_mut()
                .set_scroll(Key(key as u64), kui_core::Vec2::new(x, y));
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
                let r = lua.create_table()?;
                r.set("width", m.width)?;
                r.set("height", m.height)?;
                r.set("lines", m.lines)?;
                Ok(r)
            },
        )?,
    )?;
    Ok(t)
}

/// `env.measure_text(s, opts, max_w)` → `{ width, height, lines }` (logical
/// px): what layout would give a text node with that content and style,
/// wrapped to `max_w` when given. `s` is a string, a span list (the same
/// shape `text({...})` takes) or a whole `text(...)` node table, whose own
/// props are then the style; `opts` is a style table (`size`, `font`,
/// `wrap`, `max_lines`, `ellipsis`, ...).
fn measure_from_lua(
    ui: &mut Ui<'_>,
    s: &mlua::Value,
    opts: Option<&Table>,
    max_w: Option<f32>,
) -> mlua::Result<kui_core::TextMetrics> {
    let style = match opts {
        Some(t) => parse_props(t, false)?.style,
        None => kui_core::TextStyle::default(),
    };
    let measure_spans = |ui: &mut Ui<'_>, spans: &Table, style: &kui_core::TextStyle| {
        let parts = collect_spans(spans)?;
        let spans: Vec<Span<'_>> = parts.iter().map(span_of).collect();
        Ok(ui.measure_rich_text(&spans, style, max_w))
    };
    match s {
        mlua::Value::String(s) => Ok(ui.measure_text(&s.to_str()?, &style, max_w)),
        mlua::Value::Table(t) => {
            if t.get::<Option<String>>("type")?.as_deref() == Some("text") {
                let style = parse_props(t, false)?.style;
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
        "latency_graph" | "latency_hud" => "latencyGraph",
        other => other,
    }
}

/// The root table's `windows` list (`docs/adr/0004-multi-window.md`): each
/// entry a name, or a table
/// `{ name=, kind=, anchor=, width=, height=, activates= }`, and every one a
/// `Ui::window` declaration. `kind = "popup"` is decision 9's menu surface,
/// placed against `anchor = { x=, y=, w=, h= }` — the rect an `on_layout`
/// event reported — and non-activating unless the entry says otherwise. The
/// embedding host drains the `Open` / `Close` the declared set produces and
/// opens the surfaces; this binding has no runner of its own.
fn declare_windows(ui: &mut Ui<'_>, root: &Table) -> mlua::Result<()> {
    let Some(list) = root.get::<Option<Table>>("windows")? else {
        return Ok(());
    };
    for entry in list.sequence_values::<mlua::Value>() {
        match entry? {
            mlua::Value::String(name) => {
                ui.window(&name.to_str()?, WindowConfig::default());
            }
            mlua::Value::Table(t) => {
                let name: String = t.get("name")?;
                // An entry is plain data with a fixed shape, not a node's
                // loose prop bag, so a value that does nothing is refused
                // rather than dropped: a kind kui does not have would
                // otherwise open a normal window and read as the popup
                // having worked.
                let kind = match t.get::<Option<String>>("kind")?.as_deref() {
                    None | Some("normal") => WindowKind::Normal,
                    Some("popup") => WindowKind::Popup,
                    Some(other) => {
                        return Err(mlua::Error::runtime(format!(
                            "windows entry `{name}` has kind {other:?}; the kinds are \
                             \"normal\" and \"popup\""
                        )));
                    }
                };
                let mut cfg = WindowConfig {
                    kind,
                    // A popup that takes OS focus blurs the field that
                    // opened it, so it does not unless asked.
                    activates: kind == WindowKind::Normal,
                    ..WindowConfig::default()
                };
                if let (Some(w), Some(h)) = (
                    t.get::<Option<f32>>("width")?,
                    t.get::<Option<f32>>("height")?,
                ) {
                    cfg.size = kui_core::Size::new(w, h);
                }
                if let Some(a) = t.get::<Option<bool>>("activates")? {
                    cfg.activates = a;
                }
                if let Some(a) = t.get::<Option<Table>>("anchor")? {
                    cfg.anchor = kui_core::Rect::new(
                        a.get::<Option<f32>>("x")?.unwrap_or(0.0),
                        a.get::<Option<f32>>("y")?.unwrap_or(0.0),
                        a.get::<Option<f32>>("w")?.unwrap_or(0.0),
                        a.get::<Option<f32>>("h")?.unwrap_or(0.0),
                    );
                }
                ui.window(&name, cfg);
            }
            other => {
                return Err(mlua::Error::runtime(format!(
                    "a windows entry is a name or a table, not {}",
                    other.type_name()
                )));
            }
        }
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

fn build_node(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    let ty: String = t.get("type")?;
    check_props(ui, t, element_of(&ty))?;
    match ty.as_str() {
        "row" | "column" => {
            let p = parse_props(t, ty == "row")?;
            let key = match &p.key {
                Some(label) => ui.open_keyed(label, p.spec),
                None => ui.open(p.spec),
            };
            if p.key_focus {
                ui.take_key_focus(key);
            }
            build_children(ui, t)?;
            if let Some(hint) = &p.tooltip
                && ui.is_hovered(key)
            {
                widgets::tooltip(ui, hint);
            }
            ui.close();
            Ok(())
        }
        "text" => {
            let style = parse_props(t, false)?.style;
            if let Some(spans) = t.get::<Option<Table>>("spans")? {
                let parts = collect_spans(&spans)?;
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
            // passed to scripts as a plain integer.
            let id: i64 = t.get("id")?;
            let spec = parse_props(t, false)?.spec;
            ui.image(kui_core::ImageId::from_ffi(id as u64), spec);
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
            let p = parse_props(t, false)?;
            let label = match p.key.clone().or(t.get::<Option<String>>("label")?) {
                Some(l) => l,
                None => return Err(bad("edit needs a key (state is retained by key)")),
            };
            let initial: String = t.get::<Option<String>>("initial")?.unwrap_or_default();
            let opts = EditOptions {
                style: p.style,
                multiline: t.get::<Option<bool>>("multiline")?.unwrap_or(false),
                autofocus: t.get::<Option<bool>>("autofocus")?.unwrap_or(false),
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
            let label: String = t.get("label")?;
            let payload = match t.get::<Option<mlua::Value>>("on_click")? {
                Some(v) => lua_to_value(&v)?,
                None => Value::Null,
            };
            widgets::button(ui, &label, payload);
            Ok(())
        }
        other => Err(mlua::Error::runtime(format!("unknown node type '{other}'"))),
    }
}

struct SpanPart {
    text: String,
    bold: bool,
    italic: bool,
    color: Option<Color>,
}

fn span_of(p: &SpanPart) -> Span<'_> {
    let mut s = Span::new(&p.text);
    if p.bold {
        s = s.bold();
    }
    if p.italic {
        s = s.italic();
    }
    if let Some(c) = p.color {
        s = s.color(c);
    }
    s
}

/// `{ "plain", { "styled", bold = true, italic = true, color = 0x.. }, ... }`
fn collect_spans(spans: &Table) -> mlua::Result<Vec<SpanPart>> {
    let mut out = Vec::new();
    for item in spans.sequence_values::<mlua::Value>() {
        match item? {
            mlua::Value::String(s) => out.push(SpanPart {
                text: s.to_str()?.to_string(),
                bold: false,
                italic: false,
                color: None,
            }),
            mlua::Value::Table(t) => {
                let text: String = t
                    .get::<Option<String>>(1)?
                    .ok_or_else(|| bad("span table needs its text at [1]"))?;
                let color = match t.get::<mlua::Value>("color")? {
                    mlua::Value::Nil => None,
                    v => Some(parse_color(&v)?),
                };
                out.push(SpanPart {
                    text,
                    bold: t.get::<Option<bool>>("bold")?.unwrap_or(false),
                    italic: t.get::<Option<bool>>("italic")?.unwrap_or(false),
                    color,
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

/// Table → props. Constructor-order specials first (`dir` from the node
/// type, `size` before any style prop), then the Lua-shaped composites,
/// then every schema row by its snake_case name. Unknown keys (`type`,
/// `value`, `label`, children) fall through.
pub fn parse_props(t: &Table, is_row: bool) -> mlua::Result<PropsOut> {
    let mut out = PropsOut::new();
    if is_row {
        out.spec = kui_core::NodeSpec::row();
    }
    if let Some(size) = t.get::<Option<f32>>("size")? {
        out.style = kui_core::TextStyle::new(size);
    }
    // `radius` sets all four corners, so it must land before any
    // `radius_tl`-style override — table iteration order is undefined.
    if let Some(r) = t.get::<Option<f32>>("radius")? {
        out.with_spec(|s| s.radius(r));
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
                let pad = parse_pad(&v)?;
                out.apply_pad(pad);
            }
            "border" => {
                let b = match v {
                    mlua::Value::Table(b) => b,
                    _ => return Err(bad("border must be a table {w=, color=}")),
                };
                let w: f32 = b.get("w")?;
                let c = parse_color(&b.get::<mlua::Value>("color")?)?;
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
                    parse_value(&def.kind, &v).map_err(|e| bad(format!("{name}: {e}")))?
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
fn parse_value(kind: &Kind, v: &mlua::Value) -> mlua::Result<Option<Parsed>> {
    Ok(Some(match kind {
        Kind::F32 => Parsed::F32(number(v).ok_or_else(|| bad("expected a number"))?),
        Kind::Color => Parsed::Color(parse_color(v)?),
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
        Kind::Sizing => Parsed::Sizing(parse_sizing(v)?),
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

/// `0xRRGGBBAA` integers or `"#hex"` strings.
fn parse_color(v: &mlua::Value) -> mlua::Result<Color> {
    match v {
        mlua::Value::Integer(n) => Ok(schema::color_num(*n as u32)),
        mlua::Value::Number(n) => Ok(schema::color_num(*n as u32)),
        mlua::Value::String(s) => schema::color_hex_str(&s.to_str()?).map_err(bad),
        _ => Err(bad("color must be a 0xRRGGBBAA integer or \"#hex\" string")),
    }
}

fn parse_sizing(v: &mlua::Value) -> mlua::Result<Sizing> {
    match v {
        mlua::Value::Number(n) => Ok(Sizing::Fixed(*n as f32)),
        mlua::Value::Integer(n) => Ok(Sizing::Fixed(*n as f32)),
        mlua::Value::String(s) => schema::sizing_str(&s.to_str()?).map_err(bad),
        mlua::Value::Table(t) => {
            if let Some(p) = t.get::<Option<f32>>("pct")? {
                Ok(Sizing::Percent(p / 100.0))
            } else if let Some(f) = t.get::<Option<f32>>("grow")? {
                Ok(Sizing::Grow(f))
            } else {
                Err(bad("sizing table needs pct or grow"))
            }
        }
        _ => Err(bad("invalid sizing value")),
    }
}

/// The `pad` prop as declared: a number is the all-round shorthand, a table
/// names any of the family (`x`, `y`, `l`, `r`, `t`, `b`). What a missing
/// edge falls back to is [`PadShorthand::resolve`]'s call.
fn parse_pad(v: &mlua::Value) -> mlua::Result<PadShorthand> {
    match v {
        mlua::Value::Number(n) => Ok(PadShorthand {
            all: Some(*n as f32),
            ..PadShorthand::default()
        }),
        mlua::Value::Integer(n) => Ok(PadShorthand {
            all: Some(*n as f32),
            ..PadShorthand::default()
        }),
        mlua::Value::Table(t) => Ok(PadShorthand {
            all: t.get("all")?,
            x: t.get("x")?,
            y: t.get("y")?,
            l: t.get("l")?,
            r: t.get("r")?,
            t: t.get("t")?,
            b: t.get("b")?,
        }),
        _ => Err(bad("invalid padding value")),
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
        let p = parse_props(&t, true).unwrap();
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
        let p = parse_props(&t, false).unwrap();
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
            parse_props(&by_self, false).unwrap().spec.layout.float,
            parse_props(&by_self_at, false).unwrap().spec.layout.float
        );

        // An unknown preset names the ones that exist instead of silently
        // floating against the parent.
        let bad_preset = eval_table(&lua, r#"{ float = "beneath" }"#);
        let e = parse_props(&bad_preset, false).unwrap_err().to_string();
        assert!(e.contains("beneath") && e.contains("below"), "{e}");
    }

    #[test]
    fn text_style_props_match_the_rust_builder() {
        let lua = Lua::new();
        let t = eval_table(
            &lua,
            r#"{ size = 20, line_height = 30, color = 0x73d98cff, family = "mono" }"#,
        );
        let style = parse_props(&t, false).unwrap().style;
        assert_eq!(
            style,
            TextStyle::new(20.0)
                .line_height(30.0)
                .color(Color::hex(0x73d98cff))
                .family(FontFamily::Mono)
        );
        let t = eval_table(&lua, r#"{ wrap = "none", max_lines = 2, ellipsis = true }"#);
        assert_eq!(
            parse_props(&t, false).unwrap().style,
            TextStyle::default().nowrap().max_lines(2).ellipsis()
        );
        // `size` is applied first regardless of table iteration order, so a
        // color set alongside it survives the TextStyle::new reset.
        let t = eval_table(&lua, r##"{ color = "#fff", size = 12 }"##);
        assert_eq!(
            parse_props(&t, false).unwrap().style,
            TextStyle::new(12.0).color(Color::hex(0xffffffff))
        );
    }

    #[test]
    fn bad_values_name_the_prop() {
        let lua = Lua::new();
        let t = eval_table(&lua, r#"{ main_align = "middle" }"#);
        let e = parse_props(&t, false).unwrap_err().to_string();
        assert!(e.contains("main_align"), "{e}");
        assert!(e.contains("middle"), "{e}");
    }

    fn frame(core: &mut Core, ext: &mut LuaExtension) -> usize {
        let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&mut ui).unwrap();
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
        let err = ext.view(&mut ui).unwrap_err();
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
                  return column { gap = 4, window_title = "all nodes",
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
        // Autofocus places the caret at the start of the initial text.
        let seen: Option<String> = ext.lua.globals().get("seen").unwrap();
        assert_eq!(seen.as_deref(), Some("!hi"));
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
        ext.view(&mut ui).unwrap();
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
            ext.view(&mut ui).unwrap();
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
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        // Every key that only appears when set: a rate, a controls rect,
        // and a focused node (which the ring has to see a frame first).
        core.env.refresh_hz = Some(60.0);
        core.env.window.native_controls = Some(Rect::new(0.0, 0.0, 78.0, 28.0));
        frame(&mut core, &mut ext);
        core.set_focus(Some(Key::ROOT.str("root").str("a")));
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
        documented.sort();
        assert_eq!(
            sorted("values"),
            documented,
            "env's value keys and schema::ENV_FIELDS's Lua column disagree"
        );
        assert_eq!(
            sorted("calls"),
            [
                "announce",
                "blur",
                "edit_text",
                "focus_next",
                "focus_prev",
                "focus_window",
                "is_focused",
                "is_hovered",
                "is_pressed",
                "measure_text",
                "reveal",
                "scroll_geometry",
                "scroll_offset",
                "set_focus",
                "set_scroll",
                "set_window_size",
            ],
            "env's queries and verbs changed; update env_table's doc too"
        );
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

        // `env.focused` is the window's focus, not the node's: it stays a
        // bool through all of the above, and follows the host env instead.
        assert!(ext.lua.globals().get::<bool>("seen_window").unwrap());
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
            ext.view(&mut ui).unwrap();
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

    #[test]
    fn bad_script_reports_error_not_panic() {
        let mut ext = LuaExtension::from_source("bad", "function view() return 5 end").unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        assert!(ext.view(&mut ui).is_err());
    }
}
