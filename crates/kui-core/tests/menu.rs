//! The stock context menu (`docs/adr/0017-selection-as-a-scope.md`,
//! decision 5): what the core draws when one is open, what choosing a row
//! does, and what an app hears about either.

use kui_core::testing::click;
use kui_core::{
    Core, InputEvent, Key, Menu, MenuAction, MenuItem, MenuRole, MouseButton, NodeSpec, Size,
    TextStyle, Value, Vec2,
};

fn style() -> TextStyle {
    TextStyle::new(14.0)
}

/// A card of selectable text, and whatever menu the core has open drawn
/// over it by `Ui::finish`.
fn frame(core: &mut Core) -> Key {
    frame_in(core, Size::new(400.0, 300.0))
}

/// The same card in a window of `size`.
fn frame_in(core: &mut Core, size: Size) -> Key {
    let mut ui = core.frame(size, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let scope = ui.with_keyed("card", NodeSpec::column().grow_width().selectable(), |ui| {
        ui.text("one", style());
        ui.text("two", style());
    });
    ui.finish();
    scope
}

fn items() -> Vec<MenuItem> {
    vec![
        MenuItem::role(MenuRole::Copy),
        MenuItem::separator(),
        MenuItem::new("Inspect").id(Value::str("inspect")),
    ]
}

/// The middle of the stock row that reads `label`, from the access tree —
/// which is where a laid-out rect is readable without a query of its own,
/// and which doubles as a check that the row is in that tree at all.
fn row_center(core: &mut Core, label: &str) -> Vec2 {
    let tree = core.access_tree().clone();
    let node = tree
        .nodes
        .iter()
        .find(|n| n.role == kui_core::Role::MenuItem && n.name.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no menu row named {label:?}"));
    let r = node.rect;
    Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0)
}

/// The menu's own root rect, the same way.
fn menu_rect(core: &mut Core) -> Option<kui_core::Rect> {
    let tree = core.access_tree().clone();
    tree.nodes
        .iter()
        .find(|n| n.role == kui_core::Role::Menu)
        .map(|n| n.rect)
}

#[test]
fn an_open_menu_is_drawn_by_the_frame_that_follows() {
    let mut core = Core::new();
    let scope = frame(&mut core);
    assert!(core.menu().is_none());
    core.open_menu(Menu::new(scope, Vec2::new(40.0, 30.0), items()));
    // State, not paint: the menu shows up in the next frame, like
    // everything else the core retains.
    frame(&mut core);
    let rect = menu_rect(&mut core).expect("the menu was drawn");
    assert_eq!((rect.x, rect.y), (40.0, 30.0), "at the point it opened at");
    assert!(rect.w > 0.0 && rect.h > 0.0);
}

#[test]
fn choosing_a_custom_row_posts_it_on_the_node_the_menu_was_about() {
    let mut core = Core::new();
    let scope = frame(&mut core);
    core.open_menu(Menu::new(scope, Vec2::new(40.0, 30.0), items()));
    frame(&mut core);
    let at = row_center(&mut core, "Inspect");
    let events = click(&mut core, at);
    assert_eq!(
        events.len(),
        1,
        "one event, not a raw click too: {events:?}"
    );
    let ev = &events[0];
    assert_eq!(ev.key, scope, "posted on the menu's target");
    assert_eq!(ev.kind(), Some("menu"));
    assert_eq!(ev.payload.get_str("role"), Some("custom"));
    assert_eq!(ev.payload.get_str("item"), Some("inspect"));
    assert!(core.menu().is_none(), "choosing closes it");
}

#[test]
fn copy_puts_the_selection_on_the_hosts_clipboard() {
    let mut core = Core::new();
    let scope = frame(&mut core);
    core.select_all_in(scope);
    core.open_menu(Menu::new(scope, Vec2::new(40.0, 30.0), items()));
    frame(&mut core);
    let at = row_center(&mut core, "Copy");
    let events = click(&mut core, at);
    assert_eq!(
        core.take_menu_actions(),
        vec![MenuAction::SetClipboard {
            text: "one\ntwo".into(),
            html: Some("one<br>two".into()),
        }],
        "the core works out what to copy; the clipboard stays the host's"
    );
    assert_eq!(
        events[0].payload.get_str("role"),
        Some("copy"),
        "a standard item posts too, so an app can hear it"
    );
}

#[test]
fn a_separator_takes_no_click_and_no_focus() {
    let mut core = Core::new();
    let scope = frame(&mut core);
    core.open_menu(Menu::new(scope, Vec2::new(40.0, 30.0), items()));
    frame(&mut core);
    // The divider sits between the two rows the tree does carry.
    let copy = row_center(&mut core, "Copy");
    let inspect = row_center(&mut core, "Inspect");
    let at = Vec2::new(copy.x, (copy.y + inspect.y) / 2.0);
    let events = click(&mut core, at);
    assert!(events.is_empty(), "a divider is paint: {events:?}");
    assert!(core.menu().is_some(), "and it does not close the menu");
}

#[test]
fn a_press_outside_dismisses_it_and_the_app_hears_nothing() {
    let mut core = Core::new();
    let scope = frame(&mut core);
    core.open_menu(Menu::new(scope, Vec2::new(40.0, 30.0), items()));
    frame(&mut core);
    // Far from the menu: the modal rules make this a dismissal, and the
    // core keeps that to itself rather than reporting a modal nobody
    // declared.
    let events = click(&mut core, Vec2::new(380.0, 280.0));
    assert!(events.is_empty(), "{events:?}");
    assert!(core.menu().is_none());
    frame(&mut core);
    assert!(
        menu_rect(&mut core).is_none(),
        "and the next frame does not draw it"
    );
}

#[test]
fn escape_dismisses_it_too() {
    let mut core = Core::new();
    let scope = frame(&mut core);
    core.open_menu(Menu::new(scope, Vec2::new(40.0, 30.0), items()));
    frame(&mut core);
    let events = core.handle_input(InputEvent::Key(
        kui_core::EditKey::Escape,
        kui_core::Mods::default(),
    ));
    assert!(events.is_empty(), "{events:?}");
    assert!(core.menu().is_none());
}

#[test]
fn an_editors_selection_survives_the_menu_that_is_about_it() {
    // The rows take focus (the arrow keys are theirs), so by the time Cut
    // runs the field is not focused any more — and Cut still has to work.
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let edit = ui.text_edit(
        "field",
        "typed text",
        &kui_core::EditOptions {
            autofocus: true,
            ..Default::default()
        },
        NodeSpec::column().grow_width(),
    );
    ui.finish();
    core.handle_input(InputEvent::Key(
        kui_core::EditKey::SelectAll,
        kui_core::Mods::default(),
    ));
    core.open_menu(Menu::new(
        edit,
        Vec2::new(40.0, 30.0),
        vec![MenuItem::role(MenuRole::Cut)],
    ));
    // Two frames: one to draw the menu, and the click needs the first.
    let view = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.text_edit(
            "field",
            "typed text",
            &kui_core::EditOptions::default(),
            NodeSpec::column().grow_width(),
        );
        ui.finish();
    };
    view(&mut core);
    let at = row_center(&mut core, "Cut");
    let evs = click(&mut core, at);
    assert_eq!(
        core.take_menu_actions(),
        vec![MenuAction::SetClipboard {
            text: "typed text".into(),
            html: None,
        }]
    );
    assert_eq!(
        core.edit_text(edit).as_deref(),
        Some(""),
        "and the text is gone from the field"
    );
    // AR15: a cut is an edit, and the app mirroring the field through
    // `changed` hears it — beside the `menu` event the chosen row posts,
    // which is not the app's click.
    let changed: Vec<_> = evs.iter().filter(|e| e.kind() == Some("changed")).collect();
    assert_eq!(changed.len(), 1, "{evs:?}");
    assert_eq!(changed[0].key, edit);
}

