//! An `exit` plays when its node is removed, not when an ancestor is
//! (backlog DX19). kawoosh dropped `exit` from its columns because the fade
//! a closing column should play also played on every tab switch, which
//! removes the whole strip.

use kui_core::{Color, Core, Enter, NodeSpec, QuadKind, Size};

const VIEW: Size = Size { w: 300.0, h: 100.0 };

/// A strip (no exit) holding a column that fades out: `strip` false drops
/// the whole strip, `column` false drops only the column.
fn frame(core: &mut Core, t: f64, strip: bool, column: bool) -> usize {
    core.set_time(t);
    let mut ui = core.frame(VIEW, 1.0);
    if strip {
        ui.with_keyed("strip", NodeSpec::row().size(300.0, 100.0), |ui| {
            if column {
                ui.leaf_keyed(
                    "column",
                    NodeSpec::column()
                        .size(100.0, 100.0)
                        .bg(Color::WHITE)
                        .transition(200.0)
                        .exit(Enter::default().opacity(0.0)),
                );
            }
        });
    }
    ui.finish();
    core.output()
        .0
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid)
        .count()
}

#[test]
fn a_column_closed_fades_and_one_whose_strip_went_does_not() {
    let mut core = Core::new();
    assert_eq!(frame(&mut core, 0.0, true, true), 1);
    frame(&mut core, 0.1, true, true);
    assert_eq!(
        frame(&mut core, 0.2, true, false),
        1,
        "closed alone: its ghost fades"
    );

    let mut core = Core::new();
    assert_eq!(frame(&mut core, 0.0, true, true), 1);
    frame(&mut core, 0.1, true, true);
    assert_eq!(
        frame(&mut core, 0.2, false, false),
        0,
        "the strip went, and the column with it, at once"
    );
}
