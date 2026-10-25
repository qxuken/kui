//! Component instances from Lua (ADR 0067, which folded ADR 0014's slots
//! into components): a `components` table names what the script provides,
//! each function is told its instance and the host's props, a script
//! places components of its own (and plugins it loaded) in its own tree,
//! and what `on_event` returns speaks only through `emit`. Replay is a
//! placement's default, `fresh` its opt-out.

use kui_core::testing::click_at;
use kui_core::{
    Core, EventKind, Extension, Extensions, FillAnswer, Instance, Key, NodeSpec, OriginId, Size,
    UiEvent, Value,
};
use kui_lua::LuaExtension;

const PANEL: &str = r#"
    seen = {}
    components = {
      panel = function(env, props, inst)
        seen[#seen + 1] = {
          name = inst.name, namespace = inst.namespace, key = inst.key,
          title = props.title,
        }
        return row { key = "box", width = 160, height = 20,
          column { key = "pick", width = 40, height = 20,
            on_click = { kind = "pick", item = "foo.rs" },
            text(props.title or "untitled") },
          column { key = "plain", width = 40, height = 20,
            on_click = { kind = "plain" }, text("plain") },
        }
      end,
    }
    function on_event(ev)
      if ev.kind == "pick" then
        return emit("on_pick", { item = ev.item })
      elseif ev.kind == "plain" then
        -- Not an emit: dropped, there is no untyped reply (ADR 0067).
        return { kind = "dropped" }
      end
    end
"#;

fn host_frame(
    core: &mut Core,
    exts: &mut Extensions,
    name: &str,
    props: &Value,
) -> Option<FillAnswer> {
    let mut ui = core.frame_with(Size::new(400.0, 100.0), 1.0, exts);
    ui.configure_root(NodeSpec::row().fill());
    let fill = ui.component(name, props);
    ui.finish();
    fill
}

#[test]
fn a_script_draws_the_component_it_provides_and_reads_the_props() {
    let ext = LuaExtension::from_source("panel.lua", PANEL).unwrap();
    assert_eq!(ext.components(), ["panel".to_owned()]);
    let mut exts = Extensions::new();
    exts.push_as("fs", Box::new(ext)).unwrap();
    let mut core = Core::new();
    let props = Value::map([("key", "fs/panel".into()), ("title", "todos".into())]);
    assert_eq!(
        host_frame(&mut core, &mut exts, "fs/panel", &props),
        Some(FillAnswer::NotKept)
    );
    // Nothing moved: replayed without asking the script.
    assert_eq!(
        host_frame(&mut core, &mut exts, "fs/panel", &props),
        Some(FillAnswer::Replayed)
    );

    // Keyed under the instance, which a `key` prop names like a node.
    let inst = core.key_of("fs/panel").expect("the instance's key");
    let node = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.origin == OriginId(1) && n.name.as_deref() == Some("todos"))
        .cloned()
        .expect("the script's title");
    assert_eq!(node.key, inst.str("box").str("pick"));
    assert!(core.take_warnings().is_empty());
}

/// What the function is told, read back through the script's globals: the
/// extension driven directly with the `Instance` the runner would build,
/// since a boxed extension in an `Extensions` list cannot be read back.
#[test]
fn the_function_is_told_its_instance_its_namespace_and_the_props() {
    let mut ext = LuaExtension::from_source("panel.lua", PANEL).unwrap();
    let mut core = Core::new();
    let key = Key::ROOT.str("fs/panel");
    for params in [Value::map([("title", "todos".into())]), Value::Null] {
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        let inst = Instance {
            name: "panel",
            namespace: "fs",
            params: &params,
            key,
        };
        ext.component(&inst, &mut ui).unwrap();
        ui.finish();
    }
    let seen: mlua::Table = ext.lua().globals().get("seen").unwrap();
    assert_eq!(seen.raw_len(), 2);
    let first: mlua::Table = seen.get(1).unwrap();
    assert_eq!(first.get::<String>("name").unwrap(), "panel");
    assert_eq!(first.get::<String>("namespace").unwrap(), "fs");
    assert_eq!(first.get::<i64>("key").unwrap(), key.0 as i64);
    assert_eq!(first.get::<String>("title").unwrap(), "todos");
    // No props: an empty table, so `props.title` is nil.
    let second: mlua::Table = seen.get(2).unwrap();
    assert_eq!(
        second.get::<mlua::Value>("title").unwrap(),
        mlua::Value::Nil
    );
}

