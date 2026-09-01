//! Single source of truth for the prop surface. One `PROPS` row declares a
//! prop's name, wire id, value kind, apply function, and doc — and everything
//! else interprets it:
//!
//! - the JSON lowering path (`parse_props_json`) parses by kind and applies,
//! - the binary decoder reads by kind (see `binary.rs`) and applies,
//! - `protocol_props()` exports the schema so the JS encoder writes by kind
//!   with no hardcoded names or ids,
//! - the TS type generator (`npm run gen` in packages/kui) emits prop types
//!   from the same export.
//!
//! Adding a simple prop = one row here + regenerate types. Only composite
//! props with real logic stay hand-written on each side (the pad shorthand
//! family, border, overflow bits, float configs, and the dir/size
//! constructor-ordering invariant); they are marked `kind: "custom"` in the
//! export so the encoder knows they exist without knowing their shape.

use kui_core::{
    Align, Color, Edges, FloatConfig, FontFamily, NodeSpec, Sizing, TextStyle, Value, WindowButton,
};
use serde_json::{Map as JsonMap, Value as Json};

use crate::{Result, err, value_of};

// Wire ids, stable within a protocol VERSION (see binary.rs).
pub const P_DIR: u32 = 1;
pub const P_WIDTH: u32 = 2;
pub const P_HEIGHT: u32 = 3;
pub const P_MIN_W: u32 = 4;
pub const P_MAX_W: u32 = 5;
pub const P_MIN_H: u32 = 6;
pub const P_MAX_H: u32 = 7;
pub const P_PAD: u32 = 8;
pub const P_GAP: u32 = 9;
pub const P_MAIN_ALIGN: u32 = 10;
pub const P_CROSS_ALIGN: u32 = 11;
pub const P_BG: u32 = 12;
pub const P_BORDER: u32 = 13;
pub const P_RADIUS: u32 = 14;
pub const P_OVERFLOW: u32 = 15;
pub const P_FLOAT: u32 = 16;
pub const P_HOVERABLE: u32 = 17;
pub const P_ON_CLICK: u32 = 18;
pub const P_ON_DRAG: u32 = 19;
pub const P_ON_KEY: u32 = 20;
pub const P_WINDOW: u32 = 21;
pub const P_KEY_FOCUS: u32 = 22;
pub const P_CENTER: u32 = 23;
pub const P_SIZE: u32 = 24;
pub const P_LINE_HEIGHT: u32 = 25;
pub const P_COLOR: u32 = 26;
pub const P_FAMILY: u32 = 27;
pub const P_KEY: u32 = 28;
pub const P_TITLE: u32 = 29;

pub const ALIGNS: &[&str] = &["start", "center", "end"];
pub const WINDOW_ROLES: &[&str] = &["drag", "close", "minimize", "maximize"];
pub const FAMILIES: &[&str] = &["sans", "serif", "mono"];

/// How a prop's value is parsed (JSON) and encoded (binary slots).
pub enum Kind {
    /// One number. JSON: number. Binary: 1 slot.
    F32,
    /// One color. JSON: 0xRRGGBBAA number or #hex string. Binary: 1 slot (u32).
    Color,
    /// Marker, present-or-absent. JSON: bool (false = absent). Binary: 0 slots.
    Flag,
    /// One of a closed name list. JSON: string. Binary: 1 slot (index).
    Enum(&'static [&'static str]),
    /// A sizing. JSON: number | "fit" | "grow" | "N%" | {grow} | {percent}.
    /// Binary: 2 slots (mode, value).
    Sizing,
    /// An arbitrary message payload. JSON: any. Binary: strref to JSON text.
    Msg,
}

/// Where a parsed value lands. The `target()` split also drives which TS
/// interface the generator puts the prop in.
pub enum Apply {
    SpecF32(fn(NodeSpec, f32) -> NodeSpec),
    SpecColor(fn(NodeSpec, Color) -> NodeSpec),
    SpecFlag(fn(NodeSpec) -> NodeSpec),
    SpecEnum(fn(NodeSpec, usize) -> NodeSpec),
    SpecSizing(fn(NodeSpec, Sizing) -> NodeSpec),
    SpecMsg(fn(NodeSpec, Value) -> NodeSpec),
    StyleF32(fn(TextStyle, f32) -> TextStyle),
    StyleColor(fn(TextStyle, Color) -> TextStyle),
    StyleEnum(fn(TextStyle, usize) -> TextStyle),
}