#[test]
fn a_disabled_row_is_inert() {
    let mut core = Core::new();
    let scope = frame(&mut core);
    core.open_menu(Menu::new(
        scope,
        Vec2::new(40.0, 30.0),
        vec![MenuItem::role(MenuRole::Paste).enabled(false)],
    ));
    frame(&mut core);
    let at = row_center(&mut core, "Paste");
    let events = click(&mut core, at);
    assert!(events.is_empty(), "{events:?}");
    assert!(core.menu().is_some());
    assert!(core.take_menu_actions().is_empty());
}

/// A standard row takes its role's accelerator the way an empty label
/// takes the role's wording, and an item that spells its own keeps it.
#[test]
fn a_standard_row_reads_the_shortcut_its_role_has() {
    let mac = cfg!(target_os = "macos");
    assert_eq!(
        MenuRole::Copy.default_accel(),
        if mac { "⌘C" } else { "Ctrl+C" }
    );
    assert_eq!(
        MenuItem::role(MenuRole::SelectAll).accel_text(),
        Some(if mac { "⌘A" } else { "Ctrl+A" })
    );
    // An app's own row has no shortcut the core could know, and Look Up
    // belongs to the platform that draws its own menu anyway.
    assert_eq!(MenuItem::new("Inspect").accel_text(), None);
    assert_eq!(MenuItem::role(MenuRole::LookUp).accel_text(), None);
    // Declared beats derived, including on a standard row.
    assert_eq!(
        MenuItem::role(MenuRole::Copy).accel("F2").accel_text(),
        Some("F2")
    );
}

