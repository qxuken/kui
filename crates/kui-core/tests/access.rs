//! Accessibility as data: the core derives an access tree from the frame
//! (roles from behaviour, names from labels or text, plain boxes elided),
//! resolves assistive-technology requests the way pointer input would, and
//! reports missing names as warnings.

use kui_core::diag::{CONTROL_WITHOUT_NAME, IMAGE_WITHOUT_LABEL, SLIDER_VALUE_OUT_OF_RANGE};
use kui_core::testing::codes;
use kui_core::{
    AccessAction, AccessRequest, AccessTree, Core, EditOptions, InputEvent, Key, NodeSpec, Role,
    Size, Sizing, TextStyle, UiEvent, Value, WindowButton, WindowCommand,
};

fn kinds(evs: &[UiEvent]) -> Vec<String> {
    evs.iter()
        .map(|ev| match ev.payload.get("kind").and_then(Value::as_str) {
            Some(k) => k.to_string(),
            None => format!("{:?}", ev.payload),
        })
        .collect()
}

fn names(tree: &AccessTree) -> Vec<(Role, Option<&str>)> {
    tree.nodes
        .iter()
        .map(|n| (n.role, n.name.as_deref()))
        .collect()
}

/// A window with a titlebar strip, a labelled icon button, a text button
/// inside two plain boxes, a labelled image, a decorative image, an
/// editor and a scrolling list. Returns (button, edit, list) keys.
fn app_frame(core: &mut Core, focus_edit: bool) -> (Key, Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.window_title("Demo");
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("titlebar", NodeSpec::row().window_drag(), |ui| {
        ui.with_keyed(
            "close",
            NodeSpec::row().window_button(WindowButton::Close),
            |ui| ui.text("×", TextStyle::new(12.0)),
        );
    });
    ui.with_keyed(
        "icon",
        NodeSpec::row().on_click(Value::str("save")).label("Save"),
        |_| {},
    );
    let mut button = Key::ROOT;
    ui.with_keyed("wrap", NodeSpec::column().pad(4.0), |ui| {
        ui.with(NodeSpec::column(), |ui| {
            button = ui.with_keyed("go", NodeSpec::row().on_click(Value::str("go")), |ui| {
                ui.text("Go", TextStyle::new(14.0));
                ui.text("now", TextStyle::new(14.0));
            });
        });
    });
    let id = core_image(ui.core());
    ui.image(id, NodeSpec::column().label("Logo"));
    ui.image(id, NodeSpec::column().role(Role::None));
    let edit = ui.text_edit(
        "name",
        "hello",
        &EditOptions {
            autofocus: focus_edit,
            ..Default::default()
        },
        NodeSpec::column().width(Sizing::Fixed(200.0)).label("Name"),
    );
    let list = ui.with_keyed(
        "list",
        NodeSpec::column()
            .height(Sizing::Fixed(100.0))
            .scroll_y()
            .role(Role::List),
        |ui| {
            for i in 0..10 {
                ui.with_keyed(
                    &format!("row-{i}"),
                    NodeSpec::row()
                        .height(Sizing::Fixed(30.0))
                        .role(Role::ListItem),
                    |ui| ui.text(&format!("Row {i}"), TextStyle::new(12.0)),
                );
            }
        },
    );
    ui.finish();
    (button, edit, list)
}

fn core_image(core: &mut Core) -> kui_core::ImageId {
    core.resources.add_image(4, 4, vec![255; 64])
}

