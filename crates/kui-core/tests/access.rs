//! Accessibility as data: the core derives an access tree from the frame
//! (roles from behaviour, names from labels or text, plain boxes elided),
//! resolves assistive-technology requests the way pointer input would, and
//! reports missing names as warnings.

use kui_core::diag::{CONTROL_WITHOUT_NAME, IMAGE_WITHOUT_LABEL};
use kui_core::{
    AccessAction, AccessRequest, AccessTree, Core, EditOptions, InputEvent, Key, NodeSpec, Role,
    Size, Sizing, TextStyle, UiEvent, Value, Warning, WindowButton, WindowCommand,
};

fn codes(ws: &[Warning]) -> Vec<&'static str> {
    ws.iter().map(|w| w.code).collect()
}

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
        Some(0),
        "a fresh editor's caret sits at the start"
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
    assert_eq!(core.take_window_commands(), vec![WindowCommand::Close]);

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
