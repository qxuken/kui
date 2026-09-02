//! Line-breaking control end to end: `wrap`, `max_lines` and `ellipsis` on
//! `TextStyle` through a live `Core` (real shaping), checked on the emitted
//! glyph quads and on the boxes around them.

use kui_core::{Color, Core, NodeSpec, Quad, QuadKind, Size, Sizing, TextStyle};

const LONG: &str = "A window title that is far too long to fit inside a narrow header strip";
const BOX_W: f32 = 160.0;
const LINE_H: f32 = 22.0; // TextStyle::new(16.0) rounds 16 * 1.35

/// The glyph quads of `LONG` rendered with `style` inside a fixed-width box.
fn glyphs(core: &mut Core, style: TextStyle) -> Vec<Quad> {
    let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
    ui.configure_root(NodeSpec::column());
    ui.with(NodeSpec::column().width(Sizing::Fixed(BOX_W)), |ui| {
        ui.text(LONG, style)
    });
    ui.finish();
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| !matches!(q.kind, QuadKind::Solid))
        .copied()
        .collect()
}

/// How many text lines a set of glyph quads spans.
fn lines(glyphs: &[Quad]) -> usize {
    let top = glyphs.iter().map(|q| q.rect.y).fold(f32::MAX, f32::min);
    let bottom = glyphs
        .iter()
        .map(|q| q.rect.y + q.rect.h)
        .fold(f32::MIN, f32::max);
    ((bottom - top) / LINE_H).ceil() as usize
}

fn right_edge(glyphs: &[Quad]) -> f32 {
    glyphs
        .iter()
        .map(|q| q.rect.x + q.rect.w)
        .fold(f32::MIN, f32::max)
}

#[test]
fn default_still_wraps_between_words() {
    let mut core = Core::new();
    let g = glyphs(&mut core, TextStyle::new(16.0));
    assert!(lines(&g) >= 3, "expected several lines, got {}", lines(&g));
    assert!(
        right_edge(&g) <= BOX_W + 0.5,
        "wrapped text stays in the box"
    );
}

#[test]
fn nowrap_keeps_one_line_and_clips_to_the_box() {
    let mut core = Core::new();
    let wrapped = glyphs(&mut core, TextStyle::new(16.0));
    let g = glyphs(&mut core, TextStyle::new(16.0).nowrap());
    assert_eq!(lines(&g), 1, "no-wrap text is a single line");
    assert!(!g.is_empty());
    for q in &g {
        assert!(
            q.clip.x >= -0.5 && q.clip.x + q.clip.w <= BOX_W + 0.5,
            "glyph clip must be the node's box, got {:?}",
            q.clip
        );
        assert!(
            q.rect.x < BOX_W,
            "glyphs entirely past the box are culled, got x={}",
            q.rect.x
        );
    }
    assert!(
        g.len() < wrapped.len(),
        "the clipped line emits fewer glyphs than the whole paragraph"
    );
}

#[test]
fn ellipsis_truncates_within_the_box() {
    let mut core = Core::new();
    let whole = glyphs(&mut core, TextStyle::new(16.0));
    let g = glyphs(&mut core, TextStyle::new(16.0).ellipsis());
    assert_eq!(lines(&g), 1, "ellipsis alone means a single line");
    assert!(g.len() > 3, "something is drawn");
    assert!(
        g.len() < whole.len(),
        "the text is cut, not merely clipped: {} vs {}",
        g.len(),
        whole.len()
    );
    assert!(
        right_edge(&g) <= BOX_W + 0.5,
        "the ellipsized line fits the box: right edge {}",
        right_edge(&g)
    );
}

#[test]
fn max_lines_clamps_wrapped_text() {
    let mut core = Core::new();
    let g = glyphs(&mut core, TextStyle::new(16.0).max_lines(2));
    assert_eq!(lines(&g), 2);
    let e = glyphs(&mut core, TextStyle::new(16.0).max_lines(2).ellipsis());
    assert_eq!(lines(&e), 2, "ellipsis honours an explicit line budget");
    assert!(right_edge(&e) <= BOX_W + 0.5);
    let whole = glyphs(&mut core, TextStyle::new(16.0));
    assert!(
        e.len() < whole.len(),
        "the clamp cuts text: {} vs {}",
        e.len(),
        whole.len()
    );
}

/// The width of the first background quad after building `row` around
/// `LONG` styled with `style`.
fn parent_width(core: &mut Core, row: NodeSpec, style: TextStyle) -> f32 {
    let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
    ui.configure_root(NodeSpec::column());
    ui.with(row.bg(Color::rgb8(0x20, 0x20, 0x20)), |ui| {
        ui.text(LONG, style)
    });
    ui.finish();
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .find(|q| matches!(q.kind, QuadKind::Solid))
        .map(|q| q.rect.w)
        .expect("background quad")
}

#[test]
fn nowrap_text_takes_the_box_width_not_the_line_width() {
    let mut core = Core::new();
    let capped = || NodeSpec::row().max_width(BOX_W);
    let w = parent_width(&mut core, capped(), TextStyle::new(16.0).nowrap());
    assert!(
        (w - BOX_W).abs() < 0.5,
        "a capped fit parent stays at its cap around no-wrap text, got {w}"
    );
    let w = parent_width(&mut core, capped(), TextStyle::new(16.0).ellipsis());
    assert!((w - BOX_W).abs() < 0.5, "same for ellipsized text, got {w}");

    // Unconstrained, a fit parent still hugs the whole line.
    let free = parent_width(&mut core, NodeSpec::row(), TextStyle::new(16.0).nowrap());
    assert!(free > BOX_W * 2.0, "uncapped parent hugs the line: {free}");
}