#[test]
fn roles_derive_from_behaviour_and_plain_boxes_are_elided() {
    let mut core = Core::new();
    let (button, edit, list) = app_frame(&mut core, false);
    let tree = core.access_tree().clone();

    let root = tree.root().unwrap();
    assert_eq!(root.role, Role::Window);
    assert_eq!(root.name.as_deref(), Some("Demo"));
    assert_eq!(root.rect, kui_core::Rect::new(0.0, 0.0, 400.0, 300.0));

    // The titlebar strip, its close button (named by its chrome role, its
    // "×" glyph read as part of it), the icon button by its label, the
    // text button by its content — with the two plain boxes around it
    // gone, so it hangs off the window directly.
    let got = names(&tree);
    // The strip stays unnamed on purpose: it keeps its children (window
    // buttons have to stay reachable), so naming it the window title too
    // would have a screen reader read the title twice. Verified against
    // the macOS accessibility API in scripts/ax-audit.swift.
    assert_eq!(got[1], (Role::TitleBar, None));
    assert_eq!(
        tree.nodes
            .iter()
            .filter(|n| n.name.as_deref() == Some("Demo"))
            .count(),
        1,
        "the window title is announced once"
    );
    assert_eq!(got[2], (Role::Button, Some("Close")));
    assert_eq!(got[3], (Role::Button, Some("Save")));
    assert_eq!(got[4], (Role::Button, Some("Go now")));
    let go = tree.get(button).unwrap();
    assert_eq!(go.parent, Some(Key::ROOT));
    assert!(go.supports(AccessAction::Click));
    assert!(!go.supports(AccessAction::SetValue));

    // The labelled image stays, the decorative one is gone.
    assert_eq!(got[5], (Role::Image, Some("Logo")));
    assert_eq!(
        tree.nodes.iter().filter(|n| n.role == Role::Image).count(),
        1
    );

    // The editor: a text input carrying its text.
    let field = tree.get(edit).unwrap();
    assert_eq!(field.role, Role::TextInput);
    assert_eq!(field.name.as_deref(), Some("Name"));
    assert_eq!(field.value.as_deref(), Some("hello"));
    assert_eq!(
        field.caret,
        Some(5),
        "a fresh single-line editor's caret sits after its seeded text (F20)"
    );
    assert_eq!(field.selection, None);
    assert!(!field.focused);
    assert!(field.supports(AccessAction::SetValue));
    assert!(field.supports(AccessAction::Focus));

    // The list scrolls: a list role with scroll state, its items under it,
    // each named by its text (a list item keeps its children; here the
    // text child is what remains).
    let rows = tree.get(list).unwrap();
    assert_eq!(rows.role, Role::List);
    let scroll = rows.scroll.unwrap();
    assert_eq!((scroll.x, scroll.y), (0.0, 0.0));
    assert_eq!(scroll.max_y, 200.0);
    assert!(rows.supports(AccessAction::ScrollDown));
    let items: Vec<_> = tree.children(list).collect();
    assert_eq!(items.len(), 10);
    assert_eq!(items[0].role, Role::ListItem);
    assert_eq!(items[0].name, None, "a list item is not named by content");
    let texts: Vec<_> = tree.children(items[3].key).collect();
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0].role, Role::StaticText);
    assert_eq!(texts[0].name.as_deref(), Some("Row 3"));

    assert_eq!(tree.focus, None);
    // Nothing else leaked in: window, titlebar, close, save, go, logo,
    // edit, list, 10 items, 10 texts.
    assert_eq!(tree.nodes.len(), 28);
}

#[test]
fn the_tree_is_built_once_per_frame_and_hashes_its_content() {
    let mut core = Core::new();
    app_frame(&mut core, false);
    let first = core.access_tree().hash;
    assert_eq!(core.access_tree().hash, first, "same frame, same tree");
    app_frame(&mut core, false);
    assert_eq!(
        core.access_tree().hash,
        first,
        "identical frames hash alike"
    );
    app_frame(&mut core, true);
    assert_ne!(core.access_tree().hash, first, "focus is part of the tree");
}

