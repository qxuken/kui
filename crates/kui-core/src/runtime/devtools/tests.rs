//! The panel in a bare core: where it is in the frame, what it takes back
//! and what it hands on, and the three tabs' controls.

use super::*;
use crate::access::{AccessAction, AccessRequest};
use crate::input::{KeyMods, KeyPress};
use crate::spec::{Align, TextStyle};
use crate::ui::Ui;
use crate::widgets;

const VIEWPORT: Size = Size { w: 800.0, h: 600.0 };

/// The app under test: a column with one button, and a root it may
/// configure.
fn view(ui: &mut Ui<'_>, root: Option<NodeSpec>) {
    if let Some(spec) = root {
        ui.configure_root(spec);
    }
    ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0)),
        |ui| {
            widgets::button(ui, "press", Value::map([("kind", Value::str("pressed"))]));
            ui.text("hello", TextStyle::new(12.0));
        },
    );
}

fn frame(core: &mut Core) {
    frame_with(core, None);
}

fn frame_with(core: &mut Core, root: Option<NodeSpec>) {
    let mut ui = core.frame(VIEWPORT, 1.0);
    view(&mut ui, root);
    ui.finish();
}

fn on() -> Core {
    let mut core = Core::new();
    core.set_devtools(true);
    frame(&mut core);
    frame(&mut core);
    core
}

fn chord(c: char) -> InputEvent {
    InputEvent::KeyDown(KeyPress::new(
        KeyCode::Char(c),
        KeyMods {
            ctrl: true,
            shift: true,
            ..Default::default()
        },
    ))
}

fn click_at(core: &mut Core, x: f32, y: f32) -> Vec<UiEvent> {
    let mut evs = core.handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
    evs.extend(core.handle_input(InputEvent::mouse_down(1)));
    evs.extend(core.handle_input(InputEvent::mouse_up()));
    evs
}

fn access_click(core: &mut Core, key: Key) -> Vec<UiEvent> {
    core.handle_input(InputEvent::Access(AccessRequest::new(
        key,
        AccessAction::Click,
    )))
}

fn node(core: &Core, key: Key) -> NodeInfo {
    core.nodes()
        .iter()
        .find(|n| n.key == key)
        .cloned()
        .unwrap_or_else(|| panic!("no node {:016x}", key.0))
}

fn state<T>(core: &Core, f: impl FnOnce(&State) -> T) -> T {
    f(&core.session().state().devtools)
}

/// Off, the frame is what it always was: no wrap, no panel, the app's
/// keys as they were. On, the panel is beside the app and the app's keys
/// have not moved.
#[test]
fn the_app_s_keys_do_not_move_when_the_panel_comes() {
    let mut off = Core::new();
    frame(&mut off);
    let press_off = off.key_of("press").unwrap();
    assert!(off.key_of(DEVTOOLS_KEY).is_none());
    assert_eq!(off.nodes().len(), 0, "no snapshot unless asked");

    let mut core = on();
    let press = core.key_of("press").unwrap();
    assert_eq!(
        press, press_off,
        "the container does not rename the app's nodes"
    );
    assert!(core.key_of(DEVTOOLS_KEY).is_some());
    assert!(core.key_of("kui-devtools/stream").is_some());
    assert!(
        core.key_of("kui-devtools/app").is_none(),
        "the container is not the app's to find"
    );
    // The button is laid out in the app's area, left of the dock.
    core.set_inspect(true);
    frame(&mut core);
    let b = node(&core, press);
    assert!(b.rect.x + b.rect.w <= VIEWPORT.w - DOCK_SIDE_W);
    let dock_key = core.key_of(DEVTOOLS_KEY).unwrap();
    let dock = node(&core, dock_key);
    assert_eq!(dock.rect.x, VIEWPORT.w - DOCK_SIDE_W);
    assert_eq!(dock.origin, OriginId::DEVTOOLS);
    assert_eq!(b.origin, OriginId::HOST);
}

