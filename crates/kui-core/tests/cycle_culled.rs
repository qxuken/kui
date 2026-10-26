//! A keyframe cycle whose node the frame culled owes no frame (backlog
//! C137): a pulse scrolled out of its scroller stops asking for frames at
//! the display's rate, and is sampled again — and asks again — the frame
//! that brings it back.

use kui_core::{Color, Core, Key, Keyframe, NodeSpec, Size, Sizing, Transition, Vec2};

/// A 100 px scroller holding `above` px of rows, then a 20 px node whose
/// keyframes are `frames`, then more rows. Returns the scroller's key.
fn frame(core: &mut Core, now: f64, above: f32, frames: Vec<Keyframe>) -> Key {
    core.set_time(now);
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let page = ui.with_keyed(
        "page",
        NodeSpec::column()
            .width(Sizing::Fixed(200.0))
            .height(Sizing::Fixed(100.0))
            .scroll_y(),
        |ui| {
            ui.leaf(NodeSpec::column().size(10.0, above));
            ui.leaf_keyed(
                "pulse",
                NodeSpec::column()
                    .size(20.0, 20.0)
                    .bg(Color::WHITE)
                    .transition_with(Transition::ms(800.0))
                    .keyframes(frames),
            );
            ui.leaf(NodeSpec::column().size(10.0, 1000.0));
        },
    );
    ui.finish();
    page
}

fn pulse() -> Vec<Keyframe> {
    vec![Keyframe::default().at(0.5).bg(Color::hex(0xd43b3bff))]
}

#[test]
fn a_pulse_in_view_owes_frames() {
    let mut core = Core::new();
    frame(&mut core, 0.0, 10.0, pulse());
    assert!(core.owed().cycle, "a cycle on screen runs");
}

#[test]
fn a_pulse_scrolled_out_owes_none_and_runs_again_when_it_comes_back() {
    let mut core = Core::new();
    let page = frame(&mut core, 0.0, 300.0, pulse());
    assert!(!core.owed().cycle, "out of the scroller: no frame owed");
    assert!(!core.animating());
    core.set_scroll(page, Vec2::new(0.0, 280.0));
    frame(&mut core, 0.1, 300.0, pulse());
    frame(&mut core, 0.2, 300.0, pulse());
    assert!(core.owed().cycle, "scrolled back into view, it runs");
}

#[test]
fn a_cycle_that_moves_its_node_counts_where_its_stops_reach() {
    // Just below the scroller's edge, but its stops carry it 40 px up.
    let mut core = Core::new();
    frame(
        &mut core,
        0.0,
        110.0,
        vec![Keyframe::default().at(0.5).dy(-40.0)],
    );
    assert!(core.owed().cycle, "its stops bring it into view");
}

#[test]
fn a_cycle_of_a_size_always_counts() {
    // Laid out above what is on screen, a height that cycles moves what
    // is drawn below it.
    let mut core = Core::new();
    frame(
        &mut core,
        0.0,
        300.0,
        vec![Keyframe::default().at(0.5).height(Sizing::Fixed(40.0))],
    );
    assert!(core.owed().cycle);
}
