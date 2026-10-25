//! Extensions in a host (ADR 0014 as folded into components by ADR 0067):
//! a host places what an extension provides at a position in its own view,
//! the extension draws it there and then under the instance's key, props
//! arrive every frame as data, and the fill is bounded. Names are
//! namespaced, and the host decides the namespace when it loads the
//! extension; an extension providing nothing else draws its `root` after
//! the host's view. The runner's loop over extensions is `Extensions`'
//! `Fill`, so these drive that with a stand-in extension rather than a
//! filler of their own.

use std::cell::RefCell;
use std::rc::Rc;

use kui_core::diag::{
    EXTENSION_VIEW_ERROR, RECURSIVE_COMPONENT, UNBALANCED_EXTENSION, UNKNOWN_COMPONENT,
};
use kui_core::testing::codes;
use kui_core::{
    ANY_COMPONENT, Core, Extension, Extensions, Key, NodeSpec, OriginId, Size, Instance, Ui, UiEvent, Value,
    Vec2, split_name,
};

/// A stand-in extension: opens one keyed, focusable cell (so it is in the
/// access tree, with its key and parent), records which slot it was asked
/// to fill and with what, and misbehaves on request.
#[derive(Default)]
struct Ext {
    name: &'static str,
    components: Vec<String>,
    /// Shared with the test, since a boxed extension cannot be read back
    /// once it is in the runner's list: (namespace, name, params).
    seen: Rc<RefCell<Vec<(String, String, Value)>>>,
    /// The payloads that reached `on_event`, shared the same way.
    heard: Rc<RefCell<Vec<Value>>>,
    /// What it answers with when it hears anything.
    reply: Option<Value>,
    leave_open: bool,
    fail: bool,
}

fn cell() -> NodeSpec {
    NodeSpec::column().size(40.0, 20.0).focusable()
}

impl Extension for Ext {
    fn name(&self) -> &str {
        self.name
    }
    fn components(&self) -> &[String] {
        &self.components
    }
    fn view(&mut self, slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        self.seen.borrow_mut().push((
            slot.namespace.to_owned(),
            slot.name.to_owned(),
            without_key(slot.params),
        ));
        if self.fail {
            return Err("no view today".into());
        }
        if self.leave_open {
            ui.open(cell());
            ui.open_keyed("box", cell());
        } else {
            ui.leaf_keyed("box", cell());
        }
        Ok(())
    }
    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
        self.heard.borrow_mut().push(ev.payload.clone());
        self.reply.clone().into_iter().collect()
    }
}

fn ext(name: &'static str, components: &[&str]) -> Box<dyn Extension> {
    Box::new(Ext {
        name,
        components: components.iter().map(|s| (*s).to_owned()).collect(),
        ..Default::default()
    })
}

/// `props` with `key` — the placement's own — left out: what the tests
/// compare against what the host meant to pass.
fn without_key(props: &Value) -> Value {
    match props {
        Value::Map(entries) => {
            let rest: Vec<_> = entries.iter().filter(|(k, _)| k != "key").cloned().collect();
            if rest.is_empty() { Value::Null } else { Value::Map(rest) }
        }
        other => other.clone(),
    }
}

/// `props` keyed by the placement's full name, as a placement that must
/// keep its keys whatever the host builds around it passes.
fn keyed(name: &str, props: &Value) -> Value {
    let mut entries = match props {
        Value::Map(entries) => entries.clone(),
        _ => Vec::new(),
    };
    entries.push(("key".into(), Value::str(name)));
    Value::Map(entries)
}

/// Under their own names as namespaces.
fn load(exts: Vec<Box<dyn Extension>>) -> Extensions {
    Extensions::try_from(exts).unwrap()
}

/// A payload's `kind`, which is all the routing tests care about.
fn kind_of(v: &Value) -> String {
    match v.get("kind") {
        Some(Value::Str(s)) => s.to_string(),
        other => panic!("no kind in {other:?}"),
    }
}

/// The access node the extension's cell became: its key and its parent.
fn cell_of(core: &mut Core, origin: OriginId) -> (Key, Option<Key>) {
    let tree = core.access_tree();
    let n = tree
        .nodes
        .iter()
        .find(|n| n.origin == origin)
        .unwrap_or_else(|| panic!("no node of origin {origin:?} in the access tree"));
    (n.key, n.parent)
}

/// A root row of `before` fixed cells, then the placement (or nothing),
/// keyed by its full name, then one more cell. Returns the key of the host
/// cell after the placement.
fn host_frame(
    core: &mut Core,
    exts: &mut Extensions,
    before: usize,
    slot: Option<(&str, &Value)>,
) -> Key {
    let mut ui = core.frame_with(Size::new(600.0, 100.0), 1.0, exts);
    ui.configure_root(NodeSpec::row().fill());
    for _ in 0..before {
        ui.leaf(cell());
    }
    if let Some((name, params)) = slot {
        ui.component(name, &keyed(name, params));
    }
    let after = ui.leaf_keyed("after", cell());
    ui.finish();
    after
}

