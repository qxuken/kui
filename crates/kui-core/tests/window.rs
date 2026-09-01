//! Window chrome as data: drag/button roles on nodes turn input into
//! `WindowCommand`s for the driver, never `UiEvent`s — through a live `Core`.

use kui_core::{
    Core, InputEvent, NodeSpec, Size, Sizing, UiEvent, Vec2, WindowButton, WindowCommand,
};

/// Builds a custom-chrome-ish frame: a 40px drag strip with min/max/close
/// buttons on the right, over a plain button in the content area.
fn frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with(
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(40.0))
            .window_drag(),
        |ui| {
            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
            for b in [
                WindowButton::Minimize,
                WindowButton::Maximize,
                WindowButton::Close,
            ] {
                ui.with(
                    NodeSpec::row()
                        .width(Sizing::Fixed(40.0))
                        .height(Sizing::Grow(1.0))
                        .window_button(b),
                    |_| {},
                );
            }
        },
    );
    ui.with(
        NodeSpec::row()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(30.0))
            .on_click("content-click"),
        |_| {},
    );
    ui.finish();
}

fn drive(core: &mut Core, events: &[InputEvent]) -> Vec<UiEvent> {
    let mut out = Vec::new();
    for ev in events {
        out.extend(core.handle_input(ev.clone()));
    }
    out
}

fn click_at(x: f32, y: f32) -> [InputEvent; 3] {
    [
        InputEvent::CursorMoved(Vec2::new(x, y)),
        InputEvent::MouseDown(1),
        InputEvent::MouseUp,
    ]
}

#[test]
fn press_on_drag_strip_emits_start_drag_and_no_ui_event() {
    let mut core = Core::new();
    frame(&mut core);
    let evs = drive(
        &mut core,
        &[
            InputEvent::CursorMoved(Vec2::new(100.0, 20.0)),
            InputEvent::MouseDown(1),
        ],
    );
    assert!(
        evs.is_empty(),
        "chrome nodes must not emit UiEvents, got {evs:?}"
    );
    assert_eq!(core.take_window_commands(), vec![WindowCommand::StartDrag]);
}

#[test]
fn window_buttons_emit_their_commands_on_click() {
    let mut core = Core::new();
    frame(&mut core);
    // Buttons occupy x 280..320 (min), 320..360 (max), 360..400 (close).
    let mut evs = drive(&mut core, &click_at(300.0, 20.0));
    evs.extend(drive(&mut core, &click_at(340.0, 20.0)));
    evs.extend(drive(&mut core, &click_at(380.0, 20.0)));
    assert!(
        evs.is_empty(),
        "chrome nodes must not emit UiEvents, got {evs:?}"
    );
    assert_eq!(
        core.take_window_commands(),
        vec![
            WindowCommand::Minimize,
            WindowCommand::ToggleMaximize,
            WindowCommand::Close
        ]
    );
}

#[test]
fn buttons_win_over_the_drag_strip_below_them() {
    let mut core = Core::new();
    frame(&mut core);
    // A full click on the close button: the press must not also start a drag.
    drive(&mut core, &click_at(380.0, 20.0));
    assert_eq!(core.take_window_commands(), vec![WindowCommand::Close]);
}

#[test]
fn press_then_drag_off_a_button_emits_nothing() {
    let mut core = Core::new();
    frame(&mut core);
    drive(
        &mut core,
        &[
            InputEvent::CursorMoved(Vec2::new(380.0, 20.0)),
            InputEvent::MouseDown(1),
            InputEvent::CursorMoved(Vec2::new(200.0, 200.0)),
            InputEvent::MouseUp,
        ],
    );
    assert!(core.take_window_commands().is_empty());
}

#[test]
fn commands_drain_once() {
    let mut core = Core::new();
    frame(&mut core);
    drive(&mut core, &click_at(380.0, 20.0));
    assert_eq!(core.take_window_commands().len(), 1);
    assert!(core.take_window_commands().is_empty());
}

#[test]
fn plain_on_click_nodes_are_unaffected() {
    let mut core = Core::new();
    frame(&mut core);
    let evs = drive(&mut core, &click_at(50.0, 60.0));
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].payload.as_str(), Some("content-click"));
    assert!(core.take_window_commands().is_empty());
}

#[test]
fn titlebar_widget_declares_chrome_from_env() {
    use kui_core::{Rect, WindowEnv, widgets};
    let mut core = Core::new();
    // Custom chrome, no native controls: the widget draws its own buttons.
    core.env.window = WindowEnv {
        custom_chrome: true,
        ..Default::default()
    };
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    widgets::titlebar(&mut ui, "app");
    ui.finish();
    // Close button sits rightmost in the 46px-wide cluster.
    drive(&mut core, &click_at(400.0 - 23.0, 20.0));
    assert_eq!(core.take_window_commands(), vec![WindowCommand::Close]);

    // macOS-style: native controls present — no drawn buttons, all drag.
    core.env.window = WindowEnv {
        custom_chrome: true,
        native_controls: Some(Rect::new(0.0, 0.0, 78.0, 28.0)),
        ..Default::default()
    };
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    widgets::titlebar(&mut ui, "app");
    ui.finish();
    drive(
        &mut core,
        &[
            InputEvent::CursorMoved(Vec2::new(400.0 - 23.0, 20.0)),
            InputEvent::MouseDown(1),
        ],
    );
    assert_eq!(core.take_window_commands(), vec![WindowCommand::StartDrag]);
}
