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

// -- Variable-height virtual lists ------------------------------------------
// `virtual_column` takes one stride; these are the rows that have none. The
// heights come from a `measure` callback the widget runs for the rows it is
// about to build, everything else stands at the mean of what has been
// measured, and the prefix sums over both are what the spacers and the search
// are made of.

use kui_core::testing::click;
use kui_core::widgets::RowHeights;

const VAR_ROWS: usize = 1_000;

/// Row `i`'s true height. Short at the top and tall for the rest, so the
/// estimate the first screenful produces is badly wrong for the middle of
/// the list — which is what makes the anchoring observable.
fn var_h(i: usize) -> f32 {
    if i < 100 { 20.0 } else { 60.0 }
}

/// One frame of a variable-height list, returning what it built.
fn var_frame(core: &mut Core, heights: &mut RowHeights) -> std::ops::Range<usize> {
    let (mut first, mut last) = (usize::MAX, 0usize);
    let mut ui = core.frame(Size::new(400.0, VIEW_H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    kui_core::widgets::virtual_rows(
        &mut ui,
        "list",
        NodeSpec::column().fill(),
        heights,
        |_ui, i, _w| var_h(i),
        |ui, i| {
            first = first.min(i);
            last = i + 1;
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
    if last == 0 { 0..0 } else { first..last }
}

/// Which row is under the top edge of the window, by hit-testing rather than
/// by arithmetic — the question "did the content move" asks of the pixels.
fn row_under(core: &mut Core, y: f32) -> Option<i64> {
    hit_under(core, y).map(|(i, _)| i)
}

/// The same hit, with the key of the node that took it — the row wrapper's
/// own first child, so the wrapper's key is one step up.
fn hit_under(core: &mut Core, y: f32) -> Option<(i64, kui_core::Key)> {
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, y)));
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
    let e = evs.first()?;
    Some((e.payload.as_int()?, e.key))
}

#[test]
fn row_heights_answer_the_three_questions_a_stride_answered() {
    let mut h = RowHeights::new(10, 20.0);
    // Nothing measured: every row stands at the seed.
    assert_eq!(h.estimate(), 20.0);
    assert_eq!(h.total(), 200.0);
    assert_eq!(h.offset_of(3), 60.0);
    assert_eq!(h.row_at(0.0), 0);
    assert_eq!(h.row_at(65.0), 3);

    // Measure three rows: they take their own height and the estimate
    // becomes their mean, which moves every row that has none.
    h.set(0, 50.0);
    h.set(1, 50.0);
    h.set(2, 50.0);
    assert_eq!(h.measured(0), Some(50.0));
    assert_eq!(h.measured(9), None);
    assert_eq!(h.estimate(), 50.0);
    assert_eq!(h.get(9), 50.0);
    assert_eq!(h.total(), 500.0);
    assert_eq!(h.offset_of(3), 150.0);
    assert_eq!(h.row_at(149.0), 2);
    assert_eq!(h.row_at(150.0), 3);
    // Past the end clamps to the last row rather than panicking.
    assert_eq!(h.row_at(1e9), 9);

    // Appending keeps what is measured; clearing keeps only the length.
    h.set_len(12);
    assert_eq!(h.measured(0), Some(50.0));
    assert_eq!(h.total(), 600.0);
    h.clear();
    assert_eq!(h.measured(0), None);
    assert_eq!(h.estimate(), 20.0, "back to the seed with nothing measured");
    assert_eq!(h.len(), 12);
}

/// A width that differs from the one the heights were measured at rewraps
/// every row, so the cache is void — the frame after a resize measures again.
#[test]
fn a_new_width_drops_every_measurement() {
    let mut h = RowHeights::new(10, 20.0);
    assert!(h.set_width(300.0));
    h.set(0, 55.0);
    assert!(!h.set_width(300.0), "the same width changes nothing");
    assert_eq!(h.measured(0), Some(55.0));
    assert!(h.set_width(200.0));
    assert_eq!(h.measured(0), None);
    assert_eq!(h.width(), 200.0);
}

/// The point of the exercise, without a stride: a thousand rows of three
/// different heights build a screenful.
#[test]
fn a_variable_list_builds_only_what_shows() {
    let mut core = Core::new();
    let mut h = RowHeights::new(VAR_ROWS, 20.0);
    var_frame(&mut core, &mut h);
    let built = var_frame(&mut core, &mut h);
    // 200px of window over 20px rows at the top, plus two of overscan.
    assert!(built.len() <= 14, "built {built:?}");
    assert_eq!(built.start, 0);
    // Only what was built has been measured; the rest stands at the mean.
    assert_eq!(h.measured(built.end - 1), Some(20.0));
    assert_eq!(h.measured(built.end + 50), None);
}

/// Each row is exactly as tall as it measured, so the arithmetic above and
/// below it cannot disagree with the layout the way an estimate would.
#[test]
fn a_row_is_as_tall_as_it_measured() {
    let mut core = Core::new();
    let mut h = RowHeights::new(VAR_ROWS, 20.0);
    var_frame(&mut core, &mut h);
    var_frame(&mut core, &mut h);
    let (dl, _) = core.output();
    let mut rows: Vec<f32> = dl
        .quads
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid && q.rect.w > 100.0)
        .map(|q| q.rect.h)
        .collect();
    rows.dedup();
    assert_eq!(rows, vec![20.0], "every visible row is its measured height");
}