/// The third context item of ADR 0014: an extension's keys used to be
/// `root.index(n)` for whatever `n` the host happened to leave, so a host
/// adding a child at the root rekeyed the whole panel. Under a keyed
/// placement they are `instance.…`, whatever the host builds around them.
#[test]
fn an_extensions_keys_do_not_move_when_the_host_adds_a_sibling() {
    let mut core = Core::new();
    let mut exts = load(vec![ext("panel", &["side"])]);
    host_frame(&mut core, &mut exts, 1, Some(("panel/side", &Value::Null)));
    let (one, _) = cell_of(&mut core, OriginId(1));
    host_frame(&mut core, &mut exts, 3, Some(("panel/side", &Value::Null)));
    let (three, _) = cell_of(&mut core, OriginId(1));
    assert_eq!(
        one, three,
        "the fill's keys moved with the host's child count"
    );
    assert_eq!(one, Key::ROOT.str("panel/side").str("box"));
    assert!(core.take_warnings().is_empty());
}

/// The same for `"root"` drawn after the host's view — the fill every
/// extension got before slots existed, keyed under `root.str("ns/root")`.
#[test]
fn the_root_fill_is_keyed_by_the_namespace_and_not_by_the_hosts_child_count() {
    let mut core = Core::new();
    let mut exts = load(vec![ext("legacy", &[])]);
    host_frame(&mut core, &mut exts, 1, None);
    let (one, _) = cell_of(&mut core, OriginId(1));
    host_frame(&mut core, &mut exts, 4, None);
    let (four, _) = cell_of(&mut core, OriginId(1));
    assert_eq!(one, four);
    assert_eq!(one, Key::ROOT.str("legacy/root").str("box"));
    assert!(core.take_warnings().is_empty());
}

/// The host decides the namespace: the same extension loaded twice under
/// two names is two slots, two key namespaces and two sets of params, and
/// each instance is told which it is.
#[test]
fn one_plugin_loaded_twice_is_two_namespaces() {
    let mut core = Core::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut exts = Extensions::new();
    for ns in ["left", "right"] {
        exts.push_as(
            ns,
            Box::new(Ext {
                name: "fs",
                components: vec!["panel".into()],
                seen: Rc::clone(&seen),
                ..Default::default()
            }),
        )
        .unwrap();
    }
    let l = Value::map([("path", "/".into())]);
    let r = Value::map([("path", "/tmp".into())]);
    let mut ui = core.frame_with(Size::new(600.0, 100.0), 1.0, &mut exts);
    ui.configure_root(NodeSpec::row().fill());
    ui.component("right/panel", &keyed("right/panel", &r));
    ui.component("left/panel", &keyed("left/panel", &l));
    ui.finish();
    let (a, _) = cell_of(&mut core, OriginId(1));
    let (b, _) = cell_of(&mut core, OriginId(2));
    assert_eq!(a, Key::ROOT.str("left/panel").str("box"));
    assert_eq!(b, Key::ROOT.str("right/panel").str("box"));
    assert_eq!(
        *seen.borrow(),
        vec![
            ("right".to_owned(), "panel".to_owned(), r),
            ("left".to_owned(), "panel".to_owned(), l),
        ],
        "filled in the order the host declared them, each told its namespace"
    );
    let tree = core.access_tree();
    let x = |origin: OriginId| {
        tree.nodes
            .iter()
            .find(|n| n.origin == origin)
            .map(|n| n.rect.x)
            .unwrap()
    };
    assert_eq!(
        (x(OriginId(2)), x(OriginId(1))),
        (0.0, 40.0),
        "right was declared first"
    );
    assert!(core.take_warnings().is_empty());
}

/// Loading refuses what would make a name ambiguous: a taken namespace,
/// none at all, and a component name with the separator in it. An
/// empty namespace is not "none" — it is the extension's own name, so
/// every host spells "the plugin's own" the same way.
#[test]
fn loading_refuses_ambiguous_namespaces_and_names() {
    let mut exts = Extensions::new();
    exts.push(ext("fs", &["panel"])).unwrap();
    let err = exts.push(ext("fs", &["panel"])).unwrap_err();
    assert!(err.contains("`fs` is already"), "{err}");
    let err = exts.push_as("", ext("", &[])).unwrap_err();
    assert!(err.contains("names itself nothing"), "{err}");
    let err = exts.push_as("git", ext("git", &["side/bar"])).unwrap_err();
    assert!(err.contains("\"side/bar\""), "{err}");
    exts.push_as("", ext("other", &[])).unwrap();
    exts.push_as("also-fs", ext("fs", &["panel"])).unwrap();
    assert_eq!(exts.len(), 3);
    assert_eq!(exts.namespace_of(OriginId(2)), Some("other"));
    assert_eq!(exts.namespace_of(OriginId(3)), Some("also-fs"));
    assert_eq!(split_name("left/fs/panel"), ("left/fs", "panel"));
    assert_eq!(split_name("root"), ("", "root"));
}

