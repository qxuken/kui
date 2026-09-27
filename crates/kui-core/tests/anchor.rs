//! Scroll anchoring (backlog C26 step 3, CSS's `overflow-anchor`): a
//! scrolling node declared `anchor` keeps the first child in view where it
//! is on screen when the content before it changes size, with no
//! `set_scroll` and no arithmetic in the view.

use kui_core::{Color, Core, InputEvent, Key, NodeSpec, Size, Sizing, Vec2};

const VIEW_H: f32 = 100.0;

/// A list of `rows`, each `(key, height)`, in a 100-tall scroller.
fn frame(core: &mut Core, rows: &[(&str, f32)], anchor: bool) {
    let mut ui = core.frame(Size::new(200.0, VIEW_H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let spec = NodeSpec::column().fill().scroll_y();
    let spec = if anchor { spec.anchor() } else { spec };
    ui.with_keyed("list", spec, |ui| {
        for (key, h) in rows {
            ui.leaf_keyed(
                key,
                NodeSpec::row()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(*h))
                    .bg(Color::rgb8(40, 40, 60)),
            );
        }
    });
    ui.finish();
}

fn list() -> Key {
    Key::ROOT.str("list")
}

/// Where a row's box is on screen, from the inspector snapshot.
fn row_y(core: &mut Core, key: &str) -> Option<f32> {
    let k = list().str(key);
    core.nodes().iter().find(|n| n.key == k).map(|n| n.rect.y)
}

fn ten() -> Vec<(&'static str, f32)> {
    ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]
        .iter()
        .map(|k| (*k, 20.0))
        .collect()
}

fn scrolled_to(core: &mut Core, rows: &[(&str, f32)], anchor: bool, y: f32) {
    core.set_inspect(true);
    frame(core, rows, anchor);
    core.set_scroll(list(), Vec2::new(0.0, y));
    frame(core, rows, anchor);
}

/// A row prepended above the viewport moves the offset by its height, so
/// the row that was at the top is still at the top. Without `anchor` the
/// offset stays and the content slides down under it.
#[test]
fn a_prepended_row_moves_the_offset_by_its_height() {
    for anchor in [true, false] {
        let mut core = Core::new();
        scrolled_to(&mut core, &ten(), anchor, 60.0);
        assert_eq!(row_y(&mut core, "d"), Some(0.0), "d at the top");
        let mut rows = vec![("new", 35.0)];
        rows.extend(ten());
        frame(&mut core, &rows, anchor);
        if anchor {
            assert_eq!(core.scroll_offset(list()).y, 95.0);
            assert_eq!(row_y(&mut core, "d"), Some(0.0), "d stays at the top");
        } else {
            assert_eq!(core.scroll_offset(list()).y, 60.0);
            assert_eq!(row_y(&mut core, "d"), Some(35.0), "d slid down");
        }
    }
}

/// A row above the anchor changing height moves the offset by the change,
/// and one below it changes nothing — including rows appended at the end,
/// so a log that is tailing still asks for the end itself.
#[test]
fn only_content_before_the_anchor_moves_it() {
    let mut core = Core::new();
    scrolled_to(&mut core, &ten(), true, 60.0);
    let mut rows = ten();
    rows[1].1 = 50.0; // b grows by 30, above d
    rows[7].1 = 80.0; // h grows, below d
    rows.push(("k", 20.0));
    frame(&mut core, &rows, true);
    assert_eq!(core.scroll_offset(list()).y, 90.0);
    assert_eq!(row_y(&mut core, "d"), Some(0.0));
    rows[1].1 = 5.0; // b shrinks by 45
    frame(&mut core, &rows, true);
    assert_eq!(core.scroll_offset(list()).y, 45.0);
    assert_eq!(row_y(&mut core, "d"), Some(0.0));
}

/// A partly visible first row is the anchor, and it keeps its partial
/// offset: the correction is the edge's movement, not a snap to it.
#[test]
fn a_partly_visible_row_anchors_where_it_is() {
    let mut core = Core::new();
    scrolled_to(&mut core, &ten(), true, 50.0);
    assert_eq!(row_y(&mut core, "c"), Some(-10.0));
    let mut rows = vec![("new", 40.0)];
    rows.extend(ten());
    frame(&mut core, &rows, true);
    assert_eq!(core.scroll_offset(list()).y, 90.0);
    assert_eq!(row_y(&mut core, "c"), Some(-10.0));
}

/// A wheel notch between the frames is kept and the correction added to
/// it, and a `set_scroll` likewise: anchoring never overwrites a scroll.
#[test]
fn a_scroll_between_frames_survives_the_correction() {
    let mut core = Core::new();
    scrolled_to(&mut core, &ten(), true, 60.0);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(100.0, 50.0)));
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -10.0)));
    let mut rows = vec![("new", 30.0)];
    rows.extend(ten());
    frame(&mut core, &rows, true);
    assert_eq!(core.scroll_offset(list()).y, 100.0);
    assert_eq!(row_y(&mut core, "d"), Some(-10.0));
}

/// An anchor that is gone anchors nothing that frame, the correction is
/// clamped like any offset, and a row scroller anchors on x.
#[test]
fn a_missing_anchor_and_the_clamp() {
    let mut core = Core::new();
    scrolled_to(&mut core, &ten(), true, 60.0);
    // d, the anchor, is removed: no correction; the offset holds.
    let mut rows: Vec<(&str, f32)> = vec![("new", 30.0)];
    rows.extend(ten().into_iter().filter(|(k, _)| *k != "d"));
    frame(&mut core, &rows, true);
    assert_eq!(core.scroll_offset(list()).y, 60.0);
    // Everything but the last two rows removed: the correction would be
    // negative and large, and lands on zero.
    let short = vec![("i", 20.0), ("j", 20.0)];
    frame(&mut core, &short, true);
    assert_eq!(core.scroll_offset(list()).y, 0.0);

    let mut core = Core::new();
    core.set_inspect(true);
    let strip = |core: &mut Core, cols: &[(&str, f32)]| {
        let mut ui = core.frame(Size::new(100.0, 50.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed("list", NodeSpec::row().fill().scroll_x().anchor(), |ui| {
            for (key, w) in cols {
                ui.leaf_keyed(
                    key,
                    NodeSpec::row()
                        .width(Sizing::Fixed(*w))
                        .height(Sizing::Grow(1.0))
                        .bg(Color::rgb8(40, 40, 60)),
                );
            }
        });
        ui.finish();
    };
    strip(&mut core, &ten());
    core.set_scroll(list(), Vec2::new(60.0, 0.0));
    strip(&mut core, &ten());
    let mut cols = vec![("new", 25.0)];
    cols.extend(ten());
    strip(&mut core, &cols);
    assert_eq!(core.scroll_offset(list()).x, 85.0);
    let d = list().str("d");
    assert_eq!(
        core.nodes().iter().find(|n| n.key == d).map(|n| n.rect.x),
        Some(0.0)
    );
}
