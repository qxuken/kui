//! Components an extension provides (ADR 0060): placed by position as
//! often as a host likes, holding the host's own children where the fill
//! asks, raising events that reach the host as its own messages, and
//! replayed by default while nothing they read has moved.

use std::cell::Cell;
use std::rc::Rc;

use kui_core::slot::emit;
use kui_core::testing::click_at;
use kui_core::{
    Color, Core, EventKind, Extension, Extensions, NodeSpec, OriginId, Size, Instance, FillAnswer,
    TextStyle, Ui, UiEvent, Value,
};

const VIEW: Size = Size { w: 400.0, h: 300.0 };

/// Provides `card` (a titled box, the host's children inside it and its
/// `footer` group under them; `wrap` nests the children one box deeper)
/// and `button` (a box whose click raises the host's `onClick`, with the
/// count of clicks so far).
#[derive(Default)]
struct Kit {
    fills: Rc<Cell<usize>>,
    clicks: Rc<Cell<i64>>,
    wrap: Rc<Cell<bool>>,
    /// Ask for the unnamed group twice.
    twice: bool,
}

static COMPONENTS: std::sync::LazyLock<Vec<String>> =
    std::sync::LazyLock::new(|| vec!["card".into(), "button".into(), "pair".into()]);

impl Extension for Kit {
    fn name(&self) -> &str {
        "kit"
    }
    fn components(&self) -> &[String] {
        &COMPONENTS
    }
    fn view(&mut self, _slot: &Instance<'_>, _ui: &mut Ui<'_>) -> Result<(), String> {
        Ok(())
    }
    fn component(&mut self, inst: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        self.fills.set(self.fills.get() + 1);
        match inst.name {
            "card" => {
                let title = inst.params.get_str("title").unwrap_or("card").to_string();
                ui.with_keyed("card", NodeSpec::column().pad(4.0).gap(2.0), |ui| {
                    ui.text(&title, TextStyle::new(12.0));
                    if self.wrap.get() {
                        ui.with(NodeSpec::column(), |ui| ui.children(""));
                    } else {
                        ui.children("");
                    }
                    if self.twice {
                        ui.children("");
                    }
                    ui.children("footer");
                });
            }
            "button" => {
                // The count is the extension's own state: what a replay
                // would miss after a click.
                let clicks = self.clicks.get();
                ui.with(
                    NodeSpec::row()
                        .size(60.0, 20.0)
                        .bg(Color::hex(0x3355ffff))
                        .on_click(Value::str("pressed"))
                        .label("Kit button"),
                    |ui| ui.text(&format!("clicks {clicks}"), TextStyle::new(10.0)),
                );
            }
            // A component that places another, forwarding its own host's
            // children into it.
            "pair" => {
                // Its own component: the list cannot ask `kit` while `kit`
                // draws, so the fill is this one's.
                let wrap = self.wrap.clone();
                ui.component_by(
                    "kit/card",
                    &Value::map([("title", Value::str("inner"))]),
                    |group, ui| {
                        if group.is_empty() {
                            ui.children("");
                        }
                    },
                    |inst, ui| {
                        let title = inst.params.get_str("title").unwrap_or("").to_string();
                        let _ = &wrap;
                        ui.with(NodeSpec::column(), |ui| {
                            ui.text(&title, TextStyle::new(12.0));
                            ui.children("");
                        });
                    },
                );
            }
            other => return Err(format!("no component {other}")),
        }
        Ok(())
    }
    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
        if ev.payload.get_str("tag") == Some("pressed") {
            self.clicks.set(self.clicks.get() + 1);
            return vec![emit(
                "onClick",
                Value::map([("count", Value::Int(self.clicks.get()))]),
            )];
        }
        Vec::new()
    }
}


struct Rig {
    core: Core,
    exts: Extensions,
    fills: Rc<Cell<usize>>,
    wrap: Rc<Cell<bool>>,
}

