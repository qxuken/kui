//! `#[derive(Message)]` (backlog C50): a Rust app's message to and from the
//! `{kind, …fields}` payload, `UiEvent::message` reading one back off the
//! events its handlers declared it on (ADR 0047), and the arriving twin
//! `#[message(on = …)]` makes, which carries the event's data beside the
//! message's own fields (decision 13).

use kui_native::event::{ChangePhase, DragPhase, EditChange, SliderChange};
use kui_native::{
    Core, EditKey, EditOptions, EventKind, InputEvent, Key, Message, MessageError, MessageField,
    Mods, NodeSpec, OriginId, Size, Sizing, UiEvent, Value, Vec2, WindowEvents, widgets,
};

#[derive(Message, Clone, Copy, Debug, PartialEq)]
#[message(string)]
enum Dir {
    H,
    V,
}

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Inc,
    TabNew,
    Focus {
        pane: u64,
    },
    Split {
        path: String,
        dir: Dir,
    },
    Pick(usize),
    #[message(kind = "add10")]
    AddTen,
    Key,
    Open,
    Note {
        text: Option<String>,
        tags: Vec<String>,
        ratio: f32,
    },
}

#[derive(Message, Clone, Debug, PartialEq)]
struct Resize {
    w: f64,
    h: f64,
}

#[test]
fn a_message_is_a_map_with_a_snake_case_kind() {
    assert_eq!(
        Value::from(Msg::Inc),
        Value::map([("kind", Value::str("inc"))])
    );
    assert_eq!(
        Value::from(Msg::TabNew).get("kind"),
        Some(&Value::str("tab_new"))
    );
    assert_eq!(
        Value::from(Msg::AddTen).get("kind"),
        Some(&Value::str("add10"))
    );
    let split = Value::from(Msg::Split {
        path: "ab".into(),
        dir: Dir::H,
    });
    assert_eq!(split.get_str("path"), Some("ab"));
    // A `string` enum is a bare string where it is a field.
    assert_eq!(split.get("dir"), Some(&Value::str("h")));
    // A tuple variant's fields are keyed by position.
    assert_eq!(Value::from(Msg::Pick(7)).get_int("0"), Some(7));
    let r = Value::from(Resize { w: 2.0, h: 3.0 });
    assert_eq!(r.get("kind"), Some(&Value::str("resize")));
}

#[test]
fn every_shape_round_trips() {
    for m in [
        Msg::Inc,
        Msg::TabNew,
        Msg::Focus { pane: 3 },
        Msg::Split {
            path: "ab".into(),
            dir: Dir::V,
        },
        Msg::Pick(7),
        Msg::AddTen,
        Msg::Note {
            text: None,
            tags: vec!["a".into(), "b".into()],
            ratio: 0.25,
        },
        Msg::Note {
            text: Some("hi".into()),
            tags: vec![],
            ratio: 1.5,
        },
    ] {
        assert_eq!(Msg::try_from(&Value::from(m.clone())), Ok(m));
    }
    let r = Resize { w: 2.0, h: 3.5 };
    assert_eq!(Resize::try_from(Value::from(r.clone())), Ok(r));
}

#[test]
fn a_payload_that_is_not_one_says_why() {
    assert_eq!(Msg::try_from(&Value::Int(1)), Err(MessageError::NoKind));
    assert_eq!(
        Msg::try_from(&Value::map([("kind", Value::str("nope"))])),
        Err(MessageError::UnknownKind("nope".into()))
    );
    let wrong = Value::map([("kind", Value::str("focus")), ("pane", Value::str("3"))]);
    assert_eq!(
        Msg::try_from(&wrong),
        Err(MessageError::Field {
            kind: "focus".into(),
            field: "pane"
        })
    );
    // A negative number is not a `u64`, rather than wrapping into one.
    let negative = Value::map([("kind", Value::str("focus")), ("pane", Value::Int(-1))]);
    assert!(Msg::try_from(&negative).is_err());
    // An `Option` field may be left out altogether.
    let bare = Value::map([
        ("kind", Value::str("note")),
        ("tags", Value::List(vec![])),
        ("ratio", Value::Float(1.0)),
    ]);
    assert_eq!(
        Msg::try_from(&bare),
        Ok(Msg::Note {
            text: None,
            tags: vec![],
            ratio: 1.0
        })
    );
    assert_eq!(
        MessageError::UnknownKind("x".into()).to_string(),
        "no message is of kind \"x\""
    );
}

