//! The DTCG fixture (`tests/fixtures/dtcg`, ADR 0062) as every binding's
//! test reads it: one brand, two appearances, one skipped shadow.

use kui_core::dtcg::{self, DtcgOptions, DtcgValue};
use kui_core::{Color, Core, Size, Theme};

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dtcg/brand.resolver.json")
}

fn opts() -> DtcgOptions {
    DtcgOptions {
        prefix: "brand".into(),
        ..DtcgOptions::default()
    }
}

#[test]
fn the_fixture_loads_to_the_table_its_readme_lists() {
    let d = dtcg::from_path(fixture(), &opts()).unwrap();
    assert!(d.resolver);
    let rows: Vec<(&str, &DtcgValue)> = d
        .names
        .iter()
        .map(|n| (n.name.as_str(), &n.value))
        .collect();
    let c = |l: u32, k: u32| DtcgValue::Color {
        light: Color::hex(l),
        dark: Color::hex(k),
    };
    assert_eq!(
        rows,
        [
            ("brandSpaceGap", &DtcgValue::Length(8.0)),
            ("brandSpaceGutter", &DtcgValue::Length(16.0)),
            (
                "brandFontCode",
                &DtcgValue::Family(vec!["IBM Plex Mono".into(), "mono".into()])
            ),
            ("brandColorSurface", &c(0xffffffff, 0x1a1a18ff)),
            ("brandColorInk", &c(0x141413ff, 0xf1eee6ff)),
            ("brandColorAccent", &c(0xffd400ff, 0xffd400ff)),
            ("brandColorBrand", &c(0xffd400ff, 0xffd400ff)),
        ]
    );
    assert_eq!(d.names[4].path, "color.ink");
    let w: Vec<String> = d.warnings.iter().map(|w| w.to_string()).collect();
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].starts_with("shadow.card: `shadow`"), "{w:?}");
}

#[test]
fn added_to_a_core_the_halves_follow_the_theme_and_the_skip_is_a_warning() {
    let d = dtcg::from_path(fixture(), &opts()).unwrap();
    let mut core = Core::new();
    core.add_dtcg(&d);
    let warnings = core.take_warnings();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code, "dtcg-skipped");
    assert!(
        warnings[0].message.contains("shadow.card"),
        "{}",
        warnings[0].message
    );
    for (theme, ink) in [(Theme::light(), 0x141413ffu32), (Theme::dark(), 0xf1eee6ff)] {
        core.set_theme(theme);
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        assert_eq!(
            ui.token_color("brandColorInk").map(|c| c.to_hex()),
            Some(ink),
            "{:?}",
            theme.appearance
        );
        assert_eq!(ui.token_length("brandSpaceGutter"), Some(16.0));
        ui.finish();
    }
}
