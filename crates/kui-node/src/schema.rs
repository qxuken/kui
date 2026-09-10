//! The JSON face of the prop schema. The rows themselves live in
//! `kui_core::schema` (shared with Lua and pinned by the C parity test);
//! this module parses JSON values by kind and exports the rows for the JS
//! encoder and the TS type generator (`protocol_props`). The binary
//! decoder in `binary.rs` reads by the same kinds.
//!
//! `parse_props_json` used to lower a whole frame; since the JSON transport
//! went away its only caller is `measureText`, which reads the `style` half
//! and drops the rest. The spec half it still builds has no caller — see
//! D1/D2 in `docs/backlog/closed-2026-09.md`.

use kui_core::diag::{WARNINGS, WarningDef};
pub use kui_core::schema::*;
use kui_core::{
    Align, Color, FloatConfig, NodeSpec, OVERFLOW_CLIP, OVERFLOW_SCROLL_X, OVERFLOW_SCROLL_Y,
    PadShorthand, Sizing, TextStyle,
};
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

/// A preset name, in either place JSON spells one: `float="below"` and the
/// object form's `anchor`. Which names exist and what each one attaches to
/// is [`FloatConfig::preset`]'s call, not ours.
fn float_preset(name: &str) -> Result<FloatConfig> {
    FloatConfig::preset(name).ok_or_else(|| {
        err(format!(
            "bad float preset {name:?} (one of {})",
            kui_core::FLOAT_PRESETS.join(" | ")
        ))
    })
}

/// An `[x, y]` attach point, or `None` when the key is absent.
fn attach_of(m: &JsonMap<String, Json>, key: &str) -> Result<Option<(Align, Align)>> {
    match m.get(key).and_then(Json::as_array) {
        Some(a) if a.len() == 2 => Ok(Some((align_of(&a[0])?, align_of(&a[1])?))),
        Some(_) => Err(err(format!("float {key} must be [x, y]"))),
        None => Ok(None),
    }
}

