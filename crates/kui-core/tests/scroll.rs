//! End-to-end scrolling through a live `Core`: wheel routing, offset clamping,
//! content movement, clipping of draw + hit regions, scrollbar emission.

use kui_core::{Color, Core, InputEvent, NodeSpec, Size, Sizing, Value, Vec2};

const VIEW_H: f32 = 200.0;
const ROWS: usize = 20;
const ROW_H: f32 = 30.0; // 20 * 30 = 600 content in a 200 viewport window

fn frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(400.0, VIEW_H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("list", NodeSpec::column().fill().scroll_y(), |ui| {
        for i in 0..ROWS {
            ui.with_keyed(
                &format!("row{i}"),
                NodeSpec::row()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(ROW_H))
                    .bg(Color::rgb8(40, 40, 60))
                    .on_click(Value::Int(i as i64)),
                |_| {},
            );
        }
    });
    ui.finish();
}

/// Min/max y over emitted row quads (fully clipped rows are culled, so this
/// tracks what actually draws). Rows are the wide solid quads; the scrollbar
/// is only 4px wide.
fn row_quad_ys(core: &mut Core) -> (f32, f32) {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid && q.rect.w > 100.0)
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), q| {
            (lo.min(q.rect.y), hi.max(q.rect.y))
        })
}

fn wheel(core: &mut Core, dy: f32) {
    // Cursor over the list, then scroll (negative dy = wheel down).
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 100.0)));
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, dy)));
}

#[test]
fn wheel_scrolls_content_and_clamps() {
    let mut core = Core::new();
    frame(&mut core); // establish scroll regions
    assert_eq!(row_quad_ys(&mut core).0, 0.0);

    wheel(&mut core, -50.0); // scroll down 50: row 0 (y=-50) culls, row 1 shows at -20
    frame(&mut core);
    assert_eq!(row_quad_ys(&mut core).0, -20.0);

    wheel(&mut core, -10_000.0); // way past the end: clamps to max (400)
    frame(&mut core);
    // Last row sits flush with the bottom: y = 600 - 400 - 30 padding... = 170.
    assert_eq!(row_quad_ys(&mut core).1, VIEW_H - ROW_H);

    wheel(&mut core, 20_000.0); // way back up: clamps to 0
    frame(&mut core);
    assert_eq!(row_quad_ys(&mut core).0, 0.0);
}

#[test]
fn clipped_away_rows_are_culled_and_unclickable() {
    let mut core = Core::new();
    frame(&mut core);
    let (dl, _) = core.output();
    let solid_rows =
        dl.quads.iter().filter(|q| q.kind == kui_core::QuadKind::Solid && q.rect.w > 100.0).count();
    // 200/30 -> 7 rows intersect the viewport; the other 13 are culled.
    assert!(solid_rows < ROWS, "expected culling, got {solid_rows} rows");

    // A click at y=100 hits the visible row 3, not some scrolled-away row.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 100.0)));
    core.handle_input(InputEvent::MouseDown);
    let evs = core.handle_input(InputEvent::MouseUp);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].payload.as_int(), Some(3));

    // After scrolling down two rows, the same cursor position hits row 5.
    wheel(&mut core, -60.0);
    frame(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 100.0)));
    core.handle_input(InputEvent::MouseDown);
    let evs = core.handle_input(InputEvent::MouseUp);
    assert_eq!(evs[0].payload.as_int(), Some(5));
}

#[test]
fn scrollbar_appears_only_when_overflowing() {
    let mut core = Core::new();
    frame(&mut core);
    let (dl, _) = core.output();
    let bars = dl.quads.iter().filter(|q| q.rect.w == 4.0).count();
    assert_eq!(bars, 1, "expected one vertical scrollbar");

    // A short list needs no scrollbar.
    let mut ui = core.frame(Size::new(400.0, VIEW_H), 1.0);
    ui.with_keyed("list", NodeSpec::column().fill().scroll_y(), |ui| {
        ui.with(NodeSpec::row().height(Sizing::Fixed(50.0)).bg(Color::WHITE), |_| {});
    });
    ui.finish();
    let (dl, _) = core.output();
    assert_eq!(dl.quads.iter().filter(|q| q.rect.w == 4.0).count(), 0);
}

#[test]
fn scroll_offset_survives_and_reclamps_on_content_shrink() {
    let mut core = Core::new();
    frame(&mut core);
    wheel(&mut core, -10_000.0);
    frame(&mut core); // clamped to 400

    // Rebuild with half the rows: max becomes 10*30-200 = 100.
    let mut ui = core.frame(Size::new(400.0, VIEW_H), 1.0);
    ui.with_keyed("list", NodeSpec::column().fill().scroll_y(), |ui| {
        for i in 0..10 {
            ui.with_keyed(
                &format!("row{i}"),
                NodeSpec::row()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(ROW_H))
                    .bg(Color::rgb8(40, 40, 60)),
                |_| {},
            );
        }
    });
    ui.finish();
    // Offset re-clamped from 400 to 100: rows sit at -100 + i*30, and the
    // first one that still intersects the viewport draws at -10 (row 3).
    assert_eq!(row_quad_ys(&mut core).0, -10.0);
}
