//! A host's questions about the pointer and the keyboard (backlog F233):
//! `Core::hit`, `Core::pointer_wants`, `Core::keyboard_wants` and the
//! `host_hit` row. From Replicator Bloom, a game that draws a transparent
//! kui layer over its world and had no way to ask whether a click was the
//! UI's: it declared a catch-all `GROW` box under everything and heard the
//! world's click as that box's event.

use kui_core::testing::{click, hover, press, release};
use kui_core::{
    Align, Color, Core, EditOptions, FloatConfig, HostHit, Key, Menu, MenuItem, NodeSpec, Size,
    TextStyle, Vec2,
};

const VIEW: Size = Size { w: 400.0, h: 300.0 };
const PANEL: Color = Color {
    r: 0.1,
    g: 0.1,
    b: 0.1,
    a: 0.8,
};

/// A box at (`x`, `y`) on a transparent root, `w`×`h`.
fn at(x: f32, y: f32, w: f32, h: f32) -> NodeSpec {
    NodeSpec::column().size(w, h).float(
        FloatConfig::parent()
            .inside(Align::Start, Align::Start)
            .offset(x, y),
    )
}

/// One frame of a transparent root holding what `f` declares.
fn frame(core: &mut Core, f: impl FnOnce(&mut kui_core::Ui<'_>)) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    f(&mut ui);
    ui.finish();
}

#[test]
fn a_painted_panel_blocks_and_the_world_around_it_does_not() {
    let mut core = Core::new();
    let mut panel = Key::ROOT;
    frame(&mut core, |ui| {
        panel = ui.leaf_keyed("panel", at(20.0, 20.0, 100.0, 80.0).bg(PANEL));
        ui.with_keyed("label", at(200.0, 20.0, 100.0, 40.0), |ui| {
            ui.text("33.61M", TextStyle::default());
        });
    });
    assert_eq!(core.hit(50.0, 50.0), Some(panel), "a background takes it");
    assert_eq!(core.hit(300.0, 200.0), None, "the root paints nothing");
    assert_eq!(core.hit(220.0, 30.0), None, "nor does bare text");
    assert_eq!(core.hit(-5.0, 50.0), None, "outside the window");
}

#[test]
fn the_topmost_answers_a_button_over_a_panel_and_a_panel_over_a_button() {
    let mut core = Core::new();
    let (mut panel, mut button, mut cover) = (Key::ROOT, Key::ROOT, Key::ROOT);
    frame(&mut core, |ui| {
        panel = ui.with_keyed("panel", at(20.0, 20.0, 200.0, 120.0).bg(PANEL), |ui| {
            button = ui.leaf_keyed("go", NodeSpec::column().size(60.0, 30.0).on_click("go"));
        });
        // A sheet floated over the right half of everything after it.
        cover = ui.leaf_keyed("cover", at(200.0, 0.0, 200.0, 300.0).bg(PANEL));
    });
    assert_eq!(
        core.hit(30.0, 30.0),
        Some(button),
        "the button over its panel"
    );
    assert_eq!(core.hit(150.0, 100.0), Some(panel));
    assert_eq!(
        core.hit(210.0, 100.0),
        Some(cover),
        "the sheet over the panel"
    );
}

#[test]
fn pass_lets_a_painted_box_through_and_never_a_hit_region() {
    let mut core = Core::new();
    let mut button = Key::ROOT;
    frame(&mut core, |ui| {
        button = ui.leaf_keyed("go", at(20.0, 20.0, 60.0, 30.0).bg(PANEL).on_click("go"));
        // A damage flash over the whole window, after the button.
        ui.leaf_keyed(
            "flash",
            at(0.0, 0.0, 400.0, 300.0)
                .bg(Color {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.2,
                })
                .host_hit(HostHit::Pass),
        );
        // A button that says pass is still kui's.
        ui.leaf_keyed(
            "stubborn",
            at(300.0, 20.0, 60.0, 30.0)
                .on_click("stubborn")
                .host_hit(HostHit::Pass),
        );
    });
    assert_eq!(
        core.hit(200.0, 200.0),
        None,
        "the flash lets the world through"
    );
    assert_eq!(
        core.hit(30.0, 30.0),
        Some(button),
        "and the button under it stays"
    );
    assert!(
        core.hit(310.0, 30.0).is_some(),
        "a hit region blocks whatever it says"
    );
    // kui's own clicks are untouched.
    let evs = click(&mut core, Vec2::new(30.0, 30.0));
    assert!(evs.iter().any(|e| e.payload.get_str("tag") == Some("go")));
}

#[test]
fn block_takes_the_pointer_where_nothing_paints() {
    let mut core = Core::new();
    let mut catcher = Key::ROOT;
    frame(&mut core, |ui| {
        catcher = ui.leaf_keyed(
            "minimap",
            at(300.0, 200.0, 100.0, 100.0).host_hit(HostHit::Block),
        );
        ui.leaf_keyed("plain", at(20.0, 20.0, 100.0, 100.0));
    });
    assert_eq!(core.hit(350.0, 250.0), Some(catcher));
    assert_eq!(core.hit(50.0, 50.0), None);
}

#[test]
fn a_rounded_panel_lets_its_corners_through_and_a_clip_cuts_it() {
    let mut core = Core::new();
    let mut inner = Key::ROOT;
    frame(&mut core, |ui| {
        ui.leaf_keyed("pill", at(20.0, 20.0, 100.0, 40.0).bg(PANEL).radius(20.0));
        // A clipping strip 50 wide holding a panel 200 wide.
        ui.with_keyed("strip", at(200.0, 100.0, 50.0, 50.0).clip(), |ui| {
            inner = ui.leaf_keyed("wide", NodeSpec::column().size(200.0, 50.0).bg(PANEL));
        });
    });
    assert!(core.hit(70.0, 40.0).is_some(), "the pill's middle");
    assert_eq!(core.hit(21.0, 21.0), None, "past its corner's arc");
    assert_eq!(core.hit(220.0, 120.0), Some(inner));
    assert_eq!(core.hit(300.0, 120.0), None, "the part the strip cuts off");
}

#[test]
fn the_pointer_is_the_ui_s_over_it_and_while_a_press_holds_it() {
    let mut core = Core::new();
    let build = |core: &mut Core| {
        frame(core, |ui| {
            ui.leaf_keyed("panel", at(20.0, 20.0, 100.0, 80.0).bg(PANEL));
            ui.leaf_keyed("go", at(200.0, 20.0, 60.0, 30.0).on_click("go"));
        })
    };
    build(&mut core);
    assert!(!core.pointer_wants(), "no cursor yet");
    hover(&mut core, Vec2::new(50.0, 50.0));
    assert!(core.pointer_wants());
    hover(&mut core, Vec2::new(300.0, 200.0));
    assert!(!core.pointer_wants(), "over the world");
    // A press on the button, dragged out over the world: its release is
    // the button's, so the pointer stays kui's until it comes.
    hover(&mut core, Vec2::new(210.0, 30.0));
    press(&mut core, Vec2::new(210.0, 30.0));
    hover(&mut core, Vec2::new(300.0, 200.0));
    assert!(core.pointer_wants(), "held");
    release(&mut core);
    build(&mut core);
    assert!(!core.pointer_wants(), "let go over the world");
}

fn items() -> Vec<MenuItem> {
    vec![MenuItem::new("Copy").id("copy")]
}

#[test]
fn an_open_menu_and_a_modal_take_the_pointer_everywhere() {
    let mut core = Core::new();
    let mut card = Key::ROOT;
    frame(&mut core, |ui| {
        card = ui.leaf_keyed("card", at(20.0, 20.0, 100.0, 80.0).bg(PANEL));
    });
    hover(&mut core, Vec2::new(300.0, 250.0));
    assert!(!core.pointer_wants());
    core.open_menu(Menu::new(card, Vec2::new(20.0, 20.0), items()));
    frame(&mut core, |ui| {
        ui.leaf_keyed("card", at(20.0, 20.0, 100.0, 80.0).bg(PANEL));
    });
    assert!(core.pointer_wants(), "a press outside closes the menu");
    core.close_menu();
    frame(&mut core, |ui| {
        ui.leaf_keyed(
            "dialog",
            at(100.0, 100.0, 100.0, 50.0).bg(PANEL).modal("close"),
        );
    });
    assert!(core.pointer_wants(), "nothing behind a modal takes input");
    frame(&mut core, |_| {});
    assert!(!core.pointer_wants());
}

#[test]
fn the_keyboard_is_the_ui_s_while_an_editor_or_a_sink_has_focus() {
    let mut core = Core::new();
    let build = |core: &mut Core, sink: bool| {
        frame(core, |ui| {
            ui.text_edit(
                "name",
                "",
                &EditOptions::default(),
                at(20.0, 20.0, 150.0, 30.0).bg(PANEL),
            );
            ui.leaf_keyed("go", at(200.0, 20.0, 60.0, 30.0).on_click("go"));
            if sink {
                ui.leaf_keyed(
                    "term",
                    at(20.0, 100.0, 200.0, 100.0).on_key("key").focusable(),
                );
            }
        })
    };
    build(&mut core, true);
    assert!(!core.keyboard_wants(), "nothing focused");
    click(&mut core, Vec2::new(30.0, 30.0));
    build(&mut core, true);
    assert!(core.keyboard_wants(), "typing goes to the editor");
    click(&mut core, Vec2::new(210.0, 30.0));
    build(&mut core, true);
    assert!(
        !core.keyboard_wants(),
        "a focused button leaves WASD to the game"
    );
    click(&mut core, Vec2::new(30.0, 120.0));
    build(&mut core, true);
    assert!(core.keyboard_wants(), "a key sink takes every key");
}