/// The host's `configure_root` is split: layout and paint on the
/// container, what is addressed on the root (decision 2).
#[test]
fn configure_root_is_split_between_root_and_container() {
    let mut core = Core::new();
    core.set_devtools(true);
    core.set_inspect(true);
    let root = || {
        NodeSpec::row()
            .pad(20.0)
            .gap(7.0)
            .bg(Color::hex(0x112233ff))
            .on_key(Value::str("sink"))
            .focusable()
    };
    frame_with(&mut core, Some(root()));
    frame_with(&mut core, Some(root()));
    let nodes = core.nodes().to_vec();
    let r = &nodes[0];
    assert_eq!(r.key, Key::ROOT);
    assert!(
        r.events.iter().any(|(n, _)| *n == "key"),
        "the sink stays on the root"
    );
    assert!(r.flags.contains(&"focusable"));
    // The root itself is the side-by-side row; the app's padding and
    // direction are the container's.
    let app = nodes
        .iter()
        .find(|n| n.key == Key::ROOT.str(APP_KEY))
        .unwrap();
    assert_eq!(app.padding.t, 20.0);
    assert_eq!(app.gap, 7.0);
    assert_eq!(format!("{:?}", app.dir), "Row");
    assert_eq!(app.bg.to_hex(), 0x112233ff);
    assert!(app.events.is_empty());
    // The app's column is the container's child, and the root sink still
    // hears a key with nothing focused: it is addressed by `Key::ROOT`.
    let press = core.key_of("press").unwrap();
    let column = node(&core, press).parent.unwrap();
    assert_eq!(node(&core, column).parent, Some(app.key));
    let evs = core.handle_input(InputEvent::KeyDown(KeyPress::new(
        KeyCode::Char('x'),
        KeyMods::default(),
    )));
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, Key::ROOT);
    assert_eq!(
        evs[0].payload.get("tag").and_then(Value::as_str),
        Some("sink")
    );
}

/// The dock is a focus region: out of the app's ring, entered by
/// `Ctrl+Shift+I` — a chord the core acts on before routing, so it lands
/// with nothing focused and no sink — walked by Tab on its own, and left
/// by the same chord.
#[test]
fn the_dock_is_out_of_the_app_s_ring_and_entered_by_the_chord() {
    let mut core = on();
    let dock = core.key_of(DEVTOOLS_KEY).unwrap();
    let button = core.key_of("press").unwrap();
    core.focus_next(true);
    assert_eq!(core.focus(), Some(button));
    core.focus_next(true);
    assert_eq!(core.focus(), Some(button), "the ring has one member");
    assert_eq!(core.region(), None);
    core.set_focus(None);
    assert!(
        core.handle_input(chord('I')).is_empty(),
        "the chord is nobody's"
    );
    frame(&mut core);
    assert_eq!(core.region(), Some(dock), "Ctrl+Shift+I enters the dock");
    let first = core.focus().expect("the dock's first stop is focused");
    assert!(core.focus_visible());
    core.focus_next(true);
    let second = core.focus().unwrap();
    assert_ne!(second, first);
    assert_ne!(second, button, "Tab walks the dock, not the app");
    assert_eq!(core.region(), Some(dock));
    core.set_focus(Some(button));
    core.set_focus(Some(second));
    core.handle_input(chord('I'));
    frame(&mut core);
    assert_eq!(core.region(), None, "Ctrl+Shift+I leaves");
    assert_eq!(core.focus(), Some(button));
    core.handle_input(chord('I'));
    frame(&mut core);
    assert_eq!(
        core.focus(),
        Some(second),
        "and in again, where the dock had it"
    );
    // Hidden, the chord brings the dock back to have something to enter.
    core.set_devtools_dock(Dock::Off);
    frame(&mut core);
    assert!(core.key_of("kui-devtools/stream").is_none());
    core.handle_input(chord('I'));
    frame(&mut core);
    assert_eq!(core.devtools_dock(), Dock::Right);
    assert_eq!(core.region(), Some(core.key_of(DEVTOOLS_KEY).unwrap()));
}

/// A press on the app's dead space focuses nothing — the panel declares
/// no root sink — so the Tab after it walks the app's ring; a press in
/// the dock's dead space settles the region there, and Tab enters it.
#[test]
fn a_tab_after_a_dead_space_click_walks_the_ring_under_the_pointer() {
    let mut core = on();
    let button = core.key_of("press").unwrap();
    let dock = core.key_of(DEVTOOLS_KEY).unwrap();
    // The Tab a driver sends: the raw press, and the editor channel's
    // key, which is the one that walks the ring.
    let tab = |core: &mut Core| {
        core.handle_input(InputEvent::KeyDown(KeyPress::new(
            KeyCode::Tab,
            KeyMods::default(),
        )));
        core.handle_input(InputEvent::Key(crate::EditKey::Tab, crate::Mods::default()));
        frame(core);
    };
    click_at(&mut core, 5.0, 590.0);
    frame(&mut core);
    assert_eq!(core.focus(), None);
    assert_eq!(core.region(), None);
    tab(&mut core);
    assert_eq!(core.focus(), Some(button));
    click_at(&mut core, 795.0, 590.0);
    frame(&mut core);
    assert_eq!(
        core.region(),
        Some(dock),
        "a press in the dock settles the region"
    );
    tab(&mut core);
    let inside = core.focus().expect("Tab entered the dock");
    assert_ne!(inside, button);
}