/// The content height — the scrollbar, the travel and "jump to the end" —
/// is what has been measured plus what is estimated, and it converges on the
/// truth as the list is scrolled through.
#[test]
fn the_content_height_is_the_measured_and_the_estimated_together() {
    let mut core = Core::new();
    let mut h = RowHeights::new(VAR_ROWS, 20.0);
    var_frame(&mut core, &mut h);
    var_frame(&mut core, &mut h);
    let g = core.scroll_geometry(list_key()).expect("laid out");
    // Everything estimated at 20 (the seed, and the mean of the short rows
    // the first screenful measured).
    assert_eq!(g.content.h, VAR_ROWS as f32 * 20.0);
    assert_eq!(h.total(), VAR_ROWS as f32 * 20.0);

    // Scroll into the tall half: the rows built there are measured at 60,
    // the mean rises, and the list learns it is longer than it looked.
    core.set_scroll(list_key(), Vec2::new(0.0, 5_000.0));
    for _ in 0..4 {
        var_frame(&mut core, &mut h);
    }
    assert!(
        h.total() > VAR_ROWS as f32 * 20.0 * 1.5,
        "total {} did not grow with what was measured",
        h.total()
    );
}

/// The crux. Measuring the rows a frame builds changes the height of every
/// row it does not — including the ones above the window — so without a
/// correction the content slides out from under the pointer on the frame the
/// list learns anything. The row under the top edge has to stay the row
/// under the top edge.
#[test]
fn the_row_under_the_pointer_stays_there_while_the_estimate_moves() {
    let mut core = Core::new();
    let mut h = RowHeights::new(VAR_ROWS, 20.0);
    var_frame(&mut core, &mut h);
    var_frame(&mut core, &mut h);

    // Jump into the tall half. The estimate is still 20, so this lands
    // around row 250 by the arithmetic of the moment; which row does not
    // matter, only that it is the same one on every frame after.
    core.set_scroll(list_key(), Vec2::new(0.0, 5_000.0));
    var_frame(&mut core, &mut h);
    let settled = row_under(&mut core, 4.0);
    assert!(settled.is_some(), "nothing under the top edge");

    // Four more frames, each measuring more of the list and moving the mean
    // under everything above the window.
    for n in 0..4 {
        var_frame(&mut core, &mut h);
        assert_eq!(
            row_under(&mut core, 4.0),
            settled,
            "the content slid on frame {n} as the estimate moved",
        );
    }
}

/// "Scroll to row `i`" is the prefix sum, and it lands the row at the top —
/// exactly for a row whose height is known, and within a frame or two for
/// one standing at the estimate.
#[test]
fn set_scroll_to_a_rows_offset_lands_it_at_the_top() {
    let mut core = Core::new();
    let mut h = RowHeights::new(VAR_ROWS, 20.0);
    var_frame(&mut core, &mut h);
    var_frame(&mut core, &mut h);

    let target = 400usize;
    core.set_scroll(list_key(), Vec2::new(0.0, h.offset_of(target)));
    // Two frames: the first builds and measures around the target, the
    // second is drawn from what it learned.
    var_frame(&mut core, &mut h);
    var_frame(&mut core, &mut h);
    assert_eq!(row_under(&mut core, 4.0), Some(target as i64));
}