#[test]
fn a_message_is_a_field_of_another() {
    #[derive(Message, Clone, Debug, PartialEq)]
    enum Outer {
        Wrapped { inner: Msg, size: Resize },
    }
    let o = Outer::Wrapped {
        inner: Msg::Pick(2),
        size: Resize { w: 1.0, h: 2.0 },
    };
    assert_eq!(Outer::try_from(&Value::from(o.clone())), Ok(o));
    assert_eq!(Dir::from_value(&Value::str("v")), Some(Dir::V));
}

/// The payload of an event a handler declared is the message, with the
/// event's data in one field of it: a click's is the message itself, a
/// drag's the message with `drag: {phase, x, y, …}` beside its fields.
#[test]
fn ui_event_message_reads_a_click_and_a_drags_message() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::row().fill());
        ui.leaf(
            NodeSpec::column()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Grow(1.0))
                .label("focus")
                .on_click(Msg::Focus { pane: 4 }),
        );
        ui.leaf(
            NodeSpec::column()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Grow(1.0))
                .on_drag(Msg::Split {
                    path: "a".into(),
                    dir: Dir::H,
                }),
        );
        ui.finish();
    };
    frame(&mut core);
    frame(&mut core);

    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 50.0)));
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
    let click = evs.first().expect("a click");
    assert_eq!(click.event, EventKind::Click);
    assert_eq!(click.payload, Value::from(Msg::Focus { pane: 4 }));
    assert_eq!(click.message::<Msg>(), Some(Msg::Focus { pane: 4 }));

    core.handle_input(InputEvent::CursorMoved(Vec2::new(150.0, 50.0)));
    let evs = core.handle_input(InputEvent::mouse_down(1));
    let drag = evs
        .iter()
        .find(|e| e.is(EventKind::Drag))
        .expect("a drag start");
    assert_eq!(
        drag.message::<Msg>(),
        Some(Msg::Split {
            path: "a".into(),
            dir: Dir::H
        })
    );
    // The event's data rides in its own field, beside the message's.
    assert_eq!(drag.payload.get_str("kind"), Some("split"));
    assert_eq!(
        drag.payload.get("drag").and_then(|d| d.get_str("phase")),
        Some("start")
    );
    assert_eq!(drag.drag().map(|d| d.phase), Some(DragPhase::Start));
    // A message of another type is not this one.
    assert_eq!(drag.message::<Resize>(), None);
}

/// The app's messages and the core's events no longer share a namespace
/// (backlog F148 is gone with the `tag` wrapper): `Msg::Key` declared on a
/// sink arrives as `{kind: "key", key: {…}}` and reads as itself, a sink
/// declared with no message hears `{key: {…}}`, which is no `Msg`, and a
/// message that is not a map is read from under `tag`.
#[test]
fn a_variant_named_like_a_core_event_is_the_apps() {
    let data = Value::map([("phase", Value::str("down")), ("code", Value::str("a"))]);
    let declared = |msg: Option<&Value>| {
        UiEvent::declared(EventKind::Key, OriginId::HOST, Key::ROOT, msg, data.clone())
    };
    let ev = declared(Some(&Value::from(Msg::Key)));
    assert_eq!(ev.payload.get_str("kind"), Some("key"));
    assert_eq!(ev.message::<Msg>(), Some(Msg::Key));
    assert_eq!(declared(None).message::<Msg>(), None);
    assert_eq!(declared(Some(&Value::Null)).message::<Msg>(), None);

    let bare = UiEvent::declared(
        EventKind::Drag,
        OriginId::HOST,
        Key::ROOT,
        Some(&Value::from(Dir::V)),
        Value::map([("phase", Value::str("start"))]),
    );
    assert_eq!(bare.payload.get_str("tag"), Some("v"));
    assert_eq!(bare.message::<Dir>(), Some(Dir::V));
}

