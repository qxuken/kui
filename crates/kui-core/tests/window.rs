//! Window chrome as data: drag/button roles on nodes turn input into
//! `WindowCommand`s for the driver, never `UiEvent`s — through a live `Core`.

use kui_core::testing::{click_at, drive};
use kui_core::{Core, InputEvent, NodeSpec, Size, Vec2, WindowButton, WindowCommand, WindowId};

/// Builds a custom-chrome-ish frame: a 40px drag strip with min/max/close
/// buttons on the right, over a plain button in the content area.
fn frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with(
        NodeSpec::row().grow_width().height(40.0).window_drag(),
        |ui| {
            ui.leaf(NodeSpec::row().grow_width());
            for b in [
                WindowButton::Minimize,
                WindowButton::Maximize,
                WindowButton::Close,
            ] {
                ui.leaf(NodeSpec::row().width(40.0).grow_height().window_button(b));
            }
        },
    );
    ui.leaf(NodeSpec::row().size(100.0, 30.0).on_click("content-click"));
    ui.finish();
}

#[test]
fn press_on_drag_strip_emits_start_drag_and_no_ui_event() {
    let mut core = Core::new();
    frame(&mut core);
    let evs = drive(
        &mut core,
        &[
            InputEvent::CursorMoved(Vec2::new(100.0, 20.0)),
            InputEvent::mouse_down(1),
        ],
    );
    assert!(
        evs.is_empty(),
        "chrome nodes must not emit UiEvents, got {evs:?}"
    );
    assert_eq!(
        core.take_window_commands(),
        vec![WindowCommand::StartDrag(WindowId::MAIN)]
    );
}

#[test]
fn window_buttons_emit_their_commands_on_click() {
    let mut core = Core::new();
    frame(&mut core);
    // Buttons occupy x 280..320 (min), 320..360 (max), 360..400 (close).
    let mut evs = click_at(&mut core, 300.0, 20.0);
    evs.extend(click_at(&mut core, 340.0, 20.0));
    evs.extend(click_at(&mut core, 380.0, 20.0));
    assert!(
        evs.is_empty(),
        "chrome nodes must not emit UiEvents, got {evs:?}"
    );
    assert_eq!(
        core.take_window_commands(),
        vec![
            WindowCommand::Minimize(WindowId::MAIN),
            WindowCommand::ToggleMaximize(WindowId::MAIN),
            WindowCommand::Close(WindowId::MAIN)
        ]
    );
}

#[test]
fn buttons_win_over_the_drag_strip_below_them() {
    let mut core = Core::new();
    frame(&mut core);
    // A full click on the close button: the press must not also start a drag.
    click_at(&mut core, 380.0, 20.0);
    assert_eq!(
        core.take_window_commands(),
        vec![WindowCommand::Close(WindowId::MAIN)]
    );
}

#[test]
fn press_then_drag_off_a_button_emits_nothing() {
    let mut core = Core::new();
    frame(&mut core);
    drive(
        &mut core,
        &[
            InputEvent::CursorMoved(Vec2::new(380.0, 20.0)),
            InputEvent::mouse_down(1),
            InputEvent::CursorMoved(Vec2::new(200.0, 200.0)),
            InputEvent::mouse_up(),
        ],
    );
    assert!(core.take_window_commands().is_empty());
}

#[test]
fn commands_drain_once() {
    let mut core = Core::new();
    frame(&mut core);
    click_at(&mut core, 380.0, 20.0);
    assert_eq!(core.take_window_commands().len(), 1);
    assert!(core.take_window_commands().is_empty());
}

