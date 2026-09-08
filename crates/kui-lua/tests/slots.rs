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

// -- a script that hosts a plugin of its own --------------------------------

/// A stand-in for the plugin. Loading one is C-only — `env.add_extension`
/// opens a shared library — but what a `fill` does with a name is the
/// same whoever put the name in the list, so these drive the placement
/// and the routing with a Rust extension the host loaded and
/// `a_script_loads_a_c_plugin_and_places_it` with the real thing.
struct Panel {
    slots: Vec<String>,
}

impl Panel {
    fn new() -> Self {
        Self {
            slots: vec!["panel".into()],
        }
    }
}

impl Extension for Panel {
    fn name(&self) -> &str {
        "panel"
    }
    fn slots(&self) -> &[String] {
        &self.slots
    }
    fn view(&mut self, slot: &Slot<'_>, ui: &mut kui_core::Ui<'_>) -> Result<(), String> {
        let title = slot
            .params
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("untitled")
            .to_owned();
        ui.with_keyed(
            "row",
            NodeSpec::column()
                .width(kui_core::Sizing::Fixed(40.0))
                .height(kui_core::Sizing::Fixed(20.0))
                .focusable(),
            |ui| ui.text(&title, kui_core::TextStyle::new(12.0)),
        );
        Ok(())
    }
    fn on_event(&mut self, _ev: &UiEvent) -> Vec<Value> {
        vec![Value::map([("kind", "toggled".into())])]
    }
}

/// A script that draws a heading and then a plugin, in that order, and
/// says something about everything it hears.
const NESTING: &str = r#"
    slots = { "panel" }
    function view(env, slot)
      return column { key = "outer", width = "grow", height = "grow",
        text("the script"),
        fill { name = "todos/panel", params = { title = "from lua" } },
      }
    end
    function on_event(ev)
      return { kind = "counted", was = ev.kind, from = ev.from or "-" }
    end
"#;

fn nesting_frame(core: &mut Core, exts: &mut Extensions) {
    host_frame(core, exts, "ui/panel", &Value::Null);
}

/// Two extensions, the second placed by the first.
fn nested() -> Extensions {
    let mut exts = Extensions::new();
    exts.push_as(
        "ui",
        Box::new(LuaExtension::from_source("host.lua", NESTING).unwrap()),
    )
    .unwrap();
    exts.push_as("todos", Box::new(Panel::new())).unwrap();
    exts
}

/// A script's `fill` is a slot like the host's: declared inside the
/// script's own fill, so it is keyed there — move the script and the
/// plugin moves with it — and answered from the one list of extensions
/// the frame has.
#[test]
fn a_script_places_a_plugin_in_its_own_tree() {
    let mut exts = nested();
    let mut core = Core::new();
    nesting_frame(&mut core, &mut exts);

    // The whole chain, so that moving the script moves the plugin: the
    // script's slot, the column it drew, and the plugin's slot inside it.
    let script = core.key_of("ui/panel").expect("the script's own slot");
    let column = core.key_of("outer").expect("the script's own column");
    let inner = core
        .key_of("todos/panel")
        .expect("the slot the script declared");
    assert_eq!(column, script.str("outer"));
    assert_eq!(inner, column.str("todos/panel"));
    let node = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.origin == OriginId(2))
        .cloned()
        .expect("the plugin's box");
    assert_eq!(node.key, inner.str("row"));
    assert!(core.take_warnings().is_empty());
}

/// A reply answers the slot, so it goes to whoever declared it: the
/// plugin's reply reaches the script, and what the *script* answers is
/// what the host hears, with the script's origin on it.
#[test]
fn a_placed_plugins_reply_reaches_the_script_and_not_the_host() {
    let mut exts = nested();
    let mut core = Core::new();
    nesting_frame(&mut core, &mut exts);

    let mut to_host = Vec::new();
    exts.route(
        [UiEvent {
            origin: OriginId(2),
            window: WindowId::MAIN,
            key: Key::ROOT,
            payload: Value::map([("kind", "click".into())]),
        }],
        |ev| to_host.push(ev),
    );

    assert_eq!(to_host.len(), 1);
    assert_eq!(
        to_host[0].payload.get("kind").and_then(Value::as_str),
        Some("counted")
    );
    assert_eq!(
        to_host[0].payload.get("was").and_then(Value::as_str),
        Some("toggled"),
        "the script heard the plugin's reply, not the click"
    );
    assert_eq!(
        to_host[0].origin,
        OriginId(1),
        "and the host hears the script, which is who it placed"
    );
    // `from` is the namespace *this script* loaded the plugin under; the
    // host loaded this one, so the script is told nothing it did not do.
    assert_eq!(
        to_host[0].payload.get("from").and_then(Value::as_str),
        Some("-")
    );
}

