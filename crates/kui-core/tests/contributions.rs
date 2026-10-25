//! Contribution slots (ADR 0067, the new slot): a host declares a place by
//! name and every loaded extension that contributes to it appends one of
//! its own components there — in load order and then `order`, as siblings,
//! each an instance keyed by the slot, its provider and its component, with
//! the slot's props, its events the host's messages with `from` naming who
//! raised them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use kui_core::slot::emit;
use kui_core::testing::click_at;
use kui_core::{
    Contribution, Core, EventKind, Extension, Extensions, FillAnswer, Instance, NodeSpec, Size, Ui,
    UiEvent, Value,
};

const VIEW: Size = Size { w: 600.0, h: 200.0 };

/// What a plugin saw of each fill: (component, slot, props).
type Seen = Rc<RefCell<Vec<(String, Option<String>, Value)>>>;

/// Provides its `components`, contributes its `contributions`, and draws a
/// 60×20 button named `ns:component` whose click raises the host's `onRun`
/// with the component's name. A component named `frame` declares the slot
/// `shell.items` inside itself instead.
#[derive(Default)]
struct Plug {
    components: Vec<String>,
    contributions: Vec<Contribution>,
    fills: Rc<Cell<usize>>,
    seen: Seen,
}

fn plug(components: &[&str], contributions: Vec<Contribution>) -> Plug {
    Plug {
        components: components.iter().map(|s| (*s).to_owned()).collect(),
        contributions,
        ..Default::default()
    }
}

impl Extension for Plug {
    fn name(&self) -> &str {
        "plug"
    }
    fn components(&self) -> &[String] {
        &self.components
    }
    fn contributions(&self) -> &[Contribution] {
        &self.contributions
    }
    fn view(&mut self, _inst: &Instance<'_>, _ui: &mut Ui<'_>) -> Result<(), String> {
        Ok(())
    }
    fn component(&mut self, inst: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        self.fills.set(self.fills.get() + 1);
        self.seen.borrow_mut().push((
            inst.name.to_owned(),
            inst.slot.map(str::to_owned),
            inst.params.clone(),
        ));
        if inst.name == "frame" {
            ui.with_keyed("frame", NodeSpec::row().gap(4.0), |ui| {
                ui.slot("shell.items", &Value::Null);
            });
            return Ok(());
        }
        let label = format!("{}:{}", inst.namespace, inst.name);
        ui.with_keyed(
            "btn",
            NodeSpec::row()
                .size(60.0, 20.0)
                .on_click(Value::map([
                    ("tag", Value::str("press")),
                    ("c", Value::str(inst.name)),
                ]))
                .label(label.as_str()),
            |_| {},
        );
        Ok(())
    }
    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
        match ev.payload.get_str("c") {
            Some(c) => vec![emit("onRun", Value::map([("c", Value::str(c))]))],
            None => Vec::new(),
        }
    }
}

fn core() -> Core {
    let mut core = Core::new();
    core.set_inspect(true);
    core
}

