//! A declared devtools tab (`docs/adr/0032-a-devtools-tab-mounts-a-slot.md`):
//! the tab body is a node in the panel, and the content is a layer
//! anchored to it — from the host's own view, lazily, or from an
//! extension's fill of the slot the panel declares. Both forms are driven
//! here through a bare core: the host form with `devtools_tab_with`, the
//! extension form with a stand-in extension in an `Extensions` list, the
//! way `tests/slots.rs` drives the runner's own filler.

use std::cell::RefCell;
use std::rc::Rc;

use kui_core::diag::{DUPLICATE_TAB, UNKNOWN_SLOT};
use kui_core::testing::{click_at, codes};
use kui_core::{
    Core, DevtoolsDock, Extension, Extensions, InputEvent, Key, KeyCode, KeyMods, KeyPress,
    NodeSpec, Sizing, Slot, TextStyle, Ui, UiEvent, Value, widgets,
};

const VIEWPORT: kui_core::Size = kui_core::Size {
    w: 1000.0,
    h: 600.0,
};

/// The app: a button, then the declared tab. `built` counts the closure's
/// runs, which is the laziness under test.
fn view(ui: &mut Ui<'_>, built: &Rc<RefCell<u32>>) {
    ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0)),
        |ui| {
            widgets::button(ui, "press", Value::map([("kind", Value::str("pressed"))]));
            ui.devtools_tab_with("syntax", "Tree-sitter", |ui| {
                *built.borrow_mut() += 1;
                ui.text("identifier 12:4", TextStyle::new(12.0));
                widgets::button(ui, "jump", Value::map([("kind", Value::str("jump"))]));
            });
        },
    );
}

fn frame(core: &mut Core, built: &Rc<RefCell<u32>>) {
    let mut ui = core.frame(VIEWPORT, 1.0);
    view(&mut ui, built);
    ui.finish();
}

fn chord(c: char) -> InputEvent {
    InputEvent::KeyDown(KeyPress::new(
        KeyCode::Char(c),
        KeyMods {
            ctrl: true,
            shift: true,
            ..Default::default()
        },
    ))
}

/// The rect of the node labelled `label`, from the inspect snapshot.
fn rect_of(core: &Core, label: &str) -> Option<kui_core::Rect> {
    core.nodes()
        .iter()
        .find(|n| n.label.as_deref() == Some(label))
        .map(|n| n.rect)
}

fn texts(core: &Core) -> Vec<String> {
    core.nodes().iter().filter_map(|n| n.text.clone()).collect()
}

