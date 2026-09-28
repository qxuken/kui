//! The prop schema as the addon exports it. The rows themselves live in
//! `kui_core::schema` (shared with Lua and pinned by the C parity test);
//! this module hands them to the JS encoder and the TS type generator
//! (`protocol_props`, `protocol_tables`). The binary decoder in
//! `binary.rs` reads by the same kinds.
//!
//! There is no JSON prop parser here any more. One lowered whole frames
//! before the binary transport (D1/D2 in `docs/backlog/closed-2026-09.md`)
//! and then lived on for `measureText` alone, building a `NodeSpec` nobody
//! read; `measureText` now encodes its text the way a frame does and the
//! decoder reads it, so the encoder is the only door for props of any kind.

use kui_core::diag::{WARNINGS, WarningDef};
pub use kui_core::schema::*;
use serde_json::{Map as JsonMap, Value as Json};

use crate::{Result, err};

/// `kui_core::schema::apply` with the error lifted into napi's.
pub fn apply(def: &PropDef, value: Parsed, out: &mut PropsOut) -> Result<()> {
    kui_core::schema::apply(def, value, out).map_err(err)
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
            Kind::Max => ("max", None),
            Kind::Msg => ("msg", None),
            Kind::Tag => ("tag", None),
            Kind::Str => ("str", None),
            // A string on the wire; the stock three listed for the types.
            Kind::Family => ("family", Some(kui_core::schema::FAMILIES)),
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
        // The two `image` rows' value lists (ADR 0025, decision 4), in the
        // order the decoder indexes them.
        (
            "imageSampling",
            Json::Array(
                kui_core::Sampling::ALL
                    .iter()
                    .map(|s| Json::String(s.name().into()))
                    .collect(),
            ),
        ),
        (
            "imageFit",
            Json::Array(
                kui_core::ImageFit::ALL
                    .iter()
                    .map(|f| Json::String(f.name().into()))
                    .collect(),
            ),
        ),
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
        // A menu row's keys and the name its dropped ones are reported
        // under (backlog RG10): the encoder checks a select's option
        // objects against `keys` and hands the rest to `warnUnknownProps`
        // as `[name, key]` pairs, which `diag::unknown_prop` routes to the
        // row's own wording.
        (
            "menuItem",
            Json::Object(
                [
                    (
                        "name".to_string(),
                        Json::String(kui_core::MenuItem::NAME.into()),
                    ),
                    (
                        "keys".to_string(),
                        Json::Array(
                            kui_core::MenuItem::KEYS
                                .iter()
                                .map(|k| Json::String((*k).into()))
                                .collect(),
                        ),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        ),
        // And the two `ctx.key` / `ctx.mouse` spellings, for the same
        // reason: `EditKeyName` was the last hand-written union.
        (
            "editKeys",
            Json::Array(
                kui_core::EditKey::ALL
                    .iter()
                    .map(|k| Json::String(k.name().into()))
                    .collect(),
            ),
        ),
        // A window is not a node, but its `kind` is a list the core owns.
        (
            "windowKinds",
            Json::Array(
                kui_core::WindowKind::ALL
                    .iter()
                    .map(|k| Json::String(k.name().into()))
                    .collect(),
            ),
        ),
        (
            "mouseButtons",
            Json::Array(
                kui_core::MouseButton::NAMED
                    .iter()
                    .filter_map(|b| b.name())
                    .map(|n| Json::String(n.into()))
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
        // The metrics (backlog T2): one row per size the stock widgets are
        // built from, with the stock and the compact value beside the
        // doc, the way the palette carries its two bases.
        (
            "metrics",
            Json::Array(
                kui_core::schema::METRIC_ROLES
                    .iter()
                    .map(|r| {
                        Json::Object(
                            [
                                ("name", Json::String(r.name.into())),
                                ("node", Json::String(r.node.into())),
                                ("doc", Json::String(r.doc.into())),
                                (
                                    "stock",
                                    Json::from((r.get)(&kui_core::Metrics::default()) as f64),
                                ),
                                (
                                    "compact",
                                    Json::from((r.get)(&kui_core::Metrics::compact()) as f64),
                                ),
                                // The pair for a row that is the platform's
                                // (W13): `stock` and `compact` above are the
                                // running platform's reading, and a generator
                                // prints this instead so its output does not
                                // say where it ran.
                                (
                                    "platform",
                                    r.platform.map_or(Json::Null, |p| {
                                        Json::Object(
                                            [
                                                ("windows", Json::from(p.windows as f64)),
                                                ("elsewhere", Json::from(p.elsewhere as f64)),
                                            ]
                                            .into_iter()
                                            .map(|(k, v)| (k.to_string(), v))
                                            .collect(),
                                        )
                                    }),
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
