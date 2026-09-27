//! The secondary mouse button and `on_context_menu`: a right-click asks
//! the node under it for a menu — or the nearest enclosing node that
//! offers one, the way an unclaimed key reaches the enclosing sink
//! (backlog T1) — and changes nothing else: not focus, not the caret or
//! its selection, not a scrollbar — and never turns into a click. The
//! menu itself is an ordinary modal float the app declares, so this only
//! covers the routing that makes one buildable.

use kui_core::testing::{click_at, kinds};
use kui_core::{
    Core, EditOptions, InputEvent, Key, MouseButton, NodeSpec, Size, TextStyle, UiEvent, Vec2,
};

const W: f32 = 200.0;
const H: f32 = 200.0;

fn secondary_down() -> InputEvent {
    InputEvent::MouseDown {
        button: MouseButton::Secondary,
        clicks: 1,
    }
}

fn secondary_up() -> InputEvent {
    InputEvent::MouseUp {
        button: MouseButton::Secondary,
    }
}

/// Right-clicks at `(x, y)`: move, press, release.
fn right_click(core: &mut Core, x: f32, y: f32) -> Vec<UiEvent> {
    let mut out = core.handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
    out.extend(core.handle_input(secondary_down()));
    out.extend(core.handle_input(secondary_up()));
    out
}

struct Keys {
    panel: Key,
    row: Key,
    edit: Key,
}

/// A panel asking for a context menu, holding a clickable row that asks
/// for its own, a disabled row that asks for one too, and an editor. Rows
/// are 40px tall, stacked from y=0.
fn frame(core: &mut Core) -> Keys {
    let mut ui = core.frame(Size::new(W, H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let row = || NodeSpec::row().size(100.0, 40.0);
    let mut keys = Keys {
        panel: Key::ROOT,
        row: Key::ROOT,
        edit: Key::ROOT,
    };
    keys.panel = ui.with_keyed(
        "panel",
        NodeSpec::column().fill().gap(0.0).on_context_menu("panel"),
        |ui| {
            keys.row = ui.leaf_keyed("row", row().on_click("open").on_context_menu("row"));
            ui.leaf_keyed("off", row().disabled(true).on_context_menu("off"));
            keys.edit = ui.text_edit(
                "note",
                "hello world",
                &EditOptions {
                    style: TextStyle::new(13.0),
                    ..Default::default()
                },
                row(),
            );
        },
    );
    ui.finish();
    keys
}

#[test]
fn a_secondary_press_emits_the_menu_where_it_landed() {
    let mut core = Core::new();
    let k = frame(&mut core);
    let evs = right_click(&mut core, 30.0, 20.0);
    assert_eq!(kinds(&evs), ["contextmenu"]);
    assert_eq!(evs[0].key, k.row, "the topmost node is the one asked");
    assert_eq!(evs[0].payload.get("tag").unwrap().as_str(), Some("row"));
    assert_eq!(evs[0].payload.get("x").unwrap().as_float(), Some(30.0));
    assert_eq!(evs[0].payload.get("y").unwrap().as_float(), Some(20.0));
}

/// The container answers where no child covers it — and, since T1, where
/// the child covering it offers nothing of its own (the tests below).
#[test]
fn the_panel_answers_where_no_child_does() {
    let mut core = Core::new();
    let k = frame(&mut core);
    let evs = right_click(&mut core, 30.0, 180.0);
    assert_eq!(kinds(&evs), ["contextmenu"]);
    assert_eq!(evs[0].key, k.panel);
    assert_eq!(evs[0].payload.get("tag").unwrap().as_str(), Some("panel"));
}

/// The row carries both: the menu on the press, the click on the release
/// of the *other* button, and neither leaks into the other.
#[test]
fn a_secondary_press_is_never_a_click() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(kinds(&right_click(&mut core, 30.0, 20.0)), ["contextmenu"]);
    assert_eq!(kinds(&click_at(&mut core, 30.0, 20.0)), ["open"]);
}

/// A disabled node's own menu is stripped like its click, and the press
/// goes on to the enclosing one — as a disabled sink is skipped and the
/// key goes on to the sink around it. A disabled row in a list still
/// gets the list's menu.
#[test]
fn a_disabled_node_offers_nothing_and_the_panel_answers() {
    let mut core = Core::new();
    let k = frame(&mut core);
    let evs = right_click(&mut core, 30.0, 60.0);
    assert_eq!(kinds(&evs), ["contextmenu"]);
    assert_eq!(evs[0].key, k.panel);
    assert_eq!(evs[0].payload.get("tag").unwrap().as_str(), Some("panel"));
}

/// T1's shape: a full-window key sink (the shell pattern) over a root
/// that declared the menu. Before, the sink's region was the topmost hit
/// and swallowed every secondary press; the app looked like it had no
/// menu and nothing warned. Now the press is unclaimed at the sink and
/// reaches the root, carrying the root's key and tag and the press point.
#[test]
fn a_child_without_a_menu_reaches_the_enclosing_one() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(W, H), 1.0);
    ui.configure_root(NodeSpec::column().fill().on_context_menu("shell"));
    let sink = ui.with_keyed("sink", NodeSpec::column().fill().on_key("keys"), |ui| {
        ui.leaf_keyed("button", NodeSpec::row().size(100.0, 40.0).on_click("open"));
    });
    ui.finish();

    // On the sink's own body.
    let evs = right_click(&mut core, 150.0, 150.0);
    assert_eq!(kinds(&evs), ["contextmenu"]);
    assert_eq!(evs[0].key, Key::ROOT);
    assert_eq!(evs[0].payload.get("tag").unwrap().as_str(), Some("shell"));
    assert_eq!(evs[0].payload.get("x").unwrap().as_float(), Some(150.0));
    assert_ne!(evs[0].key, sink);

    // Two levels down, on a button that has a click and no menu: the
    // click stays the button's, the menu is still the root's.
    let evs = right_click(&mut core, 30.0, 20.0);
    assert_eq!(kinds(&evs), ["contextmenu"]);
    assert_eq!(evs[0].key, Key::ROOT);
    assert_eq!(kinds(&click_at(&mut core, 30.0, 20.0)), ["open"]);
}

