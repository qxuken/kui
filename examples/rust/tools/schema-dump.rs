//! Dumps the prop schema (`kui_core::schema`) as JSON, for a binding
//! generated outside Rust.
//!
//!     cargo run -p kui-core --example schema-dump -- target/odin/schema.json
//!
//! The Odin binding's generator reads it (`nu scripts/odin.nu gen`): the
//! `PROPS` and `CUSTOM` rows become its `Spec`, the enum name lists its
//! enums, the `DOORS` rows its verbs, and the element, event, theme, metric
//! and env tables are what it checks its hand-written half against. The
//! keys are the ones kui-node's `protocol()` hands the Node generator,
//! where the two carry the same table.

use kui_core::schema::{self, Cell, Kind, Target};
use serde_json::{Map, Value as Json, json};

fn names(list: &[&str]) -> Json {
    Json::Array(list.iter().map(|n| Json::String((*n).into())).collect())
}

fn cell(c: Cell) -> Json {
    let (cell, text) = match c {
        Cell::Is(t) => ("is", t),
        Cell::As(t) => ("as", t),
        Cell::No(t) => ("no", t),
    };
    json!({ "cell": cell, "text": text })
}

fn props() -> Json {
    let rows = schema::PROPS.iter().map(|def| {
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
            Kind::Family => ("family", Some(schema::FAMILIES)),
            Kind::Resource => ("resource", None),
            Kind::Keyframes => ("keyframes", None),
            Kind::Enter => ("enter", None),
            Kind::Gradient => ("gradient", None),
        };
        let mut p = Map::new();
        p.insert("name".into(), def.name.into());
        p.insert("snake".into(), def.snake_name().into());
        p.insert("id".into(), def.id.into());
        p.insert("kind".into(), kind.into());
        if let Some(values) = values {
            p.insert("values".into(), names(values));
        }
        let target = match def.target() {
            Target::Spec => "spec",
            Target::Style => "style",
        };
        p.insert("target".into(), target.into());
        p.insert("c".into(), schema::c_field(def).into());
        p.insert("doc".into(), def.doc.into());
        Json::Object(p)
    });
    Json::Array(rows.collect())
}

fn main() {
    let custom = schema::CUSTOM.iter().map(|c| {
        json!({
            "name": c.name, "id": c.id, "jsx_names": names(c.jsx_names),
            "lua_names": names(c.lua_names), "c": c.c, "doc": c.doc,
        })
    });
    let elements = schema::ELEMENTS
        .iter()
        .map(|e| json!({ "name": e.name, "c": e.c, "doc": e.doc }));
    let events = schema::EVENTS
        .iter()
        .map(|e| json!({ "kind": e.kind, "payload": e.payload, "doc": e.doc }));
    let doors = schema::DOORS.iter().map(|d| {
        json!({
            "rust": d.rust, "c": cell(d.c), "node": cell(d.node), "lua": cell(d.lua), "doc": d.doc,
        })
    });
    let theme = schema::THEME_ROLES
        .iter()
        .map(|r| json!({ "name": r.name, "doc": r.doc }));
    let metrics = schema::METRIC_ROLES
        .iter()
        .map(|r| json!({ "name": r.name, "doc": r.doc }));
    let env = schema::ENV_FIELDS
        .iter()
        .map(|f| json!({ "name": f.name, "c": f.c, "doc": f.doc }));
    let all = |it: &mut dyn Iterator<Item = &'static str>| Json::Array(it.map(Json::from).collect());

    let out = json!({
        "props": props(),
        "custom": Json::Array(custom.collect()),
        "elements": Json::Array(elements.collect()),
        "events": Json::Array(events.collect()),
        "doors": Json::Array(doors.collect()),
        "theme": Json::Array(theme.collect()),
        "metrics": Json::Array(metrics.collect()),
        "env": Json::Array(env.collect()),
        // The name lists no prop row carries, for the enums of the verbs
        // and readings: what an env setter takes, what a tree reports.
        "lists": {
            "appearances": names(schema::APPEARANCES),
            "motions": names(schema::MOTIONS),
            "assistive": names(schema::ASSISTIVE),
            "audioDevices": names(schema::AUDIO_DEVICES),
            "orientations": names(schema::ORIENTATIONS),
            "live": names(schema::LIVE),
            "accessRoles": all(&mut kui_core::Role::ALL.iter().map(|r| r.name())),
            "accessActions": all(&mut kui_core::AccessAction::ALL.iter().map(|a| a.name())),
            "menuRoles": all(&mut kui_core::MenuRole::ALL.iter().map(|r| r.name())),
            "editKeys": all(&mut kui_core::EditKey::ALL.iter().map(|k| k.name())),
            "windowKinds": all(&mut kui_core::WindowKind::ALL.iter().map(|k| k.name())),
            "mouseButtons": all(&mut kui_core::MouseButton::NAMED.iter().filter_map(|b| b.name())),
            "imageSampling": all(&mut kui_core::Sampling::ALL.iter().map(|s| s.name())),
            "imageFit": all(&mut kui_core::ImageFit::ALL.iter().map(|f| f.name())),
        },
    });
    let text = serde_json::to_string_pretty(&out).expect("the schema is plain data");
    match std::env::args().nth(1) {
        Some(path) => std::fs::write(&path, text).unwrap_or_else(|e| {
            eprintln!("schema-dump: {path}: {e}");
            std::process::exit(1);
        }),
        None => println!("{text}"),
    }
}
