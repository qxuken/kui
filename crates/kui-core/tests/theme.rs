//! The palette a frame actually paints with
//! (`docs/adr/0019-a-theme-derived-from-appearance-and-accent.md`): what
//! follows the OS, what an app can pin instead, and what a host that says
//! nothing still gets.

use kui_core::geom::{Size, Vec2};
use kui_core::menu::MenuItem;
use kui_core::{Appearance, Color, Core, NodeSpec, Theme, ThemeSource, Value, widgets};

/// Every colour the stock widgets put on the screen for one appearance,
/// in paint order: the frame, reduced to what this file is about.
fn painted(core: &mut Core) -> Vec<u32> {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().bg(ui.theme().bg));
    let spec = widgets::button_spec(&ui.theme(), &ui.metrics()).on_click("ok");
    widgets::button_with(&mut ui, "ok", "OK", spec, None);
    widgets::label(&mut ui, "a label");
    widgets::text_input(&mut ui, "search", "hello");
    widgets::tooltip(&mut ui, "a hint");
    widgets::context_menu(
        &mut ui,
        Vec2::new(10.0, 10.0),
        &[MenuItem::new("Copy"), MenuItem::new("Select All")],
    );
    ui.finish();
    let (list, _) = core.output();
    list.quads
        .iter()
        .flat_map(|q| [q.color.to_hex(), q.border_color.to_hex()])
        .collect()
}

/// The whole point. Before ADR 0019 the two frames were byte-identical:
/// `system.appearance` was reported to every binding and read by nothing,
/// so a light-mode user got the dark widgets.
#[test]
fn the_stock_widgets_follow_the_os_appearance() {
    let mut core = Core::new();
    core.env.system.appearance = Appearance::Light;
    let light = painted(&mut core);
    core.env.system.appearance = Appearance::Dark;
    let dark = painted(&mut core);
    assert_eq!(light.len(), dark.len(), "the same nodes either way");
    let differing = light.iter().zip(&dark).filter(|(a, b)| a != b).count();
    assert!(
        differing > 0,
        "the appearance changed and nothing repainted"
    );
}

/// And the light one is legible, which is the reason it exists. Body text
/// on the window background is the case a hardcoded `#e8e8ea` failed at
/// 1.22:1.
#[test]
fn the_light_base_is_readable_where_the_old_constant_was_not() {
    fn contrast(a: Color, b: Color) -> f32 {
        let (hi, lo) = (
            a.luminance().max(b.luminance()),
            a.luminance().min(b.luminance()),
        );
        (hi + 0.05) / (lo + 0.05)
    }
    let light = Theme::light();
    assert!(contrast(light.fg, light.bg) >= 4.5);
    // The colour that used to be the only answer, on the same page.
    assert!(
        contrast(Theme::DEFAULT_FG, light.bg) < 1.5,
        "the old constant on a light page is what this fixes"
    );
}

/// A `TextStyle` that named no colour resolves to the theme's foreground
/// as it enters the tree — so `ui.text(s, TextStyle::default())` is
/// legible on both bases without a view branching on anything.
#[test]
fn unstyled_text_takes_the_themes_foreground() {
    let glyph_colors = |core: &mut Core| -> Vec<u32> {
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        widgets::label(&mut ui, "hello");
        ui.finish();
        let (list, _) = core.output();
        list.quads
            .iter()
            .filter(|q| q.kind != kui_core::QuadKind::Solid)
            .map(|q| q.color.to_hex())
            .collect()
    };
    let mut core = Core::new();
    core.env.system.appearance = Appearance::Dark;
    let dark = glyph_colors(&mut core);
    assert!(!dark.is_empty(), "the label drew");
    assert!(dark.iter().all(|c| *c == Theme::dark().fg.to_hex()));
    core.env.system.appearance = Appearance::Light;
    let light = glyph_colors(&mut core);
    assert!(light.iter().all(|c| *c == Theme::light().fg.to_hex()));
    // An explicit colour is still exactly itself.
    let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
    ui.text(
        "hello",
        kui_core::TextStyle::new(16.0).color(Color::hex(0xff00ffff)),
    );
    ui.finish();
    let (list, _) = core.output();
    assert!(
        list.quads
            .iter()
            .filter(|q| q.kind != kui_core::QuadKind::Solid)
            .all(|q| q.color.to_hex() == 0xff00ffff)
    );
}