/// A click on the app reaches the host and lands in the stream; a click
/// on a panel control is the core's and does not; a chord is the core's
/// whatever has focus, and a named key under the same modifiers is not
/// one.
#[test]
fn events_flow_to_the_host_and_into_the_stream_and_the_panel_s_do_not() {
    let mut core = on();
    let press = core.key_of("press").unwrap();
    let base = core.key_of("kui-devtools/base").unwrap();
    assert_eq!(state(&core, |s| s.base), None);
    let evs = access_click(&mut core, base);
    assert!(evs.is_empty(), "the panel's click never leaves the core");
    assert_eq!(state(&core, |s| s.base), Some(Appearance::Light));
    let evs = access_click(&mut core, press);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, press);
    state(&core, |s| {
        let last = s.stream.back().expect("logged");
        assert_eq!(last.kind, EntryKind::Event);
        assert_eq!(last.key, press);
        assert_eq!(last.label.as_deref(), Some("press"));
        assert_eq!(
            last.payload.get("kind").and_then(Value::as_str),
            Some("pressed")
        );
        // The theme note came before it.
        assert!(s.stream.iter().any(|e| e.kind == EntryKind::Note));
    });
    // Chords, with the button focused: nothing reaches it.
    core.set_focus(Some(press));
    assert!(core.handle_input(chord('D')).is_empty());
    assert_eq!(core.devtools_dock(), Dock::Bottom);
    assert!(core.handle_input(chord('M')).is_empty());
    assert!(state(&core, |s| s.native_menus.is_some()));
    // Ctrl+Shift+Tab is not Ctrl+Shift+T.
    let named = InputEvent::KeyDown(KeyPress::new(
        KeyCode::Tab,
        KeyMods {
            ctrl: true,
            shift: true,
            ..Default::default()
        },
    ));
    core.handle_input(named);
    assert_eq!(state(&core, |s| s.base), Some(Appearance::Light));
    // Off, nothing is intercepted and nothing logged.
    core.set_devtools(false);
    let before = state(&core, |s| s.stream.len());
    assert!(
        core.handle_input(chord('D')).is_empty(),
        "no sink: nobody hears it"
    );
    assert_eq!(
        core.devtools_dock(),
        Dock::Bottom,
        "and the panel did not act"
    );
    frame(&mut core);
    access_click(&mut core, press);
    assert_eq!(state(&core, |s| s.stream.len()), before);
}

/// The theme override is applied at every frame's start and taken away
/// again, restoring the app's own source.
#[test]
fn the_theme_override_comes_and_goes_around_the_app_s_own() {
    let mut core = Core::new();
    core.set_devtools(true);
    core.set_theme_source(ThemeSource::DerivedWithAccent(Color::hex(0x00ff00ff)));
    frame(&mut core);
    core.handle_input(chord('T'));
    frame(&mut core);
    assert!(matches!(core.theme_source(), ThemeSource::Pinned(_)));
    assert_eq!(core.theme().appearance, Appearance::Light);
    core.handle_input(chord('T'));
    frame(&mut core);
    assert_eq!(core.theme().appearance, Appearance::Dark);
    core.handle_input(chord('T'));
    frame(&mut core);
    assert_eq!(
        core.theme_source(),
        ThemeSource::DerivedWithAccent(Color::hex(0x00ff00ff)),
        "back to the app's own"
    );
}

