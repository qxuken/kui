//! Props by condition from Lua (ADR 0069): a `when` list in a node's
//! props, read by the core; `min_column`; the lazy form as a child table
//! and as `lazy { }`.

use kui_core::{Core, Extensions, NodeInfo, NodeSpec, Size, Value};
use kui_lua::LuaExtension;

const SCRIPT: &str = r#"
    components = {
      card = function(env, props)
        return row {
          key = "row", width = "grow", gap = 10,
          when = {
            { width = { below = 400, band = 20 }, dir = "column", gap = 4, hidden = { "extra" } },
            { dark = true, bg = 0x202020ff },
          },
          row { key = "a", width = 100, height = 30 },
          row { key = "extra", width = 100, height = 30 },
          {
            needs = { w = 150 },
            render = function() return row { key = "wide", width = 10, height = 10 } end,
            fallback = function() return row { key = "narrow", width = 5, height = 5 } end,
          },
          lazy {
            key = "q",
            query = { width = { above = 500 } },
            render = function() return { row { key = "big1", width = 1, height = 1 } } end,
          },
        }
      end,
      grid = function(env, props)
        return row {
          key = "grid", width = 330, gap = 10, min_column = 100,
          row { key = "c0", width = 1, height = 20 },
          row { key = "c1", width = 1, height = 20 },
          row { key = "c2", width = 1, height = 20 },
          row { key = "c3", width = 1, height = 20 },
        }
      end,
    }
"#;

fn node<'a>(nodes: &'a [NodeInfo], label: &str) -> Option<&'a NodeInfo> {
    nodes.iter().find(|n| n.label.as_deref() == Some(label))
}

/// Builds a frame as a runner does, once more when a lazy branch crossed.
fn frame(core: &mut Core, exts: &mut Extensions, w: f32, name: &str) -> Vec<NodeInfo> {
    core.set_inspect(true);
    loop {
        let mut ui = core.frame_with(Size::new(w, 300.0), 1.0, exts);
        ui.configure_root(NodeSpec::column().fill());
        ui.component(name, &Value::Null);
        ui.finish();
        if !core.rerun_owed() {
            break;
        }
    }
    core.nodes()
}

fn exts() -> Extensions {
    let mut exts = Extensions::new();
    exts.push_as(
        "s",
        Box::new(LuaExtension::from_source("s.lua", SCRIPT).unwrap()),
    )
    .unwrap();
    exts
}

#[test]
fn a_when_list_and_a_lazy_child_table_from_lua() {
    let mut exts = exts();
    let mut core = Core::new();
    let nodes = frame(&mut core, &mut exts, 600.0, "s/card");
    let row = node(&nodes, "row").unwrap();
    assert_eq!(row.dir, kui_core::Dir::Row);
    assert!(
        node(&nodes, "wide").is_some(),
        "600 - 200 - 30 of gaps is room for 150"
    );
    assert!(node(&nodes, "big1").is_some(), "the query holds at 600");
    assert_eq!(node(&nodes, "extra").unwrap().rect.w, 100.0);

    let nodes = frame(&mut core, &mut exts, 350.0, "s/card");
    let row = node(&nodes, "row").unwrap();
    assert_eq!((row.dir, row.gap), (kui_core::Dir::Column, 4.0));
    assert_eq!(
        node(&nodes, "extra").unwrap().rect.w,
        0.0,
        "hidden by the entry"
    );
    assert!(node(&nodes, "big1").is_none());
    let w = core.take_warnings();
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn min_column_from_lua() {
    let mut exts = exts();
    let mut core = Core::new();
    let nodes = frame(&mut core, &mut exts, 600.0, "s/grid");
    let c0 = node(&nodes, "c0").unwrap();
    assert!((c0.rect.w - 310.0 / 3.0).abs() < 0.01, "{}", c0.rect.w);
    assert_eq!(node(&nodes, "c3").unwrap().rect.y, 20.0);
}
