//! `Core::text_hit` and `Core::caret_rect` (backlog C18): a point on a
//! text node the app owns becomes a byte offset, and a byte offset becomes
//! the rect a caret goes in — answered from the frame that finished, which
//! is the layout a click was made against, so a custom editor needs neither
//! prefix measurement nor a cell-width assumption.

use kui_core::{Color, Core, Key, NodeSpec, Size, Sizing, Span, TextHit, TextStyle, Vec2};

const LH: f32 = 20.0;

fn mono() -> TextStyle {
    TextStyle::new(14.0).mono().line_height(LH)
}

/// A frame with one keyed mono text node at (10, 20), `width` wide (grow to
/// the frame when `None`), and one unkeyed box beside it.
fn frame(core: &mut Core, text: &str, width: Option<f32>, scale: f32) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), scale);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    ui.with(NodeSpec::column().height(Sizing::Fixed(10.0)), |_| {});
    let key = ui.with_keyed(
        "line",
        NodeSpec::row().width(width.map_or(Sizing::Grow(1.0), Sizing::Fixed)),
        |ui| ui.text(text, mono()),
    );
    ui.with_keyed(
        "box",
        NodeSpec::column().height(Sizing::Fixed(10.0)),
        |_| {},
    );
    ui.finish();
    // An unkeyed text node is auto-keyed by its index under its parent.
    key.index(0)
}

fn cell(core: &mut Core) -> f32 {
    core.measure_text("M", &mono(), None).width
}

#[test]
fn a_point_becomes_a_byte_offset_and_back() {
    let mut core = Core::new();
    let key = frame(&mut core, "hello world", None, 1.0);
    let w = cell(&mut core);
    // The node sits at (10, 20): root padding 10, then a 10-tall spacer.
    let origin = core.caret_rect(key, 0).expect("drawn");
    assert_eq!(
        (origin.x, origin.y, origin.w, origin.h),
        (10.0, 20.0, 0.0, LH)
    );
    // The left half of the fourth glyph is before it; the right half after.
    let left = core.text_hit(key, Vec2::new(10.0 + 3.0 * w + 0.2 * w, 25.0));
    assert_eq!(left, Some(TextHit { byte: 3, line: 0 }));
    let right = core.text_hit(key, Vec2::new(10.0 + 3.0 * w + 0.8 * w, 25.0));
    assert_eq!(right, Some(TextHit { byte: 4, line: 0 }));
    // Past the end lands at the end; before the start at the start.
    assert_eq!(
        core.text_hit(key, Vec2::new(300.0, 25.0)),
        Some(TextHit { byte: 11, line: 0 })
    );
    assert_eq!(
        core.text_hit(key, Vec2::new(0.0, 25.0)),
        Some(TextHit { byte: 0, line: 0 })
    );
    // A caret at byte n is n cells in, and the end is after the last one.
    let r4 = core.caret_rect(key, 4).unwrap();
    assert!((r4.x - (10.0 + 4.0 * w)).abs() < 0.75, "{}", r4.x);
    let end = core.caret_rect(key, 11).unwrap();
    assert!((end.x - (10.0 + 11.0 * w)).abs() < 0.75, "{}", end.x);
    // Past the content clamps to the end.
    assert_eq!(core.caret_rect(key, 999), Some(end));
}

#[test]
fn a_wrapped_node_answers_per_visual_line() {
    let mut core = Core::new();
    let w = cell(&mut core);
    // Six cells wide: "hello world" breaks after "hello ".
    let key = frame(&mut core, "hello world", Some(6.5 * w), 1.0);
    let second = core.text_hit(key, Vec2::new(10.0 + 1.2 * w, 20.0 + LH + 5.0));
    assert_eq!(second, Some(TextHit { byte: 7, line: 1 }));
    let r = core.caret_rect(key, 8).unwrap();
    assert!((r.y - (20.0 + LH)).abs() < 0.01, "second line: {}", r.y);
    assert!((r.x - (10.0 + 2.0 * w)).abs() < 0.75, "{}", r.x);
}