fn rig(kit: Kit) -> Rig {
    let fills = kit.fills.clone();
    let wrap = kit.wrap.clone();
    let mut exts = Extensions::new();
    exts.push_as("kit", Box::new(kit)).unwrap();
    let mut core = Core::new();
    core.set_inspect(true);
    Rig {
        core,
        exts,
        fills,
        wrap,
    }
}

impl Rig {
    fn frame(&mut self, view: impl FnOnce(&mut Ui<'_>)) {
        let mut ui = self.core.frame_with(VIEW, 1.0, &mut self.exts);
        ui.configure_root(NodeSpec::column().pad(8.0).gap(4.0));
        view(&mut ui);
        ui.finish();
    }

    fn key_of(&mut self, label: &str) -> kui_core::Key {
        self.core
            .key_of(label)
            .unwrap_or_else(|| panic!("no node {label}"))
    }

    fn origin_of(&mut self, label: &str) -> OriginId {
        let key = self.key_of(label);
        self.core
            .nodes()
            .iter()
            .find(|n| n.key == key)
            .map(|n| n.origin)
            .expect("a node")
    }

    /// Clicks at a point and routes what comes back, answering with what
    /// reached the host.
    fn click(&mut self, x: f32, y: f32) -> Vec<UiEvent> {
        let events = click_at(&mut self.core, x, y);
        let mut host = Vec::new();
        self.exts.route(events, |ev| host.push(ev));
        host
    }
}

fn card(ui: &mut Ui<'_>, title: &str, body: &str) -> Option<FillAnswer> {
    let body = body.to_string();
    ui.component_with(
        "kit/card",
        &Value::map([("title", Value::str(title)), ("key", Value::str(title))]),
        move |group, ui| match group {
            "" => {
                ui.text_in_keyed(
                    &format!("{body}-text"),
                    NodeSpec::row().on_click(Value::str(body.as_str())),
                    &body,
                    TextStyle::new(12.0),
                );
            }
            "footer" => ui.text("footer", TextStyle::new(10.0)),
            _ => {}
        },
    )
}

#[test]
fn instances_repeat_and_hold_the_hosts_children_under_the_hosts_origin() {
    let mut r = rig(Kit::default());
    r.frame(|ui| {
        card(ui, "one", "alpha");
        card(ui, "two", "beta");
    });
    assert_eq!(r.fills.get(), 2);
    assert_eq!(r.origin_of("alpha-text"), OriginId::HOST);
    assert_eq!(r.origin_of("beta-text"), OriginId::HOST);
    let title = r
        .core
        .nodes()
        .iter()
        .find(|n| n.text.as_deref() == Some("one"))
        .map(|n| n.origin)
        .expect("the card's title");
    assert_ne!(
        title,
        OriginId::HOST,
        "the card's own nodes are the extension's"
    );
    let texts: Vec<String> = r
        .core
        .nodes()
        .iter()
        .filter_map(|n| n.text.clone())
        .collect();
    assert_eq!(texts.iter().filter(|t| *t == "footer").count(), 2);
    let codes: Vec<&str> = r.core.take_warnings().iter().map(|w| w.code).collect();
    assert!(!codes.contains(&"children-misplaced"), "{codes:?}");
}

#[test]
fn the_hosts_children_are_keyed_by_the_instance_not_by_the_fills_nesting() {
    let mut r = rig(Kit::default());
    r.frame(|ui| {
        card(ui, "one", "alpha");
    });
    let flat = r.key_of("alpha-text");
    r.wrap.set(true);
    r.frame(|ui| {
        card(ui, "one", "alpha");
    });
    assert_eq!(r.key_of("alpha-text"), flat);
}

#[test]
fn a_click_on_the_hosts_children_is_the_hosts_and_names_no_slot() {
    let mut r = rig(Kit::default());
    r.frame(|ui| {
        card(ui, "one", "alpha");
    });
    let at = r
        .core
        .nodes()
        .iter()
        .find(|n| n.text.as_deref() == Some("alpha"))
        .map(|n| n.rect)
        .expect("the host's text");
    let host = r.click(at.x + 2.0, at.y + 2.0);
    assert_eq!(host.len(), 1, "{host:?}");
    assert_eq!(host[0].origin, OriginId::HOST);
    assert_eq!(host[0].event, EventKind::Click);
    assert_eq!(host[0].instance, None);
    assert_eq!(host[0].payload.get_str("tag"), Some("alpha"));
}

#[test]
fn an_emit_reaches_the_host_as_its_own_message() {
    let mut r = rig(Kit::default());
    let props = Value::map([("onClick", Value::map([("save", Value::Bool(true))]))]);
    r.frame(|ui| {
        ui.component("kit/button", &props);
    });
    // The button sits at the root's padding.
    let host = r.click(12.0, 12.0);
    assert_eq!(host.len(), 1, "{host:?}");
    let ev = &host[0];
    assert_eq!(ev.event, EventKind::Component);
    assert_eq!(ev.origin, OriginId::HOST);
    assert_eq!(ev.payload.get("save"), Some(&Value::Bool(true)));
    assert_eq!(
        ev.payload.get("click").and_then(|c| c.get("count")),
        Some(&Value::Int(1))
    );
    // A host that passed no `onClick` hears nothing.
    r.frame(|ui| {
        ui.component("kit/button", &Value::map([("label", Value::str("x"))]));
    });
    assert_eq!(r.click(12.0, 12.0), Vec::new());
}

#[test]
fn an_unchanged_instance_is_replayed_and_still_builds_the_hosts_children() {
    let mut r = rig(Kit::default());
    let built = Rc::new(Cell::new(0));
    let view = |ui: &mut Ui<'_>, built: &Rc<Cell<usize>>, title: &str| {
        let built = built.clone();
        ui.component_with(
            "kit/card",
            &Value::map([("title", Value::str(title))]),
            move |group, ui| {
                if group.is_empty() {
                    built.set(built.get() + 1);
                    ui.text_in_keyed("body", NodeSpec::row(), "body", TextStyle::new(12.0));
                }
            },
        )
    };
    let mut answers = Vec::new();
    for title in ["a", "a", "a", "b"] {
        let b = built.clone();
        r.frame(|ui| answers.push(view(ui, &b, title)));
    }
    assert_eq!(
        answers,
        [
            Some(FillAnswer::NotKept),
            Some(FillAnswer::Replayed),
            Some(FillAnswer::Replayed),
            Some(FillAnswer::Params),
        ]
    );
    assert_eq!(
        r.fills.get(),
        2,
        "filled on the first frame and when the props moved"
    );
    assert_eq!(built.get(), 4, "the host's children every frame");
    assert_eq!(r.origin_of("body"), OriginId::HOST);
}

