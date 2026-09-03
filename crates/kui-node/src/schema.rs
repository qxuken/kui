//! The JSON face of the prop schema. The rows themselves live in
//! `kui_core::schema` (shared with Lua and pinned by the C parity test);
//! this module parses JSON values by kind, drives the JSON lowering path
//! (`parse_props_json`), and exports the rows for the JS encoder and the TS
//! type generator (`protocol_props`). The binary decoder in `binary.rs`
//! reads by the same kinds.

pub use kui_core::schema::*;
use kui_core::{Align, Color, Edges, FloatConfig, NodeSpec, Sizing, TextStyle};
use serde_json::{Map as JsonMap, Value as Json};

use crate::{Result, err, value_of};

/// `kui_core::schema::apply` with the error lifted into napi's.
pub fn apply(def: &PropDef, value: Parsed, out: &mut PropsOut) -> Result<()> {
    kui_core::schema::apply(def, value, out).map_err(err)
}

/// Colors accept `0xRRGGBBAA` numbers or `#rgb` / `#rrggbb` / `#rrggbbaa`.
pub fn color_of(v: &Json) -> Result<Color> {
    match v {
        Json::Number(n) => {
            let hex = n
                .as_u64()
                .ok_or_else(|| err("color number must be a u32"))? as u32;
            Ok(color_num(hex))
        }
        Json::String(s) => color_hex_str(s).map_err(err),
        _ => Err(err("color must be a number or #hex string")),
    }
}

pub fn sizing_of(v: &Json) -> Result<Sizing> {
    match v {
        Json::Number(n) => Ok(Sizing::Fixed(n.as_f64().unwrap_or(0.0) as f32)),
        Json::String(s) => sizing_str(s).map_err(err),
        Json::Object(m) => {
            if let Some(g) = m.get("grow").and_then(Json::as_f64) {
                Ok(Sizing::Grow(g as f32))
            } else if let Some(p) = m.get("percent").and_then(Json::as_f64) {
                Ok(Sizing::Percent(p as f32))
            } else {
                Err(err("sizing object needs grow or percent"))
            }
        }
        _ => Err(err("bad sizing")),
    }
}

pub fn align_of(v: &Json) -> Result<Align> {
    match v.as_str().and_then(|s| ALIGNS.iter().position(|a| *a == s)) {
        Some(i) => Ok(align_idx(i)),
        None => Err(err("align must be start | center | end")),
    }
}

pub fn float_of(v: &Json) -> Result<FloatConfig> {
    match v {
        Json::String(s) => match s.as_str() {
            "below" => Ok(FloatConfig::below()),
            "above" => Ok(FloatConfig::above()),
            "parent" => Ok(FloatConfig::parent()),
            "viewport" => Ok(FloatConfig::viewport()),
            _ => Err(err(format!("bad float {s:?}"))),
        },
        Json::Object(m) => {
            let mut cfg = match m.get("anchor").and_then(Json::as_str) {
                Some("viewport") => FloatConfig::viewport(),
                _ => FloatConfig::parent(),
            };
            if let Some(at) = m.get("at").and_then(Json::as_array)
                && at.len() == 2
            {
                cfg = cfg.at(align_of(&at[0])?, align_of(&at[1])?);
            }
            if let Some(at) = m.get("self").and_then(Json::as_array)
                && at.len() == 2
            {
                cfg = cfg.self_at(align_of(&at[0])?, align_of(&at[1])?);
            }
            let dx = m.get("dx").and_then(Json::as_f64).unwrap_or(0.0) as f32;
            let dy = m.get("dy").and_then(Json::as_f64).unwrap_or(0.0) as f32;
            cfg = cfg.offset(dx, dy);
            if m.get("fit").and_then(Json::as_bool).unwrap_or(false) {
                cfg = cfg.fit();
            }
            Ok(cfg)
        }
        _ => Err(err("float must be a string preset or config object")),
    }
}

