//! What a schema may say, what it may not and why, and the files it becomes.

use kui_addon_schema::{Schema, Type, generate};

const SMALL: &str = r#"
return {
  addon = "demo",
  doc = "A demo add-on.",
  enums = { { name = "Size", doc = "How big.", "small", "large" } },
  records = {
    { name = "Choice", doc = "A choice.", fields = {
      { "value", "string", doc = "What it is." },
      { "hint", "string?" },
    } },
  },
  components = {
    { "chip", doc = "A chip.",
      props = {
        { "label", "string", doc = "Its text." },
        { "size", "Size?" },
        { "choices", "{Choice}?" },
        { "rows", "channel?" },
      },
      events = { { "pick", doc = "Picked.", fields = { { "value", "string" } } } },
      children = { { "", doc = "is its body." }, { "trailing", doc = "goes at the end." } } },
  },
}
"#;

fn refused(source: &str) -> String {
    match Schema::from_lua(source) {
        Ok(_) => panic!("accepted:\n{source}"),
        Err(e) => e.to_string(),
    }
}

/// SMALL with `from` replaced by `to`, which must be in it.
fn with(from: &str, to: &str) -> String {
    assert!(SMALL.contains(from), "{from}");
    SMALL.replacen(from, to, 1)
}

#[test]
fn a_schema_reads_into_its_parts() {
    let s = Schema::from_lua(SMALL).unwrap();
    assert_eq!(s.addon, "demo");
    assert_eq!(s.enums[0].values, ["small", "large"]);
    assert_eq!(s.records[0].fields[1].ty, Type::Optional(Box::new(Type::String)));
    let c = &s.components[0];
    assert_eq!(c.name, "chip");
    assert_eq!(c.props.len(), 4);
    assert_eq!(c.events[0].fields[0].name, "value");
    let groups: Vec<_> = c.children.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(groups, ["", "trailing"]);
}