#[test]
fn explicit_roles_win_and_carry_their_state() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let check = ui.with_keyed(
        "check",
        NodeSpec::row()
            .role(Role::Checkbox)
            .checked(true)
            .on_click(Value::str("toggle")),
        |ui| ui.text("Remember me", TextStyle::new(12.0)),
    );
    let slider = ui.with_keyed(
        "vol",
        NodeSpec::row()
            .role(Role::Slider)
            .label("Volume")
            .value_now(0.4)
            .value_min(0.0)
            .value_max(1.0)
            .on_drag(Value::str("vol")),
        |_| {},
    );
    // The same control naming its own reading: 25 in [5..60] is "36
    // percent" to a reader with only the numbers, which is the bug F8
    // reported.
    let text_slider = ui.with_keyed(
        "focus",
        NodeSpec::row()
            .role(Role::Slider)
            .label("Focus length")
            .value_now(25.0)
            .value_min(5.0)
            .value_max(60.0)
            .value_text("25 minutes"),
        |_| {},
    );
    let heading = ui.with_keyed("h", NodeSpec::row().role(Role::Heading), |ui| {
        ui.text("Settings", TextStyle::new(20.0))
    });
    // A button whose click payload is overridden by a role.
    let tab = ui.with_keyed(
        "tab",
        NodeSpec::row().role(Role::Tab).on_click(Value::Int(1)),
        |ui| ui.text("General", TextStyle::new(12.0)),
    );
    ui.finish();
    let tree = core.access_tree().clone();

    let c = tree.get(check).unwrap();
    assert_eq!(c.role, Role::Checkbox);
    assert_eq!(c.checked, Some(true));
    assert_eq!(c.name.as_deref(), Some("Remember me"));
    assert!(c.supports(AccessAction::Click));

    let s = tree.get(slider).unwrap();
    assert_eq!(s.role, Role::Slider);
    assert_eq!((s.number, s.min, s.max), (Some(0.4), Some(0.0), Some(1.0)));
    assert!(s.supports(AccessAction::Increment));
    assert!(!s.supports(AccessAction::Click), "no click payload");
    assert_eq!(
        s.value, None,
        "a slider that named no reading has no string value"
    );

    let t = tree.get(text_slider).unwrap();
    assert_eq!(
        t.value.as_deref(),
        Some("25 minutes"),
        "value_text lands in the node's one string slot, which is what the \
         platform reads instead of the number (backlog F8)"
    );
    assert_eq!(
        (t.number, t.min, t.max),
        (Some(25.0), Some(5.0), Some(60.0)),
        "the range still travels: only the reading is replaced"
    );
    assert!(t.supports(AccessAction::Increment), "still nudgeable");

    let h = tree.get(heading).unwrap();
    assert_eq!(h.role, Role::Heading);
    assert_eq!(h.name.as_deref(), Some("Settings"));
    assert!(
        tree.children(heading).next().is_none(),
        "a heading's text is its name, not a child"
    );

    assert_eq!(tree.get(tab).unwrap().role, Role::Tab);
    assert_eq!(tree.get(tab).unwrap().name.as_deref(), Some("General"));
}

/// A tab list and a picked row: `selected` distinguishes the current one
/// of a set from `checked`'s on/off, and the core says so only where the
/// state means something.
#[test]
fn selected_marks_the_current_one_of_a_set() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let tabs = ui.with_keyed("tabs", NodeSpec::row().role(Role::TabList), |ui| {
        for (i, name) in ["General", "Network", "About"].iter().enumerate() {
            ui.with_keyed(
                name,
                NodeSpec::row()
                    .role(Role::Tab)
                    .selected(i == 1)
                    .on_click(Value::Int(i as i64)),
                |ui| ui.text(name, TextStyle::new(12.0)),
            );
        }
    });
    let list = ui.with_keyed("rows", NodeSpec::column().role(Role::List), |ui| {
        for (i, name) in ["one", "two"].iter().enumerate() {
            ui.with_keyed(
                name,
                NodeSpec::row().role(Role::ListItem).selected(i == 0),
                |ui| ui.text(name, TextStyle::new(12.0)),
            );
        }
    });
    // A link that is not the current page, and one that is.
    let away = ui.with_keyed("away", NodeSpec::row().role(Role::Link), |ui| {
        ui.text("Docs", TextStyle::new(12.0))
    });
    let here = ui.with_keyed(
        "here",
        NodeSpec::row().role(Role::Link).selected(true),
        |ui| ui.text("Home", TextStyle::new(12.0)),
    );
    ui.finish();
    let tree = core.access_tree().clone();

    let states: Vec<Option<bool>> = tree.children(tabs).map(|n| n.selected).collect();
    assert_eq!(
        states,
        vec![Some(false), Some(true), Some(false)],
        "every tab reports the state, so a reader can say which one is on"
    );
    let rows: Vec<Option<bool>> = tree.children(list).map(|n| n.selected).collect();
    assert_eq!(
        rows,
        vec![Some(true), None],
        "a row says so only when picked: an ordinary list is not a selection"
    );
    assert_eq!(tree.get(away).unwrap().selected, None);
    assert_eq!(tree.get(here).unwrap().selected, Some(true));
    assert_eq!(
        tree.get(tabs).unwrap().selected,
        None,
        "the container itself is not selectable"
    );
    // Not the same state as `checked`.
    assert!(tree.children(tabs).all(|n| n.checked.is_none()));
}