fn frame(core: &mut Core, exts: &mut Extensions, view: impl FnOnce(&mut Ui<'_>)) {
    let mut ui = core.frame_with(VIEW, 1.0, exts);
    ui.configure_root(NodeSpec::row().pad(8.0).gap(4.0));
    view(&mut ui);
    ui.finish();
}

/// The labels of the contributed buttons, in tree order.
fn buttons(core: &mut Core) -> Vec<String> {
    core.access_tree()
        .nodes
        .iter()
        .filter_map(|n| n.name.clone())
        .filter(|n| n.contains(':'))
        .collect()
}

fn toolbar(ui: &mut Ui<'_>) -> usize {
    ui.slot(
        "app.toolbar",
        &Value::map([
            ("title", Value::str("Files")),
            ("onRun", Value::map([("kind", Value::str("run"))])),
        ]),
    )
}

#[test]
fn every_contribution_is_placed_in_load_order_then_order() {
    let mut exts = Extensions::new();
    exts.push_as(
        "a",
        Box::new(plug(
            &["save"],
            vec![Contribution::new("app.toolbar", "save")],
        )),
    )
    .unwrap();
    exts.push_as(
        "b",
        Box::new(plug(
            &["open", "close"],
            vec![
                Contribution::new("app.toolbar", "open").order(-1),
                Contribution::new("app.toolbar", "close").order(5),
            ],
        )),
    )
    .unwrap();
    exts.push_as(
        "c",
        Box::new(plug(&["x"], vec![Contribution::new("app.toolbar", "x")])),
    )
    .unwrap();
    let mut core = core();
    let mut placed = 0;
    frame(&mut core, &mut exts, |ui| placed = toolbar(ui));
    assert_eq!(placed, 4);
    assert_eq!(buttons(&mut core), ["b:open", "a:save", "c:x", "b:close"]);
    assert!(core.take_warnings().is_empty());
}

#[test]
fn a_contributor_gets_the_slots_props_and_knows_which_slot() {
    let p = plug(&["save"], vec![Contribution::new("app.toolbar", "save")]);
    let seen = p.seen.clone();
    let mut exts = Extensions::new();
    exts.push_as("a", Box::new(p)).unwrap();
    let mut core = core();
    frame(&mut core, &mut exts, |ui| {
        toolbar(ui);
    });
    let seen = seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, "save");
    assert_eq!(seen[0].1.as_deref(), Some("app.toolbar"));
    assert_eq!(seen[0].2.get_str("title"), Some("Files"));
}

#[test]
fn a_contributions_key_is_the_slots_its_providers_and_its_components() {
    let mut exts = Extensions::new();
    exts.push_as(
        "a",
        Box::new(plug(
            &["save"],
            vec![Contribution::new("app.toolbar", "save")],
        )),
    )
    .unwrap();
    let mut core = core();
    frame(&mut core, &mut exts, |ui| {
        toolbar(ui);
    });
    let slot = kui_core::Key::ROOT.str("app.toolbar");
    let want = slot.str("a").str("save").str("btn");
    let key = |core: &mut Core| {
        core.access_tree()
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("a:save"))
            .map(|n| n.key)
            .expect("a's button")
    };
    assert_eq!(key(&mut core), want);
    // A plugin loaded since, contributing ahead of it, moves nothing.
    exts.push_as(
        "b",
        Box::new(plug(
            &["open"],
            vec![Contribution::new("app.toolbar", "open").order(-1)],
        )),
    )
    .unwrap();
    frame(&mut core, &mut exts, |ui| {
        toolbar(ui);
    });
    assert_eq!(buttons(&mut core), ["b:open", "a:save"]);
    assert_eq!(
        key(&mut core),
        want,
        "the earlier contribution kept its key"
    );
}

#[test]
fn a_contributions_event_is_the_hosts_message_from_its_provider() {
    let mut exts = Extensions::new();
    exts.push_as(
        "a",
        Box::new(plug(
            &["save"],
            vec![Contribution::new("app.toolbar", "save")],
        )),
    )
    .unwrap();
    exts.push_as(
        "b",
        Box::new(plug(
            &["open"],
            vec![Contribution::new("app.toolbar", "open")],
        )),
    )
    .unwrap();
    let mut core = core();
    frame(&mut core, &mut exts, |ui| {
        toolbar(ui);
    });
    let at = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("b:open"))
        .map(|n| n.rect)
        .expect("b's button");
    let mut host = Vec::new();
    exts.route(click_at(&mut core, at.x + 4.0, at.y + 4.0), |ev| {
        host.push(ev)
    });
    assert_eq!(host.len(), 1, "{host:?}");
    assert_eq!(host[0].event, EventKind::Component);
    assert_eq!(host[0].payload.get_str("kind"), Some("run"));
    assert_eq!(
        host[0].payload.get("run").and_then(|r| r.get_str("c")),
        Some("open")
    );
    assert_eq!(host[0].from.as_deref(), Some("b"));
    // `data` reads a component event's fields from its prop's field.
    assert_eq!(host[0].data().and_then(|d| d.get_str("c")), Some("open"));
}