/// `fill` is a position, not a box: it takes a full name and params and
/// nothing else, and says so where a script would notice.
#[test]
fn a_fill_takes_a_full_name_and_params_and_nothing_else() {
    let refused = |body: &str| -> String {
        let mut ext = LuaExtension::from_source(
            "bad.lua",
            &format!("function view(env, slot) return column {{ {body} }} end"),
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        let err = ext
            .view(&Slot::root(), &mut ui)
            .expect_err("should not build");
        ui.finish();
        err
    };
    assert!(refused("fill {}").contains("namespace/slot"));
    assert!(refused(r#"fill { name = "panel" }"#).contains("bad slot name"));
    assert!(
        refused(r#"fill { name = "a/b", width = 20 }"#).contains("not a box"),
        "a fill is not a node and takes no node props"
    );
}

/// `env.add_extension` before the plugin exists: the answer is a message
/// the script can act on rather than an error that kills the frame, and
/// the same namespace twice is refused rather than silently rebound.
#[test]
fn add_extension_answers_with_what_went_wrong() {
    let mut ext = LuaExtension::from_source(
        "loader.lua",
        r#"
        tried = {}
        function view(env, slot)
          local ok, err = env.add_extension("todos", "no/such/plugin.dll")
          tried[#tried + 1] = { ok = ok, err = err }
          local ok2, err2 = env.add_extension("", "no/such/plugin.dll")
          tried[#tried + 1] = { ok = ok2, err = err2 }
          return column {}
        end
        "#,
    )
    .unwrap();
    let mut core = Core::new();
    let mut exts = Extensions::new();
    {
        let mut ui = core.frame_with(Size::new(400.0, 100.0), 1.0, &mut exts);
        ext.view(&Slot::root(), &mut ui).unwrap();
        ui.finish();
    }
    let tried: mlua::Table = ext.lua().globals().get("tried").unwrap();
    let first: mlua::Table = tried.get(1).unwrap();
    assert_eq!(first.get::<mlua::Value>("ok").unwrap(), mlua::Value::Nil);
    let err: String = first.get("err").unwrap();
    assert!(err.contains("plugin.dll"), "{err}");
    // An empty namespace is the list's own refusal, reaching the script
    // the same way — nothing was loaded, so nothing was named.
    let second: mlua::Table = tried.get(2).unwrap();
    assert_eq!(second.get::<mlua::Value>("ok").unwrap(), mlua::Value::Nil);
    assert!(exts.is_empty());
}

/// The whole round trip with a real plugin: the script opens the C panel
/// itself, places it, and hears what it says.
///
/// Windows only, and skipped there until `examples/c/build.ps1` has run.
/// Not a shortcut: on the unixes a plugin leaves every `kui_*` undefined
/// and takes them from the executable that loaded it, and a `cargo test`
/// binary does not export them — which is why `examples/c/build.sh` builds
/// hosts rather than test binaries. Windows has the other plugin shape,
/// `panel-dll.dll`, which names `kui_ffi.dll` as the module its imports
/// come from and so loads into anything, this test included. That also
/// puts *two* copies of the library in one process (this binary links the
/// rlib), which is the case ABI 10's reply sink exists for.
#[test]
#[cfg(windows)]
fn a_script_loads_a_c_plugin_and_places_it() {
    use kui_core::{InputEvent, Vec2};

    let plugin =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/c/panel-dll.dll");
    if !plugin.exists() {
        eprintln!(
            "skipped: build examples/c first ({} is not there)",
            plugin.display()
        );
        return;
    }
    let script = format!(
        r#"
        slots = {{ "panel" }}
        function view(env, slot)
          local ok, err = env.add_extension("todos", [[{}]])
          if not ok then error(err) end
          return column {{ key = "outer", width = "grow", height = "grow",
            text("the script"),
            fill {{ name = "todos/panel", params = {{
              title = "todos, from lua",
              on_toggle = {{ kind = "toggled" }},
            }} }},
          }}
        end
        function on_event(ev)
          if ev.from then return {{ kind = "heard", who = ev.from }} end
        end
        "#,
        // A long-bracket literal, so a Windows path needs no escaping and
        // reaches the loader exactly as it is written here.
        plugin.display()
    );

    let mut exts = Extensions::new();
    exts.push_as(
        "ui",
        Box::new(LuaExtension::from_source("loader.lua", &script).unwrap()),
    )
    .unwrap();
    // A window big enough for the panel to lay out in: `host_frame`'s is
    // 400×100, where the plugin's rows overlap its buttons and a click
    // lands on whichever is on top.
    let mut core = Core::new();
    {
        let mut ui = core.frame_with(Size::new(900.0, 600.0), 1.0, &mut exts);
        ui.configure_root(NodeSpec::row().fill());
        ui.slot("ui/panel");
        ui.finish();
    }

    // The script loaded it, so it is in the frame's list with an origin
    // of its own, and it drew where the script said.
    assert_eq!(exts.len(), 2, "the script's plugin joined the one list");
    assert_eq!(exts.namespace_of(OriginId(2)), Some("todos"));
    let row = core.access_tree().nodes.iter().find_map(|n| {
        let name = n.name.as_deref()?;
        (n.origin == OriginId(2) && name.starts_with("[ "))
            .then(|| Vec2::new(n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0))
    });
    let row = row.expect("the plugin drew no rows — did the slot fill?");
    assert!(core.take_warnings().is_empty());

    // Click one of its todos and let the frame resolve the press.
    core.handle_input(InputEvent::CursorMoved(row));
    core.handle_input(InputEvent::mouse_down(1));
    let events = core.handle_input(InputEvent::mouse_up());
    assert!(!events.is_empty(), "the click reached nothing");

    let mut to_host = Vec::new();
    exts.route(events, |ev| to_host.push(ev));
    let heard: Vec<_> = to_host
        .iter()
        .filter(|e| e.payload.get("kind").and_then(Value::as_str) == Some("heard"))
        .collect();
    assert_eq!(heard.len(), 1, "got {to_host:?}");
    assert_eq!(
        heard[0].payload.get("who").and_then(Value::as_str),
        Some("todos"),
        "the script is told which of its plugins answered"
    );
    assert_eq!(
        heard[0].origin,
        OriginId(1),
        "and the host hears the script, not the plugin it placed"
    );
}
