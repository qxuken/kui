//! Why a frame runs (backlog F111): the reasons a frame is handed
//! (`frame_cause`), who holds an owed frame (`owed_by`, the rest of it in
//! `tests/anim.rs`), and whether a frame changed what is drawn
//! (`frame_unchanged`).

use kui_core::{
    Color, Core, Easing, FrameCause, InputEvent, KeyCode, KeyMods, KeyPress, NodeSpec, Size,
    Transition, Vec2,
};

const VIEW: Size = Size { w: 200.0, h: 100.0 };

/// One frame of a white box `w` wide; returns the cause the view read.
fn frame(core: &mut Core, w: f32) -> FrameCause {
    let mut ui = core.frame(VIEW, 1.0);
    let cause = ui.core().frame_cause();
    ui.leaf_keyed("box", NodeSpec::column().size(w, 20.0).bg(Color::WHITE));
    ui.finish();
    cause
}

/// The input the core is handed is the next frame's reason, by kind; a
/// driver's note joins it; and a frame answers everything since the last
/// one began, once.
#[test]
fn a_frame_is_handed_what_reached_the_window_since_the_last_one() {
    let mut core = Core::new();
    assert!(frame(&mut core, 10.0).is_empty(), "nothing asked");
    core.handle_input(InputEvent::CursorMoved(Vec2::new(5.0, 5.0)));
    core.handle_input(InputEvent::KeyDown(KeyPress::new(
        KeyCode::Char('a'),
        KeyMods::default(),
    )));
    core.note_frame_cause(FrameCause::WAKE | FrameCause::CARET);
    let cause = frame(&mut core, 10.0);
    assert_eq!(
        cause,
        FrameCause::POINTER_MOVE | FrameCause::KEY | FrameCause::WAKE | FrameCause::CARET
    );
    assert_eq!(
        cause.names().collect::<Vec<_>>(),
        ["pointerMove", "key", "wake", "caret"]
    );
    assert_eq!(
        format!("{cause:?}"),
        "FrameCause(pointerMove|key|wake|caret)"
    );
    // Between frames it is still the last frame's.
    assert_eq!(core.frame_cause(), cause);
    assert!(frame(&mut core, 10.0).is_empty(), "answered once");
    // Input that arrives during a build is the next frame's.
    let mut ui = core.frame(VIEW, 1.0);
    ui.core().note_frame_cause(FrameCause::RESIZE);
    assert!(ui.core().frame_cause().is_empty());
    ui.finish();
    assert_eq!(frame(&mut core, 10.0), FrameCause::RESIZE);
    assert_eq!(
        FrameCause::from_bits(u32::MAX).names().count(),
        FrameCause::ALL.len()
    );
}

/// A frame that drew what the one before drew is unchanged; one that
/// moved a quad is not. Only traced, and never the first traced frame.
#[test]
fn frame_unchanged_compares_what_two_frames_drew() {
    let mut core = Core::new();
    frame(&mut core, 10.0);
    frame(&mut core, 10.0);
    assert_eq!(core.frame_unchanged(), None, "off");
    core.set_frame_trace(true);
    frame(&mut core, 10.0);
    assert_eq!(core.frame_unchanged(), None, "nothing to compare with yet");
    frame(&mut core, 10.0);
    assert_eq!(core.frame_unchanged(), Some(true));
    frame(&mut core, 20.0);
    assert_eq!(core.frame_unchanged(), Some(false));
    frame(&mut core, 20.0);
    assert_eq!(core.frame_unchanged(), Some(true));
    core.set_frame_trace(false);
    assert_eq!(core.frame_unchanged(), None);
}

/// A container easing a programmatic scroll holds the frame, by name.
#[test]
fn owed_by_names_an_easing_scroller() {
    let build = |core: &mut Core, reveal: bool| {
        let mut ui = core.frame(Size::new(100.0, 50.0), 1.0);
        let by = ui.core().owed_by().clone();
        ui.configure_root(NodeSpec::column().fill());
        let spec = NodeSpec::row()
            .fill()
            .scroll_x()
            .transition_with(Transition::ms(100.0).easing(Easing::Linear));
        let mut boxes = Vec::new();
        ui.with_keyed("ribbon", spec, |ui| {
            for i in 0..4 {
                boxes.push(ui.leaf_keyed(
                    &format!("b{i}"),
                    NodeSpec::column().width(100.0).grow_height(),
                ));
            }
        });
        if reveal {
            ui.reveal(boxes[3]);
        }
        ui.finish();
        by
    };
    let mut core = Core::new();
    core.set_frame_trace(true);
    core.set_time(0.0);
    build(&mut core, false);
    build(&mut core, true);
    assert!(core.owed().scroll);
    core.set_time(0.05);
    let by = build(&mut core, false);
    let names: Vec<_> = by.scrolls.iter().map(|h| h.name.as_str()).collect();
    assert_eq!(names, ["ribbon"]);
    core.set_time(0.2);
    build(&mut core, false);
    let by = build(&mut core, false);
    assert!(by.scrolls.is_empty(), "landed: {by:?}");
}