#[test]
fn fresh_asks_the_extension_every_frame() {
    let mut r = rig(Kit::default());
    let props = Value::map([("fresh", Value::Bool(true))]);
    for _ in 0..3 {
        r.frame(|ui| {
            ui.component("kit/button", &props);
        });
    }
    assert_eq!(r.fills.get(), 3);
}

#[test]
fn a_component_forwards_its_hosts_children_into_one_it_places() {
    let mut r = rig(Kit::default());
    r.frame(|ui| {
        ui.component_with("kit/pair", &Value::Null, |group, ui| {
            if group.is_empty() {
                ui.text_in_keyed("deep", NodeSpec::row(), "deep", TextStyle::new(12.0));
            }
        });
    });
    assert_eq!(r.origin_of("deep"), OriginId::HOST);
    let texts: Vec<String> = r
        .core
        .nodes()
        .iter()
        .filter_map(|n| n.text.clone())
        .collect();
    assert!(texts.contains(&"inner".to_string()), "{texts:?}");
    let codes: Vec<&str> = r.core.take_warnings().iter().map(|w| w.code).collect();
    assert_eq!(codes, Vec::<&str>::new(), "no warning");
}

#[test]
fn children_are_refused_twice_and_an_unknown_component_says_so() {
    let mut r = rig(Kit {
        twice: true,
        ..Kit::default()
    });
    r.frame(|ui| {
        card(ui, "one", "alpha");
        ui.component("kit/nothing", &Value::Null);
        ui.component("other/thing", &Value::Null);
    });
    let codes: Vec<&str> = r.core.take_warnings().iter().map(|w| w.code).collect();
    assert_eq!(
        codes.iter().filter(|c| **c == "children-misplaced").count(),
        1,
        "{codes:?}"
    );
    assert_eq!(
        codes.iter().filter(|c| **c == "unknown-component").count(),
        2,
        "{codes:?}"
    );
}

