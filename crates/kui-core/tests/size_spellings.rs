//! The size spellings every binding is run through (backlog RG94):
//! `tests/fixtures/size_spellings.json`, one row per spelling, read here
//! by the core — as a prop's spelling or data, nested in a `max`, and as
//! a keyframe stop's width — by kui-lua's `size_spellings.rs` and by the
//! Node package's `test.mjs`. What the core refuses every binding
//! refuses, and what it takes lays out alike.

use kui_core::Sizing;
use kui_core::calc;
use kui_core::value::Value;

const ROOM: f32 = 1000.0;

fn value_of(j: &serde_json::Value) -> Value {
    match j {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(i) => Value::Int(i),
            None => Value::Float(n.as_f64().unwrap()),
        },
        serde_json::Value::String(s) => Value::Str(s.clone()),
        serde_json::Value::Array(xs) => Value::List(xs.iter().map(value_of).collect()),
        serde_json::Value::Object(m) => {
            Value::Map(m.iter().map(|(k, v)| (k.clone(), value_of(v))).collect())
        }
    }
}

fn px(s: Sizing) -> f32 {
    match s {
        Sizing::Fixed(v) => v,
        Sizing::Percent(f) => f * ROOM,
        Sizing::Calc(c) => c.resolve(ROOM),
        other => panic!("{other:?} is no size"),
    }
}

/// A prop's reading: a string through the schema's `sizing_str`, a
/// number as px, data through `calc::sizing_value`.
fn as_prop(v: &Value) -> Result<f32, String> {
    match v {
        Value::Str(s) => kui_core::schema::sizing_str(s).map(px),
        Value::Int(_) | Value::Float(_) => Ok(v.as_float().unwrap() as f32),
        v => calc::sizing_value(v).map(px),
    }
}

/// A keyframe stop's `width`, which a Node view hands over as JSON.
fn as_stop(v: &Value) -> Result<f32, String> {
    let stops = kui_core::keyframes::parse(&Value::List(vec![Value::map([("width", v.clone())])]))?;
    Ok(px(stops[0].slots.width.expect("a width")))
}

#[test]
fn every_spelling_reads_as_the_table_says() {
    let table: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/size_spellings.json")).unwrap();
    let rows = table["rows"].as_array().unwrap();
    let mut failures = Vec::new();
    for row in rows {
        let v = value_of(&row["width"]);
        let want = row["px"].as_f64().map(|p| p as f32);
        let mut check = |how: &str, got: Result<f32, String>| {
            let ok = match (want, &got) {
                (Some(w), Ok(g)) => (w - g).abs() < 1e-3,
                (None, Err(_)) => true,
                _ => false,
            };
            if !ok {
                failures.push(format!("{} as {how}: {got:?}, want {want:?}", row["width"]));
            }
        };
        check("a prop", as_prop(&v));
        check("a keyframe stop", as_stop(&v));
        if matches!(v, Value::Str(_) | Value::Int(_) | Value::Float(_)) {
            let nested = Value::map([("max", Value::List(vec![v.clone()]))]);
            check("nested in a max", calc::sizing_value(&nested).map(px));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
