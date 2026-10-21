//! The documents the OS asks the app to open (backlog F124):
//! `InputEvent::Open`, which no ask precedes. The host hears it as the main
//! window's root's `windowEvents` `on_open` message (ADR 0048), under a
//! modal alike; documents that come before the first frame are held for
//! it; an empty list is nothing, and a window other than the main one
//! hears none.

use kui_core::{
    Core, FrameCause, InputEvent, Key, KeyMods, NodeSpec, OriginId, Size, Value, Vec2,
    WindowEvents, WindowId,
};

fn paths() -> Vec<String> {
    vec!["/tmp/a.txt".to_string(), "/tmp/b c.md".to_string()]
}

/// A frame whose root declares the message the documents arrive as.
fn declare(core: &mut Core) {
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.window_events(WindowEvents::default().on_open("docs"));
    ui.finish();
}

#[test]
fn the_documents_reach_the_host_on_the_root_with_no_ask() {
    let mut core = Core::new();
    // Before the first frame no root has declared a message for them, so
    // nothing arrives yet — and nothing is lost: a launcher hands the
    // documents over before the first frame, and they are held for the
    // first one to declare `onOpen`.
    assert!(core.handle_input(InputEvent::Open(paths())).is_empty());
    declare(&mut core);
    let held = core.take_pending_events();
    assert_eq!(held.len(), 1, "the launch's documents, held: {held:?}");
    assert_eq!(held[0].payload.get_str("tag"), Some("docs"));

    let evs = core.handle_input(InputEvent::Open(paths()));
    assert_eq!(evs.len(), 1);
    let ev = &evs[0];
    assert_eq!(ev.origin, OriginId::HOST);
    assert_eq!(ev.key, Key::ROOT);
    assert_eq!(ev.window, WindowId::MAIN);
    assert_eq!(ev.event.name(), "open");
    let listed: Vec<&str> = ev
        .data()
        .and_then(|d| d.get("paths"))
        .and_then(Value::as_list)
        .expect("a list of paths")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(listed, ["/tmp/a.txt", "/tmp/b c.md"]);
    // Nothing else rides along: the payload is the declared message and
    // the `open` field.
    assert_eq!(ev.payload.get_str("tag"), Some("docs"));
    assert!(matches!(&ev.payload, Value::Map(m) if m.len() == 2));
}

#[test]
fn no_documents_is_no_event() {
    let mut core = Core::new();
    declare(&mut core);
    assert!(core.handle_input(InputEvent::Open(Vec::new())).is_empty());
}

#[test]
fn a_modal_does_not_hold_the_documents_back() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.window_events(WindowEvents::default().on_open("docs"));
    ui.with(NodeSpec::column(), |ui| {
        ui.leaf_keyed(
            "dialog",
            NodeSpec::column().size(200.0, 100.0).modal("close"),
        );
    });
    ui.finish();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(300.0, 250.0)));
    let evs = core.handle_input(InputEvent::Open(paths()));
    assert_eq!(
        evs.iter().filter(|e| e.event.name() == "open").count(),
        1,
        "the OS's request is the app's, whatever is modal"
    );
}

/// The OS opens documents to the app, not to one of its windows, so
/// `on_open` is read on the main window's root alone: a second window that
/// declares it hears nothing, or the app would have to deduplicate the
/// documents. What that window does hear of its own is stamped with its id.
#[test]
fn a_window_other_than_the_main_one_hears_no_documents() {
    let mut core = Core::new();
    core.env.window.id = WindowId(7);
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.window_events(WindowEvents::default().on_open("docs").on_modifiers("mods"));
    ui.finish();
    assert!(core.handle_input(InputEvent::Open(paths())).is_empty());
    let evs = core.handle_input(InputEvent::Modifiers(KeyMods::NONE.with_shift()));
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].event.name(), "modifiers");
    assert_eq!(evs[0].window, WindowId(7));
}

#[test]
fn the_frame_it_wakes_says_files() {
    let mut core = Core::new();
    let ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.finish();
    core.handle_input(InputEvent::Open(paths()));
    let ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.finish();
    assert!(core.frame_cause().contains(FrameCause::FILES));
    assert_eq!(
        FrameCause::of_input(&InputEvent::Open(paths())),
        FrameCause::FILES
    );
}
