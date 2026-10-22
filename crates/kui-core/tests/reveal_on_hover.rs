//! `reveal_on_hover` (ADR 0051, backlog F183): a row's secondary actions
//! drawn only while the row — the nearest ancestor that tracks the pointer
//! — is hovered, holds a press, or holds keyboard-visible focus, and while
//! focus is inside the actions themselves. Hidden is opacity 0: the
//! actions keep their room, their hits and their place in the access tree
//! and the Tab ring.

use kui_core::testing::{codes, hover, press, release, tab};
use kui_core::{Color, Core, InputEvent, NodeSpec, QuadKind, Size, Ui, Value, Vec2, widgets};

const VIEW: Size = Size { w: 400.0, h: 300.0 };
const ROW: u32 = 0x2040_80ff;
const ACTIONS: u32 = 0xff00_00ff;

/// A row 300×40 at the top left that tracks the pointer (its anchor),
/// holding a checkbox, a spacer and, at its right end, an actions box
/// 100 wide holding one stock button; a second row under it, 40 px down,
/// with a button of its own and nothing revealed in it.
fn view(ui: &mut Ui<'_>, actions: NodeSpec) {
    ui.with(NodeSpec::column(), |ui| {
        ui.with_keyed(
            "row",
            NodeSpec::row()
                .size(300.0, 40.0)
                .bg(Color::hex(ROW))
                .cross_align(kui_core::Align::Center)
                .hoverable(),
            |ui| {
                widgets::checkbox(ui, "Pick", false, Value::str("pick"));
                ui.leaf(NodeSpec::row().grow_width().height(40.0));
                ui.with_keyed("actions", actions, |ui| {
                    widgets::button(ui, "Delete", Value::str("delete"));
                });
            },
        );
        ui.with(NodeSpec::row().size(300.0, 40.0).hoverable(), |ui| {
            widgets::button(ui, "Other", Value::str("other"));
        });
    });
}

fn actions() -> NodeSpec {
    NodeSpec::row()
        .size(100.0, 40.0)
        .bg(Color::hex(ACTIONS))
        .main_align(kui_core::Align::End)
        .cross_align(kui_core::Align::Center)
        .reveal_on_hover()
}

fn frame_with(core: &mut Core, actions: NodeSpec) {
    let mut ui = core.frame(VIEW, 1.0);
    view(&mut ui, actions);
    ui.finish();
}

fn frame(core: &mut Core) {
    frame_with(core, actions());
}

/// The actions box's alpha on the wire: `None` when no quad was drawn.
fn actions_alpha(core: &mut Core) -> Option<f32> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.color.r == 1.0 && q.color.g == 0.0)
        .map(|q| q.color.a)
        .fold(None, |a: Option<f32>, b| Some(a.map_or(b, |a| a.max(b))))
}

fn shown(core: &mut Core) -> bool {
    actions_alpha(core).is_some_and(|a| a > 0.99)
}

fn hidden(core: &mut Core) -> bool {
    actions_alpha(core).is_none_or(|a| a < 0.01)
}

#[test]
fn the_actions_show_only_while_the_row_is_hovered() {
    let mut core = Core::new();
    frame(&mut core);
    assert!(hidden(&mut core), "nothing hovered: hidden");

    hover(&mut core, Vec2::new(150.0, 20.0));
    frame(&mut core);
    assert!(shown(&mut core), "the row's bare part hovered: shown");

    hover(&mut core, Vec2::new(150.0, 60.0));
    frame(&mut core);
    assert!(hidden(&mut core), "the row below hovered: hidden again");

    core.handle_input(InputEvent::CursorLeft);
    frame(&mut core);
    assert!(hidden(&mut core), "the pointer gone: hidden");
}

/// The button inside the actions tracks the pointer itself, so over it
/// the hovered node is the button, not the row: the row is still hovered
/// for the reveal, since the button is inside it.
#[test]
fn the_pointer_over_a_control_inside_keeps_them_shown() {
    let mut core = Core::new();
    frame(&mut core);
    hover(&mut core, Vec2::new(265.0, 20.0));
    frame(&mut core);
    let button = core.key_named("Delete").unwrap();
    assert!(core.is_hovered(button), "the button takes the hover");
    assert!(
        shown(&mut core),
        "the actions stay shown over their own button"
    );
}

/// The hidden actions are still hit: the press on the hidden button
/// reaches it, which is why the pointer that gets there shows them.
#[test]
fn a_hidden_control_is_still_there_for_the_pointer() {
    let mut core = Core::new();
    frame(&mut core);
    assert!(hidden(&mut core));
    let evs = kui_core::testing::click(&mut core, Vec2::new(265.0, 20.0));
    assert_eq!(kui_core::testing::messages(&evs), ["delete"]);
}

