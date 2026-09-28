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
    ui.leaf(NodeSpec::column().height(10.0));
    let key = ui.text_in_keyed(
        "line",
        NodeSpec::row().width(width.map_or(Sizing::Grow(1.0), Sizing::Fixed)),
        text,
        mono(),
    );
    ui.leaf_keyed("box", NodeSpec::column().height(10.0));
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
        ui.text_in(
            NodeSpec::row().bg(Color::rgba8(60, 90, 212, 85)),
            "value",
            mono(),
        );
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

/// `TextHit.line` is the visual row within the node asked about (backlog
/// AR30): two runs stacked are rows 0 and 1, three side by side are one
/// row, and a wrapped run counts as many rows as it wrapped to — where it
/// used to be the wrapped line within whichever run's buffer took the
/// hit, so a two-run node answered `line: 0` for its second run.
#[test]
fn the_line_is_the_visual_row_across_the_nodes_runs() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let stacked = ui.with_keyed("stacked", NodeSpec::column(), |ui| {
        ui.text("first", mono());
        ui.text("second", mono());
        // A row of three runs, then a run that wraps onto two rows.
        ui.with(NodeSpec::row(), |ui| {
            ui.text("a ", mono());
            ui.text("b ", mono());
            ui.text("c", mono());
        });
        ui.text_in(NodeSpec::row().width(60.0), "wrapping text here", mono());
    });
    ui.finish();
    let w = cell(&mut core);
    let row = |core: &mut Core, y: f32| core.text_hit(stacked, Vec2::new(w, y)).map(|h| h.line);
    assert_eq!(row(&mut core, LH * 0.5), Some(0), "first");
    assert_eq!(row(&mut core, LH * 1.5), Some(1), "second");
    assert_eq!(
        row(&mut core, LH * 2.5),
        Some(2),
        "the row of three runs is one row"
    );
    assert_eq!(
        core.text_hit(stacked, Vec2::new(3.0 * w, LH * 2.5))
            .map(|h| h.line),
        Some(2),
        "and so is its third run"
    );
    assert_eq!(
        row(&mut core, LH * 3.5),
        Some(3),
        "the wrapped run's first row"
    );
    assert_eq!(row(&mut core, LH * 4.5), Some(4), "and its second");
}

/// A `role="none"` subtree under a `line` — a gutter's number, a fold
/// marker — is not the line's text (backlog AR30): the access tree skips
/// it, and so does a hit or a caret asked by the line's key, so `byte`
/// counts the same characters `offset` in an access event does.
#[test]
fn a_none_subtree_under_a_line_is_not_its_text() {
    use kui_core::Role;
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let line = ui.with_keyed("line", NodeSpec::row().role(Role::Line), |ui| {
        ui.text_in(NodeSpec::row().role(Role::None), "12 ", mono());
        ui.text("let x", mono());
    });
    ui.finish();
    let w = cell(&mut core);
    // The gutter takes three cells; the line's text starts after it, and
    // byte 0 is its first character, not the gutter's.
    let start = core.caret_rect(line, 0).unwrap();
    assert!((start.x - 3.0 * w).abs() < 0.75, "{}", start.x);
    assert_eq!(
        core.text_hit(line, Vec2::new(3.0 * w + 0.2 * w, LH * 0.5))
            .map(|h| h.byte),
        Some(0)
    );
    assert_eq!(
        core.caret_rect(line, 5)
            .map(|r| r.x)
            .map(|x| (x - 8.0 * w).abs() < 0.75),
        Some(true)
    );
    // The gutter's own key still answers for its text.
    let gutter = line.index(0);
    assert_eq!(
        core.text_hit(gutter, Vec2::new(0.2 * w, LH * 0.5))
            .map(|h| h.byte),
        Some(0)
    );
}

