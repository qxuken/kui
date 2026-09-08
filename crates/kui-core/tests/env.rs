//! Env + declared window title: frame-scoped data the driver reconciles.

use kui_core::{Core, Size};

#[test]
fn window_title_is_frame_scoped() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.window_title("hello");
    ui.finish();
    assert_eq!(core.window_title(), Some("hello"));

    // An undeclared frame clears it — "leave as-is" for the driver.
    let ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.finish();
    assert_eq!(core.window_title(), None);
}

#[test]
fn env_defaults_are_headless_safe() {
    let core = Core::new();
    assert_eq!(core.env.refresh_hz, None);
    assert!(core.env.focused);
    assert!((core.env.frame_budget_ms() - 1000.0 / 120.0).abs() < 1e-4);
    // Nothing asked the OS anything, so nothing claims to know: a headless
    // core reports the unknowns rather than a plausible light/full/en-US.
    assert_eq!(core.env.system, kui_core::SystemEnv::default());
    assert_eq!(core.env.system.appearance, kui_core::Appearance::Unknown);
    assert_eq!(core.env.system.locale, None);
}

/// `accent` is the one prop the environment paints: the OS colour where
/// the host reported one, the declared `bg` where it did not — so the same
/// tree is a different colour on two machines, deliberately, and never a
/// guessed colour on a host that was never told.
#[test]
fn the_accent_prop_paints_from_the_env_or_keeps_its_bg() {
    use kui_core::{Color, NodeSpec};
    const DECLARED: Color = Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 1.0,
    };
    let bg_of = |core: &mut Core, spec: NodeSpec| -> Color {
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        ui.with(
            spec.width(kui_core::Sizing::Fixed(50.0))
                .height(kui_core::Sizing::Fixed(20.0)),
            |_| {},
        );
        ui.finish();
        core.output().0.quads[0].color
    };

    // Nobody asked the OS: the declared background stands.
    let mut core = Core::new();
    assert_eq!(
        bg_of(&mut core, NodeSpec::row().bg(DECLARED).accent()),
        DECLARED
    );

    // A host that knows: the accent, and only for the node that asked.
    let accent = Color::hex(0x3b82f6ff);
    core.env.system.accent = Some(accent);
    assert_eq!(
        bg_of(&mut core, NodeSpec::row().bg(DECLARED).accent()),
        accent
    );
    assert_eq!(bg_of(&mut core, NodeSpec::row().bg(DECLARED)), DECLARED);
}

/// The stock button takes the accent further than a box does: the hover
/// and pressed shades come off it too, and the label goes black or white
/// by its luminance — a yellow accent with a white label would be the
/// feature painting an unreadable button.
#[test]
fn an_accent_button_repaints_its_whole_palette() {
    use kui_core::{Color, Value, widgets};
    let palette = |core: &mut Core, accent: Option<Color>| -> (Color, Color, Color) {
        core.env.system.accent = accent;
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        widgets::button_with(
            &mut ui,
            "ok",
            "OK",
            widgets::button_spec().accent().on_click(Value::str("ok")),
            None,
        );
        ui.finish();
        let (list, _) = core.output();
        // The button's own quad, then its label's first glyph.
        let bg = list.quads[0].color;
        let label = list
            .quads
            .iter()
            .find(|q| q.kind != kui_core::QuadKind::Solid)
            .expect("the label drew")
            .color;
        (bg, label, list.quads[0].color)
    };

    let mut core = Core::new();
    // Unknown: exactly the stock button, white label and all.
    let (bg, label, _) = palette(&mut core, None);
    assert_eq!(bg, Color::rgb8(0x3b, 0x5b, 0xd4));
    assert_eq!(label, Color::WHITE);

    // A dark accent keeps the white label; a light one flips it to black.
    let (bg, label, _) = palette(&mut core, Some(Color::hex(0x007affff)));
    assert_eq!(bg, Color::hex(0x007affff));
    assert_eq!(label, Color::WHITE);
    let (bg, label, _) = palette(&mut core, Some(Color::hex(0xffc409ff)));
    assert_eq!(bg, Color::hex(0xffc409ff));
    assert_eq!(label, Color::BLACK, "white on yellow is not a button");

    // The shades are the accent's, not the stock blue's.
    let (base, hover, pressed) = widgets::button_palette(Color::hex(0xffc409ff));
    assert_eq!(base, Color::hex(0xffc409ff));
    assert!(hover != base && pressed != base, "three distinct shades");
    assert!(
        widgets::readable_on(hover) == Color::BLACK,
        "the hover of a light accent is still light"
    );
}
