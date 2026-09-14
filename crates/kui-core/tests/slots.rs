//! Slots (`docs/adr/0014-slots-an-extension-fills-in-place.md`): a host
//! declares a position in its own view, an extension fills it there and
//! then under a key namespace the slot fixes, parameters arrive every frame
//! as data, and the fill is bounded. Slot names are namespaced, and the
//! host decides the namespace when it loads the extension. The runner's
//! loop over extensions is `Extensions`' `Fill`, so these drive that with a
//! stand-in extension rather than a filler of their own.

use std::cell::RefCell;
use std::rc::Rc;

use kui_core::diag::{
    DUPLICATE_SLOT, EXTENSION_VIEW_ERROR, RECURSIVE_SLOT, UNBALANCED_EXTENSION, UNKNOWN_SLOT,
};
use kui_core::testing::codes;
use kui_core::{
    ANY_SLOT, Core, Extension, Extensions, Key, NodeSpec, OriginId, Size, Sizing, Slot, Ui,
    UiEvent, Value, split_name,
};

/// A stand-in extension: opens one keyed, focusable cell (so it is in the
/// access tree, with its key and parent), records which slot it was asked
/// to fill and with what, and misbehaves on request.
#[derive(Default)]
struct Ext {
    name: &'static str,
    slots: Vec<String>,
    /// Shared with the test, since a boxed extension cannot be read back
    /// once it is in the runner's list: (namespace, name, params).
    seen: Rc<RefCell<Vec<(String, String, Value)>>>,
    /// The payloads that reached `on_event`, shared the same way.
    heard: Rc<RefCell<Vec<Value>>>,
    /// What it replies with when it hears anything.
    reply: Option<Value>,
    leave_open: bool,
    fail: bool,
}

fn cell() -> NodeSpec {
    NodeSpec::column()
        .width(Sizing::Fixed(40.0))
        .height(Sizing::Fixed(20.0))
        .focusable()
}

impl Extension for Ext {
    fn name(&self) -> &str {
        self.name
    }
    fn slots(&self) -> &[String] {
        &self.slots
    }
    fn view(&mut self, slot: &Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        self.seen.borrow_mut().push((
            slot.namespace.to_owned(),
            slot.name.to_owned(),
            slot.params.clone(),
        ));
        if self.fail {
            return Err("no view today".into());
        }
        if self.leave_open {
            ui.open(cell());
            ui.open_keyed("box", cell());
        } else {
            ui.with_keyed("box", cell(), |_| {});
        }
        Ok(())
    }
    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
        self.heard.borrow_mut().push(ev.payload.clone());
        self.reply.clone().into_iter().collect()
    }
}

