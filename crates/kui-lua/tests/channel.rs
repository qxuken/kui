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

/// Backlog C95: a script buckets a range in Rust and reads a flat list
/// per field, rows 0-based; the edges are the core's (an empty range, more
/// buckets than rows, NaN), and a column of strings is an error.
#[test]
fn a_script_reduces_a_range_into_buckets() {
    let ext = LuaExtension::from_source(
        "reduce.lua",
        r#"
        local ys, ss = {}, {}
        for i = 1, 1000 do ys[i] = (i - 1) % 10; ss[i] = "x" end
        ys[3] = 0 / 0
        local ch = channel { y = ys, s = ss }
        local r = ch:reduce("y", 0, 1000, 4)
        count = #r.min
        b0 = { r.min[1], r.max[1], r.first[1], r.last[1],
               r.min_row[1], r.max_row[1], r.first_row[1], r.last_row[1] }
        -- Columns go in name order, so 1 is `y`.
        by_index = #ch:reduce(1, 10, 20, 100).max
        empty = #ch:reduce("y", 500, 500, 8).min
        past = #ch:reduce("y", 998, 5000, 8).min
        gap = ch:reduce("y", 2, 3, 1).min[1]
        bad = tostring(select(2, pcall(ch.reduce, ch, "s", 0, 10, 2)))
        function view(env) return column {} end
        "#,
    )
    .unwrap();
    let g = ext.lua().globals();
    assert_eq!(g.get::<i64>("count").unwrap(), 4);
    let b0: Vec<f64> = g
        .get::<mlua::Table>("b0")
        .unwrap()
        .sequence_values()
        .collect::<Result<_, _>>()
        .unwrap();
    // Rows 0..250: values (i % 10), row 2 NaN; the first 0 is row 0, the
    // first 9 row 9, the last row 249 holds 9.
    assert_eq!(b0, [0.0, 9.0, 0.0, 9.0, 0.0, 9.0, 0.0, 249.0]);
    assert_eq!(g.get::<i64>("by_index").unwrap(), 10, "a bucket a row");
    assert_eq!(g.get::<i64>("empty").unwrap(), 0);
    assert_eq!(g.get::<i64>("past").unwrap(), 2, "`to` stops at the end");
    assert!(g.get::<f64>("gap").unwrap().is_nan());
    assert!(g.get::<String>("bad").unwrap().contains("strings"));
}

/// A column is made straight from the Lua values (backlog C143): numbers
/// that start whole and turn fractional are floats, whole ones integers,
/// and a stray kind says which row.
#[test]
fn a_scripts_columns_keep_their_kinds() {
    let ext = LuaExtension::from_source(
        "kinds.lua",
        r#"
        local ch = channel { i = { 1, 2, 3 }, f = { 1, 2.5, 3 }, b = { true, false, true }, s = { "x", "yz", "" } }
        kinds = { math.type(ch:get(0, "i")), math.type(ch:get(0, "f")), tostring(ch:get(1, "b")), ch:get(1, "s") }
        empty = #channel { e = {} }
        stray = tostring(select(2, pcall(channel, { a = { true, 1 } })))
        function view(env) return column {} end
        "#,
    )
    .unwrap();
    let g = ext.lua().globals();
    let kinds: Vec<String> = g.get("kinds").unwrap();
    assert_eq!(kinds, ["integer", "float", "false", "yz"]);
    assert_eq!(g.get::<i64>("empty").unwrap(), 0);
    let stray: String = g.get("stray").unwrap();
    assert!(
        stray.contains("row 2 is a number, in a column of booleans"),
        "{stray}"
    );
}