/// The view declares `Edit`; `update` reads `EditIn`, whose marked
/// variants carry the event's data in the field named for it.
#[derive(Message, Clone, Debug, PartialEq)]
enum Edit {
    #[message(on = drag)]
    Pan,
    #[message(on = change)]
    Rename {
        id: u64,
    },
    #[message(on = slider_change)]
    Volume(u8),
    Save,
}

/// The same, with the twin named by `incoming` and given its derives.
#[derive(Message, Clone, Debug, PartialEq)]
#[message(incoming = "Incoming", incoming_derive(Debug))]
enum Renamed {
    #[message(on = drag)]
    Pan,
}

#[test]
fn the_twin_encodes_and_decodes_the_message_with_its_data() {
    let rename = EditIn::Rename {
        id: 3,
        change: EditChange { text: "hi".into() },
    };
    let v = Value::from(rename.clone());
    assert_eq!(v.get_str("kind"), Some("rename"));
    assert_eq!(v.get_int("id"), Some(3));
    assert_eq!(v.get("change").and_then(|c| c.get_str("text")), Some("hi"));
    assert_eq!(EditIn::try_from(&v), Ok(rename));
    // The message the view declares is the twin's own fields: the twin
    // written out and read as the original drops only the event's field.
    assert_eq!(Edit::try_from(&v), Ok(Edit::Rename { id: 3 }));

    let volume = EditIn::Volume(
        2,
        SliderChange {
            value: 0.5,
            phase: ChangePhase::End,
        },
    );
    let v = Value::from(volume.clone());
    assert_eq!(v.get_int("0"), Some(2), "a tuple's own fields by position");
    assert!(v.get("change").is_some(), "the event's under its name");
    assert_eq!(EditIn::try_from(v), Ok(volume));

    // A variant with no `on` is the same in both.
    assert_eq!(Value::from(EditIn::Save), Value::from(Edit::Save));
    assert_eq!(
        EditIn::from_value(&Value::from(Edit::Save)),
        Some(EditIn::Save)
    );
}

fn edit_frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    let node = |w| {
        NodeSpec::column()
            .width(Sizing::Fixed(w))
            .height(Sizing::Grow(1.0))
    };
    ui.leaf_keyed("pan", node(100.0).on_drag(Edit::Pan));
    ui.leaf_keyed("save", node(100.0).on_click(Edit::Save));
    // `Pan` marks `on = drag`; here it is declared on a click.
    ui.leaf_keyed("wrong", node(100.0).on_click(Edit::Pan));
    ui.leaf_keyed("renamed", node(50.0).on_drag(Renamed::Pan));
    ui.text_edit(
        "title",
        "a",
        &EditOptions {
            autofocus: true,
            ..Default::default()
        },
        node(50.0).on_change(Edit::Rename { id: 7 }),
    );
    widgets::slider(&mut ui, "vol", 0.5, 0.0, 1.0, 0.1, Edit::Volume(1));
    ui.finish();
}

fn edit_core() -> Core {
    let mut core = Core::new();
    edit_frame(&mut core);
    edit_frame(&mut core);
    core
}

fn click(core: &mut Core, x: f32) -> Vec<UiEvent> {
    core.handle_input(InputEvent::CursorMoved(Vec2::new(x, 50.0)));
    core.handle_input(InputEvent::mouse_down(1));
    core.handle_input(InputEvent::mouse_up())
}