/// A disclosure names its state, so "collapsed" is sayable; a node that
/// does not expand says nothing about it.
#[test]
fn expanded_is_three_state_so_a_shut_disclosure_can_say_so() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let shut = ui.with_keyed(
        "shut",
        NodeSpec::row()
            .expanded(false)
            .on_click(Value::str("toggle")),
        |ui| ui.text("Advanced", TextStyle::new(12.0)),
    );
    let open = ui.with_keyed(
        "open",
        NodeSpec::row()
            .expanded(true)
            .on_click(Value::str("toggle")),
        |ui| ui.text("Network", TextStyle::new(12.0)),
    );
    let plain = ui.with_keyed("plain", NodeSpec::row().on_click(Value::str("go")), |ui| {
        ui.text("Save", TextStyle::new(12.0))
    });
    ui.finish();
    let tree = core.access_tree().clone();

    assert_eq!(tree.get(shut).unwrap().expanded, Some(false));
    assert_eq!(tree.get(open).unwrap().expanded, Some(true));
    assert_eq!(
        tree.get(plain).unwrap().expanded,
        None,
        "an ordinary button does not expand, and says nothing about it"
    );
}

/// "3 of 7" is derived, not declared: the core numbers what a `list` or
/// `tabList` already holds.
#[test]
fn a_list_numbers_its_own_items() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let list = ui.with_keyed("rows", NodeSpec::column().role(Role::List), |ui| {
        for name in ["one", "two", "three"] {
            ui.with_keyed(name, NodeSpec::row().role(Role::ListItem), |ui| {
                ui.text(name, TextStyle::new(12.0))
            });
        }
        // A caption inside the list is not an item, so it is not counted.
        ui.with_keyed("caption", NodeSpec::row().role(Role::Heading), |ui| {
            ui.text("3 rows", TextStyle::new(12.0))
        });
    });
    let loose = ui.with_keyed("loose", NodeSpec::row().role(Role::ListItem), |ui| {
        ui.text("orphan", TextStyle::new(12.0))
    });
    ui.finish();
    let tree = core.access_tree().clone();

    assert_eq!(
        tree.get(list).unwrap().set_size,
        Some(3),
        "the count sits on the container, the way AccessKit models a set"
    );
    assert_eq!(tree.get(list).unwrap().pos_in_set, None);
    let ordinals: Vec<Option<usize>> = tree
        .children(list)
        .filter(|n| n.role == Role::ListItem)
        .map(|n| n.pos_in_set)
        .collect();
    assert_eq!(ordinals, vec![Some(0), Some(1), Some(2)], "zero-based");
    assert!(
        tree.children(list)
            .all(|n| n.role == Role::ListItem || n.pos_in_set.is_none()),
        "a heading among the rows is not one of them"
    );
    assert_eq!(
        tree.get(loose).unwrap().pos_in_set,
        None,
        "an item outside a list belongs to no set"
    );
}

#[test]
fn a_click_request_emits_what_a_pointer_click_would() {
    let mut core = Core::new();
    let (button, _, _) = app_frame(&mut core, false);
    let evs = core.handle_input(InputEvent::Access(AccessRequest {
        key: button,
        action: AccessAction::Click,
        value: None,
        anchor: None,
        focus: None,
    }));
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, button);
    assert_eq!(evs[0].payload.as_str(), Some("go"));

    // A window button issues its command instead.
    let close = Key::ROOT.str("titlebar").str("close");
    let evs = core.handle_input(InputEvent::Access(AccessRequest {
        key: close,
        action: AccessAction::Click,
        value: None,
        anchor: None,
        focus: None,
    }));
    assert!(evs.is_empty());
    assert_eq!(
        core.take_window_commands(),
        vec![WindowCommand::Close(kui_core::WindowId::MAIN)]
    );

    // A node that isn't there does nothing.
    let evs = core.handle_input(InputEvent::Access(AccessRequest {
        key: Key::ROOT.str("nope"),
        action: AccessAction::Click,
        value: None,
        anchor: None,
        focus: None,
    }));
    assert!(evs.is_empty());
}

