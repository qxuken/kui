//! IME for an editor the app owns (backlog C17). The stock editor took
//! every composition; a custom editor — an `onKey` sink drawing `line`
//! rows with `caret` — got nothing: no preedit, no commit (the platform
//! reports neither as a key press with `text`), and the OS candidate
//! window at the window's origin. Now both arrive on the sink as data and
//! the candidate window is anchored at the `line`'s caret.

use kui_core::{Color, Core, InputEvent, Key, NodeSpec, Role, Size, TextStyle, Value};

const LH: f32 = 20.0;

fn mono() -> TextStyle {
    TextStyle::new(14.0).mono().line_height(LH)
}

/// A custom editor: one sink with two lines, the caret on the second at
/// byte `caret`. Returns the sink's key and the second line's.
fn frame(core: &mut Core, caret: u32) -> (Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let sink = ui.with_keyed(
        "editor",
        NodeSpec::column()
            .on_key("ed")
            .role(Role::MultilineTextInput)
            .label("Buffer"),
        |ui| {
            ui.text_in_keyed(
                "l0",
                NodeSpec::row().height(LH).role(Role::Line),
                "first line",
                mono(),
            );
            ui.with_keyed(
                "l1",
                NodeSpec::row().height(LH).role(Role::Line).caret(caret),
                |ui| {
                    ui.text("let ", mono().color(Color::rgb8(200, 100, 255)));
                    ui.text("value", mono());
                },
            );
        },
    );
    ui.finish();
    (sink, sink.str("l1"))
}

fn kinds(events: &[kui_core::UiEvent]) -> Vec<String> {
    events
        .iter()
        .map(|e| match &e.payload {
            Value::Map(m) => m
                .iter()
                .find(|(k, _)| k == "kind")
                .and_then(|(_, v)| match v {
                    Value::Str(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default(),
            _ => String::new(),
        })
        .collect()
}

fn field<'a>(ev: &'a kui_core::UiEvent, name: &str) -> Option<&'a Value> {
    match &ev.payload {
        Value::Map(m) => m.iter().find(|(k, _)| k == name).map(|(_, v)| v),
        _ => None,
    }
}

#[test]
fn a_composition_reaches_the_focused_sink_as_data() {
    let mut core = Core::new();
    let (sink, _) = frame(&mut core, 6);
    core.set_focus(Some(sink));
    frame(&mut core, 6);

    let pre = core.handle_input(InputEvent::Preedit("日本".into(), Some((0, 6))));
    assert_eq!(kinds(&pre), ["preedit"]);
    assert_eq!(pre[0].key, sink);
    assert_eq!(field(&pre[0], "text"), Some(&Value::str("日本")));
    assert_eq!(
        field(&pre[0], "cursor"),
        Some(&Value::List(vec![Value::Int(0), Value::Int(6)]))
    );
    assert_eq!(
        field(&pre[0], "tag"),
        Some(&Value::str("ed")),
        "the sink's tag rides along"
    );

    let done = core.handle_input(InputEvent::Commit("日本語".into()));
    assert_eq!(kinds(&done), ["text"]);
    assert_eq!(field(&done[0], "text"), Some(&Value::str("日本語")));
    assert_eq!(field(&done[0], "tag"), Some(&Value::str("ed")));

    // A composition that ends without a commit says so: empty text, no cursor.
    let gone = core.handle_input(InputEvent::Preedit(String::new(), None));
    assert_eq!(kinds(&gone), ["preedit"]);
    assert_eq!(field(&gone[0], "text"), Some(&Value::str("")));
    assert_eq!(field(&gone[0], "cursor"), Some(&Value::Null));
}

/// Plain typing is not a commit: the sink hears the key press with its
/// `text`, and the `Text` channel — which every driver sends beside the
/// press — stays away from it, so nothing is typed twice.
#[test]
fn typing_reaches_a_sink_once() {
    let mut core = Core::new();
    let (sink, _) = frame(&mut core, 0);
    core.set_focus(Some(sink));
    frame(&mut core, 0);
    let events = core.press(kui_core::KeyPress {
        text: Some("a".into()),
        ..kui_core::KeyPress::new(kui_core::KeyCode::Char('a'), kui_core::KeyMods::default())
    });
    assert_eq!(kinds(&events), ["key"], "one event for one keystroke");
    let events = core.handle_input(InputEvent::Text("a".into()));
    assert!(events.is_empty(), "the text channel alone reaches no sink");
}

/// The candidate window goes where the `line` carrying `caret` says.
#[test]
fn the_ime_rect_follows_the_custom_caret() {
    let mut core = Core::new();
    let (sink, line) = frame(&mut core, 6);
    core.set_focus(Some(sink));
    frame(&mut core, 6);
    let rect = core
        .ime_rect()
        .expect("a caret row anchors the candidate window");
    assert_eq!(Some(rect), core.caret_rect(line, 6));
    let w = core.measure_text("M", &mono(), None).width;
    assert!((rect.x - (10.0 + 6.0 * w)).abs() < 0.75, "{}", rect.x);
    assert_eq!(rect.y, 10.0 + LH, "the second line");
    assert_eq!(rect.h, LH);
    // The caret moves, the anchor moves with it.
    frame(&mut core, 2);
    let moved = core.ime_rect().unwrap();
    assert!(moved.x < rect.x);
    // Nothing focused: nothing to anchor.
    core.set_focus(None);
    frame(&mut core, 2);
    assert_eq!(core.ime_rect(), None);
}

/// The stock editor still takes a composition itself: a commit there is a
/// `changed`, not a `text` event.
#[test]
fn a_stock_editor_takes_the_commit_itself() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let key = ui.text_edit(
        "note",
        "",
        &kui_core::EditOptions {
            autofocus: true,
            ..Default::default()
        },
        NodeSpec::column().grow_width(),
    );
    ui.finish();
    let events = core.handle_input(InputEvent::Commit("日本語".into()));
    assert_eq!(kinds(&events), ["changed"]);
    assert_eq!(core.edit_text(key).as_deref(), Some("日本語"));
}
