//! `onButton` (backlog F105): the non-primary buttons as events on the
//! node that claims them — a terminal's middle-click paste, and the mouse
//! reports a program in it asked for. A claimed press is captured by its
//! owner until the release; the secondary button claimed is the owner's
//! instead of a context menu; and none of it moves focus, as no
//! non-primary press does.

use kui_core::cells::{Cell, CellGrid};
use kui_core::testing::{click_at, kinds};
use kui_core::{
    ButtonPhase, Buttons, Core, EditOptions, InputEvent, Key, MouseButton, NodeSpec, Size,
    TextStyle, UiEvent, Value, Vec2,
};

const VIEW: Size = Size { w: 400.0, h: 300.0 };

fn down(button: MouseButton) -> InputEvent {
    InputEvent::MouseDown { button, clicks: 1 }
}

fn up(button: MouseButton) -> InputEvent {
    InputEvent::MouseUp { button }
}

fn to(core: &mut Core, x: f32, y: f32) -> Vec<UiEvent> {
    core.handle_input(InputEvent::CursorMoved(Vec2::new(x, y)))
}

fn mono() -> TextStyle {
    TextStyle::new(14.0).mono().line_height(20.0)
}

/// A pane claiming `buttons` with `onButton`, holding a terminal grid at
/// its top-left (4 rows × 10 cols, 20 px rows), a plain child that claims
/// nothing, an editor, and a button that takes focus when clicked. The
/// pane asks for a context menu too, so the secondary button has somewhere
/// else to go.
struct Keys {
    pane: Key,
    term: Key,
    child: Key,
    go: Key,
}

fn frame(core: &mut Core, buttons: Buttons, term_claims: bool) -> Keys {
    let cells = vec![Cell::new('x', 0xffffffff, 0); 40];
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut keys = Keys {
        pane: Key::ROOT,
        term: Key::ROOT,
        child: Key::ROOT,
        go: Key::ROOT,
    };
    keys.pane = ui.with_keyed(
        "pane",
        NodeSpec::column()
            .size(300.0, 280.0)
            .on_context_menu("menu")
            .on_button(Value::map([("kind", Value::str("pane"))]))
            .buttons(buttons),
        |ui| {
            let term = NodeSpec::default();
            ui.cells_keyed(
                "term",
                &CellGrid {
                    rows: 4,
                    cols: 10,
                    cells: &cells,
                    style: mono(),
                    cursor: None,
                    origin_line: 0,
                },
                if term_claims {
                    term.on_button(Value::map([("kind", Value::str("term"))]))
                        .buttons(Buttons::MIDDLE)
                } else {
                    term
                },
            );
            keys.child = ui.leaf_keyed("child", NodeSpec::row().size(100.0, 40.0).hoverable());
            ui.text_edit(
                "note",
                "hello world",
                &EditOptions {
                    style: TextStyle::new(13.0),
                    ..Default::default()
                },
                NodeSpec::row().size(200.0, 30.0),
            );
            keys.go = ui.leaf_keyed("go", NodeSpec::row().size(100.0, 30.0).on_click("go"));
        },
    );
    ui.finish();
    keys.term = core.key_of("term").expect("the grid is keyed");
    keys
}

fn tag_kind(ev: &UiEvent) -> Option<&str> {
    ev.payload.get("tag").and_then(|t| t.get_str("kind"))
}

/// The grid's rect, laid out: the pane's first child, 80 px tall.
const TERM_Y: f32 = 10.0;
/// The plain child, below the grid.
const CHILD_Y: f32 = 100.0;
/// The editor, below the child.
const EDIT_Y: f32 = 135.0;