#[test]
fn focus_and_value_requests_drive_an_editor() {
    let mut core = Core::new();
    let (_, edit, _) = app_frame(&mut core, false);
    assert!(!core.is_focused(edit));

    let evs = core.handle_input(InputEvent::Access(AccessRequest {
        key: edit,
        action: AccessAction::Focus,
        value: None,
        anchor: None,
        focus: None,
    }));
    assert!(evs.is_empty());
    assert!(core.is_focused(edit));
    app_frame(&mut core, false);
    let tree = core.access_tree().clone();
    assert_eq!(tree.focus, Some(edit));
    assert!(tree.get(edit).unwrap().focused);

    let evs = core.handle_input(InputEvent::Access(AccessRequest {
        key: edit,
        action: AccessAction::SetValue,
        value: Some("world".into()),
        anchor: None,
        focus: None,
    }));
    assert_eq!(kinds(&evs), ["changed"]);
    assert_eq!(evs[0].key, edit);
    assert_eq!(core.edit_text(edit).as_deref(), Some("world"));
    // The same text again changes nothing.
    let evs = core.handle_input(InputEvent::Access(AccessRequest {
        key: edit,
        action: AccessAction::SetValue,
        value: Some("world".into()),
        anchor: None,
        focus: None,
    }));
    assert!(evs.is_empty());

    core.handle_input(InputEvent::Access(AccessRequest {
        key: edit,
        action: AccessAction::Blur,
        value: None,
        anchor: None,
        focus: None,
    }));
    assert!(!core.is_focused(edit));

    // Clicking an editor focuses it, like a pointer would.
    core.handle_input(InputEvent::Access(AccessRequest {
        key: edit,
        action: AccessAction::Click,
        value: None,
        anchor: None,
        focus: None,
    }));
    assert!(core.is_focused(edit));
}

#[test]
fn slider_nudges_reach_the_app_as_access_events() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let slider = ui.with_keyed(
        "vol",
        NodeSpec::row()
            .role(Role::Slider)
            .label("Volume")
            .on_drag(Value::str("vol")),
        |_| {},
    );
    let untagged = ui.with_keyed(
        "bare",
        NodeSpec::row().role(Role::Slider).label("x"),
        |_| {},
    );
    ui.finish();

    let evs = core.handle_input(InputEvent::Access(AccessRequest {
        key: slider,
        action: AccessAction::Increment,
        value: None,
        anchor: None,
        focus: None,
    }));
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, slider);
    assert_eq!(
        evs[0].payload,
        Value::map([
            ("kind", Value::str("access")),
            ("action", Value::str("increment")),
            ("tag", Value::str("vol")),
        ])
    );
    let evs = core.handle_input(InputEvent::Access(AccessRequest {
        key: untagged,
        action: AccessAction::Decrement,
        value: None,
        anchor: None,
        focus: None,
    }));
    assert_eq!(
        evs[0].payload,
        Value::map([
            ("kind", Value::str("access")),
            ("action", Value::str("decrement")),
        ])
    );
}

