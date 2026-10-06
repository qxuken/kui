//! Env + declared window title: frame-scoped data the driver reconciles.

use kui_core::{Appearance, Assistive, Core, MotionPref, OptionAsAlt, Size, SystemEnv, Value};

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

/// The level is frame-scoped the way the title is, with one difference
/// (backlog C30): the undeclared reading is `false`, not "leave as-is",
/// so a frame that stops asking is what lowers the window — a pin
/// button toggles by declaring or not, and never has to undo anything.
#[test]
fn always_on_top_is_frame_scoped_and_defaults_off() {
    let mut core = Core::new();
    assert!(!core.always_on_top());
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.always_on_top(true);
    ui.finish();
    assert!(core.always_on_top());

    let ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.finish();
    assert!(!core.always_on_top());

    // What the app asked and what the driver applied are two facts: the
    // env reading is the driver's to write, and a frame's ask leaves it
    // alone — a headless core, which applies nothing, never reports it.
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.always_on_top(true);
    assert!(!ui.env().window.always_on_top);
    ui.finish();
    assert!(!core.env.window.always_on_top);
}

/// The secure-input ask is frame state the way the level is (backlog
/// F85): false until a frame declares it, and false again on the frame
/// that stops — which is what lets the runner turn it off without the app
/// remembering to.
#[test]
fn secure_input_is_frame_scoped_and_defaults_off() {
    let mut core = Core::new();
    assert!(!core.secure_input());
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.secure_input(true);
    ui.finish();
    assert!(core.secure_input());

    let ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.finish();
    assert!(
        !core.secure_input(),
        "a frame that stops asking turns it off"
    );
}

/// The input-method ask is frame state the way the level is (backlog
/// F125): off until a frame declares it — the platform's IME, dead keys
/// and press-and-hold, so no app changes by upgrading — and off again on
/// the frame that stops, which is how a modal editor's insert mode gets
/// them back without undoing anything.
#[test]
fn ime_off_is_frame_scoped_and_defaults_off() {
    let mut core = Core::new();
    assert!(!core.ime_off());
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.ime_off(true);
    ui.finish();
    assert!(core.ime_off());

    let ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.finish();
    assert!(
        !core.ime_off(),
        "a frame that stops asking gives the input method back"
    );
}

/// Option as Alt is frame state the way the level is (backlog F113):
/// `None` until a frame declares it — the Mac's own composing Option, so
/// no app changes by upgrading — and `None` again on the frame that
/// stops, which is what gives the Option keys back to the layout without
/// the app remembering to.
#[test]
fn option_as_alt_is_frame_scoped_and_defaults_to_none() {
    let mut core = Core::new();
    assert_eq!(core.option_as_alt(), OptionAsAlt::None);
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.option_as_alt(OptionAsAlt::Left);
    ui.finish();
    assert_eq!(core.option_as_alt(), OptionAsAlt::Left);

    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.option_as_alt(OptionAsAlt::Both);
    ui.finish();
    assert_eq!(core.option_as_alt(), OptionAsAlt::Both);

    let ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.finish();
    assert_eq!(
        core.option_as_alt(),
        OptionAsAlt::None,
        "a frame that stops declaring it gives the Option keys back"
    );
}