/// The host form: declared every frame, built only while it is the tab
/// on show, laid out over the panel's tab body, the host's to hear, the
/// dock's to Tab through.
#[test]
fn a_host_form_tab_is_lazy_and_lands_over_the_body() {
    let built = Rc::new(RefCell::new(0u32));
    let mut core = Core::new();
    core.set_devtools(true);
    core.set_devtools_dock(DevtoolsDock::Right);
    core.set_inspect(true);
    frame(&mut core, &built);
    frame(&mut core, &built);
    assert_eq!(
        *built.borrow(),
        0,
        "declared, not built: the events tab is up"
    );
    let strip: Vec<String> = core
        .nodes()
        .iter()
        .filter_map(|n| n.label.clone())
        .filter(|l| l == "kui-devtools/tab-custom:syntax")
        .collect();
    assert_eq!(
        strip.len(),
        1,
        "the strip lists the declared tab by its label"
    );

    // N walks facts → events → tree → the declared tab.
    core.handle_input(chord('N')); // tree
    core.handle_input(chord('N')); // Tree-sitter
    frame(&mut core, &built);
    assert_eq!(
        *built.borrow(),
        1,
        "the closure ran once the tab was on show"
    );
    // Exactly one tab of the strip is selected, and it is the declared one.
    let lit: Vec<String> = core
        .access_tree()
        .nodes
        .iter()
        .filter(|n| n.role == kui_core::Role::Tab && n.selected == Some(true))
        .filter_map(|n| n.name.clone())
        .collect();
    assert_eq!(lit, ["Tree-sitter"], "one lit tab, the declared one");
    frame(&mut core, &built);
    assert_eq!(*built.borrow(), 2);
    let body = rect_of(&core, "kui-devtools/tab/syntax").expect("the body is in the panel");
    let content = rect_of(&core, "jump").expect("the host's button is in the frame");
    assert!(
        body.w > 100.0 && body.h > 100.0,
        "the body takes the tab area: {body:?}"
    );
    assert!(
        content.x >= body.x && content.x + content.w <= body.x + body.w,
        "the content is laid out inside the body: {content:?} in {body:?}"
    );
    assert!(
        content.y >= body.y && content.y + content.h <= body.y + body.h,
        "vertically too: {content:?} in {body:?}"
    );
    assert!(
        texts(&core).iter().any(|t| t == "identifier 12:4"),
        "the content's text painted"
    );

    // Its events are the host's: a click on the tab's button comes out.
    let evs = click_at(&mut core, content.x + 2.0, content.y + 2.0);
    assert_eq!(evs.len(), 1, "one event, the host's: {evs:?}");
    assert_eq!(
        evs[0].payload.get("kind").and_then(Value::as_str),
        Some("jump")
    );
    assert_eq!(evs[0].key, core.key_of("jump").unwrap());

    // The content is the dock's region, not the app's ring: Tab from the
    // app's button never reaches `jump`, and inside the dock it does.
    // (The click settled the region on the dock, since the content is
    // its; back to the main ring first.)
    assert_eq!(
        core.region(),
        core.key_of("kui-devtools"),
        "the click settled the dock's region"
    );
    core.focus_region(None);
    frame(&mut core, &built);
    core.set_focus(None);
    core.focus_next(true);
    let first = core.focus();
    core.focus_next(true);
    assert_eq!(core.focus(), first, "the app's ring is the one button");
    assert_ne!(first, core.key_of("jump"));
    core.set_focus(None);
    core.handle_input(chord('I'));
    frame(&mut core, &built);
    let dock = core.key_of("kui-devtools").unwrap();
    assert_eq!(core.region(), Some(dock));
    let mut seen = Vec::new();
    for _ in 0..40 {
        core.focus_next(true);
        seen.push(core.focus());
    }
    assert!(
        seen.contains(&core.key_of("jump")),
        "Tab inside the dock reaches the tab's button: {seen:?}"
    );

    // Another tab: the closure stops running, the content is gone.
    core.handle_input(chord('N')); // facts
    frame(&mut core, &built);
    let runs = *built.borrow();
    frame(&mut core, &built);
    assert_eq!(*built.borrow(), runs, "not shown, not built");
    assert!(rect_of(&core, "jump").is_none());

    // The panel off: declared still (cheap), never built.
    core.handle_input(chord('N'));
    core.handle_input(chord('N'));
    core.handle_input(chord('N'));
    frame(&mut core, &built);
    assert!(rect_of(&core, "jump").is_some(), "back on show");
    core.set_devtools(false);
    let runs = *built.borrow();
    frame(&mut core, &built);
    assert_eq!(*built.borrow(), runs, "off, nothing built");
    assert!(rect_of(&core, "jump").is_none());
}