#[test]
fn scroll_requests_move_the_scroll_view() {
    let mut core = Core::new();
    let (_, _, list) = app_frame(&mut core, false);
    let row8 = list.str("row-8");
    let before = core.access_tree().get(row8).unwrap().rect;
    assert!(before.y + before.h > 300.0, "row 8 starts out of view");

    core.handle_input(InputEvent::Access(AccessRequest {
        key: row8,
        action: AccessAction::ScrollIntoView,
        value: None,
        anchor: None,
        focus: None,
    }));
    app_frame(&mut core, false);
    let tree = core.access_tree().clone();
    let view = tree.get(list).unwrap();
    let row = tree.get(row8).unwrap();
    assert!(row.rect.y >= view.rect.y && row.rect.y + row.rect.h <= view.rect.y + view.rect.h);
    assert!(view.scroll.unwrap().y > 0.0);

    core.handle_input(InputEvent::Access(AccessRequest {
        key: list,
        action: AccessAction::ScrollUp,
        value: None,
        anchor: None,
        focus: None,
    }));
    app_frame(&mut core, false);
    let after = core.access_tree().get(list).unwrap().scroll.unwrap().y;
    assert!(after < view.scroll.unwrap().y);
}

/// AR18: a reader's request obeys the gates every other channel obeys.
/// Only `Click` resolved against the hit list; `SetValue`, the
/// selection actions, the nudges and the scrolls reached a node behind a
/// modal — inert to a press, a key and Tab alike (ADR 0003 decision 5) —
/// and a disabled slider took a nudge. Which is what the access tree
/// refuses to advertise, so a request naming one is a reader working from
/// a stale tree, or a headless test; either way it does nothing.
#[test]
fn requests_behind_a_modal_or_on_a_disabled_node_do_nothing() {
    let mut core = Core::new();
    let build = |core: &mut Core, dialog: bool| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let field = ui.text_edit(
            "name",
            "before",
            &EditOptions::default(),
            NodeSpec::row()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(20.0)),
        );
        let slider = ui.with_keyed(
            "vol",
            NodeSpec::row()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(20.0))
                .role(Role::Slider)
                .label("Volume")
                .on_drag(Value::str("vol")),
            |_| {},
        );
        let off = ui.with_keyed(
            "off",
            NodeSpec::row()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(20.0))
                .role(Role::Slider)
                .label("Muted")
                .disabled(true)
                .on_drag(Value::str("off")),
            |_| {},
        );
        let list = ui.with_keyed(
            "list",
            NodeSpec::column()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(40.0))
                .scroll_y()
                .label("List"),
            |ui| {
                for i in 0..10 {
                    ui.with_keyed(
                        &format!("row-{i}"),
                        NodeSpec::row()
                            .width(Sizing::Fixed(100.0))
                            .height(Sizing::Fixed(20.0)),
                        |_| {},
                    );
                }
            },
        );
        if dialog {
            ui.with_keyed(
                "dialog",
                NodeSpec::column()
                    .width(Sizing::Fixed(120.0))
                    .height(Sizing::Fixed(80.0))
                    .modal(Value::str("dlg"))
                    .label("Settings"),
                |ui| {
                    ui.with_keyed(
                        "ok",
                        NodeSpec::row()
                            .width(Sizing::Fixed(80.0))
                            .height(Sizing::Fixed(20.0))
                            .on_click(Value::str("ok"))
                            .label("OK"),
                        |_| {},
                    );
                },
            );
        }
        ui.finish();
        (field, slider, off, list)
    };
    let (field, slider, off, list) = build(&mut core, true);
    let req = |key: Key, action: AccessAction| InputEvent::Access(AccessRequest::new(key, action));

    // Behind the dialog: nothing edits, moves, nudges or scrolls.
    let evs = core.handle_input(InputEvent::Access(
        AccessRequest::new(field, AccessAction::SetValue).with_value("after"),
    ));
    assert!(evs.is_empty(), "{evs:?}");
    assert_eq!(core.edit_text(field).as_deref(), Some("before"));
    assert!(
        core.handle_input(req(slider, AccessAction::Increment))
            .is_empty()
    );
    core.handle_input(req(field, AccessAction::Focus));
    assert_ne!(core.focus(), Some(field), "focus stays inside the modal");
    core.handle_input(req(list, AccessAction::ScrollDown));
    build(&mut core, true);
    assert_eq!(
        core.access_tree().get(list).unwrap().scroll.unwrap().y,
        0.0,
        "the list behind the dialog did not scroll"
    );

    // The dialog gone: the same requests act — except on the disabled
    // slider, which takes no nudge from anyone.
    build(&mut core, false);
    let evs = core.handle_input(InputEvent::Access(
        AccessRequest::new(field, AccessAction::SetValue).with_value("after"),
    ));
    assert_eq!(kinds(&evs), ["changed"]);
    assert_eq!(core.edit_text(field).as_deref(), Some("after"));
    assert_eq!(
        kinds(&core.handle_input(req(slider, AccessAction::Increment))),
        ["access"]
    );
    assert!(
        core.handle_input(req(off, AccessAction::Increment))
            .is_empty(),
        "disabled"
    );
    core.handle_input(req(list, AccessAction::ScrollDown));
    build(&mut core, false);
    assert!(core.access_tree().get(list).unwrap().scroll.unwrap().y > 0.0);
}