#[test]
fn what_on_event_emits_reaches_the_host_and_nothing_else_does() {
    let mut exts = Extensions::new();
    exts.push_as(
        "fs",
        Box::new(LuaExtension::from_source("panel.lua", PANEL).unwrap()),
    )
    .unwrap();
    let mut core = Core::new();
    let props = Value::map([("on_pick", Value::map([("kind", "picked".into())]))]);
    host_frame(&mut core, &mut exts, "fs/panel", &props);

    // The pick: the host's own message, the script's fields under `pick`.
    let events = click_at(&mut core, 20.0, 10.0);
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(events[0].instance.is_some(), "the node is the instance's");
    let mut host = Vec::new();
    exts.route(events, |ev| host.push(ev));
    assert_eq!(host.len(), 1, "{host:?}");
    assert_eq!(host[0].event, EventKind::Component);
    assert_eq!(host[0].origin, OriginId::HOST, "addressed to the placer");
    assert_eq!(host[0].payload.get_str("kind"), Some("picked"));
    assert_eq!(
        host[0].payload.get("pick").and_then(|p| p.get_str("item")),
        Some("foo.rs")
    );

    // A plain table returned from on_event is no answer at all.
    host_frame(&mut core, &mut exts, "fs/panel", &props);
    let events = click_at(&mut core, 60.0, 10.0);
    assert_eq!(events.len(), 1, "{events:?}");
    let mut host = Vec::new();
    exts.route(events, |ev| host.push(ev));
    assert!(host.is_empty(), "{host:?}");

    // A script with no `on_event` answers nothing, and one with no
    // `components` provides only its root.
    let mut mute =
        LuaExtension::from_source("mute.lua", "function view() return column {} end").unwrap();
    let ev = UiEvent::declared(
        EventKind::Click,
        OriginId(1),
        Key::ROOT,
        Some(&Value::map([("kind", "pick".into())])),
        Value::Null,
    );
    assert!(mute.on_event(&ev).is_empty());
    assert!(
        mute.components().is_empty(),
        "no `components`: the root alone"
    );
}

/// `components = { ["*"] = … }` provides every name placed under the
/// namespace (backlog K1), and `on_event` reads which one its node was in
/// as `at.component`, the full name the script was handed (backlog K2) —
/// so a script with a view per pane routes by pane without stamping
/// payloads.
#[test]
fn a_wildcard_script_provides_every_placed_name_and_hears_which_one() {
    const VIEWS: &str = r#"
        components = {
          ["*"] = function(env, props, inst)
            return column { key = "box", width = 40, height = 20,
              on_click = { kind = "cell" },
              text(inst.name),
            }
          end,
        }
        function on_event(ev, at)
          if ev.kind == "cell" then
            return emit("on_cell", { component = at.component })
          end
        end
    "#;
    let ext = LuaExtension::from_source("views.lua", VIEWS).unwrap();
    assert_eq!(ext.components(), ["*".to_owned()]);
    let mut exts = Extensions::new();
    exts.push_as("views", Box::new(ext)).unwrap();
    let mut core = Core::new();
    let routed = Value::map([("on_cell", Value::map([("kind", "routed".into())]))]);
    let mut ui = core.frame_with(Size::new(400.0, 100.0), 1.0, &mut exts);
    ui.configure_root(NodeSpec::row().fill());
    ui.component("views/pane:1", &routed);
    ui.component("views/pane:2", &routed);
    ui.finish();
    assert!(
        core.take_warnings().is_empty(),
        "a wildcard raises no unknown-component"
    );
    assert_eq!(
        core.access_tree()
            .nodes
            .iter()
            .filter(|n| n.origin == OriginId(1))
            .count(),
        2,
        "both names drawn"
    );
    // A click on the second pane's cell (x 40..80 in the row).
    let events = click_at(&mut core, 60.0, 10.0);
    assert_eq!(events.len(), 1);
    let mut host = Vec::new();
    exts.route(events, |ev| host.push(ev));
    assert_eq!(host.len(), 1);
    assert_eq!(host[0].event, EventKind::Component);
    assert_eq!(host[0].payload.get_str("kind"), Some("routed"));
    assert_eq!(
        host[0]
            .payload
            .get("cell")
            .and_then(|c| c.get_str("component")),
        Some("views/pane:2")
    );
}