/// A pick raised from a declared tab (`set_devtools_pick`) keeps that tab
/// up: the node under the pointer is `devtools_picked` while picking, the
/// press lands it in `devtools_selected`, and the tab — not the tree tab
/// — is what the panel shows after. Raised with no declared tab up, it is
/// the chord's pick and shows the tree.
#[test]
fn a_pick_raised_from_a_tab_lands_in_selected_and_keeps_the_tab() {
    let built = Rc::new(RefCell::new(0u32));
    let mut core = Core::new();
    core.set_devtools(true);
    core.set_devtools_dock(DevtoolsDock::Right);
    core.set_inspect(true);
    frame(&mut core, &built);
    frame(&mut core, &built);
    core.handle_input(chord('N'));
    core.handle_input(chord('N'));
    frame(&mut core, &built);
    assert!(
        core.key_of("kui-devtools/tab/syntax").is_some(),
        "the tab is up"
    );
    assert!(!core.devtools_picking());
    core.set_devtools_pick(true);
    assert!(core.devtools_picking());
    frame(&mut core, &built);
    assert!(
        core.key_of("kui-devtools/picker").is_some(),
        "the overlay is up"
    );
    assert!(
        core.key_of("kui-devtools/tab/syntax").is_some(),
        "and the declared tab stayed up"
    );
    let press = core.key_of("press").unwrap();
    let b = rect_of(&core, "press").unwrap();
    core.handle_input(InputEvent::CursorMoved(kui_core::Vec2::new(
        b.x + b.w / 2.0,
        b.y + b.h / 2.0,
    )));
    frame(&mut core, &built);
    assert_eq!(
        core.devtools_picked(),
        Some(press),
        "the node under the pointer"
    );
    let evs = click_at(&mut core, b.x + b.w / 2.0, b.y + b.h / 2.0);
    assert!(evs.is_empty(), "the press is the picker's: {evs:?}");
    assert!(!core.devtools_picking());
    assert_eq!(
        core.devtools_selected(),
        Some(press),
        "the pick landed in selected"
    );
    frame(&mut core, &built);
    assert!(
        core.key_of("kui-devtools/tab/syntax").is_some(),
        "the tab is still the one on show"
    );
    assert!(core.key_of("kui-devtools/picker").is_none());
    // Put away from outside, too.
    core.set_devtools_pick(true);
    core.set_devtools_pick(false);
    assert!(!core.devtools_picking());
    // With no declared tab up, the door is the chord's pick: the tree tab.
    core.handle_input(chord('N')); // facts
    frame(&mut core, &built);
    core.set_devtools_pick(true);
    frame(&mut core, &built);
    assert!(core.key_of("kui-devtools/tab/syntax").is_none());
    assert!(
        core.key_of("kui-devtools/tree").is_some() || core.key_of("kui-devtools/picker").is_some()
    );
}

/// A left dock is built before the host's view, so the body precedes the
/// content in tree order; the layer does not care which came first.
#[test]
fn a_left_dock_mounts_the_host_form_too() {
    let built = Rc::new(RefCell::new(0u32));
    let mut core = Core::new();
    core.set_devtools(true);
    core.set_devtools_dock(DevtoolsDock::Left);
    core.set_inspect(true);
    frame(&mut core, &built);
    frame(&mut core, &built);
    core.handle_input(chord('N'));
    core.handle_input(chord('N'));
    frame(&mut core, &built);
    frame(&mut core, &built);
    let body = rect_of(&core, "kui-devtools/tab/syntax").expect("the body");
    let content = rect_of(&core, "jump").expect("the content");
    assert!(body.x < 10.0, "a left dock: {body:?}");
    assert!(
        content.x >= body.x && content.x + content.w <= body.x + body.w,
        "{content:?} in {body:?}"
    );
}

/// A name declared twice in one frame is one tab and one warning; the
/// first declaration stands.
#[test]
fn a_tab_declared_twice_warns_and_keeps_the_first() {
    let mut core = Core::new();
    core.set_devtools(true);
    core.set_inspect(true);
    let build = |core: &mut Core| {
        let mut ui = core.frame(VIEWPORT, 1.0);
        ui.with(NodeSpec::column().width(Sizing::Grow(1.0)), |ui| {
            ui.devtools_tab_with("x", "First", |_| {});
            ui.devtools_tab("x", "Second", "ext/x");
            ui.devtools_tab("y", "Other", "ext/y");
        });
        ui.finish();
    };
    build(&mut core);
    build(&mut core);
    let ws = core.take_warnings();
    assert_eq!(codes(&ws), [DUPLICATE_TAB], "{ws:?}");
    let labels: Vec<String> = core
        .nodes()
        .iter()
        .filter_map(|n| n.label.clone())
        .collect();
    assert!(
        labels.iter().any(|l| l == "kui-devtools/tab-custom:x"),
        "{labels:?}"
    );
    assert!(labels.iter().any(|l| l == "kui-devtools/tab-custom:y"));
    assert_eq!(
        labels
            .iter()
            .filter(|l| l.starts_with("kui-devtools/tab-custom:"))
            .count(),
        2,
        "two tabs, not three"
    );
    let texts = texts(&core);
    assert!(texts.iter().any(|t| t == "First"), "{texts:?}");
    assert!(!texts.iter().any(|t| t == "Second"));
}