/// The instance lands where the host placed it: after the host's children
/// before it, before the ones after it, as children of the node the host
/// was inside — and its key label is what `key_of` answers.
#[test]
fn an_instance_is_drawn_where_it_was_placed_and_answers_to_its_key() {
    let mut core = Core::new();
    let mut exts = load(vec![ext("panel", &["side"])]);
    let after = host_frame(&mut core, &mut exts, 2, Some(("panel/side", &Value::Null)));
    let (key, parent) = cell_of(&mut core, OriginId(1));
    assert_eq!(
        parent,
        Some(Key::ROOT),
        "a fill is a child of the enclosing node"
    );
    assert_eq!(core.key_of("panel/side"), Some(Key::ROOT.str("panel/side")));
    let tree = core.access_tree();
    let x_of = |k: Key| tree.nodes.iter().find(|n| n.key == k).unwrap().rect.x;
    assert_eq!(x_of(key), 80.0, "two host cells of 40 come first");
    assert_eq!(x_of(after), 120.0, "the host's next child follows the fill");
    // The host's own keys are what they would be without the fill: the
    // fill borrowed the counter and gave it back.
    assert_eq!(after, Key::ROOT.str("after"));
}

/// Props are a `Value` declared every frame: what the host passes this
/// frame is what the extension reads this frame, and none of it is
/// identity.
#[test]
fn params_arrive_each_frame_as_declared() {
    let mut core = Core::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut exts = Extensions::new();
    exts.push(Box::new(Ext {
        name: "panel",
        components: vec!["side".into()],
        seen: Rc::clone(&seen),
        ..Default::default()
    }))
    .unwrap();
    let path = Value::map([("path", "/Users".into())]);
    host_frame(&mut core, &mut exts, 0, Some(("panel/side", &path)));
    host_frame(&mut core, &mut exts, 0, Some(("panel/side", &Value::Null)));
    let tmp = Value::map([("path", "/tmp".into())]);
    host_frame(&mut core, &mut exts, 0, Some(("panel/side", &tmp)));
    let side = |v: Value| ("panel".to_owned(), "side".to_owned(), v);
    assert_eq!(
        *seen.borrow(),
        vec![side(path), side(Value::Null), side(tmp)]
    );
    let (key, _) = cell_of(&mut core, OriginId(1));
    assert_eq!(key, Key::ROOT.str("panel/side").str("box"));
    assert!(core.take_warnings().is_empty());
}

/// The fill after the host's view is the instance named `"root"` under
/// the extension's namespace, with no props; a direct `view` call — how a
/// test drives an extension without a runner — gets `Instance::root()`.
#[test]
fn the_fill_after_the_hosts_view_is_the_instance_named_root() {
    let mut core = Core::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut exts = Extensions::new();
    exts.push(Box::new(Ext {
        name: "legacy",
        seen: Rc::clone(&seen),
        ..Default::default()
    }))
    .unwrap();
    host_frame(&mut core, &mut exts, 0, None);
    assert_eq!(
        *seen.borrow(),
        vec![("legacy".to_owned(), "root".to_owned(), Value::Null)]
    );
    let root = Instance::root();
    assert_eq!(
        (root.name, root.namespace, root.key),
        ("root", "", Key::ROOT.str("root"))
    );
    assert_eq!(root.full_name(), "root");
    assert_eq!(*root.params, Value::Null);
}

/// A host that places `"ns/root"` moves the fill after its view to that
/// position, and `finish` does not draw it a second time.
#[test]
fn placing_root_moves_the_fill_after_the_view() {
    let mut core = Core::new();
    let mut exts = load(vec![ext("legacy", &[])]);
    let after = host_frame(&mut core, &mut exts, 1, Some(("legacy/root", &Value::Null)));
    let tree = core.access_tree();
    let ext_nodes: Vec<_> = tree
        .nodes
        .iter()
        .filter(|n| n.origin == OriginId(1))
        .collect();
    assert_eq!(ext_nodes.len(), 1, "filled once, where declared");
    assert_eq!(ext_nodes[0].rect.x, 40.0, "after the host's first cell");
    let after_x = tree.nodes.iter().find(|n| n.key == after).unwrap().rect.x;
    assert_eq!(after_x, 80.0, "and before its second");
    assert!(core.take_warnings().is_empty());
}