/// The accelerator is paint. `menuItem` is a name-from-content role, so a
/// row that stopped naming itself would start reading "Copy Ctrl+C" — or,
/// worse, hand a reader the shortcut as a row of its own. Neither: the
/// name is the label, and the second text run adds no node.
#[test]
fn an_accelerator_is_drawn_and_is_not_read_as_part_of_the_row() {
    let mut core = Core::new();
    let scope = frame(&mut core);
    core.open_menu(Menu::new(
        scope,
        Vec2::new(40.0, 30.0),
        vec![
            MenuItem::role(MenuRole::Copy),
            MenuItem::new("Inspect").accel("F12"),
        ],
    ));
    frame(&mut core);
    assert_eq!(rows(&mut core), ["Copy", "Inspect"]);
    let tree = core.access_tree().clone();
    let statics = tree
        .nodes
        .iter()
        .filter(|n| n.role == kui_core::Role::StaticText)
        .count();
    assert_eq!(statics, 2, "the card's two lines, and no accelerator rows");
}

/// A portable accelerator is drawn the way the platform writes it, as the
/// menu bar's already was: `"mod+shift+n"` was drawn as those eleven
/// characters (backlog F127, from Noticon). A spelling kui cannot parse
/// is the app's own hint and is drawn as written (ADR 0018, decision 7).
#[test]
fn a_portable_accelerator_reads_as_the_platform_writes_it() {
    let mac = cfg!(target_os = "macos");
    let new = if mac {
        "\u{21e7}\u{2318}N"
    } else {
        "Ctrl+Shift+N"
    };
    let row = MenuItem::new("New folder").accel("mod+shift+n");
    assert_eq!(row.accel_label().as_deref(), Some(new));
    assert_eq!(
        row.accel_text(),
        Some("mod+shift+n"),
        "the declaration stays"
    );
    assert_eq!(
        MenuItem::new("Go to definition")
            .accel("gd")
            .accel_label()
            .as_deref(),
        Some("gd")
    );
    // Already the platform's: read the same again.
    assert_eq!(
        MenuItem::new("New").accel(new).accel_label().as_deref(),
        Some(new)
    );

    // And what a host showing the menu itself reads back is the same
    // string, so the drawn menu and an `NSMenu` agree.
    let mut core = Core::new();
    let scope = frame(&mut core);
    core.open_menu(Menu::new(
        scope,
        Vec2::new(40.0, 30.0),
        vec![row, MenuItem::new("Peek").accel("gd")],
    ));
    let menu = core.menu().unwrap();
    assert_eq!(menu.items[0].accel.as_deref(), Some(new));
    assert_eq!(menu.items[1].accel.as_deref(), Some("gd"));
}