#[test]
fn a_slots_global_or_a_bad_components_table_is_refused_at_load() {
    let err = LuaExtension::from_source("old.lua", r#"slots = { "panel" }"#)
        .err()
        .expect("`slots` is gone");
    assert!(err.to_string().contains("`slots` is gone"), "{err}");
    let err = LuaExtension::from_source("bad.lua", "components = 'panel'")
        .err()
        .expect("a string is not a table of functions");
    assert!(
        err.to_string().contains("`components` must be a table"),
        "{err}"
    );
    let err = LuaExtension::from_source("bad.lua", "components = { panel = 1 }")
        .err()
        .expect("a number is not a function");
    assert!(err.to_string().contains("must be a function"), "{err}");
    // A separator in a name is refused where the host loads it.
    let ext = LuaExtension::from_source(
        "bad.lua",
        r#"components = { ["a/b"] = function() return column {} end }"#,
    )
    .unwrap();
    let err = Extensions::new().push(Box::new(ext)).unwrap_err();
    assert!(err.contains("\"a/b\""), "{err}");
}

// -- a script that hosts a plugin of its own --------------------------------

/// A stand-in for the plugin. Loading one is C-only — `env.add_extension`
/// opens a shared library — but what a `component` does with a name is the
/// same whoever put the name in the list, so these drive the placement
/// and the routing with a Rust extension the host loaded and
/// `a_script_loads_a_c_plugin_and_places_it` with the real thing.
struct Panel {
    components: Vec<String>,
}

impl Panel {
    fn new() -> Self {
        Self {
            components: vec!["panel".into()],
        }
    }
}

impl Extension for Panel {
    fn name(&self) -> &str {
        "panel"
    }
    fn components(&self) -> &[String] {
        &self.components
    }
    fn view(&mut self, inst: &Instance<'_>, ui: &mut kui_core::Ui<'_>) -> Result<(), String> {
        let title = inst
            .params
            .get_str("title")
            .unwrap_or("untitled")
            .to_owned();
        ui.text_in_keyed(
            "row",
            NodeSpec::column()
                .width(kui_core::Sizing::Fixed(40.0))
                .height(kui_core::Sizing::Fixed(20.0))
                .focusable(),
            &title,
            kui_core::TextStyle::new(12.0),
        );
        Ok(())
    }
    fn on_event(&mut self, _ev: &UiEvent) -> Vec<Value> {
        vec![kui_core::slot::emit("on_toggle", Value::Null)]
    }
}

/// A script that draws a heading and then a plugin, in that order, and
/// passes on what it hears from the plugin as an emit of its own.
const NESTING: &str = r#"
    components = {
      panel = function(env, props)
        return column { key = "outer", width = "grow", height = "grow",
          text("the script"),
          component("todos/panel", {
            key = "todos/panel", title = "from lua", on_toggle = { kind = "toggled" },
          }),
        }
      end,
    }
    function on_event(ev, at)
      return emit("on_counted", { was = ev.kind, event = at.event, from = at.from or "-" })
    end
"#;

fn nesting_frame(core: &mut Core, exts: &mut Extensions) {
    let props = Value::map([
        ("key", "ui/panel".into()),
        ("on_counted", Value::map([("kind", "counted".into())])),
    ]);
    host_frame(core, exts, "ui/panel", &props);
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

/// A script's `component` is a placement like the host's: made inside the
/// script's own instance, so it is keyed there — move the script and the
/// plugin moves with it — and answered from the one list of extensions
/// the frame has.
#[test]
fn a_script_places_a_plugin_in_its_own_tree() {
    let mut exts = nested();
    let mut core = Core::new();
    nesting_frame(&mut core, &mut exts);

    // The whole chain, so that moving the script moves the plugin: the
    // script's instance, the column it drew, and the plugin's instance
    // inside it.
    let script = core.key_of("ui/panel").expect("the script's own instance");
    let column = core.key_of("outer").expect("the script's own column");
    let inner = core
        .key_of("todos/panel")
        .expect("the instance the script placed");
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

/// An emit goes to whoever placed the instance: the plugin's reaches the
/// script as the script's own message, and what the *script* emits is
/// what the host hears, as the host's own.
#[test]
fn a_placed_plugins_emit_reaches_the_script_and_not_the_host() {
    let mut exts = nested();
    let mut core = Core::new();
    nesting_frame(&mut core, &mut exts);
    let inner = core.key_of("todos/panel").expect("the plugin's instance");

    let mut click = UiEvent::declared(
        EventKind::Click,
        OriginId(2),
        inner.str("row"),
        Some(&Value::map([("kind", "click".into())])),
        Value::Null,
    );
    click.instance = Some(inner);
    let mut to_host = Vec::new();
    exts.route([click], |ev| to_host.push(ev));

    assert_eq!(to_host.len(), 1, "{to_host:?}");
    let host = &to_host[0];
    assert_eq!(host.event, EventKind::Component);
    assert_eq!(host.origin, OriginId::HOST, "the host placed the script");
    assert_eq!(host.payload.get_str("kind"), Some("counted"));
    let counted = host.payload.get("counted").expect("the script's fields");
    assert_eq!(
        counted.get_str("was"),
        Some("toggled"),
        "the script heard the plugin's on_toggle as its own message, not the click"
    );
    assert_eq!(
        counted.get_str("event"),
        Some("component"),
        "and `at.event` says so"
    );
    // `from` is the namespace *this script* loaded a plugin under; the
    // host loaded this one, so the script is told nothing it did not do.
    assert_eq!(counted.get_str("from"), Some("-"));
}

/// `component` takes a full name; one without says so where a script
/// would notice.
#[test]
fn a_component_element_needs_a_name() {
    let mut ext = LuaExtension::from_source(
        "bad.lua",
        r#"function view(env) return column { { type = "component" } } end"#,
    )
    .unwrap();
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    let err = ext
        .view(&Instance::root(), &mut ui)
        .expect_err("should not build");
    ui.finish();
    assert!(err.contains("namespace/component"), "{err}");
}

/// `env.add_extension` before the plugin exists: the answer is a message
/// the script can act on rather than an error that kills the frame, and
/// an empty namespace with nothing to name it by is refused.
#[test]
fn add_extension_answers_with_what_went_wrong() {
    let mut ext = LuaExtension::from_source(
        "loader.lua",
        r#"
        tried = {}
        function view(env)
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
        ext.view(&Instance::root(), &mut ui).unwrap();
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
/// itself, places it, and hears its on_toggle.
///
/// Windows only, and skipped there until `cbuild` has run.
/// Not a shortcut: on the unixes a plugin leaves every `kui_*` undefined
/// and takes them from the executable that loaded it, and a `cargo test`
/// binary does not export them — which is why `cbuild` builds
/// hosts rather than test binaries. Windows has the plugin shape that
/// imports `kui_ffi.dll` rather than a host, `panel.dll`, and so loads
/// into anything — this test included. That also puts *two* copies of the
/// library in one process (this binary links the rlib), which is the case
/// ABI 10's answer sink exists for.
#[test]
#[cfg(windows)]
fn a_script_loads_a_c_plugin_and_places_it() {
    use kui_core::{InputEvent, Vec2};

    // kui-lua opens C plugins through the loader kui-ffi hands it (ADR 0059).
    kui_ffi::register_native_loader();

    // `target/<profile>/`, where cbuild leaves it — read off this test
    // binary, which cargo puts one level below in `deps/`, so a release
    // test looks for a release plugin rather than a stale debug one.
    let plugin = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.parent()?.join("panel.dll")))
        .expect("a test binary has a directory");
    if !plugin.exists() {
        eprintln!(
            "skipped: build examples/c first ({} is not there)",
            plugin.display()
        );
        return;
    }
    let script = format!(
        r#"
        components = {{
          panel = function(env)
            local ok, err = env.add_extension("todos", [[{}]])
            if not ok then error(err) end
            return column {{ key = "outer", width = "grow", height = "grow",
              text("the script"),
              component("todos/panel", {{
                title = "todos, from lua",
                on_toggle = {{ kind = "toggled" }},
              }}),
            }}
          end,
        }}
        function on_event(ev, at)
          if ev.kind == "toggled" then
            return emit("on_heard", {{ index = ev.toggle.index, event = at.event }})
          end
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
        ui.component(
            "ui/panel",
            &Value::map([("on_heard", Value::map([("kind", "heard".into())]))]),
        );
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
    let row = row.expect("the plugin drew no rows — was it placed?");
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
        .filter(|e| e.payload.get_str("kind") == Some("heard"))
        .collect();
    assert_eq!(heard.len(), 1, "got {to_host:?}");
    let fields = heard[0].payload.get("heard").expect("the script's fields");
    assert_eq!(
        fields.get_str("event"),
        Some("component"),
        "the script heard the plugin's on_toggle as its own message"
    );
    assert_eq!(
        heard[0].origin,
        OriginId::HOST,
        "and the host hears the script, which is who it placed"
    );
}

// -- replay, a placement's default (ADR 0067) --------------------------------

/// A plugin that counts its views: a stand-in for the panel a script
/// hosts, read back through the shared cell since the boxed extension
/// is the runner's once loaded.
struct Counted {
    components: Vec<String>,
    views: std::rc::Rc<std::cell::Cell<usize>>,
}

impl Extension for Counted {
    fn name(&self) -> &str {
        "counted"
    }
    fn components(&self) -> &[String] {
        &self.components
    }
    fn view(&mut self, inst: &Instance<'_>, ui: &mut kui_core::Ui<'_>) -> Result<(), String> {
        self.views.set(self.views.get() + 1);
        let title = inst
            .params
            .get_str("title")
            .unwrap_or("untitled")
            .to_owned();
        ui.text_in_keyed(
            "row",
            NodeSpec::column()
                .width(kui_core::Sizing::Fixed(40.0))
                .height(kui_core::Sizing::Fixed(20.0))
                .focusable(),
            &title,
            kui_core::TextStyle::new(12.0),
        );
        Ok(())
    }
    fn on_event(&mut self, _ev: &UiEvent) -> Vec<Value> {
        Vec::new()
    }
}

/// A script that places a plugin, the title from a global the test moves.
const REPLAYING: &str = r#"
    title = "first"
    components = {
      panel = function(env)
        return column { key = "outer", width = "grow", height = "grow",
          text("the script"),
          component("todos/panel", { title = title }),
        }
      end,
    }
"#;

#[test]
fn a_scripts_placement_is_replayed_until_its_props_move() {
    let views = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut exts = Extensions::new();
    let script = LuaExtension::from_source("host.lua", REPLAYING).unwrap();
    let lua = script.lua().clone();
    exts.push_as("ui", Box::new(script)).unwrap();
    exts.push_as(
        "todos",
        Box::new(Counted {
            components: vec!["panel".into()],
            views: views.clone(),
        }),
    )
    .unwrap();
    let mut core = Core::new();
    // The script reads a global no event moves, so its host asks it every
    // frame: `fresh`.
    let fresh = Value::map([("fresh", Value::Bool(true))]);

    host_frame(&mut core, &mut exts, "ui/panel", &fresh);
    assert_eq!(views.get(), 1);
    host_frame(&mut core, &mut exts, "ui/panel", &fresh);
    assert_eq!(views.get(), 1, "the plugin was not asked: replayed");
    let row = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("first"))
        .expect("the plugin's row, replayed");
    assert_eq!(row.origin, OriginId(2));

    // Other props: the plugin runs, and draws the new title.
    lua.globals().set("title", "second").unwrap();
    host_frame(&mut core, &mut exts, "ui/panel", &fresh);
    assert_eq!(views.get(), 2);
    assert!(
        core.access_tree()
            .nodes
            .iter()
            .any(|n| n.name.as_deref() == Some("second"))
    );
    assert!(core.take_warnings().is_empty());
}

/// A script whose fill places another extension's component, with no
/// host children, is itself replayed: the placement is one op of its kept
/// fill, not a closure no replay could call.
#[test]
fn a_script_that_places_a_component_is_replayed_itself() {
    let views = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut exts = Extensions::new();
    exts.push_as(
        "ui",
        Box::new(LuaExtension::from_source("host.lua", REPLAYING).unwrap()),
    )
    .unwrap();
    exts.push_as(
        "todos",
        Box::new(Counted {
            components: vec!["panel".into()],
            views: views.clone(),
        }),
    )
    .unwrap();
    let mut core = Core::new();
    let none = Value::map([]);
    assert_eq!(
        host_frame(&mut core, &mut exts, "ui/panel", &none),
        Some(FillAnswer::NotKept)
    );
    assert_eq!(
        host_frame(&mut core, &mut exts, "ui/panel", &none),
        Some(FillAnswer::Replayed),
        "the script's own fill was replayed"
    );
    // A component placed inside a fill being kept is kept from its next
    // placement on: the replay places it, it fills once more, and settles.
    let settled = views.get();
    host_frame(&mut core, &mut exts, "ui/panel", &none);
    host_frame(&mut core, &mut exts, "ui/panel", &none);
    assert_eq!(views.get(), settled, "nothing asks the plugin once both are kept");
    assert!(
        core.access_tree()
            .nodes
            .iter()
            .any(|n| n.name.as_deref() == Some("first")),
        "and the plugin's row came back with it"
    );
}

/// A script that draws from `env.theme`: replayed by its host while the
/// palette holds, run again when it changes (backlog F155).
const THEMED: &str = r#"
    views = 0
    components = {
      panel = function(env)
        views = views + 1
        return column { key = "box", width = 40, height = 20, bg = "$accent",
          text(string.format("%08x", env.theme.accent)),
        }
      end,
    }
"#;

#[test]
fn a_script_that_read_the_theme_is_not_replayed_across_a_theme_change() {
    let script = LuaExtension::from_source("themed.lua", THEMED).unwrap();
    let lua = script.lua().clone();
    let mut exts = Extensions::new();
    exts.push_as("themed", Box::new(script)).unwrap();
    let mut core = Core::new();
    let frame = |core: &mut Core, exts: &mut Extensions| {
        host_frame(core, exts, "themed/panel", &Value::Null).map(|f| f.name())
    };
    let views = |lua: &mlua::Lua| lua.globals().get::<i64>("views").unwrap();
    assert_eq!(frame(&mut core, &mut exts), Some("not-kept"));
    assert_eq!(frame(&mut core, &mut exts), Some("replayed"));
    assert_eq!(views(&lua), 1);

    let pink = kui_core::Color::hex(0xff00ffff);
    core.set_theme(kui_core::Theme::light().with_accent(pink));
    assert_eq!(frame(&mut core, &mut exts), Some("reads"));
    assert_eq!(views(&lua), 2);
    assert!(
        core.access_tree()
            .nodes
            .iter()
            .any(|n| n.name.as_deref() == Some("ff00ffff")),
        "drawn in the new accent"
    );
    assert_eq!(frame(&mut core, &mut exts), Some("replayed"));
    assert_eq!(views(&lua), 2);
}

/// ADR 0059: a folder holding `init.lua` loads as one extension named for
/// the folder, its modules beside `init.lua`.
#[test]
fn a_folder_with_init_lua_loads_and_requires_its_modules() {
    let dir = std::env::temp_dir().join(format!("kui-lua-folder-{}", std::process::id()));
    let ext_dir = dir.join("kit");
    std::fs::create_dir_all(&ext_dir).unwrap();
    std::fs::write(
        ext_dir.join("words.lua"),
        "return { hello = 'from words.lua' }",
    )
    .unwrap();
    std::fs::write(
        ext_dir.join("init.lua"),
        "local words = require('words')\nfunction view(env) return column { text(words.hello) } end\n",
    )
    .unwrap();
    assert!(kui_lua::is_lua_path(&ext_dir));
    assert!(!kui_lua::is_lua_path(&dir));
    let mut ext = LuaExtension::from_path(&ext_dir).unwrap();
    assert_eq!(ext.name(), "kit");
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
    ext.view(&Instance::root(), &mut ui).unwrap();
    ui.finish();
    let _ = std::fs::remove_dir_all(dir);
}

/// A script loads another script with `env.add_extension`, with no native
/// loader registered, and places its component; a C path with none
/// registered is answered with a message naming it.
#[test]
fn a_script_loads_a_script_and_names_a_c_path_it_cannot_open() {
    let dir = std::env::temp_dir().join(format!("kui-lua-guest-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let guest = dir.join("guest.lua");
    std::fs::write(
        &guest,
        "components = { pane = function(env) return column { key = 'guest', text('the guest') } end }\n",
    )
    .unwrap();
    let mut ext = LuaExtension::from_source(
        "host.lua",
        &format!(
            r#"
            result = {{}}
            function view(env)
              local ok, err = env.add_extension("guest", [[{}]])
              result.ok, result.err = ok, err
              local ok2, err2 = env.add_extension("native", "no/such/plugin.so")
              result.ok2, result.err2 = ok2, err2
              return column {{ component("guest/pane") }}
            end
            "#,
            guest.display()
        ),
    )
    .unwrap();
    let mut core = Core::new();
    let mut exts = Extensions::new();
    {
        let mut ui = core.frame_with(Size::new(300.0, 100.0), 1.0, &mut exts);
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
    }
    let result: mlua::Table = ext.lua().globals().get("result").unwrap();
    assert_eq!(
        result.get::<Option<bool>>("ok").unwrap(),
        Some(true),
        "{:?}",
        result.get::<Option<String>>("err")
    );
    assert_eq!(result.get::<mlua::Value>("ok2").unwrap(), mlua::Value::Nil);
    let err2: String = result.get("err2").unwrap();
    assert!(err2.contains("plugin.so"), "{err2}");
    assert!(
        core.key_of("guest").is_some()
            || core
                .access_tree()
                .nodes
                .iter()
                .any(|n| n.name.as_deref() == Some("the guest")),
        "the guest drew its component"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// ADR 0062: a script loads the DTCG fixture with `env.load_tokens`, a
/// relative path from its own folder, and paints with the brand.
#[test]
fn a_script_loads_the_dtcg_fixture_and_paints_with_it() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../kui-core/tests/fixtures/dtcg");
    let script = std::env::temp_dir().join(format!("kui-lua-dtcg-{}", std::process::id()));
    std::fs::create_dir_all(&script).unwrap();
    std::fs::write(
        script.join("brand.lua"),
        format!(
            r#"
            loaded = env_once
            function view(env)
              if not loaded then
                loaded = env.load_tokens([[{}]], {{ prefix = "brand" }})
                env.set_tokens(loaded.tokens)
              end
              return column {{ bg = "$brandColorSurface", pad = "$brandSpaceGap",
                text("brand", {{ color = "$brandColorInk", family = "$brandFontCode" }}) }}
            end
            "#,
            dir.join("brand.resolver.json").display()
        ),
    )
    .unwrap();
    let mut ext = LuaExtension::from_file(script.join("brand.lua")).unwrap();
    let mut core = Core::new();
    for _ in 0..2 {
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
    }
    let warnings: Vec<_> = core.take_warnings().into_iter().map(|w| w.code).collect();
    assert!(!warnings.contains(&"unknown-token"), "{warnings:?}");
    let loaded: mlua::Table = ext.lua().globals().get("loaded").unwrap();
    assert!(loaded.get::<bool>("resolver").unwrap());
    let w: mlua::Table = loaded.get("warnings").unwrap();
    assert_eq!(w.len().unwrap(), 1);
    let names: mlua::Table = loaded.get("names").unwrap();
    let ink: mlua::Table = names.get(5).unwrap();
    assert_eq!(ink.get::<String>("path").unwrap(), "color.ink");
    let _ = std::fs::remove_dir_all(script);
}