/// Decision 5: an extension that returns with nodes open is closed at the
/// depth the fill began, with a warning once, and the host's next child
/// is the host's — not the extension's grandchild.
#[test]
fn an_unbalanced_extension_is_closed_for_it_and_warned_once() {
    let mut core = Core::new();
    let mut exts = load(vec![Box::new(Ext {
        name: "sloppy",
        components: vec!["side".into()],
        leave_open: true,
        ..Default::default()
    })]);
    let after = host_frame(&mut core, &mut exts, 0, Some(("sloppy/side", &Value::Null)));
    let tree = core.access_tree();
    let after_node = tree.nodes.iter().find(|n| n.key == after).unwrap();
    assert_eq!(
        after_node.parent,
        Some(Key::ROOT),
        "the host's child is the root's"
    );
    assert_eq!(after_node.origin, OriginId::HOST);
    let ws = core.take_warnings();
    assert_eq!(codes(&ws), [UNBALANCED_EXTENSION]);
    assert!(
        ws[0].message.contains("2 nodes still open"),
        "{}",
        ws[0].message
    );
    assert!(
        ws[0].message.contains("\"sloppy/side\""),
        "{}",
        ws[0].message
    );
    // Once: the same misbehaviour next frame is the same (code, key).
    host_frame(&mut core, &mut exts, 0, Some(("sloppy/side", &Value::Null)));
    assert!(core.take_warnings().is_empty());
}

/// A component nobody provides warns once and draws nothing; a name placed
/// twice is two instances, each keyed by its place (ADR 0067: no
/// `duplicate-slot`); a component nobody places is silent — an extension
/// may offer more than a host takes.
#[test]
fn an_unknown_component_warns_and_a_name_placed_twice_is_two_instances() {
    let mut core = Core::new();
    let mut exts = load(vec![ext("spare", &["unused"]), ext("panel", &["side"])]);
    for _ in 0..2 {
        let mut ui = core.frame_with(Size::new(600.0, 100.0), 1.0, &mut exts);
        ui.configure_root(NodeSpec::row().fill());
        ui.component("panel/side", &Value::Null);
        ui.component("panel/side", &Value::Null);
        ui.component("panel/status", &Value::Null);
        ui.component("nobody/home", &Value::Null);
        ui.finish();
    }
    let ws = core.take_warnings();
    assert_eq!(codes(&ws), [UNKNOWN_COMPONENT, UNKNOWN_COMPONENT]);
    let tree = core.access_tree();
    assert!(
        tree.nodes.iter().all(|n| n.origin != OriginId(1)),
        "`spare` drew nothing"
    );
    assert_eq!(
        tree.nodes
            .iter()
            .filter(|n| n.origin == OriginId(2))
            .count(),
        2,
        "`panel` drew both placements"
    );
}

/// An extension whose components are not known when it loads — a Lua host
/// whose `init.lua` registers views at runtime, one per (view, pane) —
/// lists `"*"` and draws every name placed under its namespace, each as its
/// own instance; an unrelated namespace is still nobody's (backlog K1).
#[test]
fn a_wildcard_extension_draws_whatever_the_host_places_under_its_namespace() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut core = Core::new();
    let mut exts = load(vec![Box::new(Ext {
        name: "views",
        components: vec![ANY_COMPONENT.to_owned()],
        seen: seen.clone(),
        ..Default::default()
    })]);
    for _ in 0..2 {
        let mut ui = core.frame_with(Size::new(600.0, 100.0), 1.0, &mut exts);
        ui.configure_root(NodeSpec::row().fill());
        ui.component("views/buffer:1", &Value::Int(1));
        ui.component("views/buffer:2", &Value::Int(2));
        ui.component("views/root", &Value::Null);
        ui.component("nobody/home", &Value::Null);
        ui.finish();
    }
    assert_eq!(codes(&core.take_warnings()), [UNKNOWN_COMPONENT]);
    let names: Vec<(String, String, Value)> = seen.borrow().iter().cloned().collect();
    assert_eq!(
        names
            .iter()
            .map(|(ns, n, p)| (ns.as_str(), n.as_str(), p.clone()))
            .collect::<Vec<_>>(),
        [
            ("views", "buffer:1", Value::Int(1)),
            ("views", "buffer:2", Value::Int(2)),
            ("views", "root", Value::Null),
            ("views", "root", Value::Null),
        ],
        "the first frame draws every placement; the second replays the two that read \
         nothing, and draws `root` again, which is never kept"
    );
    let tree = core.access_tree();
    assert_eq!(
        tree.nodes
            .iter()
            .filter(|n| n.origin == OriginId(1))
            .count(),
        3,
        "three fills, three cells"
    );
    // A wildcard is not the "lists nothing" case: with no declaration it
    // draws nowhere, rather than after the host's view as `root`.
    let mut ui = core.frame_with(Size::new(600.0, 100.0), 1.0, &mut exts);
    ui.configure_root(NodeSpec::row().fill());
    ui.finish();
    assert_eq!(codes(&core.take_warnings()), Vec::<&str>::new());
    assert!(
        core.access_tree()
            .nodes
            .iter()
            .all(|n| n.origin != OriginId(1)),
        "nothing declared, nothing drawn"
    );
}