/// The menu is as wide as its widest label and widest accelerator need,
/// with a gap between them, and never wraps either: a long accelerator
/// used to wrap onto a second line inside the menu's fixed width
/// (backlog F127). A menu of short rows keeps the metric's width.
#[test]
fn the_menu_widens_to_its_widest_row_and_wraps_nothing() {
    let mut core = Core::new();
    // A window wide enough that RG150's ceiling (the window less a margin)
    // never bites: the long row with its accelerator spelled out as a word
    // each, as Windows and Linux do, is wider than the 400 px frame.
    let scope = frame_in(&mut core, Size::new(800.0, 300.0));
    core.open_menu(Menu::new(
        scope,
        Vec2::new(10.0, 10.0),
        vec![MenuItem::new("Open"), MenuItem::new("Close")],
    ));
    frame_in(&mut core, Size::new(800.0, 300.0));
    let narrow = menu_rect(&mut core).unwrap();
    let row_h = row_rect(&mut core, "Open").h;
    assert_eq!(
        narrow.w,
        kui_core::widgets::MENU_WIDTH,
        "short rows keep the menu at the metric's width"
    );

    core.open_menu(Menu::new(
        scope,
        Vec2::new(10.0, 10.0),
        vec![
            MenuItem::new("Open"),
            MenuItem::new("Move the note to the trash, for good").accel("ctrl+alt+shift+backspace"),
        ],
    ));
    frame_in(&mut core, Size::new(800.0, 300.0));
    let wide = menu_rect(&mut core).unwrap();
    let long = row_rect(&mut core, "Move the note to the trash, for good");
    assert!(
        wide.w > kui_core::widgets::MENU_WIDTH,
        "widened for the long row: {} wide",
        wide.w
    );
    assert_eq!(
        long.h, row_h,
        "the long row is one line, as the short one is"
    );
    assert_eq!(
        row_rect(&mut core, "Open").w,
        long.w,
        "every row as wide as the menu"
    );

    // The accelerator sits on the row, right-aligned, clear of the label:
    // read off the glyphs drawn on the long row's line.
    // Quads are in physical px, which are logical ones at scale 1.
    use kui_core::display::QuadKind;
    let (dl, _) = core.output();
    let glyphs: Vec<kui_core::Rect> = dl
        .quads
        .iter()
        .filter(|q| matches!(q.kind, QuadKind::GlyphMask | QuadKind::GlyphSubpixel))
        .map(|q| q.rect)
        .filter(|r| r.y >= long.y && r.y + r.h <= long.y + long.h + 1.0)
        .collect();
    // The accelerator's spelling is the platform's - four glyphs on macOS
    // (`⌃⌥⇧⌫`), a word each elsewhere - so count what this one draws.
    let ink = |s: &str| s.chars().filter(|c| !c.is_whitespace()).count();
    let want = ink("Move the note to the trash, for good")
        + ink(&kui_core::Accel::label("ctrl+alt+shift+backspace"));
    assert!(
        glyphs.len() >= want,
        "the label and the accelerator drawn: {} of {want}",
        glyphs.len()
    );
    let right = glyphs.iter().map(|r| r.x + r.w).fold(f32::MIN, f32::max);
    assert!(
        right <= long.x + long.w,
        "nothing drawn past the row: {right} > {}",
        long.x + long.w
    );
}

/// The rect of the stock row that reads `label`.
fn row_rect(core: &mut Core, label: &str) -> kui_core::Rect {
    let tree = core.access_tree().clone();
    tree.nodes
        .iter()
        .find(|n| n.role == kui_core::Role::MenuItem && n.name.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no menu row named {label:?}"))
        .rect
}

/// What a reader is handed when a menu opens: a modal `menu`, its rows as
/// `menuItem`s in the order they were given, the dead one dead, and the
/// keyboard already on the first row that can take it — so the arrow keys
/// have somewhere to start and a reader is not left on the window.
#[test]
fn an_open_menu_reads_as_a_modal_menu_with_its_rows_under_it() {
    let mut core = Core::new();
    let scope = frame(&mut core);
    core.open_menu(Menu::new(
        scope,
        Vec2::new(40.0, 30.0),
        vec![
            MenuItem::role(MenuRole::Copy).enabled(false),
            MenuItem::separator(),
            MenuItem::role(MenuRole::SelectAll),
        ],
    ));
    frame(&mut core);
    let tree = core.access_tree().clone();
    let menu = tree
        .nodes
        .iter()
        .find(|n| n.role == kui_core::Role::Menu)
        .expect("a menu node");
    assert_eq!(menu.name.as_deref(), Some("Menu"));
    assert!(menu.modal, "everything behind it is inert (ADR 0003)");
    // The separator is paint too: a reader hearing "separator" between
    // every pair of rows is noise.
    assert_eq!(rows(&mut core), ["Copy", "Select All"]);
    let copy = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Copy"))
        .unwrap();
    let all = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Select All"))
        .unwrap();
    assert!(
        copy.disabled && !copy.focused,
        "dead, and not where we start"
    );
    assert!(all.focused, "the first row that can take focus has it");
    assert!(
        all.supports(kui_core::AccessAction::Click),
        "and a reader can choose it without a pointer"
    );
}

// -- The automatic path -----------------------------------------------------

fn right_click(core: &mut Core, at: Vec2) -> Vec<kui_core::UiEvent> {
    core.handle_input(InputEvent::CursorMoved(at));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Secondary,
        clicks: 1,
    })
}