/// The walk stops at the modal boundary, as the key walk does: a dialog
/// that offers no menu does not hand a press inside it to the app around
/// it, which is inert while the dialog is up.
#[test]
fn the_walk_stops_at_the_modal_boundary() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(W, H), 1.0);
    ui.configure_root(NodeSpec::column().fill().on_context_menu("shell"));
    ui.with_keyed(
        "dialog",
        NodeSpec::column()
            .float(kui_core::FloatConfig::viewport())
            .size(80.0, 40.0)
            .modal("dialog"),
        |ui| {
            ui.leaf_keyed("ok", NodeSpec::row().size(40.0, 20.0).on_click("ok"));
        },
    );
    ui.finish();
    // On the dialog's button and on its body: nothing, not the shell's.
    assert!(right_click(&mut core, 10.0, 10.0).is_empty());
    assert!(right_click(&mut core, 70.0, 35.0).is_empty());
}

/// The decision this routing rests on: on every platform kui targets, a
/// right-click leaves the keyboard where it was. A menu that acts on the
/// selection needs the selection still there when it opens.
#[test]
fn a_secondary_press_moves_neither_focus_nor_the_caret() {
    let mut core = Core::new();
    let k = frame(&mut core);
    // Focus the editor and select a word (double click), the state a
    // "Copy" menu item would act on.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, 86.0)));
    core.handle_input(InputEvent::mouse_down(2));
    core.handle_input(InputEvent::mouse_up());
    frame(&mut core);
    assert_eq!(core.focus(), Some(k.edit));
    let selected = core.copy_selection();
    assert_eq!(selected.as_deref(), Some("hello"));

    // A right-click on the clickable row: it answers, and the editor keeps
    // focus, caret and selection.
    let evs = right_click(&mut core, 30.0, 20.0);
    assert_eq!(kinds(&evs), ["contextmenu"]);
    assert_eq!(core.focus(), Some(k.edit), "focus stayed in the editor");
    assert_eq!(core.copy_selection(), selected, "and so did its selection");

    // Even right in the editor's own text.
    right_click(&mut core, 60.0, 86.0);
    assert_eq!(core.focus(), Some(k.edit));
    assert_eq!(core.copy_selection(), selected);

    // A right-click on the panel does not blur it either.
    right_click(&mut core, 30.0, 180.0);
    assert_eq!(core.focus(), Some(k.edit));
    assert_eq!(core.copy_selection(), selected);

    // Where a left-click on the panel does both.
    click_at(&mut core, 30.0, 180.0);
    assert_eq!(core.focus(), None);
}