/// An event says which instance drew its node (backlog K2): one extension
/// drawing an instance per pane routes a click by pane without stamping
/// every payload, since `origin` alone cannot tell the panes apart. The
/// host's own nodes carry none, and an answer that is not an `emit`
/// reaches nobody (ADR 0067).
#[test]
fn an_event_carries_the_instance_that_drew_its_node() {
    /// Fills any slot with one clickable cell.
    struct Clicky;
    impl Extension for Clicky {
        fn name(&self) -> &str {
            "views"
        }
        fn components(&self) -> &[String] {
            std::slice::from_ref(Box::leak(Box::new(ANY_COMPONENT.to_owned())))
        }
        fn view(&mut self, _slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
            ui.leaf_keyed(
                "box",
                cell().on_click(Value::map([("kind", "cell".into())])),
            );
            Ok(())
        }
        fn on_event(&mut self, _ev: &UiEvent) -> Vec<Value> {
            vec![Value::map([("kind", "seen".into())])]
        }
    }
    let mut core = Core::new();
    let mut exts = load(vec![Box::new(Clicky)]);
    // A row: the host's cell at x 0..40, pane 1's at 40..80, pane 2's at
    // 80..120.
    let mut ui = core.frame_with(Size::new(600.0, 100.0), 1.0, &mut exts);
    ui.configure_root(NodeSpec::row().fill());
    ui.leaf_keyed(
        "host",
        cell().on_click(Value::map([("kind", "host".into())])),
    );
    ui.component("views/pane:1", &keyed("views/pane:1", &Value::Null));
    ui.component("views/pane:2", &keyed("views/pane:2", &Value::Null));
    ui.finish();
    let pane1 = core.key_of("views/pane:1").expect("the instance's key");
    let pane2 = core.key_of("views/pane:2").expect("the instance's key");
    assert_ne!(pane1, pane2);

    let stamped = |core: &mut Core, x: f32| -> Vec<(Key, Option<Key>)> {
        kui_core::testing::click_at(core, x, 10.0)
            .into_iter()
            .map(|e| (e.key, e.slot))
            .collect()
    };
    let host = core.key_of("host").unwrap();
    assert_eq!(
        stamped(&mut core, 20.0),
        [(host, None)],
        "the host's own node: no instance"
    );
    assert_eq!(stamped(&mut core, 60.0), [(pane1.str("box"), Some(pane1))]);
    let raw = kui_core::testing::click_at(&mut core, 100.0, 10.0);
    assert_eq!(
        raw.iter().map(|e| (e.key, e.slot)).collect::<Vec<_>>(),
        [(pane2.str("box"), Some(pane2))]
    );
    // Routed, the extension's plain answer goes nowhere: only an `emit`
    // reaches whoever placed it.
    let mut host_got = Vec::new();
    exts.route(raw, |ev| host_got.push(ev));
    assert!(host_got.is_empty(), "{host_got:?}");
}

/// A view that errors leaves its message in the tree where the fill would
/// have been and is reported once, not once per frame.
#[test]
fn a_view_error_is_drawn_in_place_and_warned_once() {
    let mut core = Core::new();
    let mut exts = load(vec![Box::new(Ext {
        name: "broken",
        components: vec!["side".into()],
        fail: true,
        ..Default::default()
    })]);
    for _ in 0..2 {
        host_frame(&mut core, &mut exts, 0, Some(("broken/side", &Value::Null)));
    }
    let ws = core.take_warnings();
    assert_eq!(codes(&ws), [EXTENSION_VIEW_ERROR]);
    assert!(ws[0].message.contains("no view today"), "{}", ws[0].message);
    let tree = core.access_tree();
    let text = tree
        .nodes
        .iter()
        .find(|n| n.origin == OriginId(1))
        .and_then(|n| n.name.clone());
    assert_eq!(text.as_deref(), Some("[broken] no view today"));
}

/// An extension that hosts one of its own: it loads `inner` on the frame
/// it first draws and places that plugin's component in the middle of its
/// own view — the Lua-view-with-a-native-panel case, as the smallest thing
/// that is it.
struct Loader {
    /// Handed over on the first `view`; `None` after, the way a script
    /// loads once and draws every frame.
    inner: Option<Box<dyn Extension>>,
    /// The full name it places, its guest's or (for the cycle) its own.
    declares: &'static str,
    /// What it passes the placement besides its key: event props.
    props: Value,
    heard: Rc<RefCell<Vec<Value>>>,
    reply: Option<Value>,
    components: Vec<String>,
}

