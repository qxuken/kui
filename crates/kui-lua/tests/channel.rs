//! Channels from Lua (ADR 0065): a host's channel arrives in a component's
//! props as userdata read by row and column; a script makes and publishes
//! its own; a channel crosses back to Rust as its handle.

use kui_core::channel::{Cell, Channel, Column};
use kui_core::{Core, Extensions, NodeSpec, OriginId, Size, Value};
use kui_lua::LuaExtension;

const KIT: &str = r#"
    components = {
      list = function(env, props)
        local rows = props.rows
        local kids = { text("rows " .. #rows .. " rev " .. rows.rev) }
        for i = 0, 2 do
          kids[#kids + 1] = text(rows:get(i, "name") .. " " .. rows:get(i, "ms"))
        end
        local r = rows:row(1)
        kids[#kids + 1] = text("row1 " .. r.name)
        return column(kids)
      end,
    }
"#;

fn texts(core: &mut Core) -> Vec<String> {
    core.nodes().iter().filter_map(|n| n.text.clone()).collect()
}

#[test]
fn a_hosts_channel_reads_by_row_in_a_lua_component() {
    let names: Vec<String> = (0..100_000).map(|i| format!("job {i}")).collect();
    let ch = Channel::table(vec![
        ("name".into(), Column::strs(&names)),
        (
            "ms".into(),
            Column::F64((0..100_000).map(|i| i as f64 / 2.0).collect()),
        ),
    ])
    .unwrap();
    let mut exts = Extensions::new();
    exts.push_as(
        "kit",
        Box::new(LuaExtension::from_source("kit.lua", KIT).unwrap()),
    )
    .unwrap();
    let mut core = Core::new();
    core.set_inspect(true);
    let props = Value::map([("rows", ch.handle())]);
    let mut ui = core.frame_with(Size::new(300.0, 300.0), 1.0, &mut exts);
    ui.configure_root(NodeSpec::column());
    ui.component("kit/list", &props);
    ui.finish();
    let t = texts(&mut core);
    assert!(t.contains(&"rows 100000 rev 1".to_string()), "{t:?}");
    assert!(t.contains(&"job 2 1.0".to_string()), "{t:?}");
    assert!(t.contains(&"row1 job 1".to_string()), "{t:?}");
    assert!(core.take_warnings().is_empty());
    let _ = OriginId::HOST;
}

#[test]
fn a_script_makes_reads_and_publishes_its_own() {
    let ext = LuaExtension::from_source(
        "make.lua",
        r#"
        local ch = channel { name = { "a", "b" }, n = { 1, 2 }, x = { 1.5, 2 } }
        names = table.concat(ch.names, ",")
        n_kind = math.type(ch:get(1, "n"))
        x1 = ch:get(1, "x")
        col = ch:column("name")
        local next = ch:publish { name = { "a", "b", "c" } }
        published = { #next, next.rev, next.id == ch.id }
        bad = tostring(select(2, pcall(channel, { a = { 1 }, b = { 1, 2 } })))
        mixed = tostring(select(2, pcall(channel, { a = { 1, "x" } })))
        handle_of = ch
        function view(env) return column {} end
        "#,
    )
    .unwrap();
    let g = ext.lua().globals();
    assert_eq!(g.get::<String>("names").unwrap(), "n,name,x");
    assert_eq!(g.get::<String>("n_kind").unwrap(), "integer");
    assert_eq!(g.get::<f64>("x1").unwrap(), 2.0);
    let col: Vec<String> = g
        .get::<mlua::Table>("col")
        .unwrap()
        .sequence_values()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(col, ["a", "b"]);
    let p: mlua::Table = g.get("published").unwrap();
    assert_eq!(p.get::<i64>(1).unwrap(), 3);
    assert_eq!(p.get::<i64>(2).unwrap(), 2);
    assert!(p.get::<bool>(3).unwrap());
    assert!(g.get::<String>("bad").unwrap().contains("one length"));
    assert!(
        g.get::<String>("mixed")
            .unwrap()
            .contains("row 2 is a string")
    );
    // A script's channel crosses to Rust as its handle, which names the
    // identity: it resolves to the latest revision, the published one.
    let v = kui_lua::lua_to_value(&g.get::<mlua::Value>("handle_of").unwrap()).unwrap();
    let ch = Channel::resolve(&v).expect("held by the script");
    assert_eq!((ch.len(), ch.rev()), (3, 2));
    assert_eq!(ch.get(0, ch.col("name").unwrap()), Cell::Str("a"));
}