fn ext(name: &'static str, slots: &[&str]) -> Box<dyn Extension> {
    Box::new(Ext {
        name,
        slots: slots.iter().map(|s| (*s).to_owned()).collect(),
        ..Default::default()
    })
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

/// A root row of `before` fixed cells, then the slot (or nothing), then one
/// more cell. Returns the key of the host cell after the slot.
fn host_frame(
    core: &mut Core,
    exts: &mut Extensions,
    before: usize,
    slot: Option<(&str, &Value)>,
) -> Key {
    let mut ui = core.frame_with(Size::new(600.0, 100.0), 1.0, exts);
    ui.configure_root(NodeSpec::row().fill());
    for _ in 0..before {
        ui.with(cell(), |_| {});
    }
    if let Some((name, params)) = slot {
        ui.slot_with(name, params);
    }
    let after = ui.with_keyed("after", cell(), |_| {});
    ui.finish();
    after
}

/// The third context item of the ADR: an extension's keys used to be
/// `root.index(n)` for whatever `n` the host happened to leave, so a host
/// adding a child at the root rekeyed the whole panel. Under a slot they
/// are `slot.…`, whatever the host builds around them.
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

/// The same for the reserved `"root"` slot — the fill every extension got
/// before slots existed, now keyed under `root.str("ns/root")`.
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
                slots: vec!["panel".into()],
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
    ui.slot_with("right/panel", &r);
    ui.slot_with("left/panel", &l);
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

/// Loading refuses what would make a slot name ambiguous: a taken
/// namespace, none at all, and a slot name with the separator in it. An
/// empty namespace is not "none" — it is the extension's own name, so
/// every host spells "the plugin's own" the same way.
#[test]
fn loading_refuses_ambiguous_namespaces_and_slot_names() {
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

/// The fill lands where the host declared it: after the host's children
/// before the slot, before the ones after it, as children of the node the
/// host was inside — and the slot's full name is what `key_of` answers.
#[test]
fn a_fill_is_placed_at_the_slot_and_the_slot_is_named() {
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

/// Parameters are a `Value` declared every frame: what the host passes
/// this frame is what the extension reads this frame, `Null` for `slot`,
/// and none of it is identity.
#[test]
fn params_arrive_each_frame_as_declared() {
    let mut core = Core::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut exts = Extensions::new();
    exts.push(Box::new(Ext {
        name: "panel",
        slots: vec!["side".into()],
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

/// The `"root"` fill is seen as the slot named `"root"` under the
/// extension's namespace, with no params; a direct `view` call — how a
/// test drives an extension without a runner — gets `Slot::root()`.
#[test]
fn the_root_fill_is_the_slot_named_root() {
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
    let root = Slot::root();
    assert_eq!(
        (root.name, root.namespace, root.key),
        ("root", "", Key::ROOT.str("root"))
    );
    assert_eq!(root.full_name(), "root");
    assert_eq!(*root.params, Value::Null);
}

/// A host that declares `"ns/root"` moves the legacy fill to that
/// position, and `finish` does not fill it a second time.
#[test]
fn declaring_root_moves_the_legacy_fill() {
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
        slots: vec!["side".into()],
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

/// A slot nobody declared and a name declared twice each warn once, and
/// neither draws twice: the unknown fill is skipped, the duplicate ignored.
/// A declared slot no extension fills is silent — the host may offer more
/// than a plugin takes.
#[test]
fn unknown_and_duplicate_slots_warn_once_and_draw_nothing_extra() {
    let mut core = Core::new();
    let mut exts = load(vec![ext("lost", &["nowhere"]), ext("panel", &["side"])]);
    for _ in 0..2 {
        let mut ui = core.frame_with(Size::new(600.0, 100.0), 1.0, &mut exts);
        ui.configure_root(NodeSpec::row().fill());
        ui.slot("panel/side");
        ui.slot("panel/side");
        ui.slot("panel/status");
        ui.slot("nobody/home");
        ui.finish();
    }
    let ws = core.take_warnings();
    let mut got = codes(&ws);
    got.sort_unstable();
    assert_eq!(got, [DUPLICATE_SLOT, UNKNOWN_SLOT]);
    let unknown = ws.iter().find(|w| w.code == UNKNOWN_SLOT).unwrap();
    assert!(
        unknown.message.contains("`lost` (namespace `lost`)"),
        "{}",
        unknown.message
    );
    assert!(
        unknown.message.contains("\"lost/nowhere\""),
        "{}",
        unknown.message
    );
    let tree = core.access_tree();
    assert!(
        tree.nodes.iter().all(|n| n.origin != OriginId(1)),
        "`lost` drew nothing"
    );
    assert_eq!(
        tree.nodes
            .iter()
            .filter(|n| n.origin == OriginId(2))
            .count(),
        1,
        "`panel` filled the first declaration only"
    );
}

/// An extension whose slots are not known when it loads — a Lua host whose
/// `init.lua` registers views at runtime, one slot per (view, pane) — lists
/// `"*"` and fills every name the host declares under its namespace, each
/// as its own slot; it gets no `unknown-slot`, since there is no list to
/// check, and an unrelated namespace is still nobody's (backlog K1).
#[test]
fn a_wildcard_extension_fills_whatever_the_host_declares_under_its_namespace() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut core = Core::new();
    let mut exts = load(vec![Box::new(Ext {
        name: "views",
        slots: vec![ANY_SLOT.to_owned()],
        seen: seen.clone(),
        ..Default::default()
    })]);
    for _ in 0..2 {
        let mut ui = core.frame_with(Size::new(600.0, 100.0), 1.0, &mut exts);
        ui.configure_root(NodeSpec::row().fill());
        ui.slot_with("views/buffer:1", &Value::Int(1));
        ui.slot_with("views/buffer:2", &Value::Int(2));
        ui.slot("views/root");
        ui.slot("nobody/home");
        ui.finish();
    }
    assert_eq!(codes(&core.take_warnings()), Vec::<&str>::new());
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
            ("views", "buffer:1", Value::Int(1)),
            ("views", "buffer:2", Value::Int(2)),
            ("views", "root", Value::Null),
        ],
        "every declared name, each frame, `root` as one more name and not the auto-fill"
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

/// An event says which slot's fill drew its node (backlog K2): one
/// extension filling a slot per pane routes a click by pane without
/// stamping every payload, since `origin` alone cannot tell the panes
/// apart. The host's own nodes carry none, and a reply keeps the slot of
/// the event it answers.
#[test]
fn an_event_carries_the_slot_its_node_was_filled_into() {
    /// Fills any slot with one clickable cell.
    struct Clicky;
    impl Extension for Clicky {
        fn name(&self) -> &str {
            "views"
        }
        fn slots(&self) -> &[String] {
            std::slice::from_ref(Box::leak(Box::new(ANY_SLOT.to_owned())))
        }
        fn view(&mut self, _slot: &Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
            ui.with_keyed(
                "box",
                cell().on_click(Value::map([("kind", "cell".into())])),
                |_| {},
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
    ui.with_keyed(
        "host",
        cell().on_click(Value::map([("kind", "host".into())])),
        |_| {},
    );
    ui.slot("views/pane:1");
    ui.slot("views/pane:2");
    ui.finish();
    let pane1 = core.key_of("views/pane:1").expect("the slot's key");
    let pane2 = core.key_of("views/pane:2").expect("the slot's key");
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
        "the host's own node: no slot"
    );
    assert_eq!(stamped(&mut core, 60.0), [(pane1.str("box"), Some(pane1))]);
    let raw = kui_core::testing::click_at(&mut core, 100.0, 10.0);
    assert_eq!(
        raw.iter().map(|e| (e.key, e.slot)).collect::<Vec<_>>(),
        [(pane2.str("box"), Some(pane2))]
    );
    // Routed, the reply is about the same node and keeps its slot.
    let mut host_got = Vec::new();
    exts.route(raw, |ev| host_got.push(ev));
    assert_eq!(host_got.len(), 1);
    assert_eq!(kind_of(&host_got[0].payload), "seen");
    assert_eq!(host_got[0].slot, Some(pane2));
    assert_eq!(host_got[0].origin, OriginId(1));
}

/// A view that errors leaves its message in the tree where the fill would
/// have been and is reported once, not once per frame.
#[test]
fn a_view_error_is_drawn_in_place_and_warned_once() {
    let mut core = Core::new();
    let mut exts = load(vec![Box::new(Ext {
        name: "broken",
        slots: vec!["side".into()],
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
/// it first draws and declares that plugin's slot in the middle of its own
/// view — the Lua-view-with-a-native-panel case, as the smallest thing
/// that is it.
struct Loader {
    /// Handed over on the first `view`; `None` after, the way a script
    /// loads once and draws every frame.
    inner: Option<Box<dyn Extension>>,
    /// The full name it declares, its guest's or (for the cycle) its own.
    declares: &'static str,
    heard: Rc<RefCell<Vec<Value>>>,
    reply: Option<Value>,
    slots: Vec<String>,
}

impl Extension for Loader {
    fn name(&self) -> &str {
        "loader"
    }
    fn slots(&self) -> &[String] {
        &self.slots
    }
    fn view(&mut self, _slot: &Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        if let Some(inner) = self.inner.take() {
            ui.add_extension("inner", inner)?;
        }
        ui.with_keyed("mine", cell(), |_| {});
        ui.slot(self.declares);
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
        heard: Rc::default(),
        reply: None,
        slots: Vec::new(),
    }
}

/// The ADR left one question open — "whether one may offer slots is a
/// decision for the day one asks". The answer: an extension may load
/// extensions and declare their slots, and what it declares is a slot
/// like any other. Keys nest, so the guest's guest is keyed inside the
/// fill that placed it and moves with it.
#[test]
fn an_extension_hosts_an_extension_of_its_own() {
    let inner = Ext {
        name: "inner",
        slots: vec!["panel".into()],
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

    // Both slots are named, and the inner one is keyed inside the outer
    // fill rather than off the host's root.
    let outer = core.key_of("outer/root").expect("the outer root fill");
    let panel = core
        .key_of("inner/panel")
        .expect("the slot the guest declared");
    assert_eq!(panel, outer.str("inner/panel"));
    // Two extensions, two origins, and the guest's guest drew.
    assert_eq!(exts.len(), 2);
    assert_eq!(cell_of(&mut core, OriginId(2)).0, panel.str("box"));
    assert!(core.take_warnings().is_empty());
}

/// Decision 6 read as it is written: a reply answers the slot, so it goes
/// to whoever declared it. For everything a host placed that is the host,
/// as before; for a plugin a guest placed it is the guest, and what the
/// guest answers travels on up.
#[test]
fn a_nested_extensions_replies_reach_whoever_placed_it() {
    let inner = Ext {
        name: "inner",
        slots: vec!["panel".into()],
        reply: Some(Value::map([("kind", "toggled".into())])),
        ..Default::default()
    };
    let mut outer = loader("inner/panel", Some(Box::new(inner)));
    outer.reply = Some(Value::map([("kind", "summarised".into())]));
    let outer_heard = outer.heard.clone();

    let mut core = Core::new();
    let mut exts = Extensions::new();
    exts.push_as("outer", Box::new(outer)).unwrap();
    host_frame(&mut core, &mut exts, 0, None);

    let mut to_host = Vec::new();
    exts.route(
        [UiEvent {
            origin: OriginId(2),
            window: kui_core::WindowId::MAIN,
            key: Key::ROOT,
            payload: Value::map([("kind", "click".into())]),
            slot: None,
        }],
        |ev| to_host.push(ev),
    );

    // The click reached the panel; its reply reached the extension that
    // placed the panel, and not the host.
    assert_eq!(
        outer_heard.borrow().iter().map(kind_of).collect::<Vec<_>>(),
        ["toggled"]
    );
    // The host heard only what the placer said about it, with the
    // placer's origin on it.
    assert_eq!(
        to_host
            .iter()
            .map(|e| kind_of(&e.payload))
            .collect::<Vec<_>>(),
        ["summarised"]
    );
    assert_eq!(to_host[0].origin, OriginId(1));
}

/// The one thing nesting cannot do. An extension is out of the list while
/// it draws, so a slot of its own that it declares finds nobody: an empty
/// position and one warning, rather than a view calling itself.
#[test]
fn an_extension_cannot_fill_its_own_slot() {
    let mut outer = loader("self/status", None);
    outer.slots = vec!["panel".into(), "status".into()];
    let mut core = Core::new();
    let mut exts = Extensions::new();
    exts.push_as("self", Box::new(outer)).unwrap();
    host_frame(&mut core, &mut exts, 0, Some(("self/panel", &Value::Null)));

    assert_eq!(codes(&core.take_warnings()), [RECURSIVE_SLOT]);
    // It drew once, in the slot the host declared, and the position it
    // asked to fill twice is empty.
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
        fn slots(&self) -> &[String] {
            &[]
        }
        fn view(&mut self, _slot: &Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
            // Asked before this frame declares it — the last frame's
            // answer — the way a script reads an editor at the top of
            // its view.
            let asked = ui.key_of("filter");
            let mine = ui.with_keyed("filter", cell(), |_| {});
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
        host_keys.push(ui.with_keyed("filter", cell(), |_| {}));
        // And the host asks for its own, this frame's — not the guest's.
        assert_eq!(ui.key_of("filter"), host_keys.last().copied());
        ui.slot_with("asker/root", &Value::Null);
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