impl Extension for Loader {
    fn name(&self) -> &str {
        "loader"
    }
    fn components(&self) -> &[String] {
        &self.components
    }
    fn view(&mut self, _slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        if let Some(inner) = self.inner.take() {
            ui.add_extension("inner", inner)?;
        }
        ui.leaf_keyed("mine", cell());
        ui.component(self.declares, &keyed(self.declares, &self.props));
        Ok(())
    }
    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
        self.heard.borrow_mut().push(ev.payload.clone());
        self.reply.clone().into_iter().collect()
    }
}

fn loader(declares: &'static str, inner: Option<Box<dyn Extension>>) -> Loader {
    Loader {
        inner,
        declares,
        props: Value::Null,
        heard: Rc::default(),
        reply: None,
        components: Vec::new(),
    }
}

/// ADR 0014 left one question open — "whether one may offer slots is a
/// decision for the day one asks". The answer: an extension may load
/// extensions and place their components, like any host. Keys nest, so the
/// guest's guest is keyed inside the fill that placed it and moves with it.
#[test]
fn an_extension_hosts_an_extension_of_its_own() {
    let inner = Ext {
        name: "inner",
        components: vec!["panel".into()],
        ..Default::default()
    };
    let mut core = Core::new();
    let mut exts = Extensions::new();
    exts.push_as(
        "outer",
        Box::new(loader("inner/panel", Some(Box::new(inner)))),
    )
    .unwrap();
    host_frame(&mut core, &mut exts, 0, None);

    // Both are named, and the inner one is keyed inside the outer fill
    // rather than off the host's root.
    let outer = core.key_of("outer/root").expect("the outer root fill");
    let panel = core
        .key_of("inner/panel")
        .expect("the instance the guest placed");
    assert_eq!(panel, outer.str("inner/panel"));
    // Two extensions, two origins, and the guest's guest drew.
    assert_eq!(exts.len(), 2);
    assert_eq!(cell_of(&mut core, OriginId(2)).0, panel.str("box"));
    assert!(core.take_warnings().is_empty());
}

/// An instance's events reach whoever placed it: for a plugin a guest
/// placed that is the guest, which may emit in turn to whoever placed *it*
/// — here the host. The untyped reply of ADR 0014 is gone (ADR 0067).
#[test]
fn a_nested_instances_events_reach_whoever_placed_it() {
    let inner = Ext {
        name: "inner",
        components: vec!["panel".into()],
        reply: Some(kui_core::slot::emit("on_toggle", Value::Null)),
        ..Default::default()
    };
    let mut outer = loader("inner/panel", Some(Box::new(inner)));
    outer.props = Value::map([("on_toggle", Value::map([("kind", "toggled".into())]))]);
    outer.reply = Some(kui_core::slot::emit("on_summary", Value::Null));
    let outer_heard = outer.heard.clone();

    let mut core = Core::new();
    let mut exts = Extensions::new();
    exts.push_as("outer", Box::new(outer)).unwrap();
    let summary = Value::map([("on_summary", Value::map([("kind", "summarised".into())]))]);
    host_frame(&mut core, &mut exts, 0, Some(("outer/root", &summary)));

    let panel = core.key_of("inner/panel").expect("the guest's instance");
    let mut to_host = Vec::new();
    exts.route(
        [{
            let mut ev = UiEvent::declared(
                kui_core::EventKind::Click,
                OriginId(2),
                panel.str("box"),
                Some(&Value::map([("kind", "click".into())])),
                Value::Null,
            );
            ev.slot = Some(panel);
            ev
        }],
        |ev| to_host.push(ev),
    );

    // The click reached the panel; its emit reached the extension that
    // placed the panel as that placer's own message, and not the host.
    assert_eq!(
        outer_heard.borrow().iter().map(kind_of).collect::<Vec<_>>(),
        ["toggled"]
    );
    // The host heard only what the placer emitted, as the host's own
    // message for the prop.
    assert_eq!(
        to_host
            .iter()
            .map(|e| kind_of(&e.payload))
            .collect::<Vec<_>>(),
        ["summarised"]
    );
    assert_eq!(to_host[0].origin, OriginId::HOST);
}

/// The one thing nesting cannot do without a spare handle. An extension is
/// out of the list while it draws, so a component of its own that it
/// places finds nobody: an empty position and one warning, rather than a
/// view calling itself.
#[test]
fn an_extension_without_a_spare_cannot_draw_its_own_component() {
    let mut outer = loader("self/status", None);
    outer.components = vec!["panel".into(), "status".into()];
    let mut core = Core::new();
    let mut exts = Extensions::new();
    exts.push_as("self", Box::new(outer)).unwrap();
    host_frame(&mut core, &mut exts, 0, Some(("self/panel", &Value::Null)));

    assert_eq!(codes(&core.take_warnings()), [RECURSIVE_COMPONENT]);
    // It drew once, where the host placed it, and the position it asked
    // to draw itself in is empty.
    let tree = core.access_tree();
    assert_eq!(
        tree.nodes
            .iter()
            .filter(|n| n.origin == OriginId(1))
            .count(),
        1
    );
}

