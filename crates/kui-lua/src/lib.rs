//! Lua extensions for kui. A script defines `view(env)` returning a plain
//! table tree (built with the injected `row`/`column`/`text`/`button`
//! prelude) and optionally `on_event(ev)`. `env` carries host facts
//! (refresh rate, focus, viewport) and a few queries (`env.edit_text(key)`,
//! `env.is_focused(key)`, `env.is_hovered(key)`,
//! `env.measure_text(s, opts, max_w)`); the root table may set
//! `window_title`. Because the IR is data all the way down, the binding is
//! just table-to-node conversion — no closures cross the boundary.
//!
//! Props come from the shared schema (`kui_core::schema`): every row is
//! reachable from Lua under its snake_case name (`min_width`, `on_click`,
//! `line_height`, ...), so Lua and the Node binding accept the same surface
//! by construction. Only the composites keep Lua-flavored shapes:
//! `pad = 8 | {l,r,t,b}`, `border = {w, color}`, `scroll`/`scroll_x`/
//! `scroll_y`/`clip` booleans, `float = "below" | {anchor, at, self_at, dx,
//! dy, fit}`, sizing `{pct = 50} | {grow = 2}`, and `tooltip = "hint"` on a
//! container (hover-gated).
//!
//! Events arrive as their payload table plus `node_key` (the emitting
//! node's key as an integer), which is what `env.edit_text` takes.

use kui_core::schema::{self, Kind, Parsed, PropsOut};
use kui_core::{
    Align, Color, Edges, EditOptions, Extension, FloatConfig, Key, Sizing, Span, Ui, UiEvent,
    Value, widgets,
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

/// Host facts handed to `view(env)`: `refresh_hz` (nil if unknown),
/// `frame_budget_ms`, `focused`, `viewport_w`/`viewport_h` (logical px),
/// `window` chrome facts, and the queries `edit_text(key)`,
/// `is_focused(key)`, `is_hovered(key)` (keys are the integers events
/// carry) and `measure_text(s, opts, max_w)` (see `measure_from_lua`).
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
    t.set("focused", env.focused)?;
    // Keyboard focus as data: the focused node's key (an integer, as
    // events carry them; nil for none) and whether it shows — it got there
    // by Tab or assistive technology rather than a click.
    if let Some(k) = focus {
        t.set("focus", k.0 as i64)?;
    }
    t.set("focus_visible", focus_visible)?;
    t.set("viewport_w", vp.w)?;
    t.set("viewport_h", vp.h)?;
    let win = env.window;
    let wt = lua.create_table()?;
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

fn build_node(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    let ty: String = t.get("type")?;
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
    for pair in t.pairs::<mlua::Value, mlua::Value>() {
        let (k, v) = pair?;
        let mlua::Value::String(k) = k else { continue };
        let k = k.to_str()?;
        match k.as_ref() {
            "size" | "radius" => {}
            "pad" => {
                let e = parse_edges(&v)?;
                out.with_spec(|s| s.padding(e));
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
            "clip" => {
                if truthy(&v) {
                    out.with_spec(kui_core::NodeSpec::clip);
                }
            }
            "scroll_x" => {
                if truthy(&v) {
                    out.with_spec(kui_core::NodeSpec::scroll_x);
                }
            }
            "scroll" | "scroll_y" => {
                if truthy(&v) {
                    out.with_spec(kui_core::NodeSpec::scroll_y);
                }
            }
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
                let hint = s.to_str()?.to_string();
                // The hint is the accessible description too.
                out.with_spec(|s| s.hoverable().description(hint.as_str()));
                out.tooltip = Some(hint);
            }
            name => {
                // `repeat` is a Lua keyword, so that row also answers to
                // CSS's own name for it.
                let name = if name == "direction" { "repeat" } else { name };
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
    Ok(out)
}

fn truthy(v: &mlua::Value) -> bool {
    matches!(v, mlua::Value::Boolean(true))
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

fn parse_edges(v: &mlua::Value) -> mlua::Result<Edges> {
    match v {
        mlua::Value::Number(n) => Ok(Edges::all(*n as f32)),
        mlua::Value::Integer(n) => Ok(Edges::all(*n as f32)),
        mlua::Value::Table(t) => Ok(Edges {
            l: t.get::<Option<f32>>("l")?.unwrap_or(0.0),
            r: t.get::<Option<f32>>("r")?.unwrap_or(0.0),
            t: t.get::<Option<f32>>("t")?.unwrap_or(0.0),
            b: t.get::<Option<f32>>("b")?.unwrap_or(0.0),
        }),
        _ => Err(bad("invalid padding value")),
    }
}

fn parse_align(s: &str) -> mlua::Result<Align> {
    schema::enum_index(schema::ALIGNS, s)
        .map(schema::align_idx)
        .map_err(bad)
}

fn parse_float(v: &mlua::Value) -> mlua::Result<FloatConfig> {
    let f = match v {
        mlua::Value::String(s) => {
            return match s.to_str()?.as_ref() {
                "below" => Ok(FloatConfig::below()),
                "above" => Ok(FloatConfig::above()),
                "parent" => Ok(FloatConfig::parent()),
                "viewport" => Ok(FloatConfig::viewport()),
                other => Err(bad(format!("bad float '{other}'"))),
            };
        }
        mlua::Value::Table(f) => f,
        _ => return Err(bad("float must be a preset string or a table")),
    };
    let mut cfg = match f.get::<Option<String>>("anchor")?.as_deref() {
        Some("viewport") => FloatConfig::viewport(),
        _ => FloatConfig::parent(),
    };
    if let Some(at) = f.get::<Option<Table>>("at")? {
        cfg = cfg.at(
            parse_align(&at.get::<String>(1)?)?,
            parse_align(&at.get::<String>(2)?)?,
        );
    }
    if let Some(at) = f.get::<Option<Table>>("self_at")? {
        cfg = cfg.self_at(
            parse_align(&at.get::<String>(1)?)?,
            parse_align(&at.get::<String>(2)?)?,
        );
    }
    cfg = cfg.offset(
        f.get::<Option<f32>>("dx")?.unwrap_or(0.0),
        f.get::<Option<f32>>("dy")?.unwrap_or(0.0),
    );
    if f.get::<Option<bool>>("fit")?.unwrap_or(false) {
        cfg = cfg.fit();
    }
    Ok(cfg)
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
        Core, FontFamily, InputEvent, NodeSpec, OriginId, Size, TextStyle, Vec2, WindowButton,
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
                on_click = {kind = "hit"}, on_drag = "d", on_key = 7,
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
            .on_key(Value::Int(7));
        assert_eq!(p.spec, expected);
        assert_eq!(p.key.as_deref(), Some("panel"));
        assert!(p.key_focus);
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
        });
        let count: i64 = ext.lua.globals().get("count").unwrap();
        assert_eq!(count, 42);
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

    /// `role` / `label` / `checked` / `value_*` are schema rows, so a script
    /// declares semantics like any other prop; the access tree shows them,
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

    #[test]
    fn bad_script_reports_error_not_panic() {
        let mut ext = LuaExtension::from_source("bad", "function view() return 5 end").unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        assert!(ext.view(&mut ui).is_err());
    }
}
