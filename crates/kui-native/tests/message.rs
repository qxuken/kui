//! `#[derive(Message)]` (backlog C50): a Rust app's message to and from the
//! `{kind, …fields}` payload, and `UiEvent::message` reading one back off a
//! click or out of a core event's `tag`.

use kui_native::{
    Core, InputEvent, Message, MessageError, MessageField, NodeSpec, Size, Value, Vec2,
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
    assert_eq!(split.get("path").and_then(Value::as_str), Some("ab"));
    // A `string` enum is a bare string where it is a field.
    assert_eq!(split.get("dir"), Some(&Value::str("h")));
    // A tuple variant's fields are keyed by position.
    assert_eq!(
        Value::from(Msg::Pick(7)).get("0").and_then(Value::as_int),
        Some(7)
    );
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

/// The two places a message arrives: a click's payload is the message
/// itself; a drag's is the core's `drag` event, with the message as its
/// `tag` beside `phase`, `x` and `y`.
#[test]
fn ui_event_message_reads_a_click_and_a_drags_tag() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::row().fill());
        ui.with(
            NodeSpec::column()
                .width(kui_native::Sizing::Fixed(100.0))
                .height(kui_native::Sizing::Grow(1.0))
                .label("focus")
                .on_click(Msg::Focus { pane: 4 }),
            |_| {},
        );
        ui.with(
            NodeSpec::column()
                .width(kui_native::Sizing::Fixed(100.0))
                .height(kui_native::Sizing::Grow(1.0))
                .on_drag(Msg::Split {
                    path: "a".into(),
                    dir: Dir::H,
                }),
            |_| {},
        );
        ui.finish();
    };
    frame(&mut core);
    frame(&mut core);

    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 50.0)));
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
    let click = evs.first().expect("a click");
    assert_eq!(click.message::<Msg>(), Some(Msg::Focus { pane: 4 }));

    core.handle_input(InputEvent::CursorMoved(Vec2::new(150.0, 50.0)));
    let evs = core.handle_input(InputEvent::mouse_down(1));
    let drag = evs
        .iter()
        .find(|e| e.payload.get("kind").and_then(Value::as_str) == Some("drag"))
        .expect("a drag start");
    assert_eq!(
        drag.message::<Msg>(),
        Some(Msg::Split {
            path: "a".into(),
            dir: Dir::H
        })
    );
    // The event's own fields stay on the payload.
    assert_eq!(
        drag.payload.get("phase").and_then(Value::as_str),
        Some("start")
    );
    // A message of another type is not this one.
    assert_eq!(drag.message::<Resize>(), None);
}