/// The press is not held: releasing over another node cannot click it,
/// and the node under the press never reads as pressed.
#[test]
fn a_secondary_press_holds_nothing() {
    let mut core = Core::new();
    let k = frame(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(30.0, 20.0)));
    core.handle_input(secondary_down());
    assert!(!core.interaction.is_pressed(k.row));
    let out = core.handle_input(InputEvent::CursorMoved(Vec2::new(30.0, 180.0)));
    assert!(out.is_empty());
    let out = core.handle_input(secondary_up());
    assert!(out.is_empty());
}

/// A right-click during a held left-drag is a pass-through: the drag keeps
/// its capture and still ends on the primary release.
#[test]
fn a_secondary_press_does_not_break_a_drag() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(W, H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed(
        "handle",
        NodeSpec::row()
            .size(100.0, 40.0)
            .on_drag("h")
            .on_context_menu("h"),
    );
    ui.finish();

    core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, 20.0)));
    let out = core.handle_input(InputEvent::mouse_down(1));
    assert_eq!(kinds(&out), ["drag"]);
    // The menu still comes out; the drag is untouched.
    let out = core.handle_input(secondary_down());
    assert_eq!(kinds(&out), ["contextmenu"]);
    let out = core.handle_input(secondary_up());
    assert!(out.is_empty());
    let out = core.handle_input(InputEvent::CursorMoved(Vec2::new(60.0, 20.0)));
    assert_eq!(kinds(&out), ["drag"]);
    let out = core.handle_input(InputEvent::mouse_up());
    assert_eq!(kinds(&out), ["drag"]);
    assert_eq!(
        out[0].payload.get("phase").unwrap().as_str(),
        Some("end"),
        "the primary release is what ends the drag"
    );
}

/// A middle press no `on_button` claims routes nowhere (backlog F105 is
/// the node that claims it) — and it must not be mistaken for a primary
/// press on the way through.
#[test]
fn the_middle_button_routes_nowhere() {
    let mut core = Core::new();
    let k = frame(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(30.0, 20.0)));
    let out = core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Middle,
        clicks: 1,
    });
    assert!(out.is_empty());
    let out = core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Middle,
    });
    assert!(out.is_empty());
    assert_eq!(core.focus(), None);
    assert!(!core.interaction.is_pressed(k.row));
}

/// A modal contains the secondary button like every other input: nodes
/// behind it are inert, and a press outside still asks it to go away
/// (that is how a menu closes when you right-click somewhere else).
#[test]
fn a_modal_contains_and_dismisses_on_secondary_presses() {
    let mut core = Core::new();
    let menu = |core: &mut Core| {
        let mut ui = core.frame(Size::new(W, H), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let behind = ui.leaf_keyed(
            "behind",
            NodeSpec::row().size(W, 100.0).on_context_menu("behind"),
        );
        let menu = ui.leaf_keyed(
            "menu",
            NodeSpec::column()
                .float(kui_core::FloatConfig::viewport())
                .size(80.0, 40.0)
                .modal("menu")
                .on_context_menu("menu"),
        );
        ui.finish();
        (behind, menu)
    };
    let (_, menu_key) = menu(&mut core);

    // Inside the modal: the menu's own node answers.
    let evs = right_click(&mut core, 20.0, 20.0);
    assert_eq!(kinds(&evs), ["contextmenu"]);
    assert_eq!(evs[0].key, menu_key);

    // Outside it: the node behind is inert, and the modal is asked to go.
    let evs = right_click(&mut core, 20.0, 80.0);
    assert_eq!(kinds(&evs), ["dismiss"]);
    assert_eq!(evs[0].key, menu_key);
    assert_eq!(
        evs[0].payload.get("reason").unwrap().as_str(),
        Some("outside")
    );
}