#[test]
fn middle_press_move_and_release_reach_the_grid_with_their_cells() {
    let mut core = Core::new();
    let k = frame(&mut core, Buttons::ALL, true);
    let w = core.measure_text("M", &mono(), None).width.round();
    to(&mut core, 2.5 * w, TERM_Y);
    let press = core.handle_input(down(MouseButton::Middle));
    assert_eq!(kinds(&press), ["button"], "{press:?}");
    let b = press[0].button().expect("a button event");
    assert_eq!(press[0].key, k.term, "the grid's own claim wins");
    assert_eq!(tag_kind(&press[0]), Some("term"));
    assert_eq!(b.phase, ButtonPhase::Press);
    assert_eq!(b.button, MouseButton::Middle);
    assert_eq!(press[0].payload.get_str("button"), Some("middle"));
    assert_eq!(b.clicks, Some(1));
    assert_eq!(b.cell, Some((0, 2)));

    let moved = to(&mut core, 5.5 * w, 45.0);
    let m = moved
        .iter()
        .find_map(UiEvent::button)
        .expect("the move while held");
    assert_eq!(m.phase, ButtonPhase::Move);
    assert_eq!(m.cell, Some((2, 5)));
    assert_eq!(m.clicks, None, "the count is the press's");

    let released = core.handle_input(up(MouseButton::Middle));
    assert_eq!(kinds(&released), ["button"]);
    let r = released[0].button().unwrap();
    assert_eq!(r.phase, ButtonPhase::Release);
    assert_eq!(r.cell, Some((2, 5)));
    assert_eq!(released[0].key, k.term);

    // Released, the motion is nobody's.
    assert!(
        to(&mut core, 10.0, 10.0)
            .iter()
            .all(|e| e.button().is_none())
    );
}

#[test]
fn the_capture_follows_the_pointer_off_the_node_until_the_release() {
    let mut core = Core::new();
    let k = frame(&mut core, Buttons::ALL, true);
    let w = core.measure_text("M", &mono(), None).width.round();
    to(&mut core, 20.0, TERM_Y);
    core.handle_input(down(MouseButton::Middle));
    // Off the grid, onto the child, then out of the window altogether.
    let over_child = to(&mut core, 20.0, CHILD_Y);
    let moves: Vec<_> = over_child.iter().filter(|e| e.button().is_some()).collect();
    assert_eq!(moves.len(), 1);
    assert_eq!(
        moves[0].key, k.term,
        "the move is the grid's, not the child's"
    );
    assert_eq!(
        moves[0].button().unwrap().cell,
        Some((3, (20.0 / w).floor() as u32)),
        "a cell past the grid is clamped to it"
    );
    to(&mut core, 380.0, 290.0);
    core.handle_input(InputEvent::CursorLeft);
    let released = core.handle_input(up(MouseButton::Middle));
    assert_eq!(kinds(&released), ["button"]);
    assert_eq!(released[0].key, k.term);
    let r = released[0].button().unwrap();
    assert_eq!(r.phase, ButtonPhase::Release);
    assert_eq!(
        r.pos,
        Vec2::new(380.0, 290.0),
        "where the pointer was last seen"
    );
}

#[test]
fn a_child_that_claims_nothing_is_its_ancestors() {
    let mut core = Core::new();
    let k = frame(&mut core, Buttons::ALL, true);
    to(&mut core, 20.0, CHILD_Y);
    let press = core.handle_input(down(MouseButton::Middle));
    assert_eq!(kinds(&press), ["button"]);
    assert_eq!(press[0].key, k.pane, "the owner's key and tag");
    assert_eq!(tag_kind(&press[0]), Some("pane"));
    assert_eq!(press[0].payload.get("cell"), None, "the pane is no grid");
    let released = core.handle_input(up(MouseButton::Middle));
    assert_eq!(released[0].key, k.pane);

    // The grid claims the middle button only: its other buttons are the
    // pane's, since each button walks to the nearest node claiming it.
    to(&mut core, 20.0, TERM_Y);
    let other = MouseButton::Other(0);
    let press = core.handle_input(down(other));
    assert_eq!(press[0].key, k.pane);
    assert_eq!(press[0].payload.get("button"), Some(&Value::Int(3)));
    assert_eq!(press[0].button().unwrap().button, other);
    core.handle_input(up(other));
}

