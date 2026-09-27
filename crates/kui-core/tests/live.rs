//! Live regions and one-off announcements
//! (`docs/adr/0008-live-regions-and-announcements.md`): the `live` row on
//! a node whose text changes, and the queue for a message with no node
//! behind it.

use kui_core::diag::{ANNOUNCEMENT_REPEATED, LIVE_REGION_WITHOUT_NAME};
use kui_core::testing::{click_at, codes};
use kui_core::{Core, Live, NodeSpec, Role, Size, TextStyle};

/// A column root holding one `live` box with `text` inside it.
fn region(core: &mut Core, live: Live, text: &str) {
    let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("status", NodeSpec::column().live(live), |ui| {
        if !text.is_empty() {
            ui.text(text, TextStyle::new(12.0));
        }
    });
    ui.finish();
}

/// A live region reads as one message: the box survives elision, takes the
/// text inside it as its name, and that text is not a node of its own.
/// One named node is one announcement — see the module doc for what macOS
/// does with the other shape.
#[test]
fn a_live_box_survives_elision_and_is_named_by_its_message() {
    let mut core = Core::new();
    region(&mut core, Live::Polite, "3 results");
    let rows: Vec<_> = core
        .access_tree()
        .nodes
        .iter()
        .map(|n| (n.role, n.live, n.name.clone()))
        .collect();
    assert_eq!(
        rows,
        vec![
            (Role::Window, Live::Off, None),
            // A plain box: without `live` it would not be here at all,
            // and the text inside is read as part of it.
            (Role::Group, Live::Polite, Some("3 results".to_string())),
        ]
    );
}

/// The name is what moves when the message does, which is the whole
/// mechanism: every platform derives its live-region event from a named
/// node whose name changed.
#[test]
fn changing_the_message_changes_the_regions_name() {
    let mut core = Core::new();
    region(&mut core, Live::Polite, "0 results");
    assert_eq!(
        core.access_tree().nodes[1].name.as_deref(),
        Some("0 results")
    );
    region(&mut core, Live::Polite, "3 results");
    assert_eq!(
        core.access_tree().nodes[1].name.as_deref(),
        Some("3 results")
    );
}

/// The same box without the row is structure, and leaves no trace — which
/// is what decision 2 is there to prevent.
#[test]
fn the_same_box_without_the_row_is_elided() {
    let mut core = Core::new();
    region(&mut core, Live::Off, "3 results");
    let roles: Vec<_> = core.access_tree().nodes.iter().map(|n| n.role).collect();
    assert_eq!(roles, vec![Role::Window, Role::StaticText]);
}

/// Liveness changes the tree hash, so a driver that sent the tree once
/// sends it again when a region becomes live.
#[test]
fn liveness_moves_the_tree_hash() {
    let mut core = Core::new();
    region(&mut core, Live::Off, "3 results");
    let off = core.access_tree().hash;
    region(&mut core, Live::Polite, "3 results");
    let polite = core.access_tree().hash;
    region(&mut core, Live::Assertive, "3 results");
    assert_ne!(off, polite);
    assert_ne!(polite, core.access_tree().hash);
}

#[test]
fn a_live_region_with_nothing_to_say_is_reported() {
    let mut core = Core::new();
    core.set_diagnostics(true);
    region(&mut core, Live::Polite, "");
    assert_eq!(codes(&core.take_warnings()), [LIVE_REGION_WITHOUT_NAME]);
    // Once per node, however many frames repeat it.
    region(&mut core, Live::Polite, "");
    assert!(core.take_warnings().is_empty());
}

/// A `label` is the other way to name one — text inside is not required,
/// only a name of some kind.
#[test]
fn a_labelled_live_region_is_not_reported() {
    let mut core = Core::new();
    core.set_diagnostics(true);
    let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed(
        "status",
        NodeSpec::column().live(Live::Polite).label("Saved"),
    );
    ui.finish();
    assert!(core.take_warnings().is_empty());
    assert_eq!(core.access_tree().nodes[1].name.as_deref(), Some("Saved"));
}

#[test]
fn announcements_queue_and_drain_once() {
    let mut core = Core::new();
    core.announce("Saved", Live::Polite);
    core.announce("3 results", Live::Assertive);
    let drained = core.take_announcements();
    assert_eq!(
        drained
            .iter()
            .map(|a| (a.text.as_str(), a.live))
            .collect::<Vec<_>>(),
        [("Saved", Live::Polite), ("3 results", Live::Assertive)]
    );
    assert!(core.take_announcements().is_empty());
}

/// Both no-ops of decision 4: `off` so a caller can gate politeness
/// without an `if`, empty because no platform can say nothing.
#[test]
fn off_and_empty_queue_nothing() {
    let mut core = Core::new();
    core.announce("Saved", Live::Off);
    core.announce("", Live::Assertive);
    assert!(core.take_announcements().is_empty());
}

#[test]
fn the_same_text_on_consecutive_frames_is_reported() {
    let mut core = Core::new();
    core.set_diagnostics(true);
    for _ in 0..3 {
        let ui = core.frame(Size::new(200.0, 100.0), 1.0);
        ui.finish();
        core.announce("Saved", Live::Polite);
    }
    // Once for the text, not once per frame — and the announcements
    // themselves all went through.
    assert_eq!(codes(&core.take_warnings()), [ANNOUNCEMENT_REPEATED]);
    assert_eq!(core.take_announcements().len(), 3);
}

/// The same message after the user did something is not the unguarded
/// case, and is not reported.
#[test]
fn the_same_text_frames_apart_is_not_reported() {
    let mut core = Core::new();
    core.set_diagnostics(true);
    core.announce("Saved", Live::Polite);
    for _ in 0..3 {
        let ui = core.frame(Size::new(200.0, 100.0), 1.0);
        ui.finish();
    }
    core.announce("Saved", Live::Polite);
    assert!(core.take_warnings().is_empty());
}

/// Two presses of Copy in a row, in a window that redraws only on input,
/// are two consecutive frames — and are not the unguarded case, because
/// the app was handed an event between them. The macOS audit
/// (`scripts/ax-audit.swift`, "the same message twice in a row is said
/// twice") is exactly this, and it raised the warning before the rule read
/// the events.
#[test]
fn the_same_text_after_a_press_is_not_reported() {
    let mut core = Core::new();
    core.set_diagnostics(true);
    let build = |core: &mut Core| {
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.leaf_keyed(
            "copy",
            NodeSpec::row()
                .width(kui_core::Sizing::Fixed(80.0))
                .height(kui_core::Sizing::Fixed(30.0))
                .on_click(kui_core::Value::str("copy"))
                .label("Copy"),
        );
        ui.finish();
    };
    build(&mut core);
    for _ in 0..2 {
        // The press is the handler's cue, and the announcement follows
        // it, then the frame it caused.
        assert_eq!(click_at(&mut core, 10.0, 10.0).len(), 1);
        core.announce("Copied to clipboard", Live::Polite);
        build(&mut core);
    }
    assert!(core.take_warnings().is_empty());
    assert_eq!(core.take_announcements().len(), 2);
    // And a view announcing on the next frame with nothing pressed is
    // still the reported shape.
    core.announce("Copied to clipboard", Live::Polite);
    assert_eq!(codes(&core.take_warnings()), [ANNOUNCEMENT_REPEATED]);
}

/// `Ui::announce` reaches the same queue, which is the only place three of
/// the four bindings can call it from.
#[test]
fn a_view_can_announce() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
    ui.announce("Saved", Live::Polite);
    ui.finish();
    assert_eq!(core.take_announcements().len(), 1);
}