/// The tree tab lists the app's nodes and not the panel's; a row's hover
/// and click outline the node over the app, the click selects it and the
/// inspector opens; the disclosure folds a subtree.
#[test]
fn the_tree_tab_lists_the_app_and_outlines_what_is_picked() {
    let mut core = on();
    core.handle_input(chord('N'));
    frame(&mut core);
    assert_eq!(state(&core, |s| s.tab), Tab::Tree);
    frame(&mut core);
    frame(&mut core);
    let press = core.key_of("press").unwrap();
    let dock_key = core.key_of(DEVTOOLS_KEY).unwrap();
    let rows: Vec<NodeInfo> = core
        .nodes()
        .iter()
        .filter(|n| n.label.as_deref().is_some_and(|l| l.starts_with("node:")))
        .cloned()
        .collect();
    assert!(
        rows.len() >= 3,
        "root, column, button, text: {}",
        rows.len()
    );
    assert!(
        !rows.iter().any(|n| n
            .label
            .as_deref()
            .unwrap()
            .contains(&format!("{:016x}", dock_key.0))),
        "no row lists the dock"
    );
    let press_row = core
        .key_of(&format!("node:{:016x}", press.0))
        .expect("the app's button has a row");
    let r = node(&core, press_row).rect;
    core.handle_input(InputEvent::CursorMoved(Vec2::new(
        r.x + r.w / 2.0,
        r.y + r.h / 2.0,
    )));
    frame(&mut core);
    let hover = core
        .key_of("kui-devtools/outline-hover")
        .expect("the hovered row outlines its node");
    assert_eq!(node(&core, hover).rect.x, node(&core, press).rect.x);
    assert!(core.key_of("kui-devtools/outline-selected").is_none());
    assert!(access_click(&mut core, press_row).is_empty());
    assert_eq!(state(&core, |s| s.selected), Some(press));
    frame(&mut core);
    let outline = core
        .key_of("kui-devtools/outline-selected")
        .expect("outlined");
    let (o, b) = (node(&core, outline), node(&core, press));
    assert_eq!(
        (o.rect.x, o.rect.y, o.rect.w),
        (b.rect.x, b.rect.y, b.rect.w)
    );
    assert!(core.key_of("kui-devtools/inspector").is_some());
    // Fold the app's column: its rows go, the count on the row says so.
    let column = b.parent.unwrap();
    let fold = core
        .key_of(&format!("fold:{:016x}", column.0))
        .expect("the column's disclosure");
    access_click(&mut core, fold);
    frame(&mut core);
    assert!(
        core.key_of(&format!("node:{:016x}", press.0)).is_none(),
        "folded away"
    );
    assert!(state(&core, |s| s.collapsed.contains(&column)));
    // A filter shows the match and its ancestors, folded or not.
    let filter = core.key_of("kui-devtools/tree-filter").unwrap();
    core.set_edit_text(filter, "press");
    frame(&mut core);
    frame(&mut core);
    assert!(core.key_of(&format!("node:{:016x}", press.0)).is_some());
    assert!(core.key_of(&format!("node:{:016x}", column.0)).is_some());
    let hello = core
        .nodes()
        .iter()
        .find(|n| n.text.as_deref() == Some("hello"))
        .map(|n| n.key)
        .unwrap();
    assert!(core.key_of(&format!("node:{:016x}", hello.0)).is_none());
}

/// The picker: an overlay over the app's area, the node under the pointer
/// outlined and named, a press selecting it and reaching nobody else, and
/// Escape leaving.
#[test]
fn the_picker_finds_the_node_under_the_pointer() {
    let mut core = on();
    core.handle_input(chord('P'));
    frame(&mut core);
    assert!(state(&core, |s| s.pick && s.tab == Tab::Tree));
    let overlay = core
        .key_of("kui-devtools/picker")
        .expect("the overlay is up");
    frame(&mut core);
    let o = node(&core, overlay);
    assert_eq!((o.rect.x, o.rect.w), (0.0, VIEWPORT.w - DOCK_SIDE_W));
    let press = core.key_of("press").unwrap();
    let b = node(&core, press).rect;
    core.handle_input(InputEvent::CursorMoved(Vec2::new(
        b.x + b.w / 2.0,
        b.y + b.h / 2.0,
    )));
    frame(&mut core);
    assert_eq!(state(&core, |s| s.pick_hover), Some(press));
    let pick = core.key_of("kui-devtools/outline-pick").expect("outlined");
    assert_eq!(node(&core, pick).rect.x, b.x);
    assert!(core.key_of("kui-devtools/badge").is_some());
    // The press lands on the overlay, not the button.
    let evs = click_at(&mut core, b.x + b.w / 2.0, b.y + b.h / 2.0);
    assert!(evs.is_empty(), "nothing reached the host: {evs:?}");
    assert_eq!(state(&core, |s| (s.selected, s.pick)), (Some(press), false));
    frame(&mut core);
    assert!(core.key_of("kui-devtools/picker").is_none());
    assert!(core.key_of("kui-devtools/outline-selected").is_some());
    // Escape leaves without picking — both halves of the press a driver
    // sends, so the editor channel's does not go on to the app.
    core.handle_input(chord('P'));
    let escape = InputEvent::KeyDown(KeyPress::new(KeyCode::Escape, KeyMods::default()));
    assert!(core.handle_input(escape).is_empty());
    assert!(!state(&core, |s| s.pick));
    assert!(
        core.devtools_intercept(&InputEvent::Key(
            crate::EditKey::Escape,
            crate::Mods::default()
        )),
        "the editor channel's Escape is owed to the picker"
    );
    assert!(
        !core.devtools_intercept(&InputEvent::Key(
            crate::EditKey::Escape,
            crate::Mods::default()
        )),
        "and only once"
    );
    // A headless core never reads the environment for the panel.
    assert!(!Core::new().devtools());
}