/// The four spellings every door shares: the name a prop and a Lua field
/// write, the number C and the binary IR carry, and which side each
/// covers.
#[test]
fn option_as_alt_names_numbers_and_sides_agree() {
    for v in OptionAsAlt::ALL {
        assert_eq!(OptionAsAlt::from_name(v.name()), Some(v));
        assert_eq!(OptionAsAlt::from_index(v.index()), Some(v));
    }
    assert_eq!(OptionAsAlt::from_name("Left"), None);
    assert_eq!(OptionAsAlt::from_index(4), None);
    use kui_core::KeyLocation::{Left, Numpad, Right, Standard};
    assert!(OptionAsAlt::Left.covers(Left) && !OptionAsAlt::Left.covers(Right));
    assert!(OptionAsAlt::Right.covers(Right) && !OptionAsAlt::Right.covers(Left));
    assert!(OptionAsAlt::Both.covers(Left) && OptionAsAlt::Both.covers(Right));
    assert!(!OptionAsAlt::Both.covers(Standard) && !OptionAsAlt::Both.covers(Numpad));
    assert!(!OptionAsAlt::None.covers(Left) && !OptionAsAlt::None.covers(Right));
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
        ui.leaf(
            spec.width(kui_core::Sizing::Fixed(50.0))
                .height(kui_core::Sizing::Fixed(20.0)),
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
    use kui_core::{Color, widgets};
    let palette = |core: &mut Core, accent: Option<Color>| -> (Color, Color, Color) {
        core.env.system.accent = accent;
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        let spec = widgets::button_spec(&ui.theme(), &ui.metrics())
            .accent()
            .on_click("ok");
        widgets::button_with(&mut ui, "ok", "OK", spec, None);
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

/// A `system` event is how a host that retains its tree learns the OS
/// settings changed: its `view` runs for a message, and the driver's
/// redraw only re-lowers what it was handed (backlog F40).
#[test]
fn a_changed_system_reading_becomes_an_event() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        core.frame(Size::new(100.0, 100.0), 1.0).finish();
        core.take_pending_events()
    };

    // The first frame establishes the reading rather than reporting it,
    // the way the viewport does.
    core.env.system = SystemEnv {
        appearance: Appearance::Light,
        motion: MotionPref::Full,
        ..Default::default()
    };
    assert!(
        kinds(&frame(&mut core)).is_empty(),
        "the first frame has nothing to compare against"
    );
    assert!(kinds(&frame(&mut core)).is_empty(), "nothing changed");

    // The user switched to dark and asked for less motion.
    core.env.system.appearance = Appearance::Dark;
    core.env.system.motion = MotionPref::Reduced;
    let evs = frame(&mut core);
    assert_eq!(kinds(&evs), vec!["system"], "one event, on the root");
    let p = &evs[0].payload;
    assert_eq!(evs[0].key, kui_core::Key::ROOT);
    assert_eq!(p.get_str("appearance"), Some("dark"));
    assert_eq!(p.get_str("motion"), Some("reduced"));
    // The whole reading, in `env().system`'s own spellings and nulls.
    assert!(matches!(p.get("accent"), Some(Value::Null)));
    assert!(matches!(p.get("locale"), Some(Value::Null)));
    assert_eq!(p.get_str("assistive"), Some("unknown"));

    // And once only: a reading that stops changing stops reporting.
    assert!(kinds(&frame(&mut core)).is_empty());
}

/// A screen reader attaching is the same kind of change as the appearance
/// flipping — a retained-tree host's alert cannot start announcing
/// unless a message says something is listening (backlog F48). The bridge
/// writes `assistive` into `env.system` and the core reports it through
/// the one `system` event, so a host that already handles that event for
/// its palette hears this too.
#[test]
fn assistive_technology_attaching_is_a_system_event() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        core.frame(Size::new(100.0, 100.0), 1.0).finish();
        core.take_pending_events()
    };
    // A driver with a bridge and no client yet.
    core.env.system.assistive = Assistive::None;
    assert!(
        kinds(&frame(&mut core)).is_empty(),
        "the first frame establishes"
    );

    // A client asked for the tree.
    core.env.system.assistive = Assistive::Listening;
    let evs = frame(&mut core);
    assert_eq!(kinds(&evs), vec!["system"]);
    let p = &evs[0].payload;
    assert_eq!(p.get_str("assistive"), Some("listening"));
    // The other four ride along unchanged, so a handler keeps the whole
    // reading as it does for any other `system` event.
    assert_eq!(p.get_str("appearance"), Some("unknown"));
    assert_eq!(p.get_str("motion"), Some("unknown"));
    assert!(kinds(&frame(&mut core)).is_empty(), "reported once");

    // Where the adapter reports deactivation (AT-SPI), the fall is a
    // change like the rise.
    core.env.system.assistive = Assistive::None;
    let evs = frame(&mut core);
    assert_eq!(kinds(&evs), vec!["system"]);
    assert_eq!(evs[0].payload.get_str("assistive"), Some("none"));
}