#[test]
fn several_buttons_are_held_at_once_each_its_own_capture() {
    let mut core = Core::new();
    let k = frame(&mut core, Buttons::ALL, true);
    to(&mut core, 20.0, TERM_Y);
    core.handle_input(down(MouseButton::Middle));
    to(&mut core, 20.0, CHILD_Y);
    core.handle_input(down(MouseButton::Secondary));
    assert_eq!(
        core.interaction.button_owner(MouseButton::Middle),
        Some(k.term)
    );
    assert_eq!(
        core.interaction.button_owner(MouseButton::Secondary),
        Some(k.pane)
    );
    let moved = to(&mut core, 30.0, CHILD_Y);
    let owners: Vec<_> = moved
        .iter()
        .filter_map(|e| e.button().map(|b| (e.key, b.button)))
        .collect();
    assert_eq!(
        owners,
        [
            (k.term, MouseButton::Middle),
            (k.pane, MouseButton::Secondary)
        ]
    );
    let released = core.handle_input(up(MouseButton::Secondary));
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].key, k.pane);
    assert_eq!(
        core.interaction.button_owner(MouseButton::Middle),
        Some(k.term)
    );
    core.handle_input(up(MouseButton::Middle));
    assert_eq!(core.interaction.button_owner(MouseButton::Middle), None);
}

#[test]
fn a_claimed_secondary_press_is_the_button_event_and_no_menu() {
    let mut core = Core::new();
    let k = frame(&mut core, Buttons::ALL, false);
    // Over the editor: unclaimed, this would open the stock Cut / Copy /
    // Paste menu.
    to(&mut core, 20.0, EDIT_Y);
    let press = core.handle_input(down(MouseButton::Secondary));
    assert_eq!(kinds(&press), ["button"], "{press:?}");
    assert_eq!(press[0].key, k.pane);
    assert_eq!(press[0].payload.get_str("button"), Some("secondary"));
    assert!(core.menu().is_none(), "no stock menu");
    let released = core.handle_input(up(MouseButton::Secondary));
    assert_eq!(kinds(&released), ["button"]);
}

#[test]
fn without_secondary_in_the_mask_the_context_menu_is_as_it_was() {
    let mut core = Core::new();
    let k = frame(&mut core, Buttons::MIDDLE | Buttons::OTHER, false);
    to(&mut core, 20.0, CHILD_Y);
    let press = core.handle_input(down(MouseButton::Secondary));
    assert_eq!(kinds(&press), ["contextmenu"]);
    assert_eq!(press[0].key, k.pane);
    assert!(core.handle_input(up(MouseButton::Secondary)).is_empty());
    assert_eq!(core.interaction.button_owner(MouseButton::Secondary), None);
    // And the stock menu over the editor, as before.
    to(&mut core, 20.0, EDIT_Y);
    core.handle_input(down(MouseButton::Secondary));
    assert!(core.menu().is_some(), "the editor's stock menu");
}

#[test]
fn a_nearer_context_menu_wins_the_secondary_button() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.with_keyed(
        "pane",
        NodeSpec::column().size(300.0, 200.0).on_button("pane"),
        |ui| {
            ui.leaf_keyed(
                "row",
                NodeSpec::row().size(100.0, 40.0).on_context_menu("row"),
            );
        },
    );
    ui.finish();
    to(&mut core, 20.0, 20.0);
    let press = core.handle_input(down(MouseButton::Secondary));
    assert_eq!(kinds(&press), ["contextmenu"], "the nested declaration");
    core.handle_input(up(MouseButton::Secondary));
    let press = core.handle_input(down(MouseButton::Middle));
    assert_eq!(kinds(&press), ["button"], "the row offers no middle button");
    assert_eq!(press[0].payload.get_str("tag"), Some("pane"));
}

#[test]
fn middle_with_no_owner_does_nothing() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.leaf_keyed(
        "row",
        NodeSpec::row()
            .size(100.0, 40.0)
            .on_click("go")
            .on_context_menu("row"),
    );
    ui.finish();
    to(&mut core, 20.0, 20.0);
    assert!(core.handle_input(down(MouseButton::Middle)).is_empty());
    assert!(to(&mut core, 30.0, 20.0).is_empty());
    assert!(core.handle_input(up(MouseButton::Middle)).is_empty());
    assert_eq!(core.focus(), None);
}