pub struct PropDef {
    pub name: &'static str,
    pub id: u32,
    pub kind: Kind,
    pub apply: Apply,
    pub doc: &'static str,
}

pub fn align_idx(i: usize) -> Align {
    match i {
        1 => Align::Center,
        2 => Align::End,
        _ => Align::Start,
    }
}

/// Binary sizing decode: (mode, value) → Sizing.
pub fn sizing_num(mode: u32, value: f64) -> Sizing {
    match mode {
        1 => Sizing::Fit,
        2 => Sizing::Grow(value as f32),
        3 => Sizing::Percent(value as f32),
        _ => Sizing::Fixed(value as f32),
    }
}

pub const PROPS: &[PropDef] = &[
    PropDef {
        name: "width",
        id: P_WIDTH,
        kind: Kind::Sizing,
        apply: Apply::SpecSizing(|s, v| s.width(v)),
        doc: "Main-axis size: px | \"fit\" | \"grow\" | \"N%\".",
    },
    PropDef {
        name: "height",
        id: P_HEIGHT,
        kind: Kind::Sizing,
        apply: Apply::SpecSizing(|s, v| s.height(v)),
        doc: "Cross-axis size: px | \"fit\" | \"grow\" | \"N%\".",
    },
    PropDef {
        name: "minWidth",
        id: P_MIN_W,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.min_width(v)),
        doc: "Lower width clamp (logical px).",
    },
    PropDef {
        name: "maxWidth",
        id: P_MAX_W,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.max_width(v)),
        doc: "Upper width clamp; grow+maxWidth is the responsive-width pattern.",
    },
    PropDef {
        name: "minHeight",
        id: P_MIN_H,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.min_height(v)),
        doc: "Lower height clamp (logical px).",
    },
    PropDef {
        name: "maxHeight",
        id: P_MAX_H,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.max_height(v)),
        doc: "Upper height clamp (logical px).",
    },
    PropDef {
        name: "gap",
        id: P_GAP,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.gap(v)),
        doc: "Space between children along the main axis.",
    },
    PropDef {
        name: "mainAlign",
        id: P_MAIN_ALIGN,
        kind: Kind::Enum(ALIGNS),
        apply: Apply::SpecEnum(|s, i| s.main_align(align_idx(i))),
        doc: "Child alignment along the main axis.",
    },
    PropDef {
        name: "crossAlign",
        id: P_CROSS_ALIGN,
        kind: Kind::Enum(ALIGNS),
        apply: Apply::SpecEnum(|s, i| s.cross_align(align_idx(i))),
        doc: "Child alignment across the main axis.",
    },
    PropDef {
        name: "bg",
        id: P_BG,
        kind: Kind::Color,
        apply: Apply::SpecColor(|s, c| s.bg(c)),
        doc: "Background fill.",
    },
    PropDef {
        name: "radius",
        id: P_RADIUS,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.radius(v)),
        doc: "Corner radius (logical px).",
    },
    PropDef {
        name: "center",
        id: P_CENTER,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.center()),
        doc: "Center children on both axes.",
    },
    PropDef {
        name: "hoverable",
        id: P_HOVERABLE,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.hoverable()),
        doc: "Hover-track without a click payload (for isHovered-driven styling).",
    },
    PropDef {
        name: "onClick",
        id: P_ON_CLICK,
        kind: Kind::Msg,
        apply: Apply::SpecMsg(|s, v| s.on_click(v)),
        doc: "Message emitted when clicked (data, not a callback).",
    },
    PropDef {
        name: "onDrag",
        id: P_ON_DRAG,
        kind: Kind::Msg,
        apply: Apply::SpecMsg(|s, v| s.on_drag(v)),
        doc: "Drag tag: emits {kind:\"drag\", phase, x, y, dx, dy, parent, tag} events.",
    },
    PropDef {
        name: "onKey",
        id: P_ON_KEY,
        kind: Kind::Msg,
        apply: Apply::SpecMsg(|s, v| s.on_key(v)),
        doc: "Key-sink tag: with key focus held, presses arrive as {kind:\"key\", ...} events.",
    },
    PropDef {
        name: "window",
        id: P_WINDOW,
        kind: Kind::Enum(WINDOW_ROLES),
        apply: Apply::SpecEnum(|s, i| match i {
            0 => s.window_drag(),
            1 => s.window_button(WindowButton::Close),
            2 => s.window_button(WindowButton::Minimize),
            3 => s.window_button(WindowButton::Maximize),
            _ => s,
        }),
        doc: "Window-chrome role: interactions become window commands, not events.",
    },
    PropDef {
        name: "lineHeight",
        id: P_LINE_HEIGHT,
        kind: Kind::F32,
        apply: Apply::StyleF32(|t, v| t.line_height(v)),
        doc: "Line height (logical px); default size * 1.35.",
    },
    PropDef {
        name: "color",
        id: P_COLOR,
        kind: Kind::Color,
        apply: Apply::StyleColor(|t, c| t.color(c)),
        doc: "Text color; default foreground when omitted.",
    },
    PropDef {
        name: "family",
        id: P_FAMILY,
        kind: Kind::Enum(FAMILIES),
        apply: Apply::StyleEnum(|t, i| match i {
            1 => t.family(FontFamily::Serif),
            2 => t.family(FontFamily::Mono),
            _ => t,
        }),
        doc: "Font family.",
    },
];