/// A reader's click on a control behind a modal is the press outside it
/// (ADR 0003 decisions 5 and 6): the control fires nothing, the modal is
/// asked to go away with `reason: "outside"`, and focus stays where it
/// is. The tree is not pruned (decision 7), so a reader can name the
/// control; its click used to be dropped where the pointer's press on
/// the same spot dismissed (backlog RG13). The modal's own control still
/// clicks, a window button behind the modal is still the platform's,
/// and a disabled control behind it is outside like any other.
#[test]
fn a_click_behind_a_modal_fires_nothing_and_asks_the_modal_to_go() {
    let mut core = Core::new();
    let build = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let close = ui.with_keyed(
            "close",
            NodeSpec::row()
                .width(Sizing::Fixed(20.0))
                .height(Sizing::Fixed(20.0))
                .window_button(WindowButton::Close)
                .label("Close"),
            |_| {},
        );
        let save = ui.with_keyed(
            "save",
            NodeSpec::row()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(20.0))
                .on_click(Value::map([("kind", Value::str("save"))]))
                .label("Save"),
            |_| {},
        );
        let off = ui.with_keyed(
            "off",
            NodeSpec::row()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(20.0))
                .on_click(Value::map([("kind", Value::str("off"))]))
                .disabled(true)
                .label("Off"),
            |_| {},
        );
        let dialog = ui.with_keyed(
            "dialog",
            NodeSpec::column()
                .width(Sizing::Fixed(120.0))
                .height(Sizing::Fixed(80.0))
                .modal(Value::str("dlg"))
                .label("Settings"),
            |ui| {
                ui.with_keyed(
                    "ok",
                    NodeSpec::row()
                        .width(Sizing::Fixed(80.0))
                        .height(Sizing::Fixed(20.0))
                        .on_click(Value::map([("kind", Value::str("ok"))]))
                        .label("OK"),
                    |_| {},
                );
            },
        );
        let ok = ui.core().key_of("ok").unwrap();
        ui.finish();
        (close, save, off, dialog, ok)
    };
    let (close, save, off, dialog, ok) = build(&mut core);
    assert_eq!(core.focus(), Some(ok), "focus went into the modal");
    let click = |core: &mut Core, key: Key| {
        core.handle_input(InputEvent::Access(AccessRequest::new(
            key,
            AccessAction::Click,
        )))
    };

    // The control behind the dialog: no `save`, a `dismiss` on the
    // dialog, and focus untouched.
    let evs = click(&mut core, save);
    assert_eq!(kinds(&evs), ["dismiss"]);
    assert_eq!(evs[0].key, dialog);
    assert_eq!(
        evs[0].payload.get("reason").and_then(Value::as_str),
        Some("outside")
    );
    assert_eq!(
        evs[0].payload.get("tag").and_then(Value::as_str),
        Some("dlg")
    );
    assert_eq!(core.focus(), Some(ok));
    // A disabled one behind it is outside all the same — the pointer's
    // press there finds no region either.
    assert_eq!(kinds(&click(&mut core, off)), ["dismiss"]);
    // The dialog's own control clicks.
    assert_eq!(kinds(&click(&mut core, ok)), ["ok"]);
    // Window chrome stays live: the platform's, not a dismissal.
    assert!(click(&mut core, close).is_empty());
    assert_eq!(
        core.take_window_commands(),
        [WindowCommand::Close(kui_core::WindowId::MAIN)]
    );

    // The dialog's own background is not outside: nothing is heard.
    assert!(click(&mut core, dialog).is_empty());

    // Without the dialog the same click is the control's.
    let mut plain = Core::new();
    let mut ui = plain.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let save = ui.with_keyed(
        "save",
        NodeSpec::row()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(20.0))
            .on_click(Value::map([("kind", Value::str("save"))]))
            .label("Save"),
        |_| {},
    );
    ui.finish();
    assert_eq!(kinds(&click(&mut plain, save)), ["save"]);
}

