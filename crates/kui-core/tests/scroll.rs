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
    let solid_rows = dl
        .quads
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid && q.rect.w > 100.0)
        .count();
    // 200/30 -> 7 rows intersect the viewport; the other 13 are culled.
    assert!(solid_rows < ROWS, "expected culling, got {solid_rows} rows");

    // A click at y=100 hits the visible row 3, not some scrolled-away row.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 100.0)));
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].payload.as_int(), Some(3));

    // After scrolling down two rows, the same cursor position hits row 5.
    wheel(&mut core, -60.0);
    frame(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 100.0)));
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
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
        ui.with(
            NodeSpec::row().height(Sizing::Fixed(50.0)).bg(Color::WHITE),
            |_| {},
        );
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

// -- The scroll API apps drive from outside -------------------------------
// `reveal` / `set_scroll` / `scroll_offset` are the wheel's moves asked for
// by name: the offsets are retained per key, and until these existed only
// the core could reach them.

fn list_key() -> kui_core::Key {
    kui_core::Key::ROOT.str("list")
}

fn row_key(i: usize) -> kui_core::Key {
    list_key().str(&format!("row{i}"))
}

/// The y a row's quad drew at, or None when it was culled.
fn row_y(core: &mut Core, i: usize) -> Option<f32> {
    // Rows are the wide solids; the i-th is at content y = i * ROW_H, so
    // identify it by the offset the frame drew it with.
    let (dl, _) = core.output();
    let ys: Vec<f32> = dl
        .quads
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid && q.rect.w > 100.0)
        .map(|q| q.rect.y)
        .collect();
    let want = i as f32 * ROW_H - core.scroll_offset(list_key()).y;
    ys.into_iter().find(|y| (*y - want).abs() < 0.01)
}

#[test]
fn reveal_scrolls_a_row_into_view_and_is_idempotent() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(core.scroll_offset(list_key()), Vec2::ZERO);

    // Row 15 sits at 450..480 in a 200-tall window: off the bottom.
    core.reveal(row_key(15));
    frame(&mut core);
    let y = row_y(&mut core, 15).expect("row 15 should draw");
    assert!(
        y >= 0.0 && y + ROW_H <= VIEW_H,
        "row 15 not inside the viewport: y={y}"
    );
    let after = core.scroll_offset(list_key());
    assert!(after.y > 0.0, "reveal moved nothing: {after:?}");

    // The frame that resolved it also cleared it: another frame drifts nowhere.
    frame(&mut core);
    assert_eq!(core.scroll_offset(list_key()), after);

    // Revealing something already visible is a no-op, not a re-alignment.
    core.reveal(row_key(15));
    frame(&mut core);
    assert_eq!(core.scroll_offset(list_key()), after);
}

#[test]
fn reveal_scrolls_back_up() {
    let mut core = Core::new();
    frame(&mut core);
    core.set_scroll(list_key(), Vec2::new(0.0, 400.0));
    frame(&mut core);
    assert_eq!(core.scroll_offset(list_key()).y, 400.0);

    core.reveal(row_key(0));
    frame(&mut core);
    assert_eq!(core.scroll_offset(list_key()), Vec2::ZERO);
    assert_eq!(row_y(&mut core, 0), Some(0.0));
}

#[test]
fn reveal_of_a_key_the_frame_does_not_declare_is_a_no_op() {
    let mut core = Core::new();
    frame(&mut core);
    let before = core.scroll_offset(list_key());

    core.reveal(kui_core::Key::ROOT.str("no-such-node"));
    frame(&mut core);
    assert_eq!(core.scroll_offset(list_key()), before);

    // And it is not remembered: a later frame that *does* declare row 15
    // still does not move, because the request was spent.
    frame(&mut core);
    assert_eq!(core.scroll_offset(list_key()), before);
}