#[test]
fn names_must_be_spelled_as_lua_spells_them() {
    assert!(refused(&with(r#"addon = "demo""#, r#"addon = "Demo""#)).contains("snake_case"));
    assert!(refused(&with(r#"{ "chip","#, r#"{ "Chip","#)).contains("snake_case"));
    assert!(refused(&with(r#"{ "label", "string""#, r#"{ "Label", "string""#)).contains("snake_case"));
    assert!(refused(&with(r#"name = "Size""#, r#"name = "size""#)).contains("capital"));
}

#[test]
fn a_type_that_would_shadow_a_bindings_own_is_refused() {
    let e = refused(&with(r#"name = "Choice""#, r#"name = "Option""#).replace("{Choice}", "{Option}"));
    assert!(e.contains("shadow"), "{e}");
}

#[test]
fn an_undefined_or_twice_defined_type_is_refused() {
    let e = refused(&with(r#""Size?""#, r#""Weight?""#));
    assert!(e.contains("no type `Weight`"), "{e}");
    let e = refused(&with(r#"name = "Choice""#, r#"name = "Size""#));
    assert!(e.contains("defined twice"), "{e}");
}

#[test]
fn key_fresh_and_on_props_belong_to_the_core_and_to_events() {
    for name in ["key", "fresh", "on_pick"] {
        let e = refused(&with(r#"{ "label", "string""#, &format!(r#"{{ "{name}", "string""#)));
        assert!(e.contains("declare events"), "{name}: {e}");
    }
}

#[test]
fn an_events_fields_are_what_every_binding_reads_off_a_payload() {
    let e = refused(&with(r#"fields = { { "value", "string" } }"#, r#"fields = { { "value", "Choice" } }"#));
    assert!(e.contains("an event's field"), "{e}");
    let e = refused(&with(r#"{ "pick", doc"#, r#"{ "Pick", doc"#));
    assert!(e.contains("snake_case"), "{e}");
}

#[test]
fn a_bad_type_says_where_it_is() {
    let e = refused(&with(r#""channel?""#, r#""{channel""#));
    assert!(e.contains("chip.rows"), "{e}");
}

#[test]
fn it_writes_a_file_for_every_binding() {
    let s = Schema::from_lua(SMALL).unwrap();
    let mut paths: Vec<_> = generate(&s).into_iter().map(|f| f.path).collect();
    paths.sort();
    assert_eq!(
        paths,
        [
            "c/demo.h",
            "lua/demo/ui.lua",
            "node/demo.d.ts",
            "node/demo.js",
            "odin/demo/demo.odin",
            "reference.md",
            "rust/Cargo.toml",
            "rust/src/lib.rs",
            "teal/demo/ui.d.tl",
        ]
    );
    for f in generate(&s) {
        assert!(f.text.contains("do not edit"), "{} has no banner", f.path);
        if !f.path.ends_with(".toml") {
            assert!(f.text.contains("chip"), "{} has no chip", f.path);
        }
    }
}

#[test]
fn the_reference_has_a_section_a_component_with_its_props_events_and_children() {
    let s = Schema::from_lua(SMALL).unwrap();
    let md = generate(&s).into_iter().find(|f| f.path == "reference.md").unwrap().text;
    assert!(md.contains("## demo.chip\n\nA chip."), "{md}");
    assert!(md.contains("| `label` | `label` | string | Its text. |"), "{md}");
    assert!(md.contains("| `size` | `size` | [Size](#size)? |"), "{md}");
    assert!(md.contains("{[Choice](#choice)}?"), "{md}");
    assert!(md.contains("| `on_pick` | `onPick` | `pick` = { `value`: string"), "{md}");
    assert!(md.contains("- `trailing` goes at the end."), "{md}");
    assert!(md.contains("### Size\n\nHow big.") && md.contains("`\"small\"`"), "{md}");
    assert!(md.contains("### Choice") && md.contains("| `hint` | `hint` | string? |"), "{md}");
}

#[test]
fn every_binding_places_under_the_addons_namespace_and_can_take_another() {
    let s = Schema::from_lua(SMALL).unwrap();
    let text = |p: &str| generate(&s).into_iter().find(|f| f.path == p).unwrap().text;
    assert!(text("lua/demo/ui.lua").contains(r#"namespace .. "/chip""#));
    assert!(text("lua/demo/ui.lua").contains("function M.at(other)"));
    assert!(text("rust/src/lib.rs").contains(r#"ui.component("demo/chip""#));
    assert!(text("rust/src/lib.rs").contains("pub fn place_in("));
    assert!(text("c/demo.h").contains(r#"DEMO_NAMESPACE "/chip""#));
    assert!(text("odin/demo/demo.odin").contains("chip_in :: proc"));
    assert!(text("node/demo.js").contains("/chip"));
}

#[test]
fn optional_props_are_left_out_unset_rather_than_sent_as_null() {
    let s = Schema::from_lua(SMALL).unwrap();
    let text = |p: &str| generate(&s).into_iter().find(|f| f.path == p).unwrap().text;
    let c = text("c/demo.h");
    assert!(c.contains("if (p->size != 0)"), "an unset enum is zero in C");
    assert!(c.contains("if (p->rows != NULL)"));
    let rust = text("rust/src/lib.rs");
    assert!(rust.contains("if let Some(v) = &self.size"));
    let odin = text("odin/demo/demo.odin");
    assert!(odin.contains("if x, ok := p.size.?; ok"));
}

#[test]
fn keywords_are_escaped_where_a_language_would_refuse_them() {
    let source = with(r#"{ "chip","#, r#"{ "switch","#);
    let s = Schema::from_lua(&source).unwrap();
    let odin = generate(&s).into_iter().find(|f| f.path.ends_with(".odin")).unwrap().text;
    assert!(odin.contains("switch_ :: proc"), "Odin's `switch` is a keyword");
}
