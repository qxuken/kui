//! `UiEvent`'s typed views (backlog DX7), read off events real input
//! produced: a view that drifts from what the core emits fails here, not
//! in an app's handler.

use kui_core::testing::{hover, key_down, key_up, press, release};
use kui_core::{
    Core, DragPhase, HoverPhase, InputEvent, KeyCode, KeyMods, KeyPhase, NodeSpec, Rect, Size, Vec2,
};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

fn frame(core: &mut Core, spec: NodeSpec) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.with(NodeSpec::row().pad(20.0), |ui| {
        ui.leaf_keyed("n", spec.size(200.0, 100.0));
    });
    ui.finish();
}

#[test]
fn a_drag_reads_as_its_phases_with_the_distance_from_the_press() {
    let mut core = Core::new();
    frame(&mut core, NodeSpec::row().on_drag("split"));
    let start = press(&mut core, Vec2::new(40.0, 50.0));
    let moved = hover(&mut core, Vec2::new(120.0, 70.0));
    let end = release(&mut core);
    let drags: Vec<_> = [start, moved, end]
        .concat()
        .iter()
        .filter_map(|e| e.drag())
        .collect();
    let phases: Vec<_> = drags.iter().map(|d| d.phase).collect();
    assert_eq!(phases, [DragPhase::Start, DragPhase::Move, DragPhase::End]);
    let m = drags[1];
    assert_eq!(m.pos, Vec2::new(120.0, 70.0));
    assert_eq!(
        m.delta,
        Vec2::new(80.0, 20.0),
        "from the press, not the last move"
    );
    assert_eq!(
        m.parent,
        Rect::new(0.0, 0.0, 240.0, 140.0),
        "the padded row"
    );
    assert_eq!(m.ratio(), Vec2::new(0.5, 0.5));
    assert_eq!((m.cell, m.line, m.byte), (None, None, None));
}

#[test]
fn a_key_reads_back_as_the_press_that_made_it() {
    let mut core = Core::new();
    frame(&mut core, NodeSpec::row().on_key("keys").key_up());
    let sink = core.key_of("n").unwrap();
    core.set_key_focus(Some(sink));
    let mods = KeyMods::NONE.with_ctrl().with_shift();
    let down = key_down(&mut core, KeyCode::Char('Z'), mods);
    let (phase, k) = down
        .iter()
        .find_map(|e| e.key_press())
        .expect("a key event");
    assert_eq!(phase, KeyPhase::Down);
    assert_eq!(
        (k.code, k.physical, k.mods),
        (KeyCode::Char('Z'), KeyCode::Char('z'), mods)
    );
    assert_eq!(down[0].tag().and_then(|t| t.as_str()), Some("keys"));
    let up = key_up(&mut core, KeyCode::Char('Z'), mods);
    let (phase, k) = up.iter().find_map(|e| e.key_press()).expect("a release");
    assert_eq!((phase, k.text), (KeyPhase::Up, None));
    assert!(down[0].drag().is_none(), "a view of another kind is None");
}

#[test]
fn a_wheel_a_hover_and_a_layout_read_as_theirs() {
    let mut core = Core::new();
    frame(
        &mut core,
        NodeSpec::row()
            .on_scroll("wheel")
            .on_hover("over")
            .on_layout("placed"),
    );
    let placed: Vec<_> = core.take_pending_events();
    let l = placed
        .iter()
        .find_map(|e| e.layout())
        .expect("a layout report");
    assert_eq!(l.rect, Rect::new(20.0, 20.0, 200.0, 100.0));
    assert_eq!(l.scale, 1.0);

    let entered = hover(&mut core, Vec2::new(50.0, 50.0));
    assert_eq!(
        entered.iter().find_map(|e| e.hover()),
        Some(HoverPhase::Enter)
    );
    let wheel = core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -30.0)));
    let s = wheel.iter().find_map(|e| e.scroll()).expect("a scroll");
    assert_eq!(
        (s.pos, s.delta, s.lines),
        (Vec2::new(50.0, 50.0), Vec2::new(0.0, -30.0), None)
    );
    let left = hover(&mut core, Vec2::new(290.0, 190.0));
    assert_eq!(left.iter().find_map(|e| e.hover()), Some(HoverPhase::Leave));
}

#[test]
fn modifiers_and_text_read_as_theirs() {
    let mut core = Core::new();
    frame(&mut core, NodeSpec::row().on_key("keys"));
    let sink = core.key_of("n").unwrap();
    core.set_key_focus(Some(sink));
    let m = core.handle_input(InputEvent::Modifiers(KeyMods::NONE.with_alt()));
    assert_eq!(
        m.iter().find_map(|e| e.modifiers()),
        Some(KeyMods::NONE.with_alt())
    );
    let t = core.handle_input(InputEvent::Commit("ё".into()));
    let text = t.iter().find_map(|e| e.text()).expect("a text event");
    assert_eq!(
        (text.text, text.concealed, text.transient),
        ("ё", false, false)
    );
}
