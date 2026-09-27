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
        entered.iter().find_map(|e| e.hover()).map(|h| h.phase),
        Some(HoverPhase::Enter)
    );
    let wheel = core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -30.0)));
    let s = wheel.iter().find_map(|e| e.scroll()).expect("a scroll");
    assert_eq!(
        (s.pos, s.delta, s.lines),
        (Vec2::new(50.0, 50.0), Vec2::new(0.0, -30.0), None)
    );
    let left = hover(&mut core, Vec2::new(290.0, 190.0));
    assert_eq!(
        left.iter().find_map(|e| e.hover()).map(|h| h.phase),
        Some(HoverPhase::Leave)
    );
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

/// DX14: a paste's answer says it is one. kawoosh kept a `clip_probe` and
/// an `awaiting_paste` of its own to tell the clipboard's text, read into
/// a register, from an IME's commit, which arrives the same way.
#[test]
fn a_paste_answer_is_marked_pasted_and_a_commit_is_not() {
    let mut core = Core::new();
    frame(&mut core, NodeSpec::row().on_key("keys"));
    let sink = core.key_of("n").unwrap();
    core.set_key_focus(Some(sink));
    let text = |evs: &[kui_core::UiEvent]| {
        let t = evs.iter().find_map(|e| e.text()).expect("a text event");
        (t.text.to_string(), t.pasted)
    };

    let ime = core.handle_input(InputEvent::Commit("か".into()));
    assert_eq!(
        text(&ime),
        ("か".into(), false),
        "a commit nobody asked for"
    );

    core.request_paste();
    let bare = core.handle_input(InputEvent::Commit("clip".into()));
    assert_eq!(
        text(&bare),
        ("clip".into(), true),
        "a bare commit answering the ask"
    );
    let after = core.handle_input(InputEvent::Commit("か".into()));
    assert_eq!(text(&after), ("か".into(), false), "the ask was spent");

    let paste = core.handle_input(InputEvent::Paste {
        text: "p".into(),
        marks: Default::default(),
    });
    assert_eq!(
        text(&paste),
        ("p".into(), true),
        "a paste reply, asked or not"
    );
}

/// DX20: a hover says what moved. kawoosh's picker kept its own
/// bookkeeping, because the rows sliding under a still pointer as the list
/// scrolled by the keys moved its selection as a pointer would.
#[test]
fn a_hover_says_whether_the_pointer_or_the_content_moved() {
    use kui_core::HoverBy;
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with_keyed(
            "list",
            NodeSpec::column().size(200.0, 100.0).scroll_y(),
            |ui| {
                for i in 0..20 {
                    ui.leaf_indexed(i, NodeSpec::row().size(200.0, 20.0).on_hover(i as i64));
                }
            },
        );
        ui.finish();
    };
    let mut core = Core::new();
    frame(&mut core);
    let entered = hover(&mut core, Vec2::new(10.0, 10.0));
    let h = entered
        .iter()
        .find_map(|e| e.hover())
        .expect("row 0 entered");
    assert_eq!((h.phase, h.by), (HoverPhase::Enter, HoverBy::Pointer));

    // The keys scroll the list a row; the pointer never moved.
    let list = core.key_of("list").unwrap();
    core.set_scroll(list, Vec2::new(0.0, 20.0));
    frame(&mut core);
    frame(&mut core);
    let moved: Vec<_> = core.take_pending_events();
    let by: Vec<_> = moved
        .iter()
        .filter_map(|e| e.hover())
        .map(|h| (h.phase, h.by))
        .collect();
    assert_eq!(
        by,
        [
            (HoverPhase::Leave, HoverBy::Content),
            (HoverPhase::Enter, HoverBy::Content)
        ],
        "row 0 left and row 1 entered under a still pointer"
    );
}