/// `set_window_size` / `focus_window` are requests, queued behind whatever
/// the chrome produced and drained in that order — the driver applies them
/// on its next pump, and a headless core just keeps them (ADR 0004 decision
/// 5: size is a command and not a declared row, so the frame that follows
/// neither repeats the request nor changes the viewport it was given).
#[test]
fn size_and_focus_requests_queue_in_order_with_chrome_commands() {
    let mut core = Core::new();
    frame(&mut core);
    core.set_window_size(WindowId::MAIN, Size::new(640.0, 480.0));
    click_at(&mut core, 300.0, 20.0); // minimize
    core.focus_window(WindowId::MAIN);
    assert_eq!(
        core.take_window_commands(),
        vec![
            WindowCommand::SetSize {
                window: WindowId::MAIN,
                size: Size::new(640.0, 480.0),
            },
            WindowCommand::Minimize(WindowId::MAIN),
            WindowCommand::Focus(WindowId::MAIN),
        ]
    );
    assert!(core.take_window_commands().is_empty());

    // From a view, through `Ui`, the same queue — and spent by the drain,
    // not re-raised by the frame after.
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.focus_window(WindowId::MAIN);
    ui.set_window_size(WindowId::MAIN, Size::new(800.0, 600.0));
    ui.finish();
    assert_eq!(
        core.take_window_commands(),
        vec![
            WindowCommand::Focus(WindowId::MAIN),
            WindowCommand::SetSize {
                window: WindowId::MAIN,
                size: Size::new(800.0, 600.0),
            },
        ]
    );
    frame(&mut core);
    assert!(core.take_window_commands().is_empty());
    assert_eq!(core.viewport(), Size::new(400.0, 300.0));
}

#[test]
fn plain_on_click_nodes_are_unaffected() {
    let mut core = Core::new();
    frame(&mut core);
    let evs = click_at(&mut core, 50.0, 60.0);
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
    click_at(&mut core, 400.0 - 23.0, 20.0);
    assert_eq!(
        core.take_window_commands(),
        vec![WindowCommand::Close(WindowId::MAIN)]
    );

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
            InputEvent::mouse_down(1),
        ],
    );
    assert_eq!(
        core.take_window_commands(),
        vec![WindowCommand::StartDrag(WindowId::MAIN)]
    );
}
/// The cluster alone in a row that fits its content — the titlebar
/// example's second use of it — is a titlebar tall, and clicks there. A
/// grow child adds nothing to a fit parent's height, and the cluster's row
/// and buttons were all `grow`: 0 px tall, the glyphs hanging out of the
/// row's padding and nothing under them to click (backlog RG50).
#[test]
fn the_cluster_alone_in_a_fitted_row_is_a_titlebar_tall() {
    use kui_core::{WindowEnv, widgets};
    let mut core = Core::new();
    core.set_inspect(true);
    core.env.window = WindowEnv {
        custom_chrome: true,
        ..Default::default()
    };
    let h = core.metrics().titlebar_h;
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let wrap = ui.with_keyed("wrap", NodeSpec::row().pad(6.0), |ui| {
        widgets::window_buttons(ui)
    });
    ui.finish();
    let r = core
        .nodes()
        .into_iter()
        .find(|n| n.key == wrap)
        .expect("the row is laid out")
        .rect;
    assert_eq!(r.h, h + 12.0, "the buttons' height and the padding");
    // Close is the third 46 px button, past the 6 px pad.
    click_at(&mut core, 6.0 + 46.0 * 2.0 + 23.0, 6.0 + h / 2.0);
    assert_eq!(
        core.take_window_commands(),
        vec![WindowCommand::Close(WindowId::MAIN)]
    );
}