/// The three sources, through the API a host actually calls.
#[test]
fn a_host_can_derive_pin_or_bring_its_own_accent() {
    let mut core = Core::new();
    core.env.system.appearance = Appearance::Light;
    core.env.system.accent = Some(Color::hex(0x007affff));

    // Derived: both facts.
    assert_eq!(core.theme_source(), ThemeSource::Derived);
    core.frame(Size::new(10.0, 10.0), 1.0).finish();
    assert_eq!(core.theme().accent, Color::hex(0x007affff));
    assert_eq!(core.theme().bg, Theme::light().bg);

    // A brand colour, still following the OS's light/dark.
    let brand = Color::hex(0xd2691eff);
    core.set_accent(brand);
    core.frame(Size::new(10.0, 10.0), 1.0).finish();
    assert_eq!(core.theme().accent, brand);
    assert_eq!(core.theme().bg, Theme::light().bg);
    core.env.system.appearance = Appearance::Dark;
    core.frame(Size::new(10.0, 10.0), 1.0).finish();
    assert_eq!(core.theme().bg, Theme::dark().bg, "still follows the OS");
    assert_eq!(core.theme().accent, brand);

    // Pinned: follows nothing.
    core.set_theme(Theme::light());
    core.env.system.appearance = Appearance::Dark;
    core.env.system.accent = Some(Color::WHITE);
    core.frame(Size::new(10.0, 10.0), 1.0).finish();
    assert_eq!(core.theme().bg, Theme::light().bg);
    assert_eq!(core.theme().accent, Theme::light().accent);

    // And back.
    core.derive_theme();
    core.frame(Size::new(10.0, 10.0), 1.0).finish();
    assert_eq!(core.theme().accent, Color::WHITE);
}

/// The stock button paints from the palette like every other stock widget
/// (backlog AR41): a plain `button` — no `accent` row — is the theme's
/// accent trio, so a brand colour set on the theme recolours it along
/// with the ring, the selection and the menu rows, where it used to stay
/// kui's blue unless every button said `accent`. And on a silent host the
/// trio is byte-for-byte what the button always was.
#[test]
fn a_plain_button_paints_from_the_theme_s_accent() {
    let button_bg = |core: &mut Core| -> Color {
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        widgets::button(&mut ui, "OK", Value::str("ok"));
        ui.finish();
        let (list, _) = core.output();
        list.quads[0].color
    };
    let mut core = Core::new();
    assert_eq!(
        button_bg(&mut core),
        Color::rgb8(0x3b, 0x5b, 0xd4),
        "the stock blue, as ever"
    );
    let brand = Color::hex(0xd2691eff);
    core.set_accent(brand);
    assert_eq!(
        button_bg(&mut core),
        brand,
        "the brand, with no row declared"
    );
    core.set_theme(Theme::light().with_accent(Color::hex(0x2e8b57ff)));
    assert_eq!(
        button_bg(&mut core),
        Color::hex(0x2e8b57ff),
        "a pinned palette's too"
    );
    // The stock trio is the dark theme's trio, so `Theme::dark()` cannot
    // drift from what the button paints on a silent host.
    let stock = widgets::button_spec(&Theme::dark(), &kui_core::Metrics::default());
    assert_eq!(stock.style.bg, Theme::ACCENT);
    assert_eq!(
        stock.interact().hover_bg,
        Some(Color::rgb8(0x47, 0x6c, 0xe0))
    );
    assert_eq!(
        stock.interact().pressed_bg,
        Some(Color::rgb8(0x2f, 0x54, 0xc4))
    );
}

/// A host that reports nothing sees no change at all — the property that
/// let this be the default rather than an opt-in.
#[test]
fn a_silent_host_gets_what_kui_always_painted() {
    let mut core = Core::new();
    assert_eq!(core.env.system.appearance, Appearance::Unknown);
    assert_eq!(core.env.system.accent, None);
    core.frame(Size::new(10.0, 10.0), 1.0).finish();
    let t = *core.theme();
    assert_eq!(t.fg, Theme::DEFAULT_FG);
    assert_eq!(t.accent, Theme::ACCENT);
    assert_eq!(t.focus_ring, Color::rgb8(0x7f, 0x9c, 0xf5), "ADR 0002's");
    assert_eq!(t.selection, kui_core::select::TINT);
}

/// The focus ring and the scrollbar are core chrome, not widgets, and
/// they moved too — a ring that cannot be seen on a white page is not a
/// focus indicator.
#[test]
fn the_cores_own_chrome_follows_the_theme_as_well() {
    assert_ne!(Theme::light().focus_ring, Theme::dark().focus_ring);
    assert_ne!(Theme::light().scrollbar, Theme::dark().scrollbar);
    // The dark scrollbar is a white wash; the light one is not.
    assert!(Theme::dark().scrollbar.r > 0.5);
    assert!(Theme::light().scrollbar.r < 0.5);
}