/// Each event a real core sent decodes as the twin in one flat `match`.
#[test]
fn the_twin_reads_real_events_in_one_match() {
    let mut core = edit_core();
    let mut seen = Vec::new();
    let mut read = |evs: Vec<UiEvent>| {
        for ev in evs {
            match ev.message::<EditIn>() {
                Some(EditIn::Pan { drag }) => seen.push(format!("pan {:?}", drag.phase)),
                Some(EditIn::Rename { id, change }) => {
                    seen.push(format!("rename {id} {}", change.text))
                }
                Some(EditIn::Volume(ch, change)) => {
                    seen.push(format!("volume {ch} {:.1}", change.value))
                }
                Some(EditIn::Save) => seen.push("save".into()),
                None => {}
            }
        }
    };
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 50.0)));
    read(core.handle_input(InputEvent::mouse_down(1)));
    read(core.handle_input(InputEvent::CursorMoved(Vec2::new(30.0, 50.0))));
    read(core.handle_input(InputEvent::mouse_up()));
    read(click(&mut core, 150.0));
    // The click took focus off the editor; give it back, and type.
    let title = core.key_of("title").unwrap();
    core.set_focus(Some(title));
    read(core.handle_input(InputEvent::Text("b".into())));
    let vol = core.key_of("vol").unwrap();
    core.set_focus(Some(vol));
    read(core.handle_input(InputEvent::Key(EditKey::Right, Mods::NONE)));
    assert_eq!(
        seen,
        [
            "pan Start",
            "pan Move",
            "pan End",
            "save",
            "rename 7 ab",
            "volume 1 0.6",
        ]
    );
}

/// `Pan` declared on `on_click` arrives without a `drag`, so it does not
/// decode as the twin and the error names the missing field; the original
/// enum, which names no event, still reads it.
#[test]
fn a_message_on_the_wrong_handler_does_not_decode_as_the_twin() {
    let mut core = edit_core();
    let evs = click(&mut core, 250.0);
    let ev = evs.first().expect("a click");
    assert_eq!(ev.message::<Edit>(), Some(Edit::Pan));
    assert_eq!(ev.message::<EditIn>(), None);
    assert_eq!(
        EditIn::try_from(&ev.payload),
        Err(MessageError::Field {
            kind: "pan".into(),
            field: "drag"
        })
    );
}

#[test]
fn incoming_renames_the_twin() {
    let mut core = edit_core();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(325.0, 50.0)));
    let evs = core.handle_input(InputEvent::mouse_down(1));
    let got = evs.iter().find_map(|e| e.message::<Incoming>());
    let Some(Incoming::Pan { drag }) = got else {
        panic!("{got:?}");
    };
    assert_eq!(drag.phase, DragPhase::Start);
    core.handle_input(InputEvent::mouse_up());
}

/// A window's own events arrive as the messages its root declared (ADR
/// 0048), so a twin marks them as it marks a node's.
#[derive(Message, Clone, Debug, PartialEq)]
enum Win {
    #[message(on = resize)]
    Fit,
    #[message(on = modifiers)]
    Held,
    #[message(on = window)]
    Keyboard,
}

#[test]
fn the_twin_reads_a_window_s_own_events() {
    let mut core = Core::new();
    let frame = |core: &mut Core, w: f32| {
        let mut ui = core.frame(Size::new(w, 200.0), 1.0);
        ui.window_events(
            WindowEvents::default()
                .on_resize(Win::Fit)
                .on_modifiers(Win::Held)
                .on_focus(Win::Keyboard),
        );
        ui.finish();
        core.take_pending_events()
    };
    frame(&mut core, 300.0);
    let evs = frame(&mut core, 280.0);
    let Some(WinIn::Fit { resize }) = evs.iter().find_map(|e| e.message::<WinIn>()) else {
        panic!("{evs:?}");
    };
    assert_eq!(resize.size.w, 280.0);
    let evs = core.handle_input(InputEvent::Modifiers(kui_native::KeyMods {
        ctrl: true,
        ..Default::default()
    }));
    let Some(WinIn::Held { modifiers }) = evs.iter().find_map(|e| e.message::<WinIn>()) else {
        panic!("{evs:?}");
    };
    assert!(modifiers.ctrl);
    core.set_focused(false);
    let evs = core.take_pending_events();
    let Some(WinIn::Keyboard { window }) = evs.iter().find_map(|e| e.message::<WinIn>()) else {
        panic!("{evs:?}");
    };
    assert_eq!(window.phase, kui_native::event::WindowPhase::Blurred);
    assert_eq!(window.id, None, "the window's own keyboard names no other");
}
