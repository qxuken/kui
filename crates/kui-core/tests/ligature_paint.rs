//! A glyph's ink painted by the places it lies over (backlog F162): a
//! ligature of two characters from spans of different colours is cut at
//! the place between them — the x a caret inside it goes to, the edge of
//! the spans' backgrounds — each slice in its own span's colour, where it
//! was one quad in the first span's. kawoosh's block caret on a `##`
//! heading drew the `#` under it in the heading's colour, on the caret's.

use kui_core::{Color, Core, FontId, NodeSpec, Quad, QuadKind, Size, Span, TextStyle};

const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const BLUE: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};
const GRAY: Color = Color {
    r: 0.5,
    g: 0.5,
    b: 0.5,
    a: 1.0,
};

fn glyphs(core: &mut Core) -> Vec<Quad> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| matches!(q.kind, QuadKind::GlyphMask | QuadKind::GlyphSubpixel))
        .copied()
        .collect()
}

fn solids(core: &mut Core) -> Vec<Quad> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid)
        .copied()
        .collect()
}

fn draw(core: &mut Core, spans: &[Span<'_>], style: TextStyle) {
    let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.rich_text(spans, style);
    ui.finish();
}

fn fixture(core: &mut Core) -> FontId {
    core.add_font_data(kui_core::testing::liga_font())
        .expect("the fixture face registers")
}

/// The fixture face's `fi` is one glyph for two characters, 700 units
/// wide: `f` red and `i` blue are one ligature still — colour does not
/// part a shaping run — and it is drawn as two slices of that glyph, cut
/// where the caret between them would stand, half its advance, which is
/// where `f`'s background ends.
#[test]
fn a_ligature_across_two_colours_is_painted_in_slices_at_the_caret_between_them() {
    let mut core = Core::new();
    let id = fixture(&mut core);
    let style = TextStyle::new(20.0).font(id);

    // One colour: the ligature is one quad, as it always was.
    draw(
        &mut core,
        &[Span::new("f").color(RED), Span::new("i").color(RED)],
        style,
    );
    let whole = glyphs(&mut core);
    assert_eq!(whole.len(), 1, "f + i is one glyph: {whole:?}");
    let whole = whole[0];
    assert_eq!(whole.color, RED);

    draw(
        &mut core,
        &[
            Span::new("f").color(RED).bg(GRAY),
            Span::new("i").color(BLUE),
        ],
        style,
    );
    let bg = solids(&mut core);
    assert_eq!(bg.len(), 1, "{bg:?}");
    let bg = bg[0];
    // 700 units at 20 px is 14 px; `f` takes half.
    assert!(
        (bg.rect.w - 7.0).abs() < 0.01,
        "f's place is half the ligature: {bg:?}"
    );
    let caret_x = bg.rect.x + bg.rect.w;

    let slices = glyphs(&mut core);
    assert_eq!(slices.len(), 2, "one slice a colour: {slices:?}");
    let (a, b) = (slices[0], slices[1]);
    assert_eq!((a.color, b.color), (RED, BLUE));
    assert!(
        (a.rect.x + a.rect.w - caret_x).abs() < 0.01 && (b.rect.x - caret_x).abs() < 0.01,
        "cut where f's background ends ({caret_x}): {a:?} {b:?}"
    );
    // The two together are the glyph: the same rect, the same texels.
    assert_eq!(a.rect.x, whole.rect.x);
    assert_eq!(a.rect.w + b.rect.w, whole.rect.w);
    assert_eq!((a.rect.y, a.rect.h), (whole.rect.y, whole.rect.h));
    assert_eq!(a.uv[0], whole.uv[0]);
    assert_eq!(a.uv[0] + a.uv[2], b.uv[0]);
    assert_eq!(a.uv[2] + b.uv[2], whole.uv[2]);
    assert_eq!((a.uv[1], a.uv[3]), (whole.uv[1], whole.uv[3]));
    assert_eq!((b.uv[1], b.uv[3]), (whole.uv[1], whole.uv[3]));
    assert_eq!(a.rect.w, a.uv[2] as f32, "a slice keeps one texel a pixel");

    // `i`'s background meets `f`'s at the same x.
    draw(
        &mut core,
        &[
            Span::new("f").color(RED),
            Span::new("i").color(BLUE).bg(GRAY),
        ],
        style,
    );
    let bg = solids(&mut core);
    assert_eq!(bg.len(), 1, "{bg:?}");
    assert!(
        (bg[0].rect.x - caret_x).abs() < 0.01 && (bg[0].rect.w - 7.0).abs() < 0.01,
        "i's place is the other half: {bg:?}"
    );
}

/// The common case costs nothing: glyphs inside their own places stay one
/// quad each, whatever colours their spans are, and an underline under the
/// second half of a ligature is in that half's colour.
#[test]
fn glyphs_inside_their_own_places_stay_whole() {
    let mut core = Core::new();
    let id = fixture(&mut core);
    let style = TextStyle::new(20.0).font(id);
    draw(
        &mut core,
        &[
            Span::new("ab").color(RED),
            Span::new("cd").color(BLUE),
            Span::new("e"),
        ],
        style,
    );
    let quads = glyphs(&mut core);
    assert_eq!(quads.len(), 5, "a glyph a character: {quads:?}");
    let colors: Vec<Color> = quads.iter().map(|q| q.color).collect();
    let own = TextStyle::new(20.0).color_or_default();
    assert_eq!(colors, [RED, RED, BLUE, BLUE, own]);

    draw(
        &mut core,
        &[
            Span::new("f").color(RED),
            Span::new("i").color(BLUE).underline(),
        ],
        style,
    );
    let lines = solids(&mut core);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].color, BLUE, "the underline is i's");
    assert!(
        (lines[0].rect.w - 7.0).abs() < 0.01,
        "under i alone: {lines:?}"
    );
}

/// The coding fonts' own ligatures, where one is installed (the CI runners
/// have none; the fixture's test above holds everywhere). Fira Code draws
/// `->` as an empty glyph and an arrow over both cells; Berkeley Mono
/// draws `##` the same way; Cascadia Code puts the drawing on the first.
/// Each character's colour stays over its own cell: no red column right
/// of the first cell, no blue one left of it.
#[test]
fn a_coding_font_s_ligature_keeps_each_character_s_colour_in_its_cell() {
    let mut core = Core::new();
    let fonts: Vec<FontId> = [
        "Berkeley Mono",
        "Fira Code",
        "Cascadia Code",
        "JetBrains Mono",
    ]
    .iter()
    .filter_map(|name| core.add_system_font(name))
    .collect();
    if fonts.is_empty() {
        eprintln!("skipped: no ligature font installed");
        return;
    }
    for id in fonts {
        for pair in ["->", "##", "**", "!=", "=>"] {
            let (first, second) = pair.split_at(1);
            draw(
                &mut core,
                &[
                    Span::new(first).color(RED).bg(GRAY),
                    Span::new(second).color(BLUE),
                    Span::new(" x"),
                ],
                TextStyle::new(20.0).font(id),
            );
            let edge = solids(&mut core)
                .first()
                .map(|q| q.rect.x + q.rect.w)
                .expect("the first cell's background");
            for q in glyphs(&mut core) {
                if q.color == RED {
                    assert!(
                        q.rect.x + q.rect.w <= edge + 0.5,
                        "{pair:?}: red past the first cell ({edge}): {q:?}"
                    );
                } else if q.color == BLUE {
                    assert!(
                        q.rect.x >= edge - 0.5,
                        "{pair:?}: blue inside the first cell ({edge}): {q:?}"
                    );
                }
            }
        }
    }
}
