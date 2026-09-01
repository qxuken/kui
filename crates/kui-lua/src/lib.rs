//! Lua extensions for kui. A script defines `view(env)` returning a plain
//! table tree (built with the injected `row`/`column`/`text`/`button`
//! prelude) and optionally `on_event(ev)`. `env` carries host facts
//! (refresh rate, focus, viewport); the root table may set `window_title`.
//! Because the IR is data all the way down, the binding is just
//! table-to-node conversion — no closures cross the boundary.

use kui_core::{
    Align, Color, Edges, Extension, FloatConfig, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value,
    WindowButton, widgets,
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
        let env = env_table(&self.lua, ui).map_err(|e| format!("env: {e}"))?;
        let root: Table = view.call(env).map_err(|e| format!("view(): {e}"))?;
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
        let payload = value_to_lua(&self.lua, &ev.payload);
        if let Err(e) = payload.and_then(|p| f.call::<()>(p)) {
            eprintln!("kui-lua: '{}' on_event error: {e}", self.name);
        }
    }
}

/// Host facts handed to `view(env)`: `refresh_hz` (nil if unknown),
/// `frame_budget_ms`, `focused`, `viewport_w`/`viewport_h` (logical px).
fn env_table(lua: &Lua, ui: &Ui<'_>) -> mlua::Result<Table> {
    let env = ui.env();
    let t = lua.create_table()?;
    if let Some(hz) = env.refresh_hz {
        t.set("refresh_hz", hz)?;
    }
    t.set("frame_budget_ms", env.frame_budget_ms())?;
    t.set("focused", env.focused)?;
    let vp = ui.viewport();
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
    Ok(t)
}

// ---------------------------------------------------------------------------
// Table tree -> IR