/// A text further below its `line` than a place remembers cannot be found
/// from the row's key; the frame says so (backlog AR30) instead of
/// answering byte 0 in silence.
#[test]
fn a_text_too_deep_under_its_line_is_a_named_diagnostic() {
    use kui_core::Role;
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("line", NodeSpec::row().role(Role::Line), |ui| {
        ui.with(NodeSpec::row(), |ui| {
            ui.with(NodeSpec::row(), |ui| {
                ui.with(NodeSpec::row(), |ui| {
                    ui.with(NodeSpec::row(), |ui| {
                        ui.text_in(NodeSpec::row(), "deep", mono());
                    });
                });
            });
        });
    });
    ui.finish();
    let codes = kui_core::testing::codes(&core.take_warnings());
    assert_eq!(codes, ["text-beyond-line"]);
    // Four wrappers is within reach: no warning.
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("line", NodeSpec::row().role(Role::Line), |ui| {
        ui.with(NodeSpec::row(), |ui| {
            ui.with(NodeSpec::row(), |ui| {
                ui.text_in(NodeSpec::row(), "deep", mono());
            });
        });
    });
    ui.finish();
    assert!(core.take_warnings().is_empty());
}

#[test]
fn a_space_a_word_break_swallows_ends_the_row_it_broke() {
    // cosmic-text drops the space a `Word` break falls at: it has no
    // glyph, and its caret went to the end of the whole paragraph — two
    // rows down here (backlog F106).
    let mut core = Core::new();
    let w = cell(&mut core);
    let key = frame(&mut core, "aaa bbb ccc", Some(5.5 * w), 1.0);
    let r = core.caret_rect(key, 3).unwrap();
    assert!((r.y - 20.0).abs() < 0.01, "first row: {}", r.y);
    assert!((r.x - (10.0 + 3.0 * w)).abs() < 0.75, "{}", r.x);
    let r = core.caret_rect(key, 7).unwrap();
    assert!((r.y - (20.0 + LH)).abs() < 0.01, "second row: {}", r.y);
    // The end of the paragraph is still the end of its last row.
    let r = core.caret_rect(key, 11).unwrap();
    assert!((r.y - (20.0 + 2.0 * LH)).abs() < 0.01, "last row: {}", r.y);
}

#[test]
fn one_text_at_two_widths_answers_at_each() {
    // The same label in two boxes of different widths shares one shaped
    // run; each node answers `caret_rect` and `text_hit` at its own
    // width, not at the one drawn last (backlog RG72).
    let mut core = Core::new();
    let w = cell(&mut core);
    let text = "aaa bbb ccc";
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let at = |cells: f32| NodeSpec::row().width(Sizing::Fixed(cells * w + 0.5));
    let narrow = ui.text_in_keyed("narrow", at(6.0), text, mono()).index(0);
    ui.leaf(NodeSpec::column().height(100.0));
    let wide = ui.text_in_keyed("wide", at(20.0), text, mono()).index(0);
    ui.finish();
    let row_x = |r: kui_core::Rect, y0: f32| (((r.y - y0) / LH).round(), (r.x / w).round());
    // "aaa ", "bbb ", "ccc" in six cells; one row in twenty.
    assert_eq!(row_x(core.caret_rect(narrow, 8).unwrap(), 0.0), (2.0, 0.0));
    assert_eq!(row_x(core.caret_rect(wide, 8).unwrap(), 160.0), (0.0, 8.0));
    assert_eq!(
        core.text_hit(narrow, Vec2::new(1.2 * w, LH + 5.0)),
        Some(TextHit { byte: 5, line: 1 })
    );
    assert_eq!(
        core.text_hit(wide, Vec2::new(9.2 * w, 165.0)),
        Some(TextHit { byte: 9, line: 0 })
    );
    // The glyphs were drawn at each width all along.
    let (dl, _) = core.output();
    let rows = |y0: f32, y1: f32| {
        let mut r: Vec<i32> = dl
            .quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::GlyphMask && q.rect.y >= y0 && q.rect.y < y1)
            .map(|q| ((q.rect.y - y0) / LH).floor() as i32)
            .collect();
        r.dedup();
        r
    };
    assert_eq!(rows(0.0, 100.0), [0, 1, 2]);
    assert_eq!(rows(160.0, 300.0), [0]);
}