pub fn by_name(name: &str) -> Option<&'static PropDef> {
    PROPS.iter().find(|d| d.name == name)
}

pub fn by_id(id: u32) -> Option<&'static PropDef> {
    PROPS.iter().find(|d| d.id == id)
}

/// A parsed prop value, transport-independent.
pub enum Parsed {
    F32(f32),
    Color(Color),
    Flag,
    Enum(usize),
    Sizing(Sizing),
    Msg(Value),
}

/// Everything a prop list can carry; elements pick the parts they use.
pub struct PropsOut {
    pub spec: NodeSpec,
    pub style: TextStyle,
    pub key: Option<String>,
    pub title: Option<String>,
    pub key_focus: bool,
}

impl PropsOut {
    pub fn new() -> Self {
        PropsOut {
            spec: NodeSpec::column(),
            style: TextStyle::new(16.0),
            key: None,
            title: None,
            key_focus: false,
        }
    }
}

pub fn apply(def: &PropDef, value: Parsed, out: &mut PropsOut) -> Result<()> {
    let spec = std::mem::take(&mut out.spec);
    let style = out.style;
    match (&def.apply, value) {
        (Apply::SpecF32(f), Parsed::F32(v)) => out.spec = f(spec, v),
        (Apply::SpecColor(f), Parsed::Color(v)) => out.spec = f(spec, v),
        (Apply::SpecFlag(f), Parsed::Flag) => out.spec = f(spec),
        (Apply::SpecEnum(f), Parsed::Enum(v)) => out.spec = f(spec, v),
        (Apply::SpecSizing(f), Parsed::Sizing(v)) => out.spec = f(spec, v),
        (Apply::SpecMsg(f), Parsed::Msg(v)) => out.spec = f(spec, v),
        (Apply::StyleF32(f), Parsed::F32(v)) => {
            out.spec = spec;
            out.style = f(style, v);
        }
        (Apply::StyleColor(f), Parsed::Color(v)) => {
            out.spec = spec;
            out.style = f(style, v);
        }
        (Apply::StyleEnum(f), Parsed::Enum(v)) => {
            out.spec = spec;
            out.style = f(style, v);
        }
        _ => return Err(err(format!("prop {} value/kind mismatch", def.name))),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Value parsing shared by both transports

/// Colors accept `0xRRGGBBAA` numbers or `#rgb` / `#rrggbb` / `#rrggbbaa`.
pub fn color_of(v: &Json) -> Result<Color> {
    match v {
        Json::Number(n) => {
            let hex = n
                .as_u64()
                .ok_or_else(|| err("color number must be a u32"))? as u32;
            Ok(color_num(hex))
        }
        Json::String(s) => {
            let hex = s
                .strip_prefix('#')
                .ok_or_else(|| err(format!("bad color {s:?}")))?;
            let expanded = match hex.len() {
                3 => hex
                    .chars()
                    .flat_map(|c| [c, c])
                    .chain("ff".chars())
                    .collect::<String>(),
                6 => format!("{hex}ff"),
                8 => hex.to_string(),
                _ => return Err(err(format!("bad color {s:?}"))),
            };
            let n =
                u32::from_str_radix(&expanded, 16).map_err(|_| err(format!("bad color {s:?}")))?;
            Ok(Color::hex(n))
        }
        _ => Err(err("color must be a number or #hex string")),
    }
}

/// 0 = transparent, anything else 0xRRGGBBAA.
pub fn color_num(hex: u32) -> Color {
    if hex == 0 {
        Color::TRANSPARENT
    } else {
        Color::hex(hex)
    }
}

pub fn sizing_of(v: &Json) -> Result<Sizing> {
    match v {
        Json::Number(n) => Ok(Sizing::Fixed(n.as_f64().unwrap_or(0.0) as f32)),
        Json::String(s) => match s.as_str() {
            "fit" => Ok(Sizing::Fit),
            "grow" => Ok(Sizing::Grow(1.0)),
            s if s.ends_with('%') => {
                let pct: f32 = s[..s.len() - 1]
                    .parse()
                    .map_err(|_| err(format!("bad percent {s:?}")))?;
                Ok(Sizing::Percent(pct / 100.0))
            }
            _ => Err(err(format!(
                "bad sizing {s:?} (fit | grow | number | \"N%\")"
            ))),
        },
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
            let i = names
                .iter()
                .position(|n| *n == s)
                .ok_or_else(|| err(format!("bad value {s:?} (one of {names:?})")))?;
            Parsed::Enum(i)
        }
        Kind::Sizing => Parsed::Sizing(sizing_of(v)?),
        Kind::Msg => Parsed::Msg(value_of(v)),
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
    out.spec = std::mem::take(&mut out.spec).padding(Edges {
        l: f32_prop(props, "padL").unwrap_or(px),
        r: f32_prop(props, "padR").unwrap_or(px),
        t: f32_prop(props, "padT").unwrap_or(py),
        b: f32_prop(props, "padB").unwrap_or(py),
    });
    if let Some(w) = f32_prop(props, "borderW") {
        let color = match props.get("borderColor") {
            Some(v) => color_of(v)?,
            None => Color::TRANSPARENT,
        };
        out.spec = std::mem::take(&mut out.spec).border(w, color);
    }
    if bool_prop(props, "clip") {
        out.spec = std::mem::take(&mut out.spec).clip();
    }
    if bool_prop(props, "scrollX") {
        out.spec = std::mem::take(&mut out.spec).scroll_x();
    }
    if bool_prop(props, "scrollY") {
        out.spec = std::mem::take(&mut out.spec).scroll_y();
    }
    if let Some(v) = props.get("float") {
        out.spec = std::mem::take(&mut out.spec).float(float_of(v)?);
    }
    out.key_focus = bool_prop(props, "keyFocus");
    out.title = props
        .get("title")
        .and_then(Json::as_str)
        .map(str::to_string);
    // The table drives the rest; unknown names are ignored (element-level
    // props like `initial` or `src` land here too and fall through).
    for (k, v) in props {
        if v.is_null() {
            continue;
        }
        let Some(def) = by_name(k) else { continue };
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
        };
        p.insert("kind".into(), Json::String(kind.into()));
        if let Some(names) = values {
            p.insert(
                "values".into(),
                Json::Array(names.iter().map(|n| Json::String((*n).into())).collect()),
            );
        }
        let target = match def.apply {
            Apply::StyleF32(_) | Apply::StyleColor(_) | Apply::StyleEnum(_) => "style",
            _ => "spec",
        };
        p.insert("target".into(), Json::String(target.into()));
        p.insert("doc".into(), Json::String(def.doc.into()));
        o.insert(def.name.into(), Json::Object(p));
    }
    for (name, id) in [
        ("dir", P_DIR),
        ("size", P_SIZE),
        ("pad", P_PAD),
        ("border", P_BORDER),
        ("overflow", P_OVERFLOW),
        ("float", P_FLOAT),
        ("keyFocus", P_KEY_FOCUS),
        ("key", P_KEY),
        ("title", P_TITLE),
    ] {
        let mut p = JsonMap::new();
        p.insert("id".into(), Json::from(id));
        p.insert("kind".into(), Json::String("custom".into()));
        o.insert(name.into(), Json::Object(p));
    }
    Json::Object(o)
}