/// The rows of the open menu, in order, as `(label, enabled)`. Read from
/// the access tree, which is where the disabled ones are visible: a
/// disabled row is inert, so it declares no click.
fn rows(core: &mut Core) -> Vec<String> {
    let tree = core.access_tree().clone();
    tree.nodes
        .iter()
        .filter(|n| n.role == kui_core::Role::MenuItem)
        .map(|n| n.name.clone().unwrap_or_default())
        .collect()
}

#[test]
fn a_right_click_in_a_scope_opens_the_stock_menu() {
    let mut core = Core::new();
    let scope = frame(&mut core);
    let events = right_click(&mut core, Vec2::new(20.0, 8.0));
    assert!(events.is_empty(), "nothing reaches the app yet: {events:?}");
    let menu = core.menu().expect("a menu opened").clone();
    assert_eq!(menu.target, scope);
    assert_eq!(menu.at, Vec2::new(20.0, 8.0), "at the press point");
    frame(&mut core);
    assert_eq!(rows(&mut core), ["Copy", "Select All"]);
    // Copy is dead while nothing is selected, and the row stays where it
    // is rather than vanishing.
    let tree = core.access_tree().clone();
    let copy = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Copy"))
        .unwrap();
    assert!(copy.disabled, "nothing is selected yet");
}

#[test]
fn a_right_click_in_an_editor_offers_the_four_a_field_has() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.text_edit(
        "field",
        "typed text",
        &kui_core::EditOptions {
            autofocus: true,
            ..Default::default()
        },
        NodeSpec::column().grow_width(),
    );
    ui.finish();
    right_click(&mut core, Vec2::new(20.0, 8.0));
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.text_edit(
        "field",
        "typed text",
        &kui_core::EditOptions::default(),
        NodeSpec::column().grow_width(),
    );
    ui.finish();
    assert_eq!(rows(&mut core), ["Cut", "Copy", "Paste", "Select All"]);
}

#[test]
fn a_node_that_declares_its_own_context_menu_keeps_it() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.text_in_keyed(
        "card",
        NodeSpec::column()
            .grow_width()
            .height(60.0)
            .selectable()
            .on_context_menu("mine"),
        "one",
        style(),
    );
    ui.finish();
    let events = right_click(&mut core, Vec2::new(20.0, 8.0));
    assert_eq!(
        events.iter().filter_map(|e| e.kind()).collect::<Vec<_>>(),
        ["contextmenu"],
        "the app's declaration wins"
    );
    assert!(core.menu().is_none(), "and no stock menu opens over it");
}

#[test]
fn a_right_click_on_a_plain_box_opens_nothing() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed(
        "plain",
        NodeSpec::column().grow_width().height(60.0).hoverable(),
    );
    ui.finish();
    let events = right_click(&mut core, Vec2::new(20.0, 8.0));
    assert!(events.is_empty(), "{events:?}");
    assert!(core.menu().is_none());
}

// -- A host that shows menus itself ----------------------------------------

#[test]
fn a_native_host_gets_the_state_and_draws_nothing() {
    let mut core = Core::new();
    core.set_native_menus(true);
    let scope = frame(&mut core);
    right_click(&mut core, Vec2::new(20.0, 8.0));
    // The menu is open — the host is expected to show it — but the frame
    // draws none of it.
    let menu = core.menu().expect("the core still holds it").clone();
    assert_eq!(menu.target, scope);
    assert_eq!(menu.items.len(), 2, "Copy and Select All");
    frame(&mut core);
    assert!(
        menu_rect(&mut core).is_none(),
        "nothing drawn: the host is showing it"
    );
}