/// The events tab: a row per entry, a click opening one into its payload,
/// the filter narrowing the rows, pause holding the stream.
#[test]
fn the_events_tab_opens_a_row_into_its_payload_and_filters() {
    let mut core = on();
    core.set_inspect(true);
    let press = core.key_of("press").unwrap();
    access_click(&mut core, press);
    frame(&mut core);
    let (seq, len) = state(&core, |s| (s.stream.back().unwrap().seq, s.stream.len()));
    assert_eq!(state(&core, |s| s.rows.len()), len);
    let row = core.key_of(&format!("row:{seq}")).expect("the click's row");
    assert!(access_click(&mut core, row).is_empty());
    assert!(state(&core, |s| s.expanded.contains(&seq)));
    frame(&mut core);
    // Open, the row is taller than a closed one, and its payload's line
    // is a text of its own.
    let h = state(&core, |s| s.heights.get(s.rows.len() - 1));
    assert!(h > STREAM_ROW_H, "{h}");
    assert!(
        core.nodes()
            .iter()
            .any(|n| n.text.as_deref() == Some("kind: pressed")),
        "the payload's line"
    );
    // The filter.
    let filter = core.key_of("kui-devtools/stream-filter").unwrap();
    core.set_edit_text(filter, "pressed");
    frame(&mut core);
    frame(&mut core);
    assert_eq!(state(&core, |s| s.rows.len()), 1);
    core.set_edit_text(filter, "");
    // Pause.
    let pause = core.key_of("kui-devtools/pause").unwrap();
    access_click(&mut core, pause);
    let before = state(&core, |s| s.stream.len());
    access_click(&mut core, press);
    assert_eq!(state(&core, |s| s.stream.len()), before, "paused");
    access_click(&mut core, pause);
    access_click(&mut core, press);
    assert_eq!(state(&core, |s| s.stream.len()), before + 1);
    // Clear, by chord.
    core.handle_input(chord('C'));
    assert_eq!(state(&core, |s| s.stream.len()), 0);
    // The cap: the stream is a ring.
    for _ in 0..(STREAM_CAP + 10) {
        access_click(&mut core, press);
    }
    assert!(state(&core, |s| s.stream.len() <= STREAM_CAP));
}

/// Popped out: the main window declares the panel's window under the
/// devtools origin, a core for that window builds nothing the host asks
/// for and the panel instead, events there are the core's, and each side
/// asks the other to redraw.
#[test]
fn the_panel_pops_out_into_a_window_of_its_own() {
    let mut core = on();
    core.set_devtools_dock(Dock::Window);
    frame(&mut core);
    let cmds = core.take_window_commands();
    let open = cmds
        .iter()
        .find_map(|c| match c {
            WindowCommand::Open {
                id, origin, config, ..
            } => Some((*id, *origin, *config)),
            _ => None,
        })
        .expect("the panel's window opens");
    assert_eq!(open.1, OriginId::DEVTOOLS);
    assert_eq!(open.2.size, WINDOW_SIZE);
    assert!(
        core.key_of(DEVTOOLS_KEY).is_some(),
        "the holder for the outlines"
    );
    assert!(core.key_of("kui-devtools/stream").is_none(), "but no dock");
    // The panel's window's core: the host's view builds nothing there.
    let mut panel = Core::new_in(core.session());
    panel.env.window.id = open.0;
    panel.set_inspect(true);
    frame(&mut panel);
    assert!(panel.devtools_window());
    assert_eq!(&*panel.window_name(), DEVTOOLS_WINDOW);
    assert!(
        panel.key_of("press").is_none(),
        "the app's button is not there"
    );
    assert!(
        panel.key_of("kui-devtools/stream").is_some(),
        "the panel is"
    );
    assert_eq!(panel.window_title(), Some("kui devtools — kui"));
    // An event in the main window is logged and asks the panel to redraw.
    let press = core.key_of("press").unwrap();
    // The click, and the `resize` the dock's leaving owed the app.
    let evs = access_click(&mut core, press);
    assert!(evs.iter().any(|e| e.key == press));
    assert!(
        core.take_window_commands()
            .contains(&WindowCommand::Redraw(open.0))
    );
    frame(&mut panel);
    let seq = state(&core, |s| s.stream.back().unwrap().seq);
    assert!(
        panel.key_of(&format!("row:{seq}")).is_some(),
        "the stream in the panel's window has the click"
    );
    // A control in the panel's window acts, reaches no host, and asks the
    // main window to redraw; a resize there is nobody's.
    let tab = panel.key_of("kui-devtools/tab-tree").unwrap();
    assert!(access_click(&mut panel, tab).is_empty());
    assert_eq!(state(&core, |s| s.tab), Tab::Tree);
    assert!(
        panel
            .take_window_commands()
            .contains(&WindowCommand::Redraw(WindowId::MAIN))
    );
    let mut ui = panel.frame(Size::new(500.0, 500.0), 1.0);
    view(&mut ui, None);
    ui.finish();
    assert!(
        panel.take_pending_events().is_empty(),
        "no resize reaches the host"
    );
    // The tree tab in the panel's window lists the main window's nodes.
    frame(&mut core);
    frame(&mut panel);
    assert!(
        panel.key_of(&format!("node:{:016x}", press.0)).is_some(),
        "the main window's button has a row in the panel's window"
    );
    // Closing the window hides the panel and tells the host nothing.
    core.window_closed(open.0);
    let evs = core.take_pending_events();
    assert!(evs.is_empty(), "{evs:?}");
    assert_eq!(core.devtools_dock(), Dock::Off);
}

