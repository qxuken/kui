//! A departing node may keep its room until its exit ends (backlog C110):
//! a toast leaving a stack lets the rest close up after it has gone, not
//! while it goes. Without `keep_room` the room closes the frame it goes.

use kui_core::{Color, Core, Enter, Key, NodeSpec, Size, testing};

const VIEW: Size = Size { w: 300.0, h: 300.0 };

fn colour(i: usize) -> Color {
    [
        Color::hex(0xff0000ff),
        Color::hex(0x00ff00ff),
        Color::hex(0x0000ffff),
    ][i]
}

/// A column of the toasts in `shown` (of 0, 1, 2), each 40 tall with a gap
/// of 10, sliding, leaving to the side; returns each drawn toast's top by
/// colour (`None` where none is drawn).
fn frame(core: &mut Core, now: f64, shown: &[usize], keep: bool) -> [Option<f32>; 3] {
    core.set_time(now);
    let mut ui = core.frame(VIEW, 1.0);
    ui.with_keyed("stack", NodeSpec::column().gap(10.0), |ui| {
        for &i in shown {
            let mut exit = Enter::from(32.0, 0.0);
            if keep {
                exit = exit.keep_room();
            }
            ui.leaf_keyed(
                &format!("t{i}"),
                NodeSpec::row()
                    .size(100.0, 40.0)
                    .bg(colour(i))
                    .transition(100.0)
                    .slide()
                    .exit(exit),
            );
        }
    });
    ui.finish();
    let quads = testing::solids(core);
    let top = |c: Color| quads.iter().find(|q| q.color == c).map(|q| q.rect.y);
    [top(colour(0)), top(colour(1)), top(colour(2))]
}

#[test]
fn a_kept_room_holds_the_rest_until_the_exit_ends() {
    let mut core = Core::new();
    frame(&mut core, 1.0, &[0, 1, 2], true);
    let [_, _, c] = frame(&mut core, 1.0, &[0, 1, 2], true);
    assert_eq!(c, Some(100.0));
    // The middle one goes: it plays out where it was, and the last one
    // stays where it was while it does.
    let [_, b, c] = frame(&mut core, 1.0, &[0, 2], true);
    assert_eq!(b, Some(50.0), "the ghost in its place");
    assert_eq!(c, Some(100.0), "the room is kept the frame it goes");
    assert!(core.keeps_room(Key::ROOT.str("stack").str("t1")));
    let [_, _, c] = frame(&mut core, 1.05, &[0, 2], true);
    assert_eq!(c, Some(100.0), "and while it goes");
    // The exit over, the room goes and the last one glides up into it.
    for t in [1.11, 1.12] {
        frame(&mut core, t, &[0, 2], true);
    }
    assert!(!core.keeps_room(Key::ROOT.str("stack").str("t1")));
    let [_, b, c] = frame(&mut core, 1.13, &[0, 2], true);
    assert_eq!(b, None, "gone");
    let c = c.unwrap();
    assert!(c < 100.0 && c > 50.0, "gliding up: {c}");
    let [_, _, c] = frame(&mut core, 1.5, &[0, 2], true);
    assert_eq!(c, Some(50.0), "in its place");
    let w = core.take_warnings();
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn without_it_the_room_closes_the_frame_it_goes() {
    let mut core = Core::new();
    frame(&mut core, 1.0, &[0, 1, 2], false);
    frame(&mut core, 1.0, &[0, 1, 2], false);
    frame(&mut core, 1.0, &[0, 2], false);
    let [_, _, c] = frame(&mut core, 1.05, &[0, 2], false);
    assert!(c.unwrap() < 100.0, "already moving: {c:?}");
}

#[test]
fn a_last_child_and_a_run_keep_their_rooms_in_order() {
    let mut core = Core::new();
    frame(&mut core, 1.0, &[0, 1, 2], true);
    frame(&mut core, 1.0, &[0, 1, 2], true);
    // The first two go at once: the last stays put.
    let [a, b, c] = frame(&mut core, 1.0, &[2], true);
    assert_eq!((a, b), (Some(0.0), Some(50.0)), "both ghosts in place");
    assert_eq!(c, Some(100.0));
    // And a last child's room holds the stack's size: one shown again
    // after the others go is laid where it would be.
    let mut core = Core::new();
    frame(&mut core, 1.0, &[0, 1], true);
    frame(&mut core, 1.0, &[0, 1], true);
    frame(&mut core, 1.0, &[0], true);
    assert!(core.keeps_room(Key::ROOT.str("stack").str("t1")));
}

#[test]
fn a_node_that_comes_back_takes_its_own_place() {
    let mut core = Core::new();
    frame(&mut core, 1.0, &[0, 1, 2], true);
    frame(&mut core, 1.0, &[0, 1, 2], true);
    frame(&mut core, 1.0, &[0, 2], true);
    let [_, b, c] = frame(&mut core, 1.02, &[0, 1, 2], true);
    assert_eq!((b, c), (Some(50.0), Some(100.0)), "no room beside it");
    assert!(!core.keeps_room(Key::ROOT.str("stack").str("t1")));
}