fn parse_json(kind: &Kind, v: &Json) -> Result<Option<Parsed>> {
    Ok(Some(match kind {
        Kind::F32 => Parsed::F32(v.as_f64().ok_or_else(|| err("expected a number"))? as f32),
        Kind::Color => Parsed::Color(color_of(v)?),
        Kind::Flag => match v.as_bool() {
            Some(true) => Parsed::Flag,
            _ => return Ok(None),
        },
        Kind::Enum(names) => {
            let s = v.as_str().ok_or_else(|| err("expected a string"))?;
            Parsed::Enum(enum_index(names, s).map_err(err)?)
        }
        Kind::Sizing => Parsed::Sizing(sizing_of(v)?),
        Kind::Msg | Kind::Tag => Parsed::Msg(value_of(v)),
        Kind::Str => Parsed::Str(
            v.as_str()
                .ok_or_else(|| err("expected a string"))?
                .to_string(),
        ),
        Kind::Resource => Parsed::Resource(crate::parse_u64(
            v.as_str()
                .ok_or_else(|| err("expected a resource id string"))?,
        )?),
        Kind::Keyframes => {
            Parsed::Keyframes(kui_core::keyframes::parse(&value_of(v)).map_err(err)?)
        }
        Kind::Enter => Parsed::Enter(kui_core::enter::parse(&value_of(v)).map_err(err)?),
    }))
}

fn f32_prop(props: &JsonMap<String, Json>, key: &str) -> Option<f32> {
    props.get(key).and_then(Json::as_f64).map(|v| v as f32)
}

fn bool_prop(props: &JsonMap<String, Json>, key: &str) -> bool {
    props.get(key).and_then(Json::as_bool).unwrap_or(false)
}

/// The JSON-path prop parser: composites and constructor-order specials
/// hand-written, everything else driven by the `PROPS` table. The binary
/// decoder mirrors this structure in `binary.rs`.
pub fn parse_props_json(props: &JsonMap<String, Json>) -> Result<PropsOut> {
    let mut out = PropsOut::new();
    // Constructors first (the binary invariant "dir/size lead" is this same
    // rule expressed in stream order).
    match props.get("dir").and_then(Json::as_str) {
        Some("row") => out.spec = NodeSpec::row(),
        Some("column") | None => {}
        Some(other) => return Err(err(format!("bad dir {other:?} (row | column)"))),
    }
    if let Some(sz) = f32_prop(props, "size") {
        out.style = TextStyle::new(sz);
    }
    // Composites.
    let pad = f32_prop(props, "pad").unwrap_or(0.0);
    let px = f32_prop(props, "padX").unwrap_or(pad);
    let py = f32_prop(props, "padY").unwrap_or(pad);
    out.with_spec(|s| {
        s.padding(Edges {
            l: f32_prop(props, "padL").unwrap_or(px),
            r: f32_prop(props, "padR").unwrap_or(px),
            t: f32_prop(props, "padT").unwrap_or(py),
            b: f32_prop(props, "padB").unwrap_or(py),
        })
    });
    if let Some(w) = f32_prop(props, "borderW") {
        let color = match props.get("borderColor") {
            Some(v) => color_of(v)?,
            None => Color::TRANSPARENT,
        };
        out.with_spec(|s| s.border(w, color));
    }
    if bool_prop(props, "clip") {
        out.with_spec(NodeSpec::clip);
    }
    if bool_prop(props, "scrollX") {
        out.with_spec(NodeSpec::scroll_x);
    }
    if bool_prop(props, "scrollY") {
        out.with_spec(NodeSpec::scroll_y);
    }
    if let Some(v) = props.get("float") {
        let cfg = float_of(v)?;
        out.with_spec(|s| s.float(cfg));
    }
    out.key_focus = bool_prop(props, "keyFocus");
    out.tooltip = props
        .get("tooltip")
        .and_then(Json::as_str)
        .map(str::to_string);
    if out.tooltip.is_some() {
        out.with_spec(NodeSpec::hoverable);
    }
    out.title = props
        .get("title")
        .and_then(Json::as_str)
        .map(str::to_string);
    // The table drives the rest; unknown names are ignored (element-level
    // props like `initial` or `src` land here too and fall through). A null
    // is absent — except for a tag, where it declares the behaviour
    // without a tag on its events.
    for (k, v) in props {
        let Some(def) = by_name(k) else { continue };
        if v.is_null() && !matches!(def.kind, Kind::Tag) {
            continue;
        }
        if let Some(parsed) = parse_json(&def.kind, v)? {
            apply(def, parsed, &mut out)?;
        }
    }
    Ok(out)
}