/// `KUI_DEVTOOLS` spells the switch and the placement.
#[test]
fn the_environment_variable_is_parsed() {
    let parse = |v: &str| {
        let s = State::from_var(Some(v));
        (s.on, s.dock)
    };
    assert_eq!(parse("1"), (true, Dock::Right));
    assert_eq!(parse("true"), (true, Dock::Right));
    assert_eq!(parse("bottom"), (true, Dock::Bottom));
    assert_eq!(parse("window"), (true, Dock::Window));
    assert_eq!(parse("off"), (true, Dock::Off));
    assert_eq!(parse("0"), (false, Dock::Right));
    assert_eq!(parse("nope"), (false, Dock::Right));
    assert!(!State::from_var(None).on);
}

/// `NodeInfo` says which layer a node paints in, and the picker prefers
/// the topmost.
#[test]
fn node_info_carries_the_layer_and_the_picker_reads_it() {
    let mut core = Core::new();
    core.set_inspect(true);
    let build = |core: &mut Core| {
        let mut ui = core.frame(VIEWPORT, 1.0);
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.with_keyed(
                "under",
                NodeSpec::row()
                    .width(Sizing::Fixed(200.0))
                    .height(Sizing::Fixed(200.0)),
                |_| {},
            );
            ui.with_keyed(
                "over",
                NodeSpec::row()
                    .float(
                        crate::spec::FloatConfig::viewport()
                            .at(Align::Start, Align::Start)
                            .self_at(Align::Start, Align::Start)
                            .offset(50.0, 50.0),
                    )
                    .width(Sizing::Fixed(50.0))
                    .height(Sizing::Fixed(50.0)),
                |_| {},
            );
        });
        ui.finish();
    };
    build(&mut core);
    build(&mut core);
    let under = core.key_of("under").unwrap();
    let over = core.key_of("over").unwrap();
    assert_eq!(node(&core, under).layer, 0);
    assert_eq!(node(&core, over).layer, 1);
    assert_eq!(
        tree::pick_target(core.nodes(), Vec2::new(60.0, 60.0)),
        Some(over)
    );
    assert_eq!(
        tree::pick_target(core.nodes(), Vec2::new(10.0, 10.0)),
        Some(under)
    );
}

