//! Text measurement as a query: `measure_text` answers exactly what layout
//! gives a text node with the same content and style, before any frame
//! and between frames, wrapped or not — so a view sizes to its labels
//! from numbers, not screenshots.

use kui_core::{Color, Core, NodeSpec, Size, Sizing, Span, TextStyle};

/// The laid-out size of a text node: a fit-sized box with a background
/// holds only the text, so its quad is the text's size.
fn text_box(core: &mut Core, content: &str, style: TextStyle, box_w: Option<f32>) -> Size {
    let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
    let mut spec = NodeSpec::column().bg(Color::WHITE);
    if let Some(w) = box_w {
        spec = spec.width(Sizing::Fixed(w));
    }
    ui.with(spec, |ui| ui.text(content, style));
    ui.finish();
    let (dl, _) = core.output();
    let q = dl.quads.first().expect("the box quad");
    Size::new(q.rect.w, q.rect.h)
}

#[test]
fn measure_matches_layout_unwrapped() {
    let mut core = Core::new();
    let style = TextStyle::new(14.0);
    let m = core.measure_text("hello world", &style, None);
    assert!(m.width > 0.0 && m.height > 0.0);
    assert_eq!(m.lines, 1);
    let laid_out = text_box(&mut core, "hello world", style, None);
    assert_eq!(Size::new(m.width, m.height), laid_out);
}

#[test]
fn measure_matches_layout_wrapped() {
    let mut core = Core::new();
    let style = TextStyle::new(14.0);
    let full = core.measure_text("hello world again", &style, None);
    let one_word = core.measure_text("hello", &style, None).width;
    let narrow = core.measure_text("hello world again", &style, Some(one_word));
    assert!(narrow.lines > 1, "wraps: {narrow:?}");
    assert!(narrow.width < full.width);
    // A wrapped line can still be wider than the width (a single word is
    // unbreakable); layout reports that truthfully and so does this.
    let laid_out = text_box(&mut core, "hello world again", style, Some(one_word));
    assert_eq!(narrow.height, laid_out.h, "wrapped height is layout's");
}

#[test]
fn a_width_that_already_fits_changes_nothing() {
    let mut core = Core::new();
    let style = TextStyle::new(16.0);
    let free = core.measure_text("fits", &style, None);
    assert_eq!(
        core.measure_text("fits", &style, Some(free.width + 100.0)),
        free
    );
}

#[test]
fn line_breaking_style_applies() {
    let mut core = Core::new();
    let base = TextStyle::new(14.0);
    let w = core.measure_text("hello", &base, None).width;
    let clamped = core.measure_text("hello world again", &base.max_lines(1).ellipsis(), Some(w));
    assert_eq!(clamped.lines, 1, "a one-line clamp measures one line");
    assert!(clamped.width <= w + 0.5, "and no wider than its box");
    let nowrap = core.measure_text("hello world again", &base.nowrap(), Some(w));
    assert_eq!(nowrap.lines, 1);
}

#[test]
fn rich_text_measures_as_one_paragraph() {
    let mut core = Core::new();
    let style = TextStyle::new(14.0);
    let plain = core.measure_text("hello world", &style, None);
    let same = core.measure_rich_text(&[Span::new("hello "), Span::new("world")], &style, None);
    assert_eq!(same, plain, "unstyled spans measure like the plain string");
    let bold = core.measure_rich_text(
        &[Span::new("hello "), Span::new("world").bold()],
        &style,
        None,
    );
    assert!(bold.width > 0.0 && bold.lines == 1);
}

#[test]
fn measurement_is_logical_px_at_the_frame_scale() {
    let mut core = Core::new();
    let style = TextStyle::new(14.0);
    let at_1x = core.measure_text("hello world", &style, None);
    let mut ui = core.frame(Size::new(400.0, 300.0), 2.0);
    ui.with(NodeSpec::column(), |_| {});
    ui.finish();
    let at_2x = core.measure_text("hello world", &style, None);
    assert!(
        (at_2x.width - at_1x.width).abs() < 1.5,
        "logical px either way: {} vs {}",
        at_1x.width,
        at_2x.width
    );
    assert_eq!(at_2x.height, at_1x.height);
}
