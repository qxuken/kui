//! Text inside a turned node (backlog RG155, ADR 0043): a press finds the
//! node where it is drawn, and so does what the press means for its text —
//! the caret it places, the drag that moves it, a static paragraph's
//! selection, `text_hit` and `caret_rect`, the IME's anchor. Each test turns
//! its card half a turn about its centre, so a point drawn at the card's
//! right end is its upright left end: the arithmetic is a mirror.

use kui_core::testing::{edit_key, press, release};
use kui_core::{
    Core, EditKey, EditOptions, InputEvent, Key, Mods, NodeSpec, Size, TextStyle, Vec2,
};

const VIEW: Size = Size { w: 400.0, h: 200.0 };
/// The card: 200 x 40 at (20, 20), so its centre — the pivot — is (120, 40).
const CENTRE: Vec2 = Vec2 { x: 120.0, y: 40.0 };

/// Where `upright`, a point on the card's upright layout, is drawn.
fn drawn(upright: Vec2) -> Vec2 {
    Vec2::new(2.0 * CENTRE.x - upright.x, 2.0 * CENTRE.y - upright.y)
}

fn card(turn: f32) -> NodeSpec {
    NodeSpec::column().size(200.0, 40.0).pad(8.0).rotate(turn)
}

fn field(core: &mut Core, turn: f32) -> Key {
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().pad(20.0));
    let mut key = Key(0);
    ui.with(card(turn), |ui| {
        key = ui.text_edit(
            "field",
            "mmmmmmmmmm",
            &EditOptions::default(),
            NodeSpec::column().grow_width(),
        );
    });
    ui.finish();
    key
}

fn typed_at(turn: f32, at: Vec2) -> String {
    let mut core = Core::new();
    let key = field(&mut core, turn);
    field(&mut core, turn);
    press(&mut core, at);
    release(&mut core);
    core.handle_input(InputEvent::Text("X".into()));
    core.edit_text(key).unwrap()
}

#[test]
fn a_click_places_the_caret_where_the_text_is_drawn() {
    // Upright, a click 2 px into the text puts the caret at its start.
    let start = Vec2::new(30.0, 40.0);
    assert!(typed_at(0.0, start).starts_with('X'));
    // Half turned, the start is drawn at the card's right end.
    let t = typed_at(0.5, drawn(start));
    assert!(t.starts_with('X'), "the caret at the drawn start: {t}");
}

#[test]
fn the_ime_anchor_is_where_the_caret_is_drawn() {
    let mut core = Core::new();
    let key = field(&mut core, 0.5);
    field(&mut core, 0.5);
    core.set_focus(Some(key));
    edit_key(&mut core, EditKey::Home, Mods::default());
    field(&mut core, 0.5);
    let ime = core.ime_rect().expect("an IME anchor");
    assert!(ime.x > CENTRE.x + 50.0, "and so is the IME's: {ime:?}");
}

fn paragraph(core: &mut Core, turn: f32) -> Key {
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().pad(20.0));
    let mut key = Key(0);
    ui.with(card(turn).selectable(), |ui| {
        key = ui.text_in_keyed(
            "p",
            NodeSpec::row(),
            "alpha beta gamma",
            TextStyle::new(14.0),
        );
    });
    ui.finish();
    key
}

#[test]
fn text_hit_and_a_selection_drag_read_the_drawn_text() {
    let mut core = Core::new();
    let p = paragraph(&mut core, 0.5);
    paragraph(&mut core, 0.5);
    let left = Vec2::new(29.0, 36.0);
    let hit = core.text_hit(p, drawn(left)).expect("a hit");
    assert!(hit.byte <= 1, "the drawn start is byte 0: {hit:?}");
    let r = core.caret_rect(p, 0).expect("a caret rect");
    assert!(r.x > CENTRE.x + 50.0, "byte 0 is drawn at the right: {r:?}");

    // A drag across the first word, from where its start is drawn to where
    // its end is.
    let end = core.caret_rect(p, 5).expect("the end of alpha");
    let to = Vec2::new(end.x + end.w / 2.0, end.y + end.h / 2.0);
    press(&mut core, drawn(left));
    core.handle_input(InputEvent::CursorMoved(to));
    release(&mut core);
    let got = core.selection_text().unwrap_or_default();
    assert_eq!(got.trim(), "alpha", "the drawn first word");
}
