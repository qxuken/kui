//! A surface: an island a host draws into its own world, a declared window
//! of kind `Surface` (`docs/adr/0076-an-island-is-a-window-the-host-draws.md`).
//! Driven headless as a game would: the main core declares it, the host
//! hears the `Open` and runs a core for it in the main one's session, builds
//! it at the declared size, clicks it in its own coordinates, resizes it by
//! changing the declaration, and sees it close with the window that
//! declared it.

use kui_core::testing::click;
use kui_core::{Core, NodeSpec, Size, Vec2, WindowCommand, WindowConfig, WindowId, WindowKind};

fn surface(w: f32, h: f32) -> WindowConfig {
    WindowConfig {
        kind: WindowKind::Surface,
        size: Size::new(w, h),
        ..WindowConfig::default()
    }
}

/// A main frame declaring `ring` at `size`, or nothing; its commands.
fn main_frame(core: &mut Core, ring: Option<Size>) -> Vec<WindowCommand> {
    let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
    if let Some(s) = ring {
        ui.window("ring", surface(s.w, s.h));
    }
    ui.finish();
    core.take_window_commands()
}

/// The surface's own frame, at its size and a scale of the host's: a
/// button across its left half.
fn ring_frame(core: &mut Core, size: Size, scale: f32) {
    let mut ui = core.frame(size, scale);
    ui.configure_root(NodeSpec::row().fill());
    ui.leaf_keyed(
        "gate",
        NodeSpec::column().size(100.0, 100.0).on_click("gate"),
    );
    ui.finish();
}

#[test]
fn a_surface_opens_for_the_host_follows_its_declaration_and_closes_with_its_owner() {
    let mut main = Core::new();
    let cmds = main_frame(&mut main, Some(Size::new(200.0, 100.0)));
    let (id, owner, config) = cmds
        .iter()
        .find_map(|c| match *c {
            WindowCommand::Open {
                id, owner, config, ..
            } => Some((id, owner, config)),
            _ => None,
        })
        .expect("the surface opens");
    assert_eq!(config.kind, WindowKind::Surface);
    assert_eq!(config.size, Size::new(200.0, 100.0));
    assert_eq!(owner, WindowId::MAIN);

    // The host's core for it, in the main one's session, at 2x.
    let mut ring = Core::new_in(main.session());
    ring.env.window.id = id;
    ring_frame(&mut ring, config.size, 2.0);
    assert_eq!(&*ring.window_name(), "ring");
    // A click the host un-projected to (40, 50) on the ring.
    let evs = click(&mut ring, Vec2::new(40.0, 50.0));
    assert!(
        evs.iter().any(|e| e.payload.get_str("tag") == Some("gate")),
        "{evs:?}"
    );
    assert!(
        click(&mut ring, Vec2::new(150.0, 50.0)).is_empty(),
        "past the button"
    );

    // The same size again: nothing to say.
    let cmds = main_frame(&mut main, Some(Size::new(200.0, 100.0)));
    assert!(cmds.is_empty(), "{cmds:?}");
    // A new size in the declaration resizes it, where a window's is the
    // user's once it opens.
    let cmds = main_frame(&mut main, Some(Size::new(320.0, 80.0)));
    assert_eq!(
        cmds,
        [WindowCommand::SetSize {
            window: id,
            size: Size::new(320.0, 80.0)
        }]
    );

    // Its owner stops declaring it: it closes.
    let cmds = main_frame(&mut main, None);
    assert_eq!(cmds, [WindowCommand::Close(id)]);
}

#[test]
fn a_normal_window_keeps_its_size_when_its_declaration_changes() {
    let mut main = Core::new();
    let mut ui = main.frame(Size::new(800.0, 600.0), 1.0);
    ui.window("palette", WindowConfig::default());
    ui.finish();
    main.take_window_commands();
    let mut ui = main.frame(Size::new(800.0, 600.0), 1.0);
    ui.window(
        "palette",
        WindowConfig {
            size: Size::new(900.0, 700.0),
            ..WindowConfig::default()
        },
    );
    ui.finish();
    assert!(
        main.take_window_commands().is_empty(),
        "the user owns a window's size"
    );
}
