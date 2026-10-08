//! A span in a face and a size of its own (`Span::family` / `Span::mono`,
//! `Span::size`): inline code in the monospace face inside a sans line, a
//! larger word in a paragraph. Shaped in the face it is drawn in, so the
//! caret, the hit test and the measurement read the same glyphs and byte
//! positions stay exact across the change.

use kui_core::text::Span;
use kui_core::{Core, FontFamily, Key, NodeSpec, Size, TextStyle, Vec2};

fn sans() -> TextStyle {
    TextStyle::new(14.0)
}

/// Draws `spans` in a keyed row and returns the key the text answers to.
fn draw(core: &mut Core, spans: &[Span<'_>]) -> Key {
    let mut ui = core.frame(Size::new(600.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("p", NodeSpec::row(), |ui| ui.rich_text(spans, sans()));
    ui.finish();
    Key::ROOT.str("p")
}

#[test]
fn a_mono_span_is_measured_in_the_monospace_face() {
    let mut core = Core::new();
    let text = "iiiiii";
    let as_sans = core.measure_rich_text(&[Span::new(text)], &sans(), None);
    let as_mono = core.measure_rich_text(&[Span::new(text).mono()], &sans(), None);
    let mono_style = core.measure_text(text, &TextStyle::new(14.0).family(FontFamily::Mono), None);
    assert!(
        (as_mono.width - mono_style.width).abs() < 0.5,
        "as wide as the text set in mono: {} against {}",
        as_mono.width,
        mono_style.width
    );
    assert!(
        as_mono.width > as_sans.width + 4.0,
        "narrow i's take a monospace cell each: {} against {}",
        as_mono.width,
        as_sans.width
    );
    assert_eq!(
        Span::new("x").family(FontFamily::Serif).family,
        Some(FontFamily::Serif)
    );
}

#[test]
fn carets_and_hits_agree_across_a_change_of_face() {
    let mut core = Core::new();
    let spans = [
        Span::new("run "),
        Span::new("cargo test").mono(),
        Span::new(" now"),
    ];
    let key = draw(&mut core, &spans);
    let total: usize = spans.iter().map(|s| s.text.len()).sum();
    let mut last = f32::MIN;
    for byte in 0..=total {
        let r = core.caret_rect(key, byte).expect("a caret");
        assert!(
            r.x > last,
            "the caret moves right at byte {byte}: {} after {last}",
            r.x
        );
        last = r.x;
        // Just right of the caret is the byte it stands before.
        if byte < total {
            let next = core.caret_rect(key, byte + 1).unwrap();
            let mid = Vec2::new((r.x + next.x) / 2.0 - 0.1, r.y + r.h / 2.0);
            let hit = core.text_hit(key, mid).expect("a hit");
            assert!(
                hit.byte == byte || hit.byte == byte + 1,
                "a point inside the glyph at byte {byte} lands on it: {}",
                hit.byte
            );
        }
    }
    // The mono span's own cells are equal: every caret step inside it the
    // same width, which the sans face's would not be.
    let steps: Vec<f32> = (4..14)
        .map(|b| core.caret_rect(key, b + 1).unwrap().x - core.caret_rect(key, b).unwrap().x)
        .collect();
    let first = steps[0];
    assert!(
        steps.iter().all(|s| (s - first).abs() < 0.01),
        "monospace steps: {steps:?}"
    );
}

#[test]
fn a_larger_span_makes_its_line_taller_and_a_smaller_one_never_shorter() {
    let mut core = Core::new();
    let base = core.measure_rich_text(&[Span::new("ab")], &sans(), None);
    let big = core.measure_rich_text(&[Span::new("a"), Span::new("B").size(28.0)], &sans(), None);
    assert!(
        big.height > base.height * 1.8,
        "a 28 px span in a 14 px line: {} against {}",
        big.height,
        base.height
    );
    // A line of only the smaller span keeps the paragraph's height: every
    // span carries its metrics once one is sized.
    let small = core.measure_rich_text(
        &[
            Span::new("a\n"),
            Span::new("b").size(8.0),
            Span::new("\nB").size(28.0),
        ],
        &sans(),
        None,
    );
    assert_eq!(small.lines, 3);
    assert!(
        small.height >= base.height * 2.0 + 28.0,
        "the middle line keeps the paragraph's height: {}",
        small.height
    );
    assert_eq!(Span::new("x").size(-1.0).size, None, "not a size");
}

/// Inline code's wash in a line a larger span made taller is the code's
/// own height, around its glyphs, and not the line's.
#[test]
fn a_span_background_keeps_its_own_height_in_a_taller_line() {
    let mut core = Core::new();
    let wash = kui_core::Color::hex(0x3b5bd466);
    draw(
        &mut core,
        &[
            Span::new("x ").size(14.0),
            Span::new("code").mono().bg(wash),
            Span::new(" Big").size(32.0),
        ],
    );
    let line = core.caret_rect(Key::ROOT.str("p"), 0).expect("a caret").h;
    let (dl, _) = core.output();
    let bg = dl
        .quads
        .iter()
        .find(|q| q.kind == kui_core::QuadKind::Solid && q.color == wash)
        .expect("the wash");
    assert!(
        bg.rect.h < line * 0.7 && bg.rect.h > 14.0,
        "the code's height ({}) and not the line's ({line})",
        bg.rect.h
    );
}

#[test]
fn the_caret_in_a_larger_span_is_its_line_s_height() {
    let mut core = Core::new();
    let key = draw(&mut core, &[Span::new("a "), Span::new("Big").size(28.0)]);
    let small = core
        .measure_rich_text(&[Span::new("a")], &sans(), None)
        .height;
    let r = core.caret_rect(key, 3).expect("a caret");
    assert!(r.h > small * 1.8, "one tall line: {} against {small}", r.h);
}