// ---------------------------------------------------------------------------
// The extension form.

#[derive(Default)]
struct Ext {
    seen: Rc<RefCell<Vec<(String, Value)>>>,
    heard: Rc<RefCell<Vec<Value>>>,
}

impl Extension for Ext {
    fn name(&self) -> &str {
        "ts"
    }
    fn slots(&self) -> &[String] {
        std::slice::from_ref(Box::leak(Box::new("panel".to_string())))
    }
    fn view(&mut self, slot: &Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        self.seen
            .borrow_mut()
            .push((slot.full_name(), slot.params.clone()));
        ui.text("from the plugin", TextStyle::new(12.0));
        widgets::button(ui, "plug", Value::map([("kind", Value::str("plug"))]));
        Ok(())
    }
    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
        self.heard.borrow_mut().push(ev.payload.clone());
        vec![Value::map([("kind", Value::str("reply"))])]
    }
}

fn host_view(ui: &mut Ui<'_>) {
    ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0)),
        |ui| {
            widgets::button(ui, "press", Value::map([("kind", Value::str("pressed"))]));
            ui.devtools_tab("syntax", "Tree-sitter", "ts/panel");
        },
    );
}

fn frame_with(core: &mut Core, exts: &mut Extensions) {
    let mut ui = core.frame_with(VIEWPORT, 1.0, exts);
    host_view(&mut ui);
    ui.finish();
}

/// The extension form: the slot is declared only while the tab is on
/// show — and raises no `unknown-slot` otherwise — the fill lands over
/// the body with the panel's facts as params, its events go to the
/// extension by origin, and its replies reach the host.
#[test]
fn an_extension_form_tab_is_a_slot_the_panel_declares_when_shown() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let heard = Rc::new(RefCell::new(Vec::new()));
    let mut exts = Extensions::try_from(vec![Box::new(Ext {
        seen: seen.clone(),
        heard: heard.clone(),
    }) as Box<dyn Extension>])
    .unwrap();
    let mut core = Core::new();
    core.set_devtools(true);
    core.set_devtools_dock(DevtoolsDock::Right);
    core.set_inspect(true);
    frame_with(&mut core, &mut exts);
    frame_with(&mut core, &mut exts);
    assert!(
        seen.borrow().is_empty(),
        "not on show: the extension is not asked"
    );
    let ws = core.take_warnings();
    assert!(
        !codes(&ws).contains(&UNKNOWN_SLOT),
        "a tab's slot counts as declared with the tab not on show: {ws:?}"
    );

    core.handle_input(chord('N'));
    core.handle_input(chord('N'));
    frame_with(&mut core, &mut exts);
    assert_eq!(seen.borrow().len(), 1, "asked once the tab is on show");
    let (name, params) = seen.borrow()[0].clone();
    assert_eq!(name, "ts/panel");
    assert!(
        params.get("selected").is_some(),
        "the panel's facts: {params:?}"
    );
    frame_with(&mut core, &mut exts);
    let body = rect_of(&core, "kui-devtools/tab/syntax").expect("the body");
    let content = rect_of(&core, "plug").expect("the fill's button");
    assert!(
        content.x >= body.x && content.x + content.w <= body.x + body.w,
        "{content:?} in {body:?}"
    );
    assert!(texts(&core).iter().any(|t| t == "from the plugin"));

    // The fill's click is the extension's, by origin; its reply is the
    // host's, because the panel declared the slot for the host.
    let evs = click_at(&mut core, content.x + 2.0, content.y + 2.0);
    assert_eq!(evs.len(), 1);
    let mut out = Vec::new();
    exts.route(evs, |ev| out.push(ev));
    assert_eq!(heard.borrow().len(), 1, "the extension heard its click");
    assert_eq!(out.len(), 1, "one reply for the host: {out:?}");
    assert_eq!(
        out[0].payload.get("kind").and_then(Value::as_str),
        Some("reply")
    );
    let _ = Key::ROOT;
}