/// The other half of the same env read, and the half the corpus cannot pin:
/// the conformance report carries no coordinates, so `chrome-inset`'s
/// checked-in `Expect` can say the buttons went away (a quad count) but not
/// that the title moved (a position). Only the quad *digest* covers that,
/// and the digest is font-dependent and never checked in.
///
/// The numbers are the widget's two branches: a bare 12pt margin with no
/// native controls, and the reported keep-out extent — `r.x + r.w`, the
/// trailing gap already in it — with them.
#[test]
fn titlebar_insets_past_the_native_controls() {
    use kui_core::{Rect, WindowEnv, widgets};
    let title_x = |controls: Option<Rect>| {
        let mut core = Core::new();
        core.env.window = WindowEnv {
            custom_chrome: true,
            native_controls: controls,
            ..Default::default()
        };
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        widgets::titlebar(&mut ui, "app");
        ui.finish();
        core.access_tree()
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("app"))
            .expect("the titlebar draws its title")
            .rect
            .x
    };
    assert_eq!(title_x(None), 12.0);
    assert_eq!(title_x(Some(Rect::new(0.0, 0.0, 78.0, 28.0))), 78.0);
    // The rect is a keep-out area, not a width: an origin that is not the
    // window's still has to be cleared, which is why the widget adds `x`.
    assert_eq!(title_x(Some(Rect::new(4.0, 0.0, 78.0, 28.0))), 82.0);
}

/// The other half of the keep-out rect (backlog W17): where the OS keeps
/// controls of its own over the strip, the strip is the OS's titlebar, as
/// tall as the rect says — its content then centres on buttons the OS
/// centred in *its* bar, which a 34 px strip beside macOS 27's 32 px one
/// did not. Without such controls the strip is the app's alone and the
/// metric's, and `titlebar_height` says the same number the strip drew.
#[test]
fn titlebar_is_as_tall_as_the_os_s_where_the_os_keeps_controls_over_it() {
    use kui_core::{Key, Metrics, Rect, WindowEnv, widgets};
    let strip_h = |controls: Option<Rect>| {
        let mut core = Core::new();
        core.set_inspect(true);
        core.env.window = WindowEnv {
            custom_chrome: true,
            native_controls: controls,
            ..Default::default()
        };
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let read = widgets::titlebar_height(&ui);
        widgets::titlebar(&mut ui, "app");
        ui.finish();
        let strip = Key::ROOT.str("kui:titlebar");
        let drawn = core
            .nodes()
            .iter()
            .find(|n| n.key == strip)
            .expect("the strip is laid out under its own key")
            .rect
            .h;
        assert_eq!(read, drawn, "titlebar_height is what the strip drew");
        drawn
    };
    assert_eq!(strip_h(None), Metrics::default().titlebar_h);
    assert_eq!(strip_h(Some(Rect::new(0.0, 0.0, 78.0, 32.0))), 32.0);
    assert_eq!(strip_h(Some(Rect::new(0.0, 0.0, 78.0, 28.0))), 28.0);
    // A host that reported a width and no height (C's two numbers with
    // the second left 0) gets the metric, not a strip of nothing.
    assert_eq!(
        strip_h(Some(Rect::new(0.0, 0.0, 78.0, 0.0))),
        Metrics::default().titlebar_h
    );
}