/// Tailing a log: a list told to stay at the end stays at the end while it
/// learns how long it is. The widget's own correction keeps the *rows* still
/// (which is what a reader who scrolled there wants), so "the end" is the
/// app asking for it every frame — one `set_scroll` after the widget, the
/// last write of the frame.
#[test]
fn a_list_that_asks_for_the_end_every_frame_converges_on_it() {
    let mut core = Core::new();
    let mut h = RowHeights::new(VAR_ROWS, 20.0);
    for _ in 0..8 {
        var_frame(&mut core, &mut h);
        core.set_scroll(list_key(), Vec2::new(0.0, f32::MAX));
    }
    let built = var_frame(&mut core, &mut h);
    assert_eq!(built.end, VAR_ROWS, "the last row is built at the end");
    let g = core.scroll_geometry(list_key()).expect("laid out");
    assert_eq!(g.offset, g.max_offset);
}

/// Left to itself the widget keeps the *rows* still rather than the offset,
/// so a list that was scrolled to the end of an estimate is no longer at the
/// end once the estimate grows — it is still showing what it was showing.
#[test]
fn a_list_left_alone_keeps_its_rows_not_its_place_in_the_travel() {
    let mut core = Core::new();
    let mut h = RowHeights::new(VAR_ROWS, 20.0);
    var_frame(&mut core, &mut h);
    var_frame(&mut core, &mut h);
    core.set_scroll(list_key(), Vec2::new(0.0, f32::MAX));
    var_frame(&mut core, &mut h);
    let at = row_under(&mut core, 4.0);
    for _ in 0..4 {
        var_frame(&mut core, &mut h);
    }
    assert_eq!(row_under(&mut core, 4.0), at);
    let g = core.scroll_geometry(list_key()).expect("laid out");
    assert!(g.offset.y < g.max_offset.y, "the travel grew under it");
}

/// The same list, and the same identity rule as the uniform one: a row is
/// keyed by its data index, so the built range sliding over it changes
/// nothing it retains.
#[test]
fn variable_rows_keep_their_key_as_the_range_slides() {
    let mut core = Core::new();
    let mut h = RowHeights::new(VAR_ROWS, 20.0);
    var_frame(&mut core, &mut h);
    var_frame(&mut core, &mut h);

    core.set_scroll(list_key(), Vec2::new(0.0, 3_000.0));
    var_frame(&mut core, &mut h);
    let built = var_frame(&mut core, &mut h);
    assert!(built.start > 0, "scrolled past the top: {built:?}");

    // The row under the top edge is the one the arithmetic says, and the
    // node that took the click is that row wrapper's own child — so the
    // wrapper is keyed by the *data* index, which is the key auto-keying
    // would have given it in a list that built every row.
    let (row, key) = hit_under(&mut core, 4.0).expect("a row under the edge");
    assert_eq!(key, list_key().index(row as u64).index(0));
    assert!(row as usize > built.start, "overscan sits above the window");
}

/// The bar quads of the last frame: (width, colour) of every solid that
/// is not a row.
fn bars(core: &mut Core) -> Vec<(f32, Color)> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid && q.rect.w < 100.0)
        .map(|q| (q.rect.w, q.color))
        .collect()
}