#[test]
fn the_host_reports_the_row_it_chose() {
    let mut core = Core::new();
    core.set_native_menus(true);
    let scope = frame(&mut core);
    core.select_all_in(scope);
    right_click(&mut core, Vec2::new(20.0, 8.0));
    // Copy is item 0 of a scope's default menu.
    let events = core.activate_menu_item(0).expect("Copy is enabled");
    assert_eq!(
        core.take_menu_actions(),
        vec![MenuAction::SetClipboard {
            text: "one\ntwo".into(),
            html: Some("one<br>two".into()),
        }],
        "the same path the drawn menu's row takes"
    );
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload.get_str("role"), Some("copy"));
    assert!(core.menu().is_none());
}

#[test]
fn a_row_the_host_invented_closes_the_menu_and_posts_nothing() {
    let mut core = Core::new();
    core.set_native_menus(true);
    let scope = frame(&mut core);
    core.open_menu(Menu::new(scope, Vec2::new(0.0, 0.0), items()));
    let events = core.activate_menu_item(99).expect("taken: the menu closes");
    assert!(events.is_empty(), "{events:?}");
    assert!(core.menu().is_none());
    assert_eq!(
        core.activate_menu_item(0),
        None,
        "and with no menu open there is nothing to take"
    );
}

/// A row the menu cannot choose — disabled, or a separator — is refused
/// from the door as it is from the pointer: nothing is posted and the
/// menu stays open, since a native menu never reports one and the drawn
/// row has no click (backlog RG9).
#[test]
fn a_disabled_row_or_a_separator_is_refused_and_the_menu_stays_open() {
    let mut core = Core::new();
    core.set_native_menus(true);
    let scope = frame(&mut core);
    let items = vec![
        MenuItem::new("Archive"),
        MenuItem::separator(),
        MenuItem::new("Delete").id("rm").enabled(false),
    ];
    core.open_menu(Menu::new(scope, Vec2::new(0.0, 0.0), items));
    assert_eq!(core.activate_menu_item(2), None, "disabled");
    assert!(core.menu().is_some(), "the menu is still open");
    assert_eq!(core.activate_menu_item(1), None, "a separator");
    assert!(core.menu().is_some());
    let events = core.activate_menu_item(0).expect("an enabled row is taken");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload.get_str("item"), Some("Archive"));
    assert!(core.menu().is_none());
}

// -- Force click (ADR 0017, decision 6) -------------------------------------

#[test]
fn a_force_click_over_text_selects_the_word_and_asks_for_a_panel() {
    let mut core = Core::new();
    core.set_lookup_available(true);
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let lead = ui.measure_text("hello ", &style(), None).width;
    let word = ui.measure_text("brave", &style(), None).width;
    ui.text_in_keyed(
        "card",
        NodeSpec::column().grow_width().selectable(),
        "hello brave world",
        style(),
    );
    ui.finish();
    let at = Vec2::new(lead + word / 2.0, 8.0);
    let events = core.handle_input(InputEvent::ForceClick(at));
    assert!(events.is_empty(), "nothing reaches the app: {events:?}");
    assert_eq!(core.selection_text().as_deref(), Some("brave"));
    match core.take_menu_actions().as_slice() {
        [MenuAction::LookUp { text, at }] => {
            assert_eq!(text, "brave");
            assert!(at.x > 0.0 && at.y > 0.0, "anchored somewhere: {at:?}");
        }
        other => panic!("expected one LookUp, got {other:?}"),
    }
}

#[test]
fn a_host_that_cannot_look_up_is_not_asked_to() {
    let mut core = Core::new();
    let scope = frame(&mut core);
    let events = core.handle_input(InputEvent::ForceClick(Vec2::new(4.0, 8.0)));
    assert!(events.is_empty());
    assert!(
        core.selection_text().is_some(),
        "the word is still selected"
    );
    assert!(
        core.take_menu_actions().is_empty(),
        "no panel is asked of a host that has none"
    );
    // ...and the row is not offered either.
    right_click(&mut core, Vec2::new(4.0, 8.0));
    frame(&mut core);
    assert_eq!(rows(&mut core), ["Copy", "Select All"]);
    let _ = scope;
}

