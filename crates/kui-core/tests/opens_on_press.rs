//! A press that opens a menu hands its press on (backlog C108): an
//! `opens_on_press` node clicks on the way down, and the release — over a
//! `menuItem` of the menu that click put up — clicks the item, as
//! NSPopUpButton's press, drag and release picks an option. A quick click
//! leaves the menu open for a second click; a drag released elsewhere
//! dismisses it. The stock select opens this way.

use kui_core::testing::{self, click, hover, press, release};
use kui_core::{Core, EventKind, Key, NodeSpec, Role, Size, UiEvent, Value, Vec2, widgets};

const VIEW: Size = Size { w: 400.0, h: 300.0 };
const FIELD: Vec2 = Vec2 { x: 50.0, y: 15.0 };
/// The rows: 30 px tall from y 40, "One" then "Two".
const ONE: Vec2 = Vec2 { x: 50.0, y: 55.0 };
const TWO: Vec2 = Vec2 { x: 50.0, y: 85.0 };

/// An app's own select, Kit's shape: a field that opens a modal `menu`
/// float of `menuItem` rows, the open state the app's.
struct App {
    open: bool,
    picked: Option<String>,
    opened: u32,
    /// Where the menu sits: under the field, or over it with its first row
    /// on the field, as a native pop-up opens with the current option there.
    menu_y: f32,
}

impl App {
    fn new() -> Self {
        App {
            open: false,
            picked: None,
            opened: 0,
            menu_y: 40.0,
        }
    }

    fn frame(&mut self, core: &mut Core) {
        let mut ui = core.frame(VIEW, 1.0);
        ui.leaf_keyed(
            "field",
            NodeSpec::row()
                .size(120.0, 30.0)
                .label("Choice")
                .on_click(Value::str("open"))
                .opens_on_press(),
        );
        if self.open {
            ui.with_keyed(
                "menu",
                NodeSpec::column()
                    .float(kui_core::FloatConfig::viewport().offset(0.0, self.menu_y))
                    .modal(Value::str("dismiss"))
                    .role(Role::Menu),
                |ui| {
                    for name in ["One", "Two"] {
                        ui.leaf_keyed(
                            name,
                            NodeSpec::row()
                                .size(120.0, 30.0)
                                .role(Role::MenuItem)
                                .label(name)
                                .on_click(Value::map([("pick", Value::str(name))])),
                        );
                    }
                },
            );
        }
        ui.finish();
    }

    /// What the app does with what it hears.
    fn hear(&mut self, events: Vec<UiEvent>) {
        for e in events {
            if e.event == EventKind::Dismiss {
                self.open = false;
            } else if e.payload.get_str("tag") == Some("open") {
                self.open = !self.open;
                self.opened += 1;
            } else if let Some(name) = e.payload.get_str("pick") {
                self.picked = Some(name.to_string());
                self.open = false;
            }
        }
    }
}

fn ready() -> (Core, App) {
    let mut core = Core::new();
    let mut app = App::new();
    app.frame(&mut core);
    app.frame(&mut core);
    (core, app)
}

#[test]
fn press_drag_and_release_picks_the_option() {
    let (mut core, mut app) = ready();
    let out = press(&mut core, FIELD);
    app.hear(out);
    assert!(app.open, "the press opened it");
    app.frame(&mut core);
    app.hear(hover(&mut core, Vec2::new(50.0, 35.0)));
    app.frame(&mut core);
    app.hear(hover(&mut core, TWO));
    app.frame(&mut core);
    app.hear(release(&mut core));
    assert_eq!(app.picked.as_deref(), Some("Two"), "the release picked");
    assert!(!app.open);
    assert_eq!(
        app.opened, 1,
        "the release was no second click on the field"
    );
}

#[test]
fn a_quick_click_leaves_it_open_for_a_second_click() {
    let (mut core, mut app) = ready();
    // Press and release before a frame is drawn.
    app.hear(click(&mut core, FIELD));
    assert!(app.open);
    assert_eq!(app.opened, 1, "one click, on the press");
    app.frame(&mut core);
    app.frame(&mut core);
    assert!(app.open, "still open after the release");
    app.hear(click(&mut core, ONE));
    assert_eq!(app.picked.as_deref(), Some("One"));
}

