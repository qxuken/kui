//! The size spellings every binding is run through (backlog RG94), from
//! a Lua view: each row of `kui-core/tests/fixtures/size_spellings.json`
//! as a `width`, and a string row nested in `{ max = { … } }` too. A row
//! the table marks `"only": "js"` is JS's spelling (`percent`), which
//! Lua refuses for its own `pct`.

use kui_core::{Core, Extension, OriginId, Size, Slot};
use kui_lua::LuaExtension;

/// A JSON value as a Lua literal.
fn lua(j: &serde_json::Value) -> String {
    match j {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("{s:?}"),
        serde_json::Value::Array(xs) => {
            format!("{{ {} }}", xs.iter().map(lua).collect::<Vec<_>>().join(", "))
        }
        serde_json::Value::Object(m) => format!(
            "{{ {} }}",
            m.iter()
                .map(|(k, v)| format!("{k} = {}", lua(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => panic!("no {other} in the table"),
    }
}

/// The width a box declaring `width` lays out at in a 1000 px room, or
/// the view's error.
fn laid(width: &str) -> Result<f32, String> {
    let src = format!(
        "function view(env)
           return column {{ width = 1000, pad = 0,
             row {{ key = \"r\", width = {width}, height = 4 }} }}
         end"
    );
    let mut ext = LuaExtension::from_source("sizes", &src).map_err(|e| e.to_string())?;
    let mut core = Core::new();
    core.set_inspect(true);
    let mut ui = core.frame(Size::new(1200.0, 100.0), 1.0);
    ui.set_origin(OriginId(1));
    let built = ext.view(&Slot::root(), &mut ui);
    ui.finish();
    built?;
    let nodes = core.nodes();
    Ok(nodes
        .iter()
        .find(|n| n.label.as_deref() == Some("r"))
        .expect("the row")
        .rect
        .w)
}

#[test]
fn every_spelling_lays_out_as_the_table_says() {
    let table: serde_json::Value = serde_json::from_str(include_str!(
        "../../kui-core/tests/fixtures/size_spellings.json"
    ))
    .unwrap();
    let mut failures = Vec::new();
    for row in table["rows"].as_array().unwrap() {
        let w = &row["width"];
        let want = match row["only"].as_str() {
            Some("js") => None,
            _ => row["px"].as_f64().map(|p| p as f32),
        };
        let mut check = |how: &str, got: Result<f32, String>| {
            let ok = match (want, &got) {
                (Some(want), Ok(got)) => (want - got).abs() < 1e-3,
                (None, Err(_)) => true,
                _ => false,
            };
            if !ok {
                failures.push(format!("{w} {how}: {got:?}, want {want:?}"));
            }
        };
        check("as a width", laid(&lua(w)));
        if w.is_string() || w.is_number() {
            check("nested in a max", laid(&format!("{{ max = {{ {} }} }}", lua(w))));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