pub fn float_of(v: &Json) -> Result<FloatConfig> {
    match v {
        Json::String(s) => float_preset(s),
        Json::Object(m) => Ok(FloatConfig::build(
            match m.get("anchor").and_then(Json::as_str) {
                Some(name) => float_preset(name)?,
                None => FloatConfig::parent(),
            },
            attach_of(m, "at")?,
            attach_of(m, "self")?,
            f32_prop(m, "dx"),
            f32_prop(m, "dy"),
            bool_prop(m, "fit"),
        )),
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
        Kind::Min => Parsed::Min(match v {
            Json::Number(n) => kui_core::Min::px(n.as_f64().unwrap_or(0.0) as f32),
            Json::String(s) => kui_core::schema::min_str(s).map_err(err)?,
            _ => return Err(err("bad min (number | \"fit\")")),
        }),
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

/// `bit` when the boolean prop is on; the caller ORs the family together
/// and hands the number to [`NodeSpec::overflow_bits`].
fn bits(props: &JsonMap<String, Json>, key: &str, bit: u32) -> u32 {
    if bool_prop(props, key) { bit } else { 0 }
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
    // Composites: JSON says which names it saw, the core decides what they
    // add up to.
    out.apply_pad(PadShorthand {
        all: f32_prop(props, "pad"),
        x: f32_prop(props, "padX"),
        y: f32_prop(props, "padY"),
        l: f32_prop(props, "padL"),
        r: f32_prop(props, "padR"),
        t: f32_prop(props, "padT"),
        b: f32_prop(props, "padB"),
    });
    if let Some(w) = f32_prop(props, "borderW") {
        let color = match props.get("borderColor") {
            Some(v) => color_of(v)?,
            None => Color::TRANSPARENT,
        };
        out.with_spec(|s| s.border(w, color));
    }
    let overflow = bits(props, "clip", OVERFLOW_CLIP)
        | bits(props, "scrollX", OVERFLOW_SCROLL_X)
        | bits(props, "scrollY", OVERFLOW_SCROLL_Y);
    out.with_spec(|s| s.overflow_bits(overflow));
    if let Some(v) = props.get("float") {
        let cfg = float_of(v)?;
        out.with_spec(|s| s.float(cfg));
    }
    out.key_focus = bool_prop(props, "keyFocus");
    if let Some(hint) = props.get("tooltip").and_then(Json::as_str) {
        out.apply_tooltip(hint);
    }
    out.title = props
        .get("title")
        .and_then(Json::as_str)
        .map(str::to_string);
    // The table drives the rest; unknown names are ignored (element-level
    // props like `initial` or `src` land here too and fall through). This
    // path is `measureText`'s — a query, not a frame — so it stays silent
    // about them; a view's names are checked in the encoder, which is the
    // only side that sees a name with no wire id. A null is absent — except
    // for a tag, where it declares the behaviour without a tag on its
    // events.
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
            Kind::Min => ("min", None),
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
        // Every JSX spelling of the composite, so the encoder can tell one
        // from a misspelling (`borderW` is real, `borderWidth` is not).
        p.insert(
            "names".into(),
            Json::Array(
                c.jsx_names
                    .iter()
                    .map(|n| Json::String((*n).into()))
                    .collect(),
            ),
        );
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

/// The element, event, resource, warning and env tables as data, for the
/// docs generator (and, for warnings, the `WarningCode` union in index.d.ts).
pub fn protocol_tables() -> Vec<(&'static str, Json)> {
    vec![
        ("elements", {
            let mut rows = table(
                ELEMENTS,
                &[
                    ("name", |e: &ElementDef| e.name),
                    ("jsx", |e| e.jsx),
                    ("lua", |e| e.lua),
                    ("c", |e| e.c),
                    ("doc", |e| e.doc),
                ],
            );
            // The element's own props, for the encoder's unknown-prop
            // check; the docs generator reads the columns by name and
            // ignores this one.
            for (row, def) in rows.as_array_mut().into_iter().flatten().zip(ELEMENTS) {
                row["own"] = Json::Array(
                    def.jsx_own
                        .iter()
                        .map(|n| Json::String((*n).into()))
                        .collect(),
                );
                // And the rows it admits when not every one — the
                // encoder both checks against the list and writes only
                // those, so a closed composite's look is never rebuilt.
                if let Some(admitted) = def.jsx_rows {
                    row["rows"] =
                        Json::Array(admitted.iter().map(|n| Json::String((*n).into())).collect());
                }
            }
            rows
        }),
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
        (
            "warnings",
            table(
                WARNINGS,
                &[("code", |w: &WarningDef| w.code), ("doc", |w| w.doc)],
            ),
        ),
        // The three name lists index.d.ts used to spell by hand and fell
        // behind on (`terminal` reached `Role::ALL` and not `AccessRole`):
        // every role a tree can report, every request a node can take,
        // every role a menu row can carry. The generator writes the
        // unions from these, so a list that grows reaches the types.
        (
            "accessRoles",
            Json::Array(
                kui_core::Role::ALL
                    .iter()
                    .map(|r| Json::String(r.name().into()))
                    .collect(),
            ),
        ),
        (
            "accessActions",
            Json::Array(
                kui_core::AccessAction::ALL
                    .iter()
                    .map(|a| Json::String(a.name().into()))
                    .collect(),
            ),
        ),
        (
            "menuRoles",
            Json::Array(
                kui_core::MenuRole::ALL
                    .iter()
                    .map(|r| Json::String(r.name().into()))
                    .collect(),
            ),
        ),
        // The env reading, with the per-binding key paths as arrays: the
        // docs generator lays them out, and test.mjs reads `node` back to
        // check `ctx.env()` against it key for key.
        (
            "env",
            Json::Array(
                ENV_FIELDS
                    .iter()
                    .map(|f| {
                        let keys = |ks: &[&str]| {
                            Json::Array(ks.iter().map(|k| Json::String((*k).into())).collect())
                        };
                        Json::Object(
                            [
                                ("name", Json::String(f.name.into())),
                                ("from", Json::String(f.from.into())),
                                ("node", keys(f.node)),
                                ("lua", keys(f.lua)),
                                ("c", Json::String(f.c.into())),
                                ("doc", Json::String(f.doc.into())),
                            ]
                            .into_iter()
                            .map(|(k, v)| (k.to_string(), v))
                            .collect(),
                        )
                    })
                    .collect(),
            ),
        ),
        // The palette, the same way: one row per colour role, with the
        // Node spelling beside the name every other binding uses. Not an
        // env field — it is derived from `system` rather than reported by
        // the host (ADR 0019) — so it is its own table, and the docs
        // generator gives it its own section.
        (
            "theme",
            Json::Array(
                kui_core::schema::THEME_ROLES
                    .iter()
                    .map(|r| {
                        Json::Object(
                            [
                                ("name", Json::String(r.name.into())),
                                ("node", Json::String(r.node.into())),
                                ("doc", Json::String(r.doc.into())),
                                (
                                    "dark",
                                    Json::String(format!(
                                        "#{:08x}",
                                        (r.get)(&kui_core::Theme::dark()).to_hex()
                                    )),
                                ),
                                (
                                    "light",
                                    Json::String(format!(
                                        "#{:08x}",
                                        (r.get)(&kui_core::Theme::light()).to_hex()
                                    )),
                                ),
                            ]
                            .into_iter()
                            .map(|(k, v)| (k.to_string(), v))
                            .collect(),
                        )
                    })
                    .collect(),
            ),
        ),
    ]
}