/// A press begun inside the row keeps it shown while the pointer is
/// dragged off it, until the release. On the checkbox, so the focus the
/// press gives is the row's and not the actions'.
#[test]
fn a_press_held_inside_keeps_them_shown() {
    let mut core = Core::new();
    frame(&mut core);
    press(&mut core, Vec2::new(20.0, 20.0));
    frame(&mut core);
    hover(&mut core, Vec2::new(20.0, 200.0));
    frame(&mut core);
    assert!(shown(&mut core), "the press is held inside the row");
    release(&mut core);
    frame(&mut core);
    frame(&mut core);
    assert!(hidden(&mut core), "released off the row: hidden");
}

/// Tab onto the row's checkbox (keyboard-visible focus inside the
/// anchor) shows them; Tab on to the button inside them keeps them
/// shown; Tab past both hides them.
#[test]
fn keyboard_focus_inside_the_row_shows_them() {
    let mut core = Core::new();
    frame(&mut core);
    tab(&mut core, false);
    frame(&mut core);
    assert_eq!(core.focus(), core.key_named("Pick"));
    frame(&mut core);
    assert!(shown(&mut core), "the row holds visible focus");

    tab(&mut core, false);
    frame(&mut core);
    assert_eq!(core.focus(), core.key_named("Delete"));
    frame(&mut core);
    assert!(shown(&mut core), "the button inside holds it");

    tab(&mut core, false);
    frame(&mut core);
    frame(&mut core);
    assert!(hidden(&mut core), "focus on the row below: hidden");
}

/// Focus that is not visible — a click's — inside the row does not show
/// them once the pointer has gone; inside the actions themselves it
/// does, so a control in them never vanishes while it has the keyboard.
#[test]
fn a_clicked_control_inside_stays_shown_after_the_pointer_leaves() {
    let mut core = Core::new();
    frame(&mut core);
    kui_core::testing::click(&mut core, Vec2::new(265.0, 20.0));
    frame(&mut core);
    assert_eq!(
        core.focus(),
        core.key_named("Delete"),
        "the click focused the button"
    );
    core.handle_input(InputEvent::CursorLeft);
    frame(&mut core);
    frame(&mut core);
    assert!(shown(&mut core), "focus inside the actions keeps them");

    kui_core::testing::click(&mut core, Vec2::new(20.0, 20.0));
    frame(&mut core);
    assert_eq!(
        core.focus(),
        core.key_named("Pick"),
        "the click focused the checkbox"
    );
    core.handle_input(InputEvent::CursorLeft);
    frame(&mut core);
    frame(&mut core);
    assert!(
        hidden(&mut core),
        "a click's focus elsewhere in the row is not keyboard focus: hidden"
    );
}

/// Hidden is paint: the button stays in the access tree with its name,
/// and the actions keep their room in the row.
#[test]
fn hidden_actions_keep_their_room_and_their_access_node() {
    let mut core = Core::new();
    frame(&mut core);
    assert!(hidden(&mut core));
    let tree = core.access_tree();
    let node = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Delete"))
        .expect("the hidden button is in the access tree");
    let r = node.rect;
    assert!(
        r.x > 200.0 && r.x + r.w <= 300.5,
        "laid out at the row's end: {r:?}"
    );
}

/// With a transition the reveal eases, as a `hover_bg` does: the frame
/// that sees the hover starts the tween from hidden, and the ones after
/// it draw the way up.
#[test]
fn a_transition_eases_the_reveal() {
    let mut core = Core::new();
    core.set_time(0.0);
    let eased = || actions().transition(100.0);
    frame_with(&mut core, eased());
    hover(&mut core, Vec2::new(150.0, 20.0));
    core.set_time(0.02);
    frame_with(&mut core, eased());
    core.set_time(0.07);
    frame_with(&mut core, eased());
    let a = actions_alpha(&mut core).unwrap_or(0.0);
    assert!(a > 0.05 && a < 0.95, "mid-ease: {a}");
    core.set_time(0.5);
    frame_with(&mut core, eased());
    assert!(shown(&mut core));
}

/// No ancestor tracks the pointer: nothing can show it, so it is drawn,
/// and the warning says why.
#[test]
fn with_no_anchor_it_is_drawn_and_warned_about() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.with(NodeSpec::row().size(300.0, 40.0), |ui| {
        ui.leaf(actions());
    });
    ui.finish();
    assert!(shown(&mut core), "no anchor: always drawn");
    assert!(codes(&core.take_warnings()).contains(&kui_core::diag::REVEAL_WITHOUT_ANCHOR));
}

/// A frame declaring none walks nothing, and one with an anchor warns of
/// nothing.
#[test]
fn an_anchored_reveal_raises_no_warning() {
    let mut core = Core::new();
    frame(&mut core);
    assert!(!codes(&core.take_warnings()).contains(&kui_core::diag::REVEAL_WITHOUT_ANCHOR));
}