fn kinds(evs: &[kui_core::UiEvent]) -> Vec<&str> {
    evs.iter().filter_map(|e| e.kind()).collect()
}

/// A launcher's pinned reading is a `SystemEnv` laid *over* the OS's, every
/// frame: what the app pinned wins, what it left unknown is the OS's — so
/// the pin survives the runner's per-frame write and a real change to an
/// unpinned field still arrives (backlog F47, the pomodoro's third ask).
#[test]
fn a_pinned_reading_lies_over_the_queried_one_field_by_field() {
    use kui_core::{Color, Locale};
    let queried = SystemEnv {
        appearance: Appearance::Light,
        accent: Some(Color::hex(0x0a84ffff)),
        motion: MotionPref::Full,
        locale: Locale::new("en-US"),
        assistive: Assistive::Listening,
    };
    // Unknown is "not pinned": the default pins nothing.
    assert_eq!(SystemEnv::default().over(queried), queried);
    // One field pinned leaves the other three the OS's.
    let less_motion = SystemEnv {
        motion: MotionPref::Reduced,
        ..Default::default()
    };
    assert_eq!(
        less_motion.over(queried),
        SystemEnv {
            motion: MotionPref::Reduced,
            ..queried
        }
    );
    // Every field can be pinned, and a pin beats an answer, not only an
    // unknown.
    let all = SystemEnv {
        appearance: Appearance::Dark,
        accent: Some(Color::hex(0xd2691eff)),
        motion: MotionPref::Reduced,
        locale: Locale::new("pt-BR"),
        assistive: Assistive::None,
    };
    assert_eq!(all.over(queried), all);
    // And over a host that answers nothing, the pin is the whole reading.
    assert_eq!(less_motion.over(SystemEnv::default()), less_motion);
}

/// The `system` event reports the merged reading like any other: a pinned
/// `motion` is in every event the OS's own changes raise, and a change to
/// the pinned field itself raises nothing, because the reading did not move.
#[test]
fn the_system_event_carries_the_pinned_reading() {
    let mut core = Core::new();
    let frame = |core: &mut Core, real: SystemEnv, pinned: SystemEnv| {
        // What the runner's `sync_env` writes: the pin over the OS.
        core.env.system = pinned.over(real);
        core.frame(Size::new(100.0, 100.0), 1.0).finish();
        core.take_pending_events()
    };
    let pinned = SystemEnv {
        motion: MotionPref::Reduced,
        ..Default::default()
    };
    let mut real = SystemEnv {
        appearance: Appearance::Light,
        motion: MotionPref::Full,
        ..Default::default()
    };
    assert!(
        kinds(&frame(&mut core, real, pinned)).is_empty(),
        "first frame establishes"
    );
    assert_eq!(
        core.env.system.motion,
        MotionPref::Reduced,
        "pinned from the first view"
    );

    // The user flips the OS to dark: the event carries dark *and* the pin.
    real.appearance = Appearance::Dark;
    let evs = frame(&mut core, real, pinned);
    assert_eq!(kinds(&evs), vec!["system"]);
    let p = &evs[0].payload;
    assert_eq!(p.get_str("appearance"), Some("dark"));
    assert_eq!(p.get_str("motion"), Some("reduced"));

    // The user turns reduce-motion on for real: nothing the view can see
    // changed, so nothing is reported — and off again, likewise.
    real.motion = MotionPref::Reduced;
    assert!(kinds(&frame(&mut core, real, pinned)).is_empty());
    real.motion = MotionPref::Full;
    assert!(kinds(&frame(&mut core, real, pinned)).is_empty());
}
