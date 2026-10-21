//! The arriving twin (ADR 0047, decision 13): `#[message(on = …)]` on a
//! variant makes the derive emit `<Type>In`, whose variant carries the
//! event's data in the field named for the event, read off events a
//! headless core really sent.

use kui_core::event::{DragPhase, EditChange, KeyEvent};
use kui_core::{
    Core, EditOptions, InputEvent, KeyCode, KeyMods, KeyPhase, KeyPress, MessageError, NodeSpec,
    Size, Sizing, UiEvent, Value, Vec2,
};
use kui_derive::Message;

#[derive(Message, Clone, Debug, PartialEq)]
#[message(crate = "kui_core")]
enum Msg {
    #[message(on = drag)]
    Pan,
    #[message(on = change)]
    Rename {
        id: u64,
    },
    #[message(on = key)]
    Keys(u8),
    Save,
}

/// The same messages under a twin named by `incoming`.
#[derive(Message, Clone, Debug, PartialEq)]
#[message(crate = "kui_core", incoming = "Arrived")]
enum Named {
    #[message(on = drag)]
    Pan,
}

/// A struct message's twin.
#[derive(Message, Clone, Debug, PartialEq)]
#[message(crate = "kui_core", on = change)]
struct Title {
    id: u64,
}

fn frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    let node = |w| {
        NodeSpec::column()
            .width(Sizing::Fixed(w))
            .height(Sizing::Grow(1.0))
    };
    ui.leaf_keyed("pan", node(100.0).on_drag(Msg::Pan));
    ui.leaf_keyed("save", node(100.0).on_click(Msg::Save));
    // Declared on the wrong handler: `Pan` says `on = drag`.
    ui.leaf_keyed("wrong", node(100.0).on_click(Msg::Pan));
    ui.leaf_keyed("keys", node(50.0).on_key(Msg::Keys(3)));
    ui.text_edit(
        "title",
        "a",
        &EditOptions {
            autofocus: true,
            ..Default::default()
        },
        NodeSpec::column()
            .width(Sizing::Fixed(50.0))
            .on_change(Msg::Rename { id: 7 }),
    );
    ui.finish();
}

fn core() -> Core {
    let mut core = Core::new();
    frame(&mut core);
    frame(&mut core);
    core
}

fn click(core: &mut Core, x: f32) -> Vec<UiEvent> {
    core.handle_input(InputEvent::CursorMoved(Vec2::new(x, 50.0)));
    core.handle_input(InputEvent::mouse_down(1));
    core.handle_input(InputEvent::mouse_up())
}

#[test]
fn a_drag_arrives_with_its_data() {
    let mut core = core();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 50.0)));
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::CursorMoved(Vec2::new(40.0, 60.0)));
    let moved = evs.iter().rev().find_map(|e| match e.message::<MsgIn>() {
        Some(MsgIn::Pan { drag }) => Some(drag),
        _ => None,
    });
    let drag = moved.expect("the drag, as the twin");
    assert_eq!(drag.phase, DragPhase::Move);
    assert_eq!(drag.delta, Vec2::new(30.0, 10.0));
    // The same as the core's own reading of the event.
    let ev = evs.iter().rev().find(|e| e.drag().is_some()).unwrap();
    assert_eq!(ev.drag(), Some(drag));
    // The original still reads the message alone, and the renamed twin
    // reads it too.
    assert_eq!(ev.message::<Msg>(), Some(Msg::Pan));
    assert!(matches!(ev.message::<Arrived>(), Some(Arrived::Pan { .. })));
    // Written back, the twin is the payload the core sent.
    let twin = ev.message::<MsgIn>().unwrap();
    assert_eq!(Value::from(twin), ev.payload);
    core.handle_input(InputEvent::mouse_up());
}

#[test]
fn an_editors_change_arrives_with_its_text() {
    let mut core = core();
    let evs = core.handle_input(InputEvent::Text("b".into()));
    let ev = evs.first().expect("a change");
    assert_eq!(
        ev.message::<MsgIn>(),
        Some(MsgIn::Rename {
            id: 7,
            change: EditChange { text: "ab".into() }
        })
    );
    // A struct's twin reads the same field off a struct message's payload.
    let title = Value::map([
        ("kind", Value::str("title")),
        ("id", Value::Int(2)),
        ("change", Value::map([("text", Value::str("ab"))])),
    ]);
    assert_eq!(
        TitleIn::try_from(&title),
        Ok(TitleIn {
            id: 2,
            change: EditChange { text: "ab".into() }
        })
    );
    assert_eq!(Title::try_from(&title), Ok(Title { id: 2 }));
}

#[test]
fn a_key_sinks_press_arrives_after_the_tuples_own_fields() {
    let mut core = core();
    let sink = core.key_of("keys").unwrap();
    core.set_key_focus(Some(sink));
    let evs = core.handle_input(InputEvent::KeyDown(KeyPress::new(
        KeyCode::Char('q'),
        KeyMods::default(),
    )));
    let got = evs.iter().find_map(|e| e.message::<MsgIn>());
    let Some(MsgIn::Keys(3, KeyEvent { phase, press })) = got else {
        panic!("{got:?}");
    };
    assert_eq!((phase, press.code), (KeyPhase::Down, KeyCode::Char('q')));
}

#[test]
fn a_variant_without_data_arrives_as_declared() {
    let mut core = core();
    let evs = click(&mut core, 150.0);
    let ev = evs.first().expect("a click");
    assert_eq!(ev.message::<MsgIn>(), Some(MsgIn::Save));
    assert_eq!(ev.message::<Msg>(), Some(Msg::Save));
    assert_eq!(Value::from(MsgIn::Save), Value::from(Msg::Save));
}

/// `Pan` declared on `on_click` arrives without a `drag`, so it is not a
/// `MsgIn`, and the error says which field is missing. The original enum
/// still reads it: it names no event.
#[test]
fn a_message_on_the_wrong_handler_does_not_decode() {
    let mut core = core();
    let evs = click(&mut core, 250.0);
    let ev = evs.first().expect("a click");
    assert_eq!(ev.message::<Msg>(), Some(Msg::Pan));
    assert_eq!(ev.message::<MsgIn>(), None);
    assert_eq!(
        MsgIn::try_from(&ev.payload),
        Err(MessageError::Field {
            kind: "pan".into(),
            field: "drag"
        })
    );
}