/// The header has a button per placement, and a docked pane's inner edge
/// is a handle: dragged, the pane follows the pointer on its axis, and the
/// app's area is what is left.
#[test]
fn the_placement_buttons_and_the_resize_handle() {
    let mut core = on();
    core.set_inspect(true);
    let dock_rect = |core: &mut Core| {
        let k = core.key_of(DEVTOOLS_KEY).unwrap();
        node(core, k).rect
    };
    let drag = |core: &mut Core, from: Vec2, to: Vec2| {
        core.handle_input(InputEvent::CursorMoved(from));
        core.handle_input(InputEvent::mouse_down(1));
        core.handle_input(InputEvent::CursorMoved(to));
        core.handle_input(InputEvent::mouse_up());
        frame(core);
        frame(core);
    };
    let handle_center = |core: &mut Core| {
        let k = core.key_of("kui-devtools/resize").unwrap();
        let r = node(core, k).rect;
        Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0)
    };
    // Left: the pane precedes the app, and its handle is on its right edge.
    let left = core.key_of("kui-devtools/dock:left").unwrap();
    assert!(access_click(&mut core, left).is_empty());
    assert_eq!(core.devtools_dock(), Dock::Left);
    frame(&mut core);
    frame(&mut core);
    let r = dock_rect(&mut core);
    assert_eq!((r.x, r.w), (0.0, DOCK_SIDE_W));
    let press = core.key_of("press").unwrap();
    assert!(
        node(&core, press).rect.x >= DOCK_SIDE_W,
        "the app is to the right of it"
    );
    let h = handle_center(&mut core);
    assert!(h.x > DOCK_SIDE_W - 8.0 && h.x < DOCK_SIDE_W);
    drag(&mut core, h, Vec2::new(300.0, h.y));
    assert_eq!(state(&core, |s| s.side_w), 300.0);
    assert_eq!(dock_rect(&mut core).w, 300.0);
    assert!(node(&core, press).rect.x >= 300.0);
    // Right: the handle is on the left edge, and the width is what is
    // left of the window past the pointer.
    let right = core.key_of("kui-devtools/dock:right").unwrap();
    access_click(&mut core, right);
    frame(&mut core);
    frame(&mut core);
    let r = dock_rect(&mut core);
    assert_eq!(
        (r.x, r.w),
        (VIEWPORT.w - 300.0, 300.0),
        "the width is kept across sides"
    );
    let h = handle_center(&mut core);
    assert!(h.x > r.x && h.x < r.x + 8.0);
    drag(&mut core, h, Vec2::new(VIEWPORT.w - 320.0, h.y));
    assert_eq!(dock_rect(&mut core).w, 320.0);
    // Clamped: the app keeps its minimum, the pane keeps its.
    let h = handle_center(&mut core);
    drag(&mut core, h, Vec2::new(0.0, h.y));
    assert_eq!(dock_rect(&mut core).w, VIEWPORT.w - APP_MIN);
    let h = handle_center(&mut core);
    drag(&mut core, h, Vec2::new(VIEWPORT.w, h.y));
    assert_eq!(dock_rect(&mut core).w, SIDE_MIN_W);
    // Bottom: the handle is the top edge.
    let bottom = core.key_of("kui-devtools/dock:bottom").unwrap();
    access_click(&mut core, bottom);
    frame(&mut core);
    frame(&mut core);
    let r = dock_rect(&mut core);
    assert_eq!((r.y, r.h), (VIEWPORT.h - DOCK_BOTTOM_H, DOCK_BOTTOM_H));
    let h = handle_center(&mut core);
    drag(&mut core, h, Vec2::new(h.x, VIEWPORT.h - 200.0));
    assert_eq!(dock_rect(&mut core).h, 200.0);
    assert!(node(&core, press).rect.y + node(&core, press).rect.h <= VIEWPORT.h - 200.0);
    // Undock and close are buttons too; a window has no handle.
    let window = core.key_of("kui-devtools/dock:window").unwrap();
    access_click(&mut core, window);
    assert_eq!(core.devtools_dock(), Dock::Window);
    frame(&mut core);
    assert!(core.key_of("kui-devtools/resize").is_none());
    let close = core.key_of("kui-devtools/dock:off");
    assert!(close.is_none(), "no dock in the main window while undocked");
    core.handle_input(chord('D'));
    assert_eq!(core.devtools_dock(), Dock::Off);
    core.handle_input(chord('D'));
    assert_eq!(
        core.devtools_dock(),
        Dock::Left,
        "the chord walks the same list"
    );
    // `side` still names the right, for the command lines that said it.
    assert_eq!(Dock::parse("side"), Some(Dock::Right));
}