#[test]
fn a_force_click_elsewhere_reaches_the_node_that_asked_for_it() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let key = ui.leaf_keyed(
        "chart",
        NodeSpec::column()
            .grow_width()
            .height(80.0)
            .on_force_click("peek"),
    );
    ui.finish();
    let events = core.handle_input(InputEvent::ForceClick(Vec2::new(30.0, 30.0)));
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].key, key);
    assert_eq!(events[0].kind(), Some("forceclick"));
    assert_eq!(events[0].payload.get_str("tag"), Some("peek"));
}

#[test]
fn the_lookup_row_is_offered_where_a_host_can_show_one() {
    let mut core = Core::new();
    core.set_lookup_available(true);
    let scope = frame(&mut core);
    core.select_all_in(scope);
    right_click(&mut core, Vec2::new(20.0, 8.0));
    frame(&mut core);
    assert_eq!(rows(&mut core), ["Look Up", "Copy", "Select All"]);
}

/// A wrapped paragraph, a force click on a row that is not the first:
/// the panel is anchored to the *word*, not to the paragraph's box. The
/// bug this pins put the popover under the whole paragraph — a Look Up
/// panel pointing at nothing (reported from a screenshot, 2026-09-09).
#[test]
fn the_panel_is_anchored_to_the_word_and_not_the_paragraph() {
    let mut core = Core::new();
    core.set_lookup_available(true);
    let text = "Spans are plain data, so every frontend may build them. The whole \
                paragraph is shaped together, which means wrapping crosses style \
                boundaries correctly instead of breaking at every run.";
    let build = |core: &mut Core| {
        let mut ui = core.frame(Size::new(420.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.text_in_keyed(
            "card",
            NodeSpec::column().width(400.0).selectable(),
            text,
            TextStyle::new(16.0).line_height(26.0),
        );
        ui.finish();
    };
    build(&mut core);
    // Row three of the wrapped paragraph.
    core.handle_input(InputEvent::ForceClick(Vec2::new(150.0, 60.0)));
    let rect = core.selection_rect().expect("a word is selected");
    assert!(
        rect.y >= 52.0 && rect.y < 78.0,
        "anchored on the row that was clicked, not the paragraph: {rect:?}"
    );
    assert!(
        rect.h < 30.0,
        "one line tall, not the whole paragraph: {rect:?}"
    );
    let word = core.selection_text().expect("a word");
    assert!(
        rect.w < 120.0,
        "as wide as {word:?}, not the column: {rect:?}"
    );
}

/// The press that deepened into a force click is still running, and its
/// drag used to overwrite the word the moment the pointer moved — the
/// panel said "Spans" while the highlight was one character wide.
#[test]
fn a_force_click_keeps_its_word_through_the_rest_of_the_press() {
    let mut core = Core::new();
    core.set_lookup_available(true);
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let lead = ui.measure_text("hello ", &style(), None).width;
    let word = ui.measure_text("brave", &style(), None).width;
    ui.text_in_keyed(
        "card",
        NodeSpec::column().grow_width().selectable(),
        "hello brave world",
        style(),
    );
    ui.finish();
    let at = Vec2::new(lead + word / 2.0, 8.0);
    core.handle_input(InputEvent::CursorMoved(at));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    core.handle_input(InputEvent::ForceClick(at));
    assert_eq!(core.selection_text().as_deref(), Some("brave"));
    // The finger moves a little, as a finger does, and lets go.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(at.x + 2.0, at.y + 1.0)));
    core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    assert_eq!(
        core.selection_text().as_deref(),
        Some("brave"),
        "the press does not take the word back"
    );
}

/// A single space between two words resolves to the word beside it — the
/// hit rounds to the nearer character, which is what a reader means. A
/// *run* of spaces is the case this pins: the middle of a wide gap is not
/// a word, and putting a dictionary panel over the page for it is not
/// what the gesture does anywhere else on the platform.
#[test]
fn a_force_click_in_a_wide_gap_looks_nothing_up() {
    let mut core = Core::new();
    core.set_lookup_available(true);
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let lead = ui.measure_text("hello", &style(), None).width;
    let gap = ui.measure_text("hello          ", &style(), None).width - lead;
    ui.text_in_keyed(
        "card",
        NodeSpec::column().grow_width().selectable(),
        "hello          world",
        style(),
    );
    ui.finish();
    core.handle_input(InputEvent::ForceClick(Vec2::new(lead + gap / 2.0, 8.0)));
    assert_eq!(core.selection_text(), None, "no selection over a gap");
    assert!(
        core.take_menu_actions().is_empty(),
        "and no panel over a space"
    );
}

/// A dictionary answers a word, not a page. Handed a whole paragraph, the
/// platform panel drew the entire selection back over the window as one
/// highlighted strip and then said "No Results Found" (field report,
/// 2026-09-09), so a passage is neither offered nor asked about.
#[test]
fn a_passage_is_not_offered_to_a_dictionary() {
    let mut core = Core::new();
    core.set_lookup_available(true);
    let scope = frame(&mut core);
    core.select_all_in(scope);
    // "one\ntwo" — two lines, so not a phrase.
    assert_eq!(core.lookup_text(), None, "a selection across lines");
    right_click(&mut core, Vec2::new(20.0, 8.0));
    frame(&mut core);
    let tree = core.access_tree().clone();
    let look = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Look Up"))
        .expect("the row is still there");
    assert!(look.disabled, "dimmed, not missing: the rows do not move");
    // A word, on the other hand, is exactly what it is for.
    core.set_selection(kui_core::Selection::new(
        core.selection().expect("a selection").scope,
        kui_core::Endpoint::new(core.selection().unwrap().anchor.node, 0),
        kui_core::Endpoint::new(core.selection().unwrap().anchor.node, 3),
    ));
    assert_eq!(core.lookup_text().as_deref(), Some("one"));
}

/// A row wider than the window — a recent file's whole path, a `<select>`
/// of them — caps the menu at the window, less a margin, and cuts the
/// label short with an ellipsis rather than the accelerator: before the
/// cap the panel ran off the right edge and its accelerators with it
/// (backlog RG150).
#[test]
fn a_menu_wider_than_the_window_is_capped_and_its_label_ellipsized() {
    let mut core = Core::new();
    let scope = frame(&mut core);
    let path = "/Users/someone/Documents/notes/archive/2026/october/the-long-name-of-a-note.md";
    core.open_menu(Menu::new(
        scope,
        Vec2::new(10.0, 10.0),
        vec![
            MenuItem::new("Open"),
            MenuItem::new(path).accel("ctrl+alt+shift+backspace"),
        ],
    ));
    frame(&mut core);
    let menu = menu_rect(&mut core).unwrap();
    let row_h = row_rect(&mut core, "Open").h;
    let long = row_rect(&mut core, path);
    assert!(
        menu.x >= 0.0 && menu.x + menu.w <= 400.0,
        "the menu inside the 400 px window: {menu:?}"
    );
    assert!(
        menu.w <= 400.0 - 2.0 * kui_core::widgets::MENU_EDGE,
        "and under the ceiling, the window less a margin a side: {menu:?}"
    );
    assert_eq!(long.h, row_h, "the long row is still one line");
    assert!(
        long.x + long.w <= menu.x + menu.w,
        "{long:?} inside {menu:?}"
    );

    use kui_core::display::QuadKind;
    let (dl, _) = core.output();
    let glyphs: Vec<kui_core::Rect> = dl
        .quads
        .iter()
        .filter(|q| matches!(q.kind, QuadKind::GlyphMask | QuadKind::GlyphSubpixel))
        .map(|q| q.rect)
        .filter(|r| r.y >= long.y && r.y + r.h <= long.y + long.h + 1.0)
        .collect();
    let right = glyphs.iter().map(|r| r.x + r.w).fold(f32::MIN, f32::max);
    assert!(
        right <= long.x + long.w,
        "nothing drawn past the row: {right} > {}",
        long.x + long.w
    );
    // The accelerator is whole: as many glyphs as its spelling has ink,
    // on top of a label cut short of its own.
    let ink = |s: &str| s.chars().filter(|c| !c.is_whitespace()).count();
    let accel = ink(&kui_core::Accel::label("ctrl+alt+shift+backspace"));
    assert!(
        glyphs.len() > accel && glyphs.len() < accel + ink(path),
        "{} glyphs: the accelerator's {accel} and a label cut short",
        glyphs.len()
    );
}
