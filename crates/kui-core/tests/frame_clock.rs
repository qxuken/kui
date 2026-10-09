//! The view reads the frame clock (backlog F134): `Ui::now` / `Core::now`
//! is the driver's seconds the transitions read, so an app's deadlines and
//! the core's easing agree, and a test that moves the clock moves both.
//! `request_frame_at` asks for a frame at a time on that clock (backlog
//! F135): a deadline a driver sleeps to, with nothing owed until then.

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
    let frame = |core: &mut Core, now: f64| {
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

fn frame_at(core: &mut Core, now: f64, ask: Option<f64>) {
    core.set_time(now);
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    if let Some(at) = ask {
        ui.request_frame_at(at);
    }
    ui.finish();
}

#[test]
fn a_frame_asked_at_a_time_is_a_deadline_and_owes_nothing_until_then() {
    let mut core = Core::new();
    frame_at(&mut core, 1.0, Some(4.0));
    assert_eq!(core.next_frame_at(), Some(4.0));
    assert!(!core.animating(), "nothing is owed before the time");
    // A frame for some other reason does not spend it.
    frame_at(&mut core, 2.0, None);
    assert_eq!(core.next_frame_at(), Some(4.0), "kept without asking again");
    // An earlier time wins; a later one does not move it.
    frame_at(&mut core, 2.5, Some(3.0));
    frame_at(&mut core, 2.6, Some(9.0));
    assert_eq!(core.next_frame_at(), Some(3.0));
    // The frame at or past it is that frame.
    frame_at(&mut core, 3.0, None);
    assert_eq!(core.next_frame_at(), None);
}

#[test]
fn a_time_already_past_or_with_no_clock_is_a_frame_now() {
    let mut core = Core::new();
    frame_at(&mut core, 5.0, Some(4.0));
    assert_eq!(core.next_frame_at(), None);
    assert!(core.animating(), "a past time is request_frame");
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.request_frame_at(10.0);
    ui.finish();
    assert!(
        core.animating(),
        "with no clock there is no later to wait for"
    );
    let mut core = Core::new();
    frame_at(&mut core, 1.0, Some(f64::NAN));
    assert_eq!(core.next_frame_at(), None, "not a number is ignored");
    assert!(!core.animating());
    frame_at(&mut core, 1.0, Some(f64::INFINITY));
    assert_eq!(core.next_frame_at(), None, "nor is a time that never comes");
}

/// A handler between frames asks too: the toast an event shows, due later.
#[test]
fn a_handler_between_frames_asks_for_a_frame_at_a_time() {
    let mut core = Core::new();
    frame_at(&mut core, 1.0, None);
    core.request_frame_at(2.5);
    assert_eq!(core.next_frame_at(), Some(2.5));
}