/// A guest asking by label gets its own node (found by the QA round of
/// 2026-09-14 on `lua_panel`, whose script and the C plugin it loads both
/// key an editor "filter"): the script's `env.edit_text("filter")` was a
/// whole-frame lookup, so from its second frame on it hit both and warned
/// `ambiguous-key` every frame. Labels are unique among siblings, and a
/// guest cannot know what the host or another guest called its nodes — so
/// `key_of` prefers the nodes the asking origin opened, in this frame and
/// in the last one, and only a clash among those is an ambiguity.
#[test]
fn a_label_asked_for_by_a_guest_is_the_guests_own_node() {
    /// (what `key_of` answered before the fill declared it, the key the
    /// fill then declared)
    type Asked = (Option<Key>, Key);
    struct Asker {
        seen: Rc<RefCell<Vec<Asked>>>,
    }
    impl Extension for Asker {
        fn name(&self) -> &str {
            "asker"
        }
        fn components(&self) -> &[String] {
            &[]
        }
        fn view(&mut self, _slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
            // Asked before this frame declares it — the last frame's
            // answer — the way a script reads an editor at the top of
            // its view.
            let asked = ui.key_of("filter");
            let mine = ui.leaf_keyed("filter", cell());
            self.seen.borrow_mut().push((asked, mine));
            Ok(())
        }
        fn on_event(&mut self, _ev: &UiEvent) -> Vec<Value> {
            vec![]
        }
    }
    let seen = Rc::new(RefCell::new(vec![]));
    let mut exts = load(vec![Box::new(Asker { seen: seen.clone() })]);
    let mut core = Core::new();
    core.set_diagnostics(true);
    let mut host_keys = vec![];
    for _ in 0..2 {
        let mut ui = core.frame_with(Size::new(600.0, 100.0), 1.0, &mut exts);
        ui.configure_root(NodeSpec::row().fill());
        // The host's own "filter", declared before the slot in tree order.
        host_keys.push(ui.leaf_keyed("filter", cell()));
        // And the host asks for its own, this frame's — not the guest's.
        assert_eq!(ui.key_of("filter"), host_keys.last().copied());
        ui.component("asker/root", &Value::Null);
        ui.finish();
    }
    let seen = seen.borrow();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].0, None, "nothing declared before the first frame");
    assert_eq!(
        seen[1].0,
        Some(seen[0].1),
        "the second frame's ask is answered from the first frame's fill"
    );
    assert_ne!(seen[1].1, host_keys[1], "two nodes, one label, two parents");
    assert_eq!(
        codes(&core.take_warnings()),
        Vec::<&str>::new(),
        "and no ambiguity"
    );
}

/// One extension filling many slots — a Lua host draws every pane of
/// its as a slot of its one origin — asks by label from inside each fill
/// and gets that fill's node (backlog F157, from kawoosh, where the
/// themes pane beside a launcher scrolled the launcher's "body" and six
/// panes keyed their scroller "list"). Each fill is a view of its own
/// and cannot know what the others called their nodes, so a lookup from
/// inside a fill reads that fill's labels alone, in this frame and the
/// last, for `key_of` and for the deferred `set_scroll_label` — and a
/// view that asks before it has declared its node (its first frame)
/// gets nothing rather than a sibling pane's node of the same name.
#[test]
fn a_label_asked_for_inside_a_fill_is_that_fills_own_node() {
    /// Per fill, per frame: (its slot, what `key_of` answered before it
    /// declared the label, the key it declared, what `b` got asking for
    /// a label only `a` declares).
    type Asked = (String, Option<Key>, Key, Option<Key>);
    struct Panes {
        seen: Rc<RefCell<Vec<Asked>>>,
        components: Vec<String>,
    }
    impl Extension for Panes {
        fn name(&self) -> &str {
            "panes"
        }
        fn components(&self) -> &[String] {
            &self.components
        }
        fn view(&mut self, slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
            let asked = ui.key_of("list");
            // Each pane scrolls its own list to a place of its own.
            let y = if slot.name == "a" { 30.0 } else { 70.0 };
            ui.set_scroll_label("list", Vec2::new(0.0, y));
            // And one label only pane `a` declares, asked by `b`: a
            // sibling's, so not `b`'s to find.
            let other = if slot.name == "b" {
                ui.key_of("only-a")
            } else {
                None
            };
            if slot.name == "a" {
                ui.leaf_keyed("only-a", cell());
            }
            let mine = ui.with_keyed(
                "list",
                NodeSpec::column().size(100.0, 50.0).scroll_y(),
                |ui| {
                    for _ in 0..20 {
                        ui.leaf(cell());
                    }
                },
            );
            self.seen
                .borrow_mut()
                .push((slot.name.to_string(), asked, mine, other));
            Ok(())
        }
        fn on_event(&mut self, _ev: &UiEvent) -> Vec<Value> {
            vec![]
        }
    }
    let seen = Rc::new(RefCell::new(vec![]));
    let mut exts = load(vec![Box::new(Panes {
        seen: seen.clone(),
        components: vec![ANY_COMPONENT.into()],
    })]);
    let mut core = Core::new();
    core.set_diagnostics(true);
    for _ in 0..3 {
        let mut ui = core.frame_with(Size::new(600.0, 200.0), 1.0, &mut exts);
        ui.configure_root(NodeSpec::row().fill());
        ui.component("panes/a", &Value::map([("fresh", Value::Bool(true))]));
        ui.component("panes/b", &Value::map([("fresh", Value::Bool(true))]));
        ui.finish();
    }
    let seen = seen.borrow();
    let (a, b): (Vec<_>, Vec<_>) = seen.iter().partition(|(s, ..)| s == "a");
    assert_eq!((a.len(), b.len()), (3, 3));
    assert_ne!(a[0].2, b[0].2, "two lists, one label, two fills");
    // The first frame: neither has declared its own yet, and `b`, drawn
    // after `a`, does not get `a`'s.
    assert_eq!(a[0].1, None, "nothing declared before the first frame");
    assert_eq!(b[0].1, None, "nor a sibling pane's node for b");
    assert!(
        b.iter().all(|f| f.3.is_none()),
        "a label only a declares is not b's"
    );
    // From then on each fill finds its own, from the frame before — not
    // the sibling fill this frame has already declared.
    for pane in [&a, &b] {
        for f in 1..3 {
            assert_eq!(
                pane[f].1,
                Some(pane[f - 1].2),
                "its own, from the frame before"
            );
        }
    }
    assert_eq!(
        core.scroll_offset(a[2].2).y,
        30.0,
        "a's set_scroll_label is a's"
    );
    assert_eq!(
        core.scroll_offset(b[2].2).y,
        70.0,
        "b's set_scroll_label is b's"
    );
    assert_eq!(
        codes(&core.take_warnings()),
        Vec::<&str>::new(),
        "and no ambiguity, nor a label without its node"
    );
}

