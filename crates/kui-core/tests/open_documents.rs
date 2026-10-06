//! The documents the OS asks the app to open (backlog F124):
//! `InputEvent::Open`, which no ask precedes. The host hears it on the
//! root, before any frame and under a modal alike, stamped with the
//! window it was handed to; an empty list is nothing.

use kui_core::{
    Core, FrameCause, InputEvent, Key, NodeSpec, OriginId, Size, Value, Vec2, WindowId,
};

fn paths() -> Vec<String> {
    vec!["/tmp/a.txt".to_string(), "/tmp/b c.md".to_string()]
}

#[test]
fn the_documents_reach_the_host_on_the_root_with_no_ask() {
    let mut core = Core::new();
    // Before the first frame: the launch-time open, which the runner holds
    // until the window exists, but a host driving the core itself may not.
    let evs = core.handle_input(InputEvent::Open(paths()));
    assert_eq!(evs.len(), 1);
    let ev = &evs[0];
    assert_eq!(ev.origin, OriginId::HOST);
    assert_eq!(ev.key, Key::ROOT);
    assert_eq!(ev.window, WindowId::MAIN);
    assert_eq!(ev.kind(), Some("open"));
    let listed: Vec<&str> = ev
        .payload
        .get("paths")
        .and_then(Value::as_list)
        .expect("a list of paths")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(listed, ["/tmp/a.txt", "/tmp/b c.md"]);
    // Nothing else rides along: the payload is the two fields.
    assert_eq!(ev.payload.get("tag"), None);
}

#[test]
fn no_documents_is_no_event() {
    let mut core = Core::new();
    assert!(core.handle_input(InputEvent::Open(Vec::new())).is_empty());
}

#[test]
fn a_modal_does_not_hold_the_documents_back() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
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
        evs.iter().filter(|e| e.kind() == Some("open")).count(),
        1,
        "the OS's request is the app's, whatever is modal"
    );
}

#[test]
fn a_window_other_than_the_main_one_stamps_its_own_id() {
    let mut core = Core::new();
    core.env.window.id = WindowId(7);
    let evs = core.handle_input(InputEvent::Open(paths()));
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