#[test]
fn a_paragraph_break_counts_as_content() {
    let mut core = Core::new();
    let key = frame(&mut core, "ab\ncd", None, 1.0);
    let hit = core.text_hit(key, Vec2::new(10.5, 20.0 + LH + 5.0));
    assert_eq!(
        hit,
        Some(TextHit { byte: 3, line: 1 }),
        "c is byte 3, after the newline"
    );
    let r = core.caret_rect(key, 4).unwrap();
    assert!((r.y - (20.0 + LH)).abs() < 0.01);
}

#[test]
fn logical_answers_at_every_scale() {
    let mut core = Core::new();
    let key = frame(&mut core, "hello", None, 2.0);
    let w = cell(&mut core);
    let r = core.caret_rect(key, 2).unwrap();
    assert!(
        (r.x - (10.0 + 2.0 * w)).abs() < 0.75,
        "{} vs {}",
        r.x,
        10.0 + 2.0 * w
    );
    assert_eq!(r.h, LH);
    assert_eq!(
        core.text_hit(key, Vec2::new(10.0 + 2.2 * w, 30.0)),
        Some(TextHit { byte: 2, line: 0 })
    );
}

#[test]
fn rich_text_answers_in_concatenated_bytes() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.rich_text(
        &[
            Span::new("ab").bold(),
            Span::new("cd").color(Color::rgb8(255, 0, 0)),
        ],
        mono(),
    );
    ui.finish();
    let key = Key::ROOT.index(0);
    let w = cell(&mut core);
    assert_eq!(
        core.text_hit(key, Vec2::new(3.2 * w, 5.0)),
        Some(TextHit { byte: 3, line: 0 })
    );
}

#[test]
fn only_drawn_text_nodes_answer() {
    let mut core = Core::new();
    let key = frame(&mut core, "hello", None, 1.0);
    assert!(
        core.text_hit(Key::ROOT.str("box"), Vec2::new(1.0, 1.0))
            .is_none()
    );
    assert!(core.caret_rect(Key::ROOT.str("nope"), 0).is_none());
    // A node the next frame does not draw stops answering once that
    // frame has finished.
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    // Mid-build, the last frame still answers: this is where a Rust view
    // resolves the click it recorded in `on_event`.
    assert!(ui.text_hit(key, Vec2::new(12.0, 25.0)).is_some());
    ui.finish();
    assert!(core.text_hit(key, Vec2::new(12.0, 25.0)).is_none());
}

/// The shape a syntax-highlighted editor draws: a keyed `line` row holding
/// one text run per token, one of them wrapped in a selection box. The
/// row's key answers for all of them, with byte offsets running across
/// the runs in order, the way the access tree reads the line.
#[test]
fn a_line_of_runs_answers_by_the_row_key() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let line = ui.with_keyed("line", NodeSpec::row(), |ui| {
        ui.text("let ", mono().color(Color::rgb8(200, 100, 255)));
        ui.with(NodeSpec::row().bg(Color::rgba8(60, 90, 212, 85)), |ui| {
            ui.text("value", mono())
        });
        ui.text(" = 1;", mono());
    });
    ui.finish();
    let w = cell(&mut core);
    // "let value = 1;" — a point in the fourth cell of "value" (byte 4 + 3).
    let hit = core.text_hit(line, Vec2::new(7.2 * w, 5.0));
    assert_eq!(hit, Some(TextHit { byte: 7, line: 0 }));
    // The seam between runs is the start of the later one.
    let seam = core.caret_rect(line, 4).unwrap();
    assert!((seam.x - 4.0 * w).abs() < 0.75, "{}", seam.x);
    // The end is past the last run's last glyph.
    let end = core.caret_rect(line, 14).unwrap();
    assert!((end.x - 14.0 * w).abs() < 0.75, "{}", end.x);
    // Far right of the line lands at its end; far left at its start.
    assert_eq!(
        core.text_hit(line, Vec2::new(390.0, 5.0)).map(|h| h.byte),
        Some(14)
    );
    assert_eq!(
        core.text_hit(line, Vec2::new(0.0, 5.0)).map(|h| h.byte),
        Some(0)
    );
    // The wrapper's own key answers for the run inside it alone.
    let wrapper = line.index(1);
    assert_eq!(
        core.text_hit(wrapper, Vec2::new(5.5 * w, 5.0))
            .map(|h| h.byte),
        Some(1)
    );
}