/// A window's own events arrive at whoever declared the `windowEvents`
/// group (ADR 0048): Lua, filling a slot, declares it on its root table,
/// and before the group carried its origin the core answered the host —
/// which then lost its own group to the guest's and was handed a message
/// it never wrote.
#[test]
fn a_window_event_reaches_the_extension_that_declared_the_group() {
    struct Declares {
        heard: Rc<RefCell<Vec<Value>>>,
    }
    impl Extension for Declares {
        fn name(&self) -> &str {
            "pane"
        }
        fn components(&self) -> &[String] {
            std::slice::from_ref(Box::leak(Box::new("main".to_owned())))
        }
        fn view(&mut self, _slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
            ui.window_events(kui_core::WindowEvents::default().on_resize("pane-resized"));
            ui.leaf_keyed("box", cell());
            Ok(())
        }
        fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
            self.heard.borrow_mut().push(ev.payload.clone());
            Vec::new()
        }
    }
    let heard = Rc::new(RefCell::new(Vec::new()));
    let mut core = Core::new();
    let mut exts = load(vec![Box::new(Declares {
        heard: heard.clone(),
    })]);
    let frame = |core: &mut Core, exts: &mut Extensions, w: f32, guest: bool| {
        let mut ui = core.frame_with(Size::new(w, 100.0), 1.0, exts);
        ui.window_events(kui_core::WindowEvents::default().on_resize("host-resized"));
        ui.configure_root(NodeSpec::row().fill());
        if guest {
            ui.component("pane/main", &Value::Null);
        }
        ui.finish();
    };
    let routed = |core: &mut Core, exts: &mut Extensions| {
        let mut host = Vec::new();
        exts.route(core.take_pending_events(), |ev| host.push(ev));
        host
    };

    frame(&mut core, &mut exts, 600.0, true);
    routed(&mut core, &mut exts);
    frame(&mut core, &mut exts, 700.0, true);
    let host = routed(&mut core, &mut exts);
    assert!(host.is_empty(), "the host's group was replaced: {host:?}");
    let got = heard.borrow_mut().drain(..).collect::<Vec<_>>();
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].get_str("tag"), Some("pane-resized"));

    // A frame that leaves the guest out: the host's group is the one in
    // force, and the host hears it.
    frame(&mut core, &mut exts, 600.0, false);
    routed(&mut core, &mut exts);
    frame(&mut core, &mut exts, 800.0, false);
    let host = routed(&mut core, &mut exts);
    assert_eq!(host.len(), 1, "{host:?}");
    assert_eq!(host[0].payload.get_str("tag"), Some("host-resized"));
    assert_eq!(host[0].origin, OriginId::HOST);
    assert!(heard.borrow().is_empty());
}
