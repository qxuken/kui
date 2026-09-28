//! `family` by name (ADR 0037): a stock name, an installed or loaded
//! family, or a miss — resolved where a binding parses the prop, to the
//! handle `add_system_font` gives, so a Lua view draws a face with no
//! host door and no frame in another.

use kui_core::testing::font_face;
use kui_core::{Core, FontFamily, NameRefs};

#[test]
fn a_name_resolves_to_the_handle_the_host_door_gives() {
    let mut core = Core::new();
    core.add_font_data(font_face("Fixture Mono", 400, false, true))
        .expect("the fixture loads");
    let named = {
        let mut refs = NameRefs::new(core.token_lookup());
        let named = refs.family("Fixture Mono");
        assert!(refs.take_missed_families().is_empty());
        named
    };
    let handle = core
        .add_system_font("Fixture Mono")
        .expect("the door finds it");
    assert_eq!(
        named,
        FontFamily::Custom(handle),
        "one registry, one handle"
    );

    let mut ui = core.frame(kui_core::Size::new(10.0, 10.0), 1.0);
    assert_eq!(
        ui.system_font("Fixture Mono"),
        Some(handle),
        "and Ui's pass-through"
    );
    ui.finish();
}

#[test]
fn a_stock_name_is_stock_and_a_miss_is_sans_and_remembered() {
    let core = Core::new();
    let mut refs = NameRefs::new(core.token_lookup());
    assert_eq!(refs.family("mono"), FontFamily::Mono);
    assert_eq!(refs.family("serif"), FontFamily::Serif);
    assert_eq!(refs.family("No Such Family 7"), FontFamily::Sans);
    assert_eq!(refs.family("No Such Family 7"), FontFamily::Sans);
    assert_eq!(
        refs.take_missed_families(),
        ["No Such Family 7"],
        "once per name"
    );
}

#[test]
fn a_miss_warns_once() {
    let mut core = Core::new();
    core.warn_unknown_family("No Such Family 7");
    core.warn_unknown_family("No Such Family 7");
    let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
    assert_eq!(codes, ["unknown-family"]);
}

/// A family a view names follows a face of it loaded later, as a handle
/// does (RG59's reweigh): ADR 0037 said it was built so, and no test held
/// it (the alpha.22 regression pass). A Light face with no 字, named;
/// then a Regular with 字 — the named text draws the fixture's square.
#[test]
fn a_named_family_follows_a_face_loaded_later() {
    use kui_core::testing::han_face;
    use kui_core::{NodeSpec, QuadKind, Size, TextStyle};
    let mut core = Core::new();
    core.add_font_data(font_face("Kui Named", 300, false, false))
        .expect("the fixture loads");
    let family = NameRefs::new(core.token_lookup()).family("Kui Named");
    assert!(matches!(family, FontFamily::Custom(_)));
    let style = TextStyle::new(20.0).family(family).line_height(24.0);
    let widths = |core: &mut Core| -> Vec<f32> {
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.text_in(NodeSpec::column(), "字字", style);
        ui.finish();
        let (dl, _) = core.output();
        dl.quads
            .iter()
            .filter(|q| q.kind == QuadKind::GlyphMask)
            .map(|q| q.rect.w)
            .collect()
    };
    let before = widths(&mut core);
    core.add_font_data(han_face("Kui Named"))
        .expect("a regular face of it");
    let after = widths(&mut core);
    assert_eq!(after.len(), 2, "{after:?}");
    assert_ne!(before, after, "shaped again in the face that came");
    assert!(after.iter().all(|w| *w == after[0]));
}