/// The list frame with a bar style on the scroller.
fn styled(core: &mut Core, style: impl Fn(NodeSpec) -> NodeSpec) {
    let mut ui = core.frame(Size::new(400.0, VIEW_H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("list", style(NodeSpec::column().fill().scroll_y()), |ui| {
        for i in 0..ROWS {
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
}

#[test]
fn a_hidden_scrollbar_draws_nothing_and_takes_no_press() {
    use kui_core::ScrollbarMode;
    let mut core = Core::new();
    styled(&mut core, |s| s.scrollbar(ScrollbarMode::Hidden));
    assert!(bars(&mut core).is_empty(), "no thumb");
    let list = core.key_of("list").unwrap();
    // The track a visible bar would have: a press there is content now.
    click(&mut core, Vec2::new(396.0, 190.0));
    styled(&mut core, |s| s.scrollbar(ScrollbarMode::Hidden));
    assert_eq!(core.scroll_offset(list).y, 0.0, "nothing to jump");
    // Scrolling itself is untouched.
    wheel(&mut core, -50.0);
    styled(&mut core, |s| s.scrollbar(ScrollbarMode::Hidden));
    assert_eq!(core.scroll_offset(list).y, 50.0);
    assert!(bars(&mut core).is_empty());
}

#[test]
fn a_scrollbar_takes_its_width_and_colours_from_the_node() {
    const REST: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    const ACTIVE: Color = Color {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    let style = |s: NodeSpec| {
        s.scrollbar_width(8.0)
            .scrollbar_color(REST)
            .scrollbar_active_color(ACTIVE)
    };
    let mut core = Core::new();
    styled(&mut core, style);
    assert_eq!(bars(&mut core), vec![(8.0, REST)]);
    // On the track: two wider, in the active colour. The track grew to
    // hold the wide thumb, so a pointer 11 px in is still on it.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(389.0, 100.0)));
    styled(&mut core, style);
    assert_eq!(bars(&mut core), vec![(10.0, ACTIVE)]);
    // And the press there is the track's: below the thumb, so it pages.
    let list = core.key_of("list").unwrap();
    click(&mut core, Vec2::new(389.0, 190.0));
    styled(&mut core, style);
    assert!(core.scroll_offset(list).y > 0.0);
}

#[test]
fn an_auto_scrollbar_fades_out_when_the_scroll_state_is_quiet() {
    use kui_core::ScrollbarMode;
    let auto = |s: NodeSpec| s.scrollbar(ScrollbarMode::Auto);
    let mut core = Core::new();
    core.set_time(0.0);
    styled(&mut core, auto);
    let first = bars(&mut core);
    assert_eq!(first.len(), 1, "first seen: shown");
    assert!(core.animating(), "the hold is running");
    // Inside the hold: full.
    core.set_time(0.9);
    styled(&mut core, auto);
    assert_eq!(bars(&mut core)[0].1.a, first[0].1.a);
    // Mid-fade: dimmer.
    core.set_time(1.125);
    styled(&mut core, auto);
    let mid = bars(&mut core);
    assert_eq!(mid.len(), 1);
    assert!((mid[0].1.a - first[0].1.a * 0.5).abs() < 1e-3, "{:?}", mid);
    assert!(core.animating());
    // Gone, and nothing owed.
    core.set_time(1.3);
    styled(&mut core, auto);
    assert!(bars(&mut core).is_empty());
    assert!(!core.animating());
    // A press where the track was is content now.
    let list = core.key_of("list").unwrap();
    click(&mut core, Vec2::new(396.0, 190.0));
    core.set_time(1.31);
    styled(&mut core, auto);
    assert_eq!(core.scroll_offset(list).y, 0.0);
    // A wheel brings it back for another second.
    wheel(&mut core, -30.0);
    core.set_time(1.4);
    styled(&mut core, auto);
    assert_eq!(bars(&mut core).len(), 1);
    core.set_time(2.3);
    styled(&mut core, auto);
    assert_eq!(bars(&mut core).len(), 1, "quiet since 1.4: still held");
    core.set_time(2.7);
    styled(&mut core, auto);
    assert!(bars(&mut core).is_empty());
    // The pointer on the track holds it, and asks for no frames.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(396.0, 100.0)));
    core.set_time(2.8);
    styled(&mut core, auto);
    assert_eq!(bars(&mut core).len(), 1);
    assert!(!core.animating(), "held by the pointer: input ends that");
    core.set_time(9.0);
    styled(&mut core, auto);
    assert_eq!(bars(&mut core).len(), 1);
}

#[test]
fn an_auto_scrollbar_without_a_clock_is_visible() {
    use kui_core::ScrollbarMode;
    let mut core = Core::new();
    styled(&mut core, |s| s.scrollbar(ScrollbarMode::Auto));
    assert_eq!(bars(&mut core).len(), 1);
    assert!(!core.animating());
}

/// A padded `virtual_rows` list shorter than its box asks for no frame once
/// it is laid out: the correction that puts a measured anchor back where it
/// was is for measurements, not for the clamp that keeps `top` at zero
/// while the offset is zero and the padding is not — a `set_scroll` every
/// frame was a frame every frame, which the devtools' events list paid
/// from its first event on (found building ADR 0029).
#[test]
fn a_short_padded_virtual_rows_list_settles() {
    let mut core = Core::new();
    let mut heights = kui_core::widgets::RowHeights::new(3, 20.0);
    let frame = |core: &mut Core, heights: &mut kui_core::widgets::RowHeights| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        kui_core::widgets::virtual_rows(
            &mut ui,
            "list",
            NodeSpec::column().fill().pad(6.0),
            heights,
            |_ui, _i, _w| 20.0,
            |ui, _i| {
                ui.with(NodeSpec::row().fill(), |_| {});
            },
        );
        ui.finish();
    };
    frame(&mut core, &mut heights);
    frame(&mut core, &mut heights);
    let mut asked = 0;
    for _ in 0..5 {
        frame(&mut core, &mut heights);
        asked += usize::from(core.animating());
    }
    assert_eq!(
        asked, 0,
        "a laid-out list with nothing to measure asks for no frame"
    );
}