fn build_node(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    let ty: String = t.get("type")?;
    match ty.as_str() {
        "row" | "column" => {
            let spec = parse_spec(t, ty == "row")?;
            ui.open(spec);
            for child in t.sequence_values::<Table>() {
                build_node(ui, &child?)?;
            }
            ui.close();
            Ok(())
        }
        "text" => {
            let value: String = t.get("value")?;
            let mut style = TextStyle::new(t.get::<Option<f32>>("size")?.unwrap_or(16.0));
            if let Some(c) = t.get::<Option<u32>>("color")? {
                style = style.color(Color::hex(c));
            }
            ui.text(&value, style);
            Ok(())
        }
        "input" => {
            let label: String = t.get("label")?;
            let initial: String = t.get::<Option<String>>("initial")?.unwrap_or_default();
            widgets::text_input(ui, &label, &initial);
            Ok(())
        }
        "titlebar" => {
            let title: String = t.get::<Option<String>>("title")?.unwrap_or_default();
            widgets::titlebar(ui, &title);
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

fn parse_spec(t: &Table, is_row: bool) -> mlua::Result<NodeSpec> {
    let mut spec = if is_row {
        NodeSpec::row()
    } else {
        NodeSpec::column()
    };
    if let Some(s) = t.get::<Option<mlua::Value>>("width")? {
        spec = spec.width(parse_sizing(&s)?);
    }
    if let Some(s) = t.get::<Option<mlua::Value>>("height")? {
        spec = spec.height(parse_sizing(&s)?);
    }
    if let Some(v) = t.get::<Option<f32>>("min_width")? {
        spec = spec.min_width(v);
    }
    if let Some(v) = t.get::<Option<f32>>("max_width")? {
        spec = spec.max_width(v);
    }
    if let Some(v) = t.get::<Option<f32>>("min_height")? {
        spec = spec.min_height(v);
    }
    if let Some(v) = t.get::<Option<f32>>("max_height")? {
        spec = spec.max_height(v);
    }
    if let Some(p) = t.get::<Option<mlua::Value>>("pad")? {
        spec = spec.padding(parse_edges(&p)?);
    }
    if let Some(g) = t.get::<Option<f32>>("gap")? {
        spec = spec.gap(g);
    }
    if let Some(c) = t.get::<Option<u32>>("bg")? {
        spec = spec.bg(Color::hex(c));
    }
    if let Some(r) = t.get::<Option<f32>>("radius")? {
        spec = spec.radius(r);
    }
    if let Some(b) = t.get::<Option<Table>>("border")? {
        let w: f32 = b.get("w")?;
        let c: u32 = b.get("color")?;
        spec = spec.border(w, Color::hex(c));
    }
    if let Some(a) = t.get::<Option<String>>("main_align")? {
        spec = spec.main_align(parse_align(&a)?);
    }
    if let Some(a) = t.get::<Option<String>>("cross_align")? {
        spec = spec.cross_align(parse_align(&a)?);
    }
    if t.get::<Option<bool>>("hoverable")?.unwrap_or(false) {
        spec = spec.hoverable();
    }
    if let Some(v) = t.get::<Option<mlua::Value>>("on_click")? {
        spec = spec.on_click(lua_to_value(&v)?);
    }
    if let Some(v) = t.get::<Option<mlua::Value>>("on_key")? {
        spec = spec.on_key(lua_to_value(&v)?);
    }
    if let Some(v) = t.get::<Option<mlua::Value>>("on_drag")? {
        spec = spec.on_drag(lua_to_value(&v)?);
    }
    if t.get::<Option<bool>>("clip")?.unwrap_or(false) {
        spec = spec.clip();
    }
    if t.get::<Option<bool>>("scroll")?.unwrap_or(false) {
        spec = spec.scroll_y();
    }
    if t.get::<Option<bool>>("scroll_x")?.unwrap_or(false) {
        spec = spec.scroll_x();
    }
    if let Some(role) = t.get::<Option<String>>("window")? {
        spec = match role.as_str() {
            "drag" => spec.window_drag(),
            "close" => spec.window_button(WindowButton::Close),
            "minimize" => spec.window_button(WindowButton::Minimize),
            "maximize" => spec.window_button(WindowButton::Maximize),
            other => {
                return Err(mlua::Error::runtime(format!("unknown window role '{other}'")));
            }
        };
    }
    if let Some(f) = t.get::<Option<Table>>("float")? {
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
        spec = spec.float(cfg);
    }
    Ok(spec)
}

fn parse_sizing(v: &mlua::Value) -> mlua::Result<Sizing> {
    match v {
        mlua::Value::Number(n) => Ok(Sizing::Fixed(*n as f32)),
        mlua::Value::Integer(n) => Ok(Sizing::Fixed(*n as f32)),
        mlua::Value::String(s) => match s.to_str()?.as_ref() {
            "grow" => Ok(Sizing::Grow(1.0)),
            "fit" => Ok(Sizing::Fit),
            other => Err(mlua::Error::runtime(format!("unknown sizing '{other}'"))),
        },
        mlua::Value::Table(t) => {
            if let Some(p) = t.get::<Option<f32>>("pct")? {
                Ok(Sizing::Percent(p / 100.0))
            } else if let Some(f) = t.get::<Option<f32>>("grow")? {
                Ok(Sizing::Grow(f))
            } else {
                Err(mlua::Error::runtime("sizing table needs pct or grow"))
            }
        }
        _ => Err(mlua::Error::runtime("invalid sizing value")),
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
        _ => Err(mlua::Error::runtime("invalid padding value")),
    }
}

fn parse_align(s: &str) -> mlua::Result<Align> {
    match s {
        "start" => Ok(Align::Start),
        "center" => Ok(Align::Center),
        "end" => Ok(Align::End),
        other => Err(mlua::Error::runtime(format!("unknown align '{other}'"))),
    }
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

        let mut core = kui_core::Core::new();
        let mut ui = core.frame(kui_core::Size::new(800.0, 600.0), 1.0);
        ui.set_origin(kui_core::OriginId(1));
        ext.view(&mut ui).unwrap();
        ui.finish();
        let (dl, _) = core.output();
        // Panel bg + button bg + glyphs for two strings.
        assert!(
            dl.quads.len() > 10,
            "expected panel/button/glyph quads, got {}",
            dl.quads.len()
        );

        // Events round-trip into Lua state.
        ext.on_event(&UiEvent {
            origin: kui_core::OriginId(1),
            key: kui_core::Key::ROOT,
            payload: Value::map([("kind", "bump".into())]),
        });
        let count: i64 = ext.lua.globals().get("count").unwrap();
        assert_eq!(count, 42);
    }

    #[test]
    fn bad_script_reports_error_not_panic() {
        let mut ext = LuaExtension::from_source("bad", "function view() return 5 end").unwrap();
        let mut core = kui_core::Core::new();
        let mut ui = core.frame(kui_core::Size::new(100.0, 100.0), 1.0);
        assert!(ext.view(&mut ui).is_err());
    }
}