#[test]
fn missing_names_are_warnings_raised_once() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        let id = core_image(ui.core());
        ui.image(id, NodeSpec::column());
        ui.image(id, NodeSpec::column().label("fine"));
        ui.image(id, NodeSpec::column().role(Role::None));
        // An icon button with nothing to read.
        ui.with_keyed("icon", NodeSpec::row().on_click(Value::Int(1)), |_| {});
        // A named one, and a plain box that is not a control at all.
        ui.with_keyed("ok", NodeSpec::row().on_click(Value::Int(2)), |ui| {
            ui.text("OK", TextStyle::new(12.0))
        });
        ui.with(NodeSpec::row(), |_| {});
        // An editor without a label.
        ui.text_edit("q", "", &EditOptions::default(), NodeSpec::column());
        ui.finish();
    };
    frame(&mut core);
    let ws = core.take_warnings();
    assert_eq!(
        codes(&ws),
        [
            IMAGE_WITHOUT_LABEL,
            CONTROL_WITHOUT_NAME,
            CONTROL_WITHOUT_NAME
        ]
    );
    assert_eq!(ws[1].key, Key::ROOT.str("icon"));
    assert!(ws[1].message.contains("button"), "{}", ws[1].message);
    assert!(ws[2].message.contains("textInput"), "{}", ws[2].message);
    frame(&mut core);
    assert!(core.take_warnings().is_empty(), "each once");
}

/// A slider's `valueNow` is read to a screen reader as declared, so a
/// value outside the declared range — or a range with nothing inside it —
/// is a defect only that user sees (backlog F10). Three ways to be wrong,
/// one node in range that stays silent, and a slider that declares no
/// range at all, which has nothing to be outside of.
#[test]
fn slider_value_outside_its_range_warns() {
    let mut core = Core::new();
    let slider = |label: &'static str| NodeSpec::row().role(Role::Slider).label(label);
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.with_keyed(
            "fine",
            slider("fine").value_now(5.0).value_min(0.0).value_max(10.0),
            |_| {},
        );
        ui.with_keyed(
            "below",
            slider("below")
                .value_now(-1.0)
                .value_min(0.0)
                .value_max(10.0),
            |_| {},
        );
        ui.with_keyed(
            "above",
            slider("above")
                .value_now(999.0)
                .value_min(0.0)
                .value_max(10.0),
            |_| {},
        );
        ui.with_keyed(
            "inverted",
            slider("inverted")
                .value_now(5.0)
                .value_min(10.0)
                .value_max(0.0),
            |_| {},
        );
        ui.with_keyed("open", slider("open").value_now(999.0), |_| {});
        ui.finish();
    };
    frame(&mut core);
    let ws = core.take_warnings();
    assert_eq!(
        codes(&ws),
        [
            SLIDER_VALUE_OUT_OF_RANGE,
            SLIDER_VALUE_OUT_OF_RANGE,
            SLIDER_VALUE_OUT_OF_RANGE
        ]
    );
    assert_eq!(ws[0].key, Key::ROOT.str("below"));
    assert!(
        ws[0].message.contains("below valueMin 0"),
        "{}",
        ws[0].message
    );
    assert_eq!(ws[1].key, Key::ROOT.str("above"));
    assert!(
        ws[1].message.contains("above valueMax 10"),
        "{}",
        ws[1].message
    );
    assert_eq!(ws[2].key, Key::ROOT.str("inverted"));
    assert!(
        ws[2].message.contains("valueMin 10 is above valueMax 0"),
        "{}",
        ws[2].message
    );
    frame(&mut core);
    assert!(core.take_warnings().is_empty(), "each once");
}