/// The host's viewport is what the dock leaves (ADR 0024): `viewport()`
/// says so, a dock coming, moving or being dragged is a `resize`, the
/// host's viewport floats resolve against it, and under a left dock the
/// coordinates the host is handed are its own.
#[test]
fn the_host_s_viewport_is_the_window_less_the_dock() {
    let mut core = Core::new();
    core.set_inspect(true);
    // A view with a centred viewport float, a context-menu box and a
    // layout report, to read the coordinates back through.
    let view = |ui: &mut Ui<'_>| {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.with_keyed(
                "target",
                NodeSpec::row()
                    .width(Sizing::Fixed(100.0))
                    .height(Sizing::Fixed(50.0))
                    .on_context_menu(Value::str("menu"))
                    .on_layout(Value::str("box")),
                |_| {},
            );
            ui.with_keyed(
                "centred",
                NodeSpec::row()
                    .float(
                        crate::spec::FloatConfig::viewport()
                            .at(Align::Center, Align::Center)
                            .self_at(Align::Center, Align::Center),
                    )
                    .width(Sizing::Fixed(100.0))
                    .height(Sizing::Fixed(100.0)),
                |_| {},
            );
        });
    };
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEWPORT, 1.0);
        view(&mut ui);
        ui.finish();
        core.take_pending_events()
    };
    frame(&mut core);
    assert_eq!(core.viewport(), VIEWPORT);
    let centred = core.key_of("centred").unwrap();
    assert_eq!(node(&core, centred).rect.x, (VIEWPORT.w - 100.0) / 2.0);
    // The dock comes: a resize to the app's area, and the float centres
    // in it.
    core.set_devtools(true);
    let evs = frame(&mut core);
    let resize = evs
        .iter()
        .find(|e| e.payload.get("kind").and_then(Value::as_str) == Some("resize"))
        .expect("the dock is a resize");
    assert_eq!(
        resize.payload.get("width").and_then(Value::as_float),
        Some((VIEWPORT.w - DOCK_SIDE_W) as f64)
    );
    assert_eq!(
        core.viewport(),
        Size::new(VIEWPORT.w - DOCK_SIDE_W, VIEWPORT.h)
    );
    frame(&mut core);
    assert_eq!(
        node(&core, centred).rect.x,
        (VIEWPORT.w - DOCK_SIDE_W - 100.0) / 2.0
    );
    // Dragging the pane is a resize too.
    core.set_devtools_dock(Dock::Bottom);
    let evs = frame(&mut core);
    assert!(evs.iter().any(|e| {
        e.payload.get("kind").and_then(Value::as_str) == Some("resize")
            && e.payload.get("height").and_then(Value::as_float)
                == Some((VIEWPORT.h - DOCK_BOTTOM_H) as f64)
    }));
    // A left dock: the app's origin is the pane's edge, and every
    // coordinate it is handed is relative to it.
    core.set_devtools_dock(Dock::Left);
    let evs = frame(&mut core);
    let layout = evs
        .iter()
        .find(|e| e.payload.get("kind").and_then(Value::as_str) == Some("layout"))
        .expect("the box reports its rect again: it moved");
    assert_eq!(layout.payload.get("x").and_then(Value::as_float), Some(0.0));
    let target = core.key_of("target").unwrap();
    let r = node(&core, target).rect;
    assert_eq!(
        r.x, DOCK_SIDE_W,
        "in window coordinates it sits past the dock"
    );
    assert_eq!(
        node(&core, centred).rect.x,
        DOCK_SIDE_W + (VIEWPORT.w - DOCK_SIDE_W - 100.0) / 2.0
    );
    // A right-click on it: the point comes back in the app's coordinates,
    // and the cursor reads the same way.
    let p = Vec2::new(r.x + 10.0, r.y + 10.0);
    core.handle_input(InputEvent::CursorMoved(p));
    assert_eq!(core.cursor(), Some(Vec2::new(10.0, 10.0)));
    let evs = core.handle_input(InputEvent::MouseDown {
        button: crate::MouseButton::Secondary,
        clicks: 1,
    });
    let menu = evs
        .iter()
        .find(|e| e.payload.get("kind").and_then(Value::as_str) == Some("contextmenu"))
        .expect("the context menu event");
    assert_eq!(menu.payload.get("x").and_then(Value::as_float), Some(10.0));
    // And a popup anchored where the app thinks the box is lands where
    // the box is in the window.
    {
        let mut ui = core.frame(VIEWPORT, 1.0);
        ui.window(
            "menu",
            crate::WindowConfig::popup(Rect::new(0.0, 0.0, 100.0, 50.0), 80.0, 80.0),
        );
        view(&mut ui);
        ui.finish();
    }
    let open = core
        .take_window_commands()
        .into_iter()
        .find_map(|c| match c {
            WindowCommand::Open { config, .. } => Some(config),
            _ => None,
        });
    assert_eq!(open.map(|c| c.anchor.x), Some(DOCK_SIDE_W));
}

/// Moving the panel out of its own window by one of that window's own
/// buttons closes the window without hiding the panel: the close that
/// follows is the panel's own doing, not the user's.
#[test]
fn a_placement_chosen_in_the_panel_s_window_survives_the_window_closing() {
    let mut core = on();
    core.set_devtools_dock(Dock::Window);
    frame(&mut core);
    let id = core
        .take_window_commands()
        .into_iter()
        .find_map(|c| match c {
            WindowCommand::Open { id, .. } => Some(id),
            _ => None,
        })
        .unwrap();
    let mut panel = Core::new_in(core.session());
    panel.env.window.id = id;
    frame(&mut panel);
    let right = panel.key_of("kui-devtools/dock:right").unwrap();
    access_click(&mut panel, right);
    assert_eq!(core.devtools_dock(), Dock::Right);
    // The main window's next frame stops declaring the window: the diff
    // closes it, and the driver may also report the OS close.
    frame(&mut core);
    assert!(
        core.take_window_commands()
            .iter()
            .any(|c| matches!(c, WindowCommand::Close(w) if *w == id))
    );
    core.window_closed(id);
    assert!(
        !core
            .take_pending_events()
            .iter()
            .any(|e| e.payload.get("kind").and_then(Value::as_str) == Some("window")),
        "the host hears nothing of the panel's window"
    );
    assert_eq!(
        core.devtools_dock(),
        Dock::Right,
        "the panel is where it was put"
    );
    frame(&mut core);
    assert!(core.key_of("kui-devtools/stream").is_some());
}
