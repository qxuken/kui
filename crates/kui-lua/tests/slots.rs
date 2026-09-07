//! Slots from Lua (`docs/adr/0014-slots-an-extension-fills-in-place.md`):
//! the `slots` global names what the script fills, `view(env, slot)` is
//! told which slot and with what params, and what `on_event` returns is
//! the script's replies to the host.

use kui_core::{
    Core, Extension, Extensions, Key, NodeSpec, OriginId, Size, Slot, UiEvent, Value, WindowId,
};
use kui_lua::LuaExtension;

const PANEL: &str = r#"
    slots = { "panel" }
    seen = {}
    function view(env, slot)
      seen[#seen + 1] = {
        name = slot.name, namespace = slot.namespace, key = slot.key,
        title = slot.params and slot.params.title,
      }
      on_pick = slot.params and slot.params.on_pick
      return column { key = "box", width = 40, height = 20, focusable = true,
        text(slot.params and slot.params.title or "untitled"),
      }
    end
    function on_event(ev)
      if ev.kind == "pick" and on_pick then
        local reply = { item = ev.item }
        for k, v in pairs(on_pick) do reply[k] = v end
        return reply
      elseif ev.kind == "many" then
        return { { kind = "a" }, { kind = "b" } }
      end
    end
"#;

fn host_frame(core: &mut Core, exts: &mut Extensions, name: &str, params: &Value) {
    let mut ui = core.frame_with(Size::new(400.0, 100.0), 1.0, exts);
    ui.configure_root(NodeSpec::row().fill());
    ui.slot_with(name, params);
    ui.finish();
}

#[test]
fn a_script_fills_the_slot_it_lists_and_reads_the_params() {
    let ext = LuaExtension::from_source("panel.lua", PANEL).unwrap();
    assert_eq!(ext.slots(), ["panel".to_owned()]);
    let mut exts = Extensions::new();
    exts.push_as("fs", Box::new(ext)).unwrap();
    let mut core = Core::new();
    let params = Value::map([("title", "todos".into())]);
    host_frame(&mut core, &mut exts, "fs/panel", &params);
    host_frame(&mut core, &mut exts, "fs/panel", &Value::Null);

    // Keyed under the slot's full name, whatever the script's own name is.
    let node = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.origin == OriginId(1))
        .cloned()
        .expect("the script's box");
    assert_eq!(node.key, Key::ROOT.str("fs/panel").str("box"));
    assert_eq!(core.key_of("fs/panel"), Some(Key::ROOT.str("fs/panel")));
    assert!(core.take_warnings().is_empty());
}

/// What `view` is told, read back through the script's globals: a second
/// instance driven directly with the `Slot` the runner would build, since
/// a boxed extension in an `Extensions` list cannot be read back.
#[test]
fn view_is_told_the_slot_its_namespace_and_the_params() {
    let mut ext = LuaExtension::from_source("panel.lua", PANEL).unwrap();
    let mut core = Core::new();
    let key = Key::ROOT.str("fs/panel");
    for params in [Value::map([("title", "todos".into())]), Value::Null] {
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        let slot = Slot {
            name: "panel",
            namespace: "fs",
            params: &params,
            key,
        };
        ext.view(&slot, &mut ui).unwrap();
        ui.finish();
    }
    let seen: mlua::Table = ext.lua().globals().get("seen").unwrap();
    assert_eq!(seen.raw_len(), 2);
    let first: mlua::Table = seen.get(1).unwrap();
    assert_eq!(first.get::<String>("name").unwrap(), "panel");
    assert_eq!(first.get::<String>("namespace").unwrap(), "fs");
    assert_eq!(first.get::<i64>("key").unwrap(), key.0 as i64);
    assert_eq!(first.get::<String>("title").unwrap(), "todos");
    // The params are `nil` for a slot declared without any.
    let second: mlua::Table = seen.get(2).unwrap();
    assert_eq!(
        second.get::<mlua::Value>("title").unwrap(),
        mlua::Value::Nil
    );
}

#[test]
fn what_on_event_returns_is_the_scripts_replies() {
    let mut ext = LuaExtension::from_source("panel.lua", PANEL).unwrap();
    // The template arrives through the params of a fill; `view` keeps it.
    let mut core = Core::new();
    let mut exts = Extensions::new();
    exts.push_as("fs", Box::new(ext)).unwrap();
    let params = Value::map([("on_pick", Value::map([("kind", "picked".into())]))]);
    host_frame(&mut core, &mut exts, "fs/panel", &params);
    let ext_ref = exts.by_origin(OriginId(1)).unwrap();
    let ev = |kind: &str, extra: Option<(&'static str, Value)>| UiEvent {
        origin: OriginId(1),
        window: WindowId::MAIN,
        key: Key::ROOT,
        payload: Value::map(std::iter::once(("kind", Value::str(kind))).chain(extra)),
    };
    // One table: one reply, the host's template with the field added.
    let replies = ext_ref.on_event(&ev("pick", Some(("item", "foo.rs".into()))));
    assert_eq!(replies.len(), 1);
    assert_eq!(
        replies[0].get("kind").and_then(Value::as_str),
        Some("picked")
    );
    assert_eq!(
        replies[0].get("item").and_then(Value::as_str),
        Some("foo.rs")
    );
    // A sequence: several. Nothing: none.
    let replies = ext_ref.on_event(&ev("many", None));
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[1].get("kind").and_then(Value::as_str), Some("b"));
    assert!(ext_ref.on_event(&ev("other", None)).is_empty());

    // A script with no `on_event` at all replies nothing.
    ext = LuaExtension::from_source("mute.lua", "function view() return column {} end").unwrap();
    assert!(ext.on_event(&ev("pick", None)).is_empty());
    assert!(ext.slots().is_empty(), "no `slots` global: the root fill");
}

#[test]
fn a_bad_slots_global_is_refused_at_load() {
    let err = LuaExtension::from_source("bad.lua", "slots = 'panel'")
        .err()
        .expect("a string is not a list of slots");
    assert!(err.to_string().contains("`slots` must be a list"), "{err}");
    // A separator in a slot name is refused where the host loads it.
    let ext = LuaExtension::from_source("bad.lua", r#"slots = { "a/b" }"#).unwrap();
    let err = Extensions::new().push(Box::new(ext)).unwrap_err();
    assert!(err.contains("\"a/b\""), "{err}");
}