#[test]
fn each_contribution_is_replayed_on_its_own() {
    let a = plug(&["save"], vec![Contribution::new("app.toolbar", "save")]);
    let fills = a.fills.clone();
    let mut exts = Extensions::new();
    exts.push_as("a", Box::new(a)).unwrap();
    let mut core = core();
    for _ in 0..3 {
        frame(&mut core, &mut exts, |ui| {
            toolbar(ui);
        });
    }
    assert_eq!(fills.get(), 1, "the same props: replayed");
    assert_eq!(buttons(&mut core), ["a:save"]);
}

#[test]
fn an_empty_or_unplaced_slot_is_silent() {
    let mut exts = Extensions::new();
    exts.push_as(
        "a",
        Box::new(plug(
            &["save"],
            vec![Contribution::new("app.toolbar", "save")],
        )),
    )
    .unwrap();
    let mut core = core();
    let mut placed = usize::MAX;
    // `a` contributes to a slot this frame does not declare, and the one it
    // does declare has nobody.
    frame(&mut core, &mut exts, |ui| {
        placed = ui.slot("app.status", &Value::Null)
    });
    assert_eq!(placed, 0);
    assert!(core.take_warnings().is_empty());
    assert!(buttons(&mut core).is_empty());
}

#[test]
fn a_contribution_must_be_a_component_its_extension_provides() {
    let mut exts = Extensions::new();
    let err = exts
        .push_as(
            "a",
            Box::new(plug(
                &["save"],
                vec![Contribution::new("app.toolbar", "open")],
            )),
        )
        .unwrap_err();
    assert!(err.contains("does not provide"), "{err}");
    let err = exts
        .push_as(
            "b",
            Box::new(plug(&["save"], vec![Contribution::new("", "save")])),
        )
        .unwrap_err();
    assert!(err.contains("no name"), "{err}");
    // `*` provides any name.
    exts.push_as(
        "c",
        Box::new(plug(&["*"], vec![Contribution::new("app.toolbar", "any")])),
    )
    .unwrap();
}

/// A slot inside a kept fill is an op of its own: the fill around it is
/// replayed and its providers are asked anew, so a plugin loaded since
/// shows up without the fill running.
#[test]
fn a_slot_in_a_replayed_fill_asks_its_providers_again() {
    let shell = plug(&["frame"], Vec::new());
    let shell_fills = shell.fills.clone();
    let mut exts = Extensions::new();
    exts.push_as("shell", Box::new(shell)).unwrap();
    exts.push_as(
        "a",
        Box::new(plug(
            &["item"],
            vec![Contribution::new("shell.items", "item")],
        )),
    )
    .unwrap();
    let mut core = core();
    let mut answer = None;
    let mut place = |core: &mut Core, exts: &mut Extensions| {
        frame(core, exts, |ui| {
            answer = ui.component("shell/frame", &Value::Null)
        });
        answer
    };
    assert_eq!(place(&mut core, &mut exts), Some(FillAnswer::NotKept));
    assert_eq!(place(&mut core, &mut exts), Some(FillAnswer::Replayed));
    assert_eq!(buttons(&mut core), ["a:item"]);
    exts.push_as(
        "b",
        Box::new(plug(
            &["item"],
            vec![Contribution::new("shell.items", "item")],
        )),
    )
    .unwrap();
    assert_eq!(place(&mut core, &mut exts), Some(FillAnswer::Replayed));
    assert_eq!(shell_fills.get(), 1, "the shell was not asked");
    assert_eq!(
        buttons(&mut core),
        ["a:item", "b:item"],
        "the new plugin was"
    );
}