#[test]
fn reveal_reaches_a_row_the_frame_declares_for_the_first_time() {
    // The point of resolving against the frame ahead: an app that appends a
    // row and reveals it in the same update has no last frame to find it in.
    let mut core = Core::new();
    frame(&mut core); // 20 rows

    core.reveal(row_key(39));
    let mut ui = core.frame(Size::new(400.0, VIEW_H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("list", NodeSpec::column().fill().scroll_y(), |ui| {
        for i in 0..40 {
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
    // 40 * 30 = 1200 of content in 200: the last row is flush with the end.
    assert_eq!(core.scroll_offset(list_key()).y, 1000.0);
}

#[test]
fn set_scroll_and_scroll_offset_round_trip_through_a_model() {
    let mut core = Core::new();
    frame(&mut core);
    wheel(&mut core, -150.0);
    frame(&mut core);
    let saved = core.scroll_offset(list_key());
    assert_eq!(saved, Vec2::new(0.0, 150.0));

    // A fresh core (a restarted app) restores it before the first frame.
    let mut restored = Core::new();
    restored.set_scroll(list_key(), saved);
    frame(&mut restored);
    assert_eq!(restored.scroll_offset(list_key()), saved);
    assert_eq!(row_y(&mut restored, 5), Some(0.0));
}

#[test]
fn set_scroll_is_clamped_by_the_next_layout() {
    let mut core = Core::new();
    frame(&mut core);

    // "Jump to the end" without knowing the content height.
    core.set_scroll(list_key(), Vec2::new(0.0, f32::MAX));
    frame(&mut core);
    assert_eq!(core.scroll_offset(list_key()).y, 600.0 - VIEW_H);

    // And back to the top, past it.
    core.set_scroll(list_key(), Vec2::new(0.0, -500.0));
    frame(&mut core);
    assert_eq!(core.scroll_offset(list_key()), Vec2::ZERO);
}

#[test]
fn scroll_offset_of_a_node_that_never_scrolled_is_zero() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(core.scroll_offset(row_key(0)), Vec2::ZERO);
    assert_eq!(
        core.scroll_offset(kui_core::Key::ROOT.str("gone")),
        Vec2::ZERO
    );
}

// -- Geometry and virtual lists ---------------------------------------------
// `scroll_offset` says how far a container has scrolled; on its own that is
// not enough to build only the visible rows, because the view cannot see how
// tall the container came out. `scroll_geometry` is the rest of it, and
// `widgets::virtual_column` is the two together.

/// A tall list built through the widget, in a `VIEW_H`-high window.
fn virtual_frame(core: &mut Core, rows: usize) -> usize {
    let mut built = 0usize;
    let mut ui = core.frame(Size::new(400.0, VIEW_H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    kui_core::widgets::virtual_column(
        &mut ui,
        "list",
        NodeSpec::column().fill(),
        rows,
        ROW_H,
        |ui, i| {
            built += 1;
            ui.with(
                NodeSpec::row()
                    .fill()
                    .bg(Color::rgb8(40, 40, 60))
                    .on_click(Value::Int(i as i64)),
                |_| {},
            );
        },
    );
    ui.finish();
    built
}

/// The geometry a view reads is the box and content the last layout
/// resolved — the rect an `on_layout` would have posted, without the round
/// trip, and the content height the scrollbar is drawn from.
#[test]
fn geometry_reports_the_container_box_and_its_content() {
    let mut core = Core::new();
    assert_eq!(core.scroll_geometry(list_key()), None);

    frame(&mut core);
    let g = core.scroll_geometry(list_key()).expect("laid out");
    assert_eq!(g.rect, kui_core::Rect::new(0.0, 0.0, 400.0, VIEW_H));
    assert_eq!(g.content.h, ROWS as f32 * ROW_H);
    assert_eq!(g.offset, Vec2::ZERO);
    // Content minus viewport, which is exactly how far the wheel may go.
    assert_eq!(g.max_offset.y, ROWS as f32 * ROW_H - VIEW_H);

    wheel(&mut core, -10_000.0);
    frame(&mut core);
    let g = core.scroll_geometry(list_key()).expect("laid out");
    assert_eq!(g.offset, g.max_offset);
}

/// Writing an offset at a key does not invent a container: the geometry
/// stays `None` until a layout resolves that key as one.
#[test]
fn geometry_is_none_for_a_node_that_is_not_a_scroll_container() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(core.scroll_geometry(row_key(3)), None);

    core.set_scroll(row_key(3), Vec2::new(0.0, 50.0));
    frame(&mut core);
    assert_eq!(core.scroll_geometry(row_key(3)), None);
    assert_eq!(core.scroll_geometry(kui_core::Key::ROOT.str("gone")), None);
}

/// The geometry is one coherent moment: `scroll_offset` hands back whatever
/// was written, but the geometry's offset is already clamped to the travel
/// the last layout found — so a view that slices by it cannot be handed the
/// "huge value means the end" idiom as a row index.
#[test]
fn geometry_offset_is_clamped_where_the_raw_one_is_not() {
    let mut core = Core::new();
    frame(&mut core);
    core.set_scroll(list_key(), Vec2::new(0.0, 1e9));

    assert_eq!(core.scroll_offset(list_key()).y, 1e9);
    let g = core.scroll_geometry(list_key()).expect("laid out");
    assert_eq!(g.offset.y, ROWS as f32 * ROW_H - VIEW_H);
    assert_eq!(g.offset, g.max_offset);

    // And the next layout agrees with what the geometry predicted.
    frame(&mut core);
    assert_eq!(core.scroll_offset(list_key()).y, g.offset.y);
}

/// An axis that does not scroll has no travel, however far its content
/// overflows — the list scrolls in y only.
#[test]
fn max_offset_is_zero_on_an_axis_that_does_not_scroll() {
    let mut core = Core::new();
    frame(&mut core);
    let g = core.scroll_geometry(list_key()).expect("laid out");
    assert_eq!(g.max_offset.x, 0.0);
    assert_eq!(g.max_offset.y, ROWS as f32 * ROW_H - VIEW_H);
}

/// The point of the whole exercise: a list of 10k rows builds a screenful.
#[test]
fn a_virtual_column_builds_only_what_shows() {
    let mut core = Core::new();
    // First frame has no geometry and slices by the viewport instead.
    let first = virtual_frame(&mut core, 10_000);
    assert!(first < 20, "first frame built {first} rows");
    // Steady state: VIEW_H / ROW_H visible, plus two rows of overscan each
    // side (the leading side is clipped away at offset 0).
    let n = virtual_frame(&mut core, 10_000);
    assert_eq!(n, (VIEW_H / ROW_H).ceil() as usize + 2);
}

/// Scrolling slides the built range without changing its size, and the rows
/// it builds are the ones under the window.
#[test]
fn the_built_range_follows_the_offset() {
    let mut core = Core::new();
    virtual_frame(&mut core, 10_000);
    let steady = virtual_frame(&mut core, 10_000);

    core.set_scroll(list_key(), Vec2::new(0.0, 300.0 * ROW_H));
    virtual_frame(&mut core, 10_000);
    let n = virtual_frame(&mut core, 10_000);
    // Two rows of overscan on each side now that there is room above.
    assert_eq!(n, steady + 2);

    // Row 300 is at the top of the window; row 0 and row 9_999 are not built,
    // so nothing but the spacers stands in for them.
    let (dl, _) = core.output();
    let rows = dl
        .quads
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid && q.rect.w > 100.0)
        .count();
    // Two overscan rows above the window and two below it are built but
    // clipped away, so they cost a node and no quad.
    assert_eq!(rows, n - 4);
}

/// A virtualized list is the same list: the content height, and so the
/// scrollbar and the end of the travel, match a list that builds every row.
#[test]
fn a_virtual_column_scrolls_like_a_full_one() {
    let mut core = Core::new();
    virtual_frame(&mut core, 1_000);
    virtual_frame(&mut core, 1_000);
    let g = core.scroll_geometry(list_key()).expect("laid out");
    assert_eq!(g.content.h, 1_000.0 * ROW_H);
    assert_eq!(g.max_offset.y, 1_000.0 * ROW_H - VIEW_H);

    // "Jump to the end" with no content height in the app still lands on the
    // last row, because the trailing spacer holds the space of the rows the
    // frame did not build.
    core.set_scroll(list_key(), Vec2::new(0.0, f32::MAX));
    virtual_frame(&mut core, 1_000);
    virtual_frame(&mut core, 1_000);
    assert_eq!(core.scroll_offset(list_key()).y, 1_000.0 * ROW_H - VIEW_H);
}

/// Rows are keyed by data index, not by the slot they land in, so the row
/// under the pointer keeps its identity as the built range slides. This is
/// what would have to be re-solved inside the core to virtualize there.
#[test]
fn virtual_rows_keep_their_key_as_the_range_slides() {
    let mut core = Core::new();
    virtual_frame(&mut core, 10_000);
    virtual_frame(&mut core, 10_000);
    let key_at_top = list_key().index(0);

    core.set_scroll(list_key(), Vec2::new(0.0, 500.0 * ROW_H));
    virtual_frame(&mut core, 10_000);
    virtual_frame(&mut core, 10_000);
    // Row 0's key is unchanged even though row 0 is no longer built, and it
    // is not the key of whatever row now sits in the first slot.
    assert_eq!(key_at_top, list_key().index(0));
    assert_ne!(key_at_top, list_key().index(500));
}

/// The spacers stand outside the index namespace the rows use, so the row at
/// index 0 and the spacer beside it are two nodes, not a duplicate key.
#[test]
fn spacers_do_not_collide_with_row_keys() {
    let mut core = Core::new();
    virtual_frame(&mut core, 10_000);
    core.set_scroll(list_key(), Vec2::new(0.0, 300.0 * ROW_H));
    virtual_frame(&mut core, 10_000);
    virtual_frame(&mut core, 10_000);
    let dup = core
        .take_warnings()
        .iter()
        .any(|w| w.code == "duplicate-key");
    assert!(!dup, "virtual rows and spacers collided");
}

/// The slice arithmetic on its own, for views that build their own container.
#[test]
fn visible_rows_covers_the_band_and_no_more() {
    use kui_core::widgets::visible_rows;
    // 200px window over 30px rows at the top: rows 0..7 cross it.
    assert_eq!(visible_rows(0.0, 200.0, 0.0, 30.0, 10_000, 0), 0..7);
    // Scrolled to row 100 exactly.
    assert_eq!(visible_rows(3000.0, 200.0, 0.0, 30.0, 10_000, 0), 100..107);
    // Overscan widens both ends, clamped to the list.
    assert_eq!(visible_rows(3000.0, 200.0, 0.0, 30.0, 10_000, 2), 98..109);
    assert_eq!(visible_rows(0.0, 200.0, 0.0, 30.0, 10_000, 2), 0..9);
    // Padding above the first row shifts the band down the flow.
    assert_eq!(visible_rows(3000.0, 200.0, 60.0, 30.0, 10_000, 0), 98..105);
    // Degenerate inputs answer with an empty range rather than a panic.
    assert_eq!(visible_rows(0.0, 200.0, 0.0, 30.0, 0, 2), 0..0);
    assert_eq!(visible_rows(0.0, 200.0, 0.0, 0.0, 10, 2), 0..0);
    // A list shorter than the window builds all of it, never past the end.
    assert_eq!(visible_rows(0.0, 200.0, 0.0, 30.0, 3, 2), 0..3);
}