#[test]
fn the_primary_button_is_untouched() {
    let mut core = Core::new();
    let k = frame(&mut core, Buttons::ALL, true);
    // A click on the pane's button still clicks, and no `button` event.
    let evs = click_at(&mut core, 20.0, EDIT_Y + 40.0);
    assert!(
        evs.iter().any(|e| e.payload.as_str() == Some("go")),
        "{evs:?}"
    );
    assert!(evs.iter().all(|e| e.button().is_none()));
    assert_eq!(core.key_focus(), Some(k.go));

    // A middle press held through a primary press, move and release: each
    // keeps to its own.
    to(&mut core, 20.0, CHILD_Y);
    core.handle_input(down(MouseButton::Middle));
    core.handle_input(InputEvent::mouse_down(1));
    let moved = to(&mut core, 25.0, CHILD_Y);
    assert_eq!(kinds(&moved), ["button"]);
    let released = core.handle_input(InputEvent::mouse_up());
    assert!(released.iter().all(|e| e.button().is_none()));
    assert_eq!(
        core.interaction.button_owner(MouseButton::Middle),
        Some(k.pane)
    );
    let released = core.handle_input(up(MouseButton::Middle));
    assert_eq!(kinds(&released), ["button"]);
}

#[test]
fn a_non_primary_press_moves_no_focus() {
    let mut core = Core::new();
    let k = frame(&mut core, Buttons::ALL, true);
    click_at(&mut core, 20.0, EDIT_Y + 40.0);
    assert_eq!(core.key_focus(), Some(k.go));
    for (x, y) in [(20.0, EDIT_Y), (20.0, TERM_Y), (20.0, CHILD_Y)] {
        for b in [MouseButton::Middle, MouseButton::Secondary] {
            to(&mut core, x, y);
            let press = core.handle_input(down(b));
            assert_eq!(kinds(&press), ["button"]);
            core.handle_input(up(b));
            assert_eq!(core.key_focus(), Some(k.go), "{b:?} at ({x}, {y})");
        }
    }
}

#[test]
fn a_modal_keeps_its_outside_to_itself() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed(
        "behind",
        NodeSpec::row().size(400.0, 100.0).on_button("behind"),
    );
    ui.leaf_keyed(
        "dialog",
        NodeSpec::column()
            .size(200.0, 100.0)
            .modal("dialog")
            .label("Dialog")
            .on_button("dialog"),
    );
    ui.finish();
    // Outside: no button event behind the modal.
    to(&mut core, 20.0, 20.0);
    let press = core.handle_input(down(MouseButton::Middle));
    assert!(press.iter().all(|e| e.button().is_none()), "{press:?}");
    core.handle_input(up(MouseButton::Middle));
    // Inside, the modal's own.
    to(&mut core, 20.0, 150.0);
    let press = core.handle_input(down(MouseButton::Middle));
    assert_eq!(kinds(&press), ["button"]);
    assert_eq!(press[0].payload.get_str("tag"), Some("dialog"));
}

#[test]
fn an_owner_gone_from_the_frame_lets_go() {
    let mut core = Core::new();
    frame(&mut core, Buttons::ALL, true);
    to(&mut core, 20.0, TERM_Y);
    core.handle_input(down(MouseButton::Middle));
    // The grid stops being declared while the button is held.
    let mut ui = core.frame(VIEW, 1.0);
    ui.leaf_keyed("other", NodeSpec::row().size(100.0, 40.0));
    ui.finish();
    assert_eq!(core.interaction.button_owner(MouseButton::Middle), None);
    assert!(to(&mut core, 30.0, TERM_Y).is_empty());
    assert!(core.handle_input(up(MouseButton::Middle)).is_empty());
}

#[test]
fn buttons_parse_from_their_names() {
    assert_eq!(Buttons::parse("middle"), Buttons::MIDDLE);
    assert_eq!(
        Buttons::parse("secondary middle"),
        Buttons::SECONDARY | Buttons::MIDDLE
    );
    assert_eq!(Buttons::parse("secondary, middle,other"), Buttons::ALL);
    assert_eq!(
        Buttons::parse("wheel"),
        Buttons::NONE,
        "a typo claims nothing"
    );
    assert_eq!(Buttons::default(), Buttons::ALL);
    assert_eq!(Buttons::from_bits(2), Buttons::MIDDLE);
    assert!(!Buttons::ALL.contains(MouseButton::Primary));
    assert!(Buttons::OTHER.contains(MouseButton::Other(7)));
}
