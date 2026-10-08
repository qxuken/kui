//! The view reads the frame clock (backlog F134): `Ui::now` / `Core::now`
//! is the driver's seconds the transitions read, so an app's deadlines and
//! the core's easing agree, and a test that moves the clock moves both.

use kui_core::{Core, NodeSpec, Size};

#[test]
fn the_view_reads_the_clock_the_driver_set_and_zero_before_one() {
    let mut core = Core::new();
    let mut seen = Vec::new();
    for now in [None, Some(3.25), Some(3.5)] {
        if let Some(t) = now {
            core.set_time(t);
        }
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        seen.push(ui.now());
        ui.leaf(NodeSpec::column().size(10.0, 10.0));
        ui.finish();
    }
    assert_eq!(seen, vec![0.0, 3.25, 3.5]);
    assert_eq!(core.now(), 3.5, "the core reads it between frames too");
    assert_eq!(core.env_facts().now, 3.5, "and the env reading carries it");
}

/// A toast due at a time on the frame clock: the view decides from
/// `ui.now()`, and moving the clock is all a test needs.
#[test]
fn a_deadline_read_off_the_frame_clock_moves_with_it() {
    let mut core = Core::new();
    let until = 2.0;
    let mut frame = |core: &mut Core, now: f64| {
        core.set_time(now);
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        let shown = ui.now() < until;
        if shown {
            ui.leaf_keyed("toast", NodeSpec::column().size(10.0, 10.0));
        }
        ui.finish();
        shown
    };
    assert!(frame(&mut core, 1.0));
    assert!(frame(&mut core, 1.9));
    assert!(
        !frame(&mut core, 2.0),
        "due at two seconds of the frame clock"
    );
}