/// Backlog W22, off macOS: a launcher's `medium` or `tall` titlebar is the
/// window's platform height, which the `titlebar_h` metric is while it is
/// the stock number — so the strip, the buttons, `ui.metrics()` and
/// `$titlebar_h` read it, and an app's density set (which carries the
/// caption's number) keeps it. A number of the app's own wins, and the
/// app can go back to the stock one.
#[test]
fn a_platform_titlebar_is_the_stock_metric_and_an_app_s_own_number_wins() {
    use kui_core::{Key, Metrics, WindowEnv, widgets};
    let stock = Metrics::default().titlebar_h;
    let strip_and_buttons = |core: &mut Core| {
        core.set_inspect(true);
        core.env.window = WindowEnv {
            custom_chrome: true,
            ..Default::default()
        };
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let token = ui
            .token_length("titlebar_h")
            .expect("a metrics role is a length token");
        assert_eq!(
            token,
            ui.metrics().titlebar_h,
            "`$titlebar_h` is the metric"
        );
        widgets::titlebar(&mut ui, "app");
        ui.finish();
        let strip = Key::ROOT.str("kui:titlebar");
        let strip_h = core
            .nodes()
            .iter()
            .find(|n| n.key == strip)
            .expect("the strip is laid out under its own key")
            .rect
            .h;
        // The drawn buttons are 46 wide; close is the rightmost.
        let close_h = core
            .nodes()
            .iter()
            .filter(|n| n.rect.w == 46.0)
            .max_by(|a, b| a.rect.x.total_cmp(&b.rect.x))
            .expect("custom chrome with no OS controls draws its buttons")
            .rect
            .h;
        (strip_h, close_h)
    };

    let mut core = Core::new();
    core.set_platform_titlebar_h(Some(52.0));
    assert_eq!(core.metrics().titlebar_h, 52.0);
    assert_eq!(strip_and_buttons(&mut core), (52.0, 52.0));
    // A density set carries the caption's number, which stands for the
    // window's.
    core.set_metrics(Metrics::compact());
    assert_eq!(core.metrics().titlebar_h, 52.0);
    assert_eq!(core.metrics().control_text, Metrics::compact().control_text);
    core.set_metrics(Metrics::default().scaled(1.5));
    assert_eq!(core.metrics().titlebar_h, 52.0);
    // A number of the app's own is the app's.
    core.set_metrics(Metrics {
        titlebar_h: 44.0,
        ..Metrics::default()
    });
    assert_eq!(strip_and_buttons(&mut core), (44.0, 44.0));
    // Back to the stock set: the window's height again.
    core.set_metrics(Metrics::default());
    assert_eq!(core.metrics().titlebar_h, 52.0);

    // An app that chose its own before the driver spoke keeps it; a
    // driver that names none leaves the caption's number.
    let mut own = Core::new();
    own.set_metrics(Metrics {
        titlebar_h: 44.0,
        ..Metrics::default()
    });
    own.set_platform_titlebar_h(Some(52.0));
    assert_eq!(own.metrics().titlebar_h, 44.0);
    let mut plain = Core::new();
    plain.set_platform_titlebar_h(None);
    assert_eq!(strip_and_buttons(&mut plain), (stock, stock));
}

/// ADR 0004's step 2: every event a core hands out says which window it came
/// from, and the answer is the id the driver put on `env.window` — the same
/// place `maximized` and the rest of the window facts arrive.
///
/// The producers cannot know it (a hit test has no window), so this is really
/// a test that the stamp covers *both* ways out: `handle_input`, and
/// `take_pending_events` for what a finished frame raised on its own.
#[test]
fn events_carry_the_window_the_driver_declared() {
    let mut core = Core::new();
    frame(&mut core);
    let evs = click_at(&mut core, 50.0, 60.0);
    assert_eq!(evs.len(), 1);
    assert_eq!(
        evs[0].window,
        WindowId::MAIN,
        "the window an app starts in, and 0 across every transport"
    );

    core.env.window.id = WindowId(7);
    // A changed viewport is the pending half: raised at `begin_frame`, taken
    // after `finish_frame`, and never routed through `handle_input`.
    frame(&mut core);
    let mut ui = core.frame(Size::new(500.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.finish();
    let pending = core.take_pending_events();
    assert!(
        pending
            .iter()
            .any(|ev| ev.payload.get("kind").and_then(kui_core::Value::as_str) == Some("resize")),
        "the viewport change should have raised a resize: {pending:?}"
    );
    assert!(
        pending.iter().all(|ev| ev.window == WindowId(7)),
        "pending events are stamped too: {pending:?}"
    );

    // And the other way out. This batch carries both kinds — the click, and
    // the resize back to the original viewport that rode along with it.
    frame(&mut core);
    let evs = click_at(&mut core, 50.0, 60.0);
    assert!(evs.len() > 1, "click and resize: {evs:?}");
    assert!(evs.iter().all(|ev| ev.window == WindowId(7)), "{evs:?}");
}