/// The schema as data, for `protocol()`: generic rows carry kind/values/
/// target/doc (the JS encoder and the TS generator interpret them); composite
/// and constructor props are exported as `kind: "custom"` so the encoder has
/// their ids without pretending they're mechanical.
pub fn protocol_props() -> Json {
    let mut o = JsonMap::new();
    for def in PROPS {
        let mut p = JsonMap::new();
        p.insert("id".into(), Json::from(def.id));
        let (kind, values): (&str, Option<&[&str]>) = match &def.kind {
            Kind::F32 => ("f32", None),
            Kind::Color => ("color", None),
            Kind::Flag => ("flag", None),
            Kind::Enum(names) => ("enum", Some(names)),
            Kind::Sizing => ("sizing", None),
            Kind::Msg => ("msg", None),
            Kind::Tag => ("tag", None),
            Kind::Str => ("str", None),
            Kind::Resource => ("resource", None),
            Kind::Keyframes => ("keyframes", None),
            Kind::Enter => ("enter", None),
        };
        p.insert("kind".into(), Json::String(kind.into()));
        if let Some(names) = values {
            p.insert(
                "values".into(),
                Json::Array(names.iter().map(|n| Json::String((*n).into())).collect()),
            );
        }
        let target = match def.target() {
            Target::Style => "style",
            Target::Spec => "spec",
        };
        p.insert("target".into(), Json::String(target.into()));
        p.insert("doc".into(), Json::String(def.doc.into()));
        // Spellings in the other bindings, for the generated reference.
        p.insert("lua".into(), Json::String(def.snake_name().into()));
        p.insert("c".into(), Json::String(c_field(def)));
        o.insert(def.name.into(), Json::Object(p));
    }
    for c in CUSTOM {
        let mut p = JsonMap::new();
        p.insert("id".into(), Json::from(c.id));
        p.insert("kind".into(), Json::String("custom".into()));
        p.insert("jsx".into(), Json::String(c.jsx.into()));
        p.insert("lua".into(), Json::String(c.lua.into()));
        p.insert("c".into(), Json::String(c.c.into()));
        p.insert("doc".into(), Json::String(c.doc.into()));
        o.insert(c.name.into(), Json::Object(p));
    }
    Json::Object(o)
}

/// A docs table column: its name and the accessor for a row's cell.
type Column<T> = (&'static str, fn(&T) -> &'static str);

fn table<T>(items: &[T], fields: &[Column<T>]) -> Json {
    Json::Array(
        items
            .iter()
            .map(|it| {
                Json::Object(
                    fields
                        .iter()
                        .map(|(k, f)| (k.to_string(), Json::String(f(it).into())))
                        .collect(),
                )
            })
            .collect(),
    )
}

/// The element, event and resource tables as data, for the docs generator.
pub fn protocol_tables() -> Vec<(&'static str, Json)> {
    vec![
        (
            "elements",
            table(
                ELEMENTS,
                &[
                    ("name", |e: &ElementDef| e.name),
                    ("jsx", |e| e.jsx),
                    ("lua", |e| e.lua),
                    ("c", |e| e.c),
                    ("doc", |e| e.doc),
                ],
            ),
        ),
        (
            "events",
            table(
                EVENTS,
                &[
                    ("kind", |e: &EventDef| e.kind),
                    ("payload", |e| e.payload),
                    ("doc", |e| e.doc),
                ],
            ),
        ),
        (
            "resources",
            table(
                RESOURCES,
                &[
                    ("what", |r: &ResourceDef| r.what),
                    ("node", |r| r.node),
                    ("lua", |r| r.lua),
                    ("c", |r| r.c),
                ],
            ),
        ),
    ]
}
