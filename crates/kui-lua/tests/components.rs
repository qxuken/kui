//! Components from Lua (ADR 0060, ADR 0067): a script's `components` table provides
//! them, `children()` puts a host's children where the function says,
//! `emit` raises an event the host hears as its own message — whether the
//! host is Rust or another script — and a script places its own components
//! in place, through the same instance boundary.

use kui_core::testing::click_at;
use kui_core::{Core, EventKind, Extension, Extensions, NodeSpec, OriginId, Size, Value};
use kui_lua::LuaExtension;

const KIT: &str = r#"
    fills = 0
    clicks = 0
    components = {
      card = function(env, props, inst)
        fills = fills + 1
        return column { key = "card", pad = 4, gap = 2,
          text(props.title or "card"),
          text("fills " .. fills),
          children(),
          children("footer"),
        }
      end,
      button = function(env, props)
        return row { width = 60, height = 20, bg = 0x3355ffff,
          on_click = { tag = "pressed" }, label = props.label or "Kit button" }
      end,
      -- Its own card, the host's children forwarded into it.
      pair = function(env, props)
        return column {
          component("kit/card", { title = "inner" }, { children() }),
        }
      end,
    }
    function on_event(ev)
      if ev.tag == "pressed" then
        clicks = clicks + 1
        return emit("on_click", { count = clicks })
      end
    end
"#;

const HOST: &str = r#"
    heard = {}
    function view(env)
      return column { pad = 8, gap = 4,
        component("kit/button", { label = "Save", on_click = { kind = "saved" } }),
        component("kit/card", { title = "one", key = "one" },
          { text("alpha"), footer = { text("foot") } }),
        component("kit/pair", nil, { text("deep") }),
      }
    end
    function on_event(ev)
      heard[#heard + 1] = ev.kind .. ":" .. ev.event .. ":" .. tostring(ev.click and ev.click.count)
      -- Not an emit: dropped (ADR 0067). The host placed nothing of this
      -- script's to hear it as.
      return { heard = heard[#heard] }
    end
"#;

struct Rig {
    core: Core,
    exts: Extensions,
    /// The host script's interpreter, when there is one: what it heard is
    /// read back from its globals.
    app: Option<mlua::Lua>,
}

fn rig(host: Option<&str>) -> Rig {
    let mut exts = Extensions::new();
    exts.push_as(
        "kit",
        Box::new(LuaExtension::from_source("kit.lua", KIT).unwrap()),
    )
    .unwrap();
    let app = host.map(|src| {
        let ext = LuaExtension::from_source("app.lua", src).unwrap();
        let lua = ext.lua().clone();
        exts.push_as("app", Box::new(ext)).unwrap();
        lua
    });
    let mut core = Core::new();
    core.set_inspect(true);
    Rig { core, exts, app }
}

impl Rig {
    fn frame(&mut self, view: impl FnOnce(&mut kui_core::Ui<'_>)) {
        let mut ui = self
            .core
            .frame_with(Size::new(400.0, 300.0), 1.0, &mut self.exts);
        ui.configure_root(NodeSpec::column().pad(8.0).gap(4.0));
        view(&mut ui);
        ui.finish();
    }

    fn texts(&mut self) -> Vec<(String, OriginId)> {
        self.core
            .nodes()
            .iter()
            .filter_map(|n| n.text.clone().map(|t| (t, n.origin)))
            .collect()
    }
}

#[test]
fn a_script_lists_its_components() {
    let kit = LuaExtension::from_source("kit.lua", KIT).unwrap();
    assert_eq!(kit.components(), ["button", "card", "pair"]);
}

#[test]
fn a_rust_host_places_a_lua_component_with_its_own_children() {
    let mut r = rig(None);
    let view = |ui: &mut kui_core::Ui<'_>| {
        ui.component_with(
            "kit/card",
            &Value::map([("title", Value::str("hello"))]),
            |group, ui| match group {
                "" => ui.text("from the host", kui_core::TextStyle::new(12.0)),
                "footer" => ui.text("host footer", kui_core::TextStyle::new(10.0)),
                _ => {}
            },
        );
    };
    r.frame(view);
    let texts = r.texts();
    let origin = |t: &str| texts.iter().find(|(s, _)| s == t).map(|(_, o)| *o);
    assert_eq!(origin("from the host"), Some(OriginId::HOST));
    assert_eq!(origin("host footer"), Some(OriginId::HOST));
    assert_eq!(origin("hello"), Some(OriginId(1)));
    // Replayed: the script was not asked again, so its count holds.
    r.frame(view);
    let texts = r.texts();
    assert!(texts.iter().any(|(t, _)| t == "fills 1"), "{texts:?}");
    assert!(texts.iter().any(|(t, _)| t == "from the host"), "{texts:?}");
    assert_eq!(r.core.take_warnings(), Vec::new());
}

#[test]
fn a_lua_emit_reaches_a_rust_host_as_its_message() {
    let mut r = rig(None);
    let props = Value::map([("onClick", Value::map([("save", Value::Bool(true))]))]);
    r.frame(|ui| {
        ui.component("kit/button", &props);
    });
    let events = click_at(&mut r.core, 12.0, 12.0);
    let mut host = Vec::new();
    r.exts.route(events, |ev| host.push(ev));
    assert_eq!(host.len(), 1, "{host:?}");
    assert_eq!(host[0].event, EventKind::Component);
    assert_eq!(host[0].payload.get("save"), Some(&Value::Bool(true)));
    assert_eq!(
        host[0].payload.get("click").and_then(|c| c.get("count")),
        Some(&Value::Int(1))
    );
}

#[test]
fn a_lua_host_places_components_and_hears_their_events() {
    let mut r = rig(Some(HOST));
    r.frame(|_| {});
    let texts = r.texts();
    let origin = |t: &str| texts.iter().find(|(s, _)| s == t).map(|(_, o)| *o);
    // The host script is origin 2; its children are its own.
    assert_eq!(origin("alpha"), Some(OriginId(2)));
    assert_eq!(origin("foot"), Some(OriginId(2)));
    // Through the script's own `pair`, forwarded into its own `card`.
    assert_eq!(origin("deep"), Some(OriginId(2)));
    assert_eq!(origin("inner"), Some(OriginId(1)));
    assert!(
        !r.core
            .take_warnings()
            .iter()
            .any(|w| w.code == "children-misplaced"
                || w.code == "unknown-component"
                || w.code == "recursive-component")
    );
    // The button is the host's first node, at its padding.
    let events = click_at(&mut r.core, 20.0, 20.0);
    let mut host = Vec::new();
    r.exts.route(events, |ev| host.push(ev));
    // The host script heard it as its own message; what it returned is no
    // emit, so the Rust host hears nothing.
    assert!(host.is_empty(), "{host:?}");
    let heard: mlua::Table = r.app.as_ref().unwrap().globals().get("heard").unwrap();
    assert_eq!(heard.raw_len(), 1);
    assert_eq!(heard.get::<String>(1).unwrap(), "saved:component:1");
}