/// An app whose view is its extension's `root`, as a runner app's is:
/// it places `kit/button`, so the button's instance is placed by a root
/// that `Extensions::finish` draws, not by the host's view.
struct App;

static NO_COMPONENTS: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(Vec::new);

impl Extension for App {
    fn name(&self) -> &str {
        "app"
    }
    fn components(&self) -> &[String] {
        &NO_COMPONENTS
    }
    fn view(&mut self, _slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        ui.component("kit/button", &Value::map([("onClick", Value::str("saved"))]));
        Ok(())
    }
    fn component(&mut self, _inst: &Instance<'_>, _ui: &mut Ui<'_>) -> Result<(), String> {
        Ok(())
    }
    fn on_event(&mut self, _ev: &UiEvent) -> Vec<Value> {
        Vec::new()
    }
}

#[test]
fn an_instance_a_root_placed_that_heard_its_event_is_asked_again() {
    // The heard set was pruned at the top of `Extensions::finish`, before
    // the roots drew: the button a root placed had no record of this
    // frame yet, its event was forgotten, and it replayed what it showed
    // before — a runner app's select whose menu a press outside never
    // closed.
    let kit = Kit::default();
    let fills = kit.fills.clone();
    let mut exts = Extensions::new();
    exts.push_as("kit", Box::new(kit)).unwrap();
    exts.push_as("app", Box::new(App)).unwrap();
    let mut core = Core::new();
    core.set_inspect(true);
    let frame = |core: &mut Core, exts: &mut Extensions| {
        let mut ui = core.frame_with(VIEW, 1.0, exts);
        ui.configure_root(NodeSpec::column().pad(8.0).gap(4.0));
        ui.finish();
    };
    frame(&mut core, &mut exts);
    frame(&mut core, &mut exts);
    assert_eq!(fills.get(), 1, "filled once, then replayed");
    let rect = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Kit button"))
        .expect("the button")
        .rect;
    let events = click_at(&mut core, rect.x + 4.0, rect.y + 4.0);
    exts.route(events, |_| {});
    frame(&mut core, &mut exts);
    assert_eq!(fills.get(), 2, "the click was heard: asked again");
    let texts: Vec<String> = core.nodes().iter().filter_map(|n| n.text.clone()).collect();
    assert!(texts.contains(&"clicks 1".to_string()), "{texts:?}");
}

#[test]
fn an_instance_that_heard_its_own_event_is_asked_again() {
    let mut r = rig(Kit::default());
    let props = Value::map([("onClick", Value::str("saved"))]);
    let texts = |r: &mut Rig| -> Vec<String> {
        r.core.nodes().iter().filter_map(|n| n.text.clone()).collect()
    };
    let mut answers = Vec::new();
    for _ in 0..2 {
        r.frame(|ui| answers.push(ui.component("kit/button", &props)));
    }
    assert_eq!(answers, [Some(FillAnswer::NotKept), Some(FillAnswer::Replayed)]);
    assert_eq!(r.click(12.0, 12.0).len(), 1);
    answers.clear();
    for _ in 0..2 {
        r.frame(|ui| answers.push(ui.component("kit/button", &props)));
    }
    assert_eq!(
        answers,
        [Some(FillAnswer::Heard), Some(FillAnswer::Replayed)],
        "the click moved the extension's count, so the props alone do not vouch for it"
    );
    assert!(texts(&mut r).contains(&"clicks 1".to_string()), "{:?}", texts(&mut r));
}