#[test]
fn a_quick_click_with_the_current_row_over_the_field_leaves_it_open() {
    // C115: the menu opens with its first row under the pointer; the release
    // that follows a press it never moved from is no pick.
    let (mut core, mut app) = ready();
    app.menu_y = 0.0;
    app.hear(press(&mut core, FIELD));
    assert!(app.open);
    app.frame(&mut core);
    app.frame(&mut core);
    app.hear(release(&mut core));
    assert_eq!(
        app.picked, None,
        "a release where the press was picks nothing"
    );
    assert!(app.open, "still open for a second click");
    app.hear(click(&mut core, FIELD));
    assert_eq!(app.picked.as_deref(), Some("One"), "the second click picks");
}

#[test]
fn a_press_held_and_released_on_the_field_leaves_it_open() {
    let (mut core, mut app) = ready();
    app.hear(press(&mut core, FIELD));
    app.frame(&mut core);
    app.frame(&mut core);
    app.hear(release(&mut core));
    assert!(app.open);
    assert_eq!(app.opened, 1);
}

#[test]
fn a_drag_released_elsewhere_dismisses_it() {
    let (mut core, mut app) = ready();
    app.hear(press(&mut core, FIELD));
    app.frame(&mut core);
    app.hear(hover(&mut core, Vec2::new(300.0, 250.0)));
    app.frame(&mut core);
    app.hear(release(&mut core));
    assert!(!app.open, "dismissed");
    assert_eq!(app.picked, None);
}

#[test]
fn a_press_on_the_field_while_it_is_open_closes_it() {
    let (mut core, mut app) = ready();
    app.hear(click(&mut core, FIELD));
    app.frame(&mut core);
    app.frame(&mut core);
    app.hear(click(&mut core, FIELD));
    app.frame(&mut core);
    assert!(!app.open, "the field is outside the modal");
    assert_eq!(app.opened, 1);
}

#[test]
fn the_keyboard_still_clicks_it_once() {
    let (mut core, mut app) = ready();
    core.set_focus(Some(Key::ROOT.str("field")));
    app.frame(&mut core);
    let out = testing::key_down(&mut core, kui_core::KeyCode::Enter, kui_core::KeyMods::NONE);
    app.hear(out);
    let out = testing::edit_key(&mut core, kui_core::EditKey::Enter, Default::default());
    app.hear(out);
    assert_eq!(app.opened, 1);
}

/// The stock select (`widgets::select`) opens on the press, and a
/// release over a row of its menu chooses it.
#[test]
fn the_stock_select_presses_drags_and_releases() {
    let mut core = Core::new();
    let frame = |core: &mut Core, current: usize| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill().pad(20.0));
        let key = widgets::select(&mut ui, "base", &["app", "light", "dark"], Some(current));
        ui.finish();
        key
    };
    let key = frame(&mut core, 0);
    frame(&mut core, 0);
    let rect = |core: &mut Core, role: Role, name: &str| {
        core.access_tree()
            .nodes
            .iter()
            .find(|n| n.role == role && n.name.as_deref() == Some(name))
            .map(|n| n.rect)
    };
    let field = rect(&mut core, Role::Button, "base").unwrap();
    press(
        &mut core,
        Vec2::new(field.x + field.w / 2.0, field.y + field.h / 2.0),
    );
    assert!(core.menu().is_some(), "the press opened the menu");
    frame(&mut core, 0);
    let row = rect(&mut core, Role::MenuItem, "dark").expect("drawn");
    hover(
        &mut core,
        Vec2::new(row.x + row.w / 2.0, row.y + row.h / 2.0),
    );
    frame(&mut core, 0);
    let out = release(&mut core);
    let chosen = out
        .iter()
        .find(|e| e.event.name() == "menu")
        .expect("{out:?}");
    assert_eq!(chosen.key, key);
    assert_eq!(chosen.data().and_then(|d| d.get_str("item")), Some("dark"));
    assert!(core.menu().is_none());
}
