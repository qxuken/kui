//! OpenType features on a text style (backlog C23): `liga=0 calt=0` keeps a
//! coding font from joining `->`, `tnum` lines figures up. Part of what a
//! text is shaped as, so it is in the cache key; the spelling is one
//! string every binding shares.

use kui_core::{Core, FontFeatures, NodeSpec, QuadKind, Size, TextStyle};

#[test]
fn the_spelling_round_trips() {
    let f = FontFeatures::parse("liga=0, calt=0 tnum -dlig +ss01");
    let got: Vec<(String, u32)> = f
        .iter()
        .map(|(t, v)| (String::from_utf8_lossy(t).trim_end().to_string(), v))
        .collect();
    assert_eq!(
        got,
        [
            ("liga".to_string(), 0),
            ("calt".to_string(), 0),
            ("tnum".to_string(), 1),
            ("dlig".to_string(), 0),
            ("ss01".to_string(), 1),
        ]
    );
    assert_eq!(f.to_string_spelling(), "liga=0 calt=0 tnum=1 dlig=0 ss01=1");
    assert_eq!(FontFeatures::parse(f.to_string_spelling().as_str()), f);
    // Setting a tag again replaces it; the ninth is dropped, not refused.
    let f = FontFeatures::new().set("liga", 1).set("liga", 0);
    assert_eq!(f.len(), 1);
    let many = (0..10).fold(FontFeatures::new(), |f, i| f.set(&format!("ss{i:02}"), 1));
    assert_eq!(many.len(), FontFeatures::MAX);
    assert!(FontFeatures::parse("").is_empty());
}

/// Two texts differing only in features are two shaped entries: the
/// features are what the text is shaped as.
#[test]
fn features_are_part_of_what_a_text_is_shaped_as() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.text("fi ->", TextStyle::new(16.0));
    ui.text(
        "fi ->",
        TextStyle::new(16.0).features(FontFeatures::parse("liga=0 calt=0")),
    );
    ui.text("fi ->", TextStyle::new(16.0));
    ui.finish();
    assert_eq!(core.text_cache_len(), 2);
}

/// With a font that has the ligatures, `-> != www` shapes as fewer glyphs
/// by default than with `liga`/`calt` off (Fira Code: 4 against 7; `->`
/// alone is a spacer plus an arrow in that font, so the count is taken
/// over three pairs). Skipped where no such font is installed (the CI
/// runners), so this pins the effect only where it can be seen — the
/// cache-key test above holds everywhere.
#[test]
fn ligatures_come_apart_when_asked() {
    let mut core = Core::new();
    let Some(id) = ["Fira Code", "Cascadia Code", "JetBrains Mono"]
        .iter()
        .find_map(|name| core.add_system_font(name))
    else {
        eprintln!("skipped: no ligature font installed");
        return;
    };
    let glyphs = |core: &mut Core, features: &str| {
        let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.text(
            "-> != www",
            TextStyle::new(24.0)
                .font(id)
                .features(FontFeatures::parse(features)),
        );
        ui.finish();
        let (dl, _) = core.output();
        dl.quads
            .iter()
            .filter(|q| matches!(q.kind, QuadKind::GlyphMask | QuadKind::GlyphSubpixel))
            .count()
    };
    let joined = glyphs(&mut core, "");
    let apart = glyphs(&mut core, "liga=0 calt=0 dlig=0");
    assert!(
        apart > joined,
        "the ligatures hold by default ({joined} glyphs) and come apart with the features off ({apart})"
    );
}

/// The same effect on the face the tests carry with them
/// (`testing::liga_font`, backlog AR48): pinned on every machine, CI's
/// fontless image included, where the test above pins it only where a
/// coding font happens to be installed. `fi` is one glyph under `liga`
/// and two with it off; `if` is two either way, since the ligature is
/// ordered.
#[test]
fn the_fixture_font_s_ligature_holds_by_default_and_comes_apart_with_liga_off() {
    let mut core = Core::new();
    let id = core
        .add_font_data(kui_core::testing::liga_font())
        .expect("the fixture face registers");
    assert_eq!(core.font_family(id), Some("Kui Liga"));
    let glyphs = |core: &mut Core, text: &str, features: &str| {
        let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.text(
            text,
            TextStyle::new(24.0)
                .font(id)
                .features(FontFeatures::parse(features)),
        );
        ui.finish();
        let (dl, _) = core.output();
        dl.quads
            .iter()
            .filter(|q| matches!(q.kind, QuadKind::GlyphMask | QuadKind::GlyphSubpixel))
            .count()
    };
    assert_eq!(glyphs(&mut core, "fi", ""), 1, "f + i joined by default");
    assert_eq!(
        glyphs(&mut core, "fi", "liga=0"),
        2,
        "and apart with liga off"
    );
    assert_eq!(glyphs(&mut core, "if", ""), 2, "the ligature is ordered");
    assert_eq!(
        glyphs(&mut core, "fifi", ""),
        2,
        "and applies at every match"
    );
}
