//! Contribution slots from Lua (ADR 0067): a script's `contributes` adds
//! its components to slots a host declares, a script declares one with
//! `slot(name, props)`, a contributor is told which slot it is in, and its
//! events reach the host as the host's message with `from` naming it.

use kui_core::testing::click_at;
use kui_core::{Core, EventKind, Extensions, NodeSpec, Size, UiEvent, Value};
use kui_lua::LuaExtension;

/// Contributes `save` by the map spelling; its button names its slot.
const SAVER: &str = r#"
    contributes = { ["app.toolbar"] = "save" }
    components = {
      save = function(env, props, inst)
        return row { key = "b", width = 60, height = 20, role = "button",
          label = "save in " .. tostring(inst.slot) .. " for " .. tostring(props.title),
          on_click = { kind = "press" } }
      end,
    }
    function on_event(ev)
      if ev.kind == "press" then return emit("on_run", { what = "save" }) end
    end
"#;

/// Contributes `open` ahead of everyone by the list spelling.
const OPENER: &str = r#"
    contributes = { { slot = "app.toolbar", component = "open", order = -1 } }
    components = {
      open = function(env, props, inst)
        return row { key = "b", width = 60, height = 20, role = "button",
          label = "open", on_click = { kind = "press" } }
      end,
    }
    function on_event(ev)
      if ev.kind == "press" then return emit("on_run", { what = "open" }) end
    end
"#;

fn exts(scripts: &[(&str, &str)]) -> Extensions {
    let mut exts = Extensions::new();
    for (ns, src) in scripts {
        exts.push_as(*ns, Box::new(LuaExtension::from_source(*ns, src).unwrap()))
            .unwrap();
    }
    exts
}

fn frame(core: &mut Core, exts: &mut Extensions) -> usize {
    let mut ui = core.frame_with(Size::new(400.0, 100.0), 1.0, exts);
    ui.configure_root(NodeSpec::row().fill());
    let n = ui.slot(
        "app.toolbar",
        &Value::map([
            ("title", "files".into()),
            ("on_run", Value::map([("kind", "run".into())])),
        ]),
    );
    ui.finish();
    n
}

fn names(core: &mut Core) -> Vec<String> {
    core.access_tree()
        .nodes
        .iter()
        .filter(|n| n.origin.0 > 0)
        .filter_map(|n| n.name.clone())
        .collect()
}

#[test]
fn scripts_contribute_in_order_and_know_their_slot() {
    let mut exts = exts(&[("saver", SAVER), ("opener", OPENER)]);
    let mut core = Core::new();
    assert_eq!(frame(&mut core, &mut exts), 2);
    assert_eq!(
        names(&mut core),
        ["open", "save in app.toolbar for files"],
        "the list's order -1 ahead of load order"
    );
    assert!(core.take_warnings().is_empty());
}

#[test]
fn a_contributions_emit_is_the_hosts_message_from_the_script() {
    let mut exts = exts(&[("saver", SAVER), ("opener", OPENER)]);
    let mut core = Core::new();
    frame(&mut core, &mut exts);
    // `open` is first, 60 px wide.
    let mut host: Vec<UiEvent> = Vec::new();
    exts.route(click_at(&mut core, 90.0, 10.0), |ev| host.push(ev));
    assert_eq!(host.len(), 1, "{host:?}");
    assert_eq!(host[0].event, EventKind::Component);
    assert_eq!(host[0].payload.get_str("kind"), Some("run"));
    assert_eq!(
        host[0].payload.get("run").and_then(|r| r.get_str("what")),
        Some("save")
    );
    assert_eq!(host[0].from.as_deref(), Some("saver"));
}

#[test]
fn a_script_declares_a_slot_others_contribute_to() {
    let host = r#"
        function view(env)
          return column { key = "shell",
            text("shell"),
            slot("shell.items", { on_run = { kind = "item" } }),
          }
        end
        heard = {}
        function on_event(ev, at)
          if ev.kind == "item" then heard[#heard + 1] = at.from .. ":" .. ev.run.what end
        end
    "#;
    let item = r#"
        contributes = { ["shell.items"] = "item" }
        components = {
          item = function(env, props, inst)
            return row { key = "b", width = 60, height = 20, role = "button",
              label = "item", on_click = { kind = "press" } }
          end,
        }
        function on_event(ev)
          if ev.kind == "press" then return emit("on_run", { what = "it" }) end
        end
    "#;
    let shell = LuaExtension::from_source("shell", host).unwrap();
    let lua = shell.lua().clone();
    let mut exts = Extensions::new();
    exts.push_as("shell", Box::new(shell)).unwrap();
    exts.push_as(
        "item",
        Box::new(LuaExtension::from_source("item", item).unwrap()),
    )
    .unwrap();
    let mut core = Core::new();
    let mut ui = core.frame_with(Size::new(400.0, 100.0), 1.0, &mut exts);
    ui.configure_root(NodeSpec::row().fill());
    ui.finish();
    let button = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("item"))
        .map(|n| n.rect)
        .expect("the contribution inside the script's view");
    let mut host_events = Vec::new();
    exts.route(click_at(&mut core, button.x + 4.0, button.y + 4.0), |ev| {
        host_events.push(ev)
    });
    assert!(
        host_events.is_empty(),
        "the script placed it, so the script hears it"
    );
    let heard: Vec<String> = lua.globals().get("heard").unwrap();
    assert_eq!(heard, ["item:it"]);
}

#[test]
fn a_bad_contributes_is_refused_with_how_to_write_it() {
    let err = |src: &str| match LuaExtension::from_source("bad", src) {
        Ok(_) => panic!("{src} loaded"),
        Err(e) => e.to_string(),
    };
    assert!(err("contributes = 3").contains("contributes"));
    assert!(err("contributes = { { slot = 'a.b' } }").contains("without its slot and component"));
    // A component it does not provide loads as a script but is refused
    // by the list it joins.
    let ext = LuaExtension::from_source("bad", "contributes = { ['a.b'] = 'nope' }").unwrap();
    let mut exts = Extensions::new();
    let e = exts.push_as("bad", Box::new(ext)).unwrap_err();
    assert!(e.contains("does not provide"), "{e}");
}
