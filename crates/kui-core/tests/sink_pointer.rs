//! A custom editor's mouse (backlog C34): a press or drag inside a key
//! sink carries `line` (the ordinal among the sink's `role="line"` nodes —
//! the numbering `access` events use), `byte` (where the point falls in
//! that line's text) and `clicks` (the press's count), so click-to-caret,
//! drag-select and double-click-word are `on_event` arithmetic with no
//! query and no frame of lag. And the clipboard for the same editor
//! (backlog C33): `set_clipboard` / `request_paste` are the two actions a
//! menu's Copy and Paste queue, and a paste comes back as a `text` event
//! on the sink.

use kui_core::testing::{press, release};
use kui_core::{
    ClipboardMarks, Core, InputEvent, Key, KeyCode, KeyMods, KeyPress, MenuAction, NodeSpec, Role,
    Size, TextStyle, UiEvent, Value, Vec2,
};

const LH: f32 = 20.0;
const GUTTER: f32 = 30.0;

fn mono() -> TextStyle {
    TextStyle::new(14.0).mono().line_height(LH)
}

/// The shape `modal_editor` draws: a sink with a gutter (`role="none"`)
/// and a column of `role="line"` rows, each a row of text runs. The sink
/// carries `on_drag` and a map `on_click`, as an editor that wants a mouse
/// would. Root padding 10.
fn frame(core: &mut Core, lines: &[&str]) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let sink = ui.with_keyed(
        "editor",
        NodeSpec::row()
            .fill()
            .key_sink()
            .on_drag("sel")
            .on_click(Value::map([("kind", Value::str("hit"))]))
            .role(Role::MultilineTextInput),
        |ui| {
            ui.with(NodeSpec::column().width(GUTTER).role(Role::None), |ui| {
                for (n, _) in lines.iter().enumerate() {
                    ui.text_in(NodeSpec::row().height(LH), &format!("{}", n + 1), mono());
                }
            });
            ui.with(NodeSpec::column().grow_width(), |ui| {
                for line in lines {
                    ui.with(NodeSpec::row().height(LH).role(Role::Line), |ui| {
                        // Two runs, as a line with a caret in it has.
                        let (a, b) = line.split_at(line.len() / 2);
                        ui.text(a, mono());
                        ui.text(b, mono());
                    });
                }
            });
        },
    );
    ui.take_key_focus(sink);
    ui.finish();
    sink
}

fn field(ev: &UiEvent, name: &str) -> Option<Value> {
    ev.payload.get(name).cloned()
}

fn field_of(ev: &UiEvent, name: &str) -> Option<Value> {
    field(ev, name)
}

fn kind(ev: &UiEvent) -> Option<String> {
    ev.payload.get_str("kind").map(str::to_string)
}

#[test]
fn a_press_in_a_sink_says_which_line_and_where() {
    let mut core = Core::new();
    let sink = frame(&mut core, &["hello world", "second line", "third"]);
    let w = core.measure_text("M", &mono(), None).width;
    // The text column starts at x = 10 + GUTTER; line n's row spans
    // y = 10 + n·LH .. + LH. Press on the second line, after the sixth
    // glyph (past the seam between its two runs).
    let at = Vec2::new(10.0 + GUTTER + 6.2 * w, 10.0 + LH + 5.0);
    let evs = press(&mut core, at);
    let start = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("drag"))
        .expect("the drag started");
    assert_eq!(start.key, sink);
    assert_eq!(field(start, "line"), Some(Value::Int(1)));
    assert_eq!(field(start, "byte"), Some(Value::Int(6)));
    assert_eq!(field(start, "clicks"), Some(Value::Int(1)));
    // A move carries the line and byte under the pointer now.
    let evs = core.handle_input(InputEvent::CursorMoved(Vec2::new(
        10.0 + GUTTER + 2.8 * w,
        10.0 + 2.0 * LH + 5.0,
    )));
    let mv = evs
        .iter()
        .find(|e| field(e, "phase") == Some(Value::str("move")))
        .expect("a move");
    assert_eq!(field(mv, "line"), Some(Value::Int(2)));
    assert_eq!(field(mv, "byte"), Some(Value::Int(3)));
    let evs = release(&mut core);
    let end = evs
        .iter()
        .find(|e| field(e, "phase") == Some(Value::str("end")))
        .expect("the drag ended");
    assert_eq!(field(end, "line"), Some(Value::Int(2)));
    assert_eq!(field(end, "clicks"), Some(Value::Int(1)));
}

#[test]
fn a_click_carries_the_press_count_and_the_gutter_is_the_line_beside_it() {
    let mut core = Core::new();
    frame(&mut core, &["hello world", "second line"]);
    // A double click in the gutter, level with the second line: the
    // click (no x/y of its own) reads the cursor, the line is the one
    // beside the gutter, and the byte is the line's start.
    let at = Vec2::new(10.0 + 5.0, 10.0 + LH + 2.0);
    core.handle_input(InputEvent::CursorMoved(at));
    core.handle_input(InputEvent::mouse_down(2));
    let evs = core.handle_input(InputEvent::mouse_up());
    let click = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("hit"))
        .expect("the click arrived");
    assert_eq!(field(click, "line"), Some(Value::Int(1)));
    assert_eq!(field(click, "byte"), Some(Value::Int(0)));
    assert_eq!(field(click, "clicks"), Some(Value::Int(2)));
    // The drag's start carried the same count.
}

#[test]
fn below_the_last_line_is_the_last_line_and_a_sink_without_lines_adds_nothing() {
    let mut core = Core::new();
    frame(&mut core, &["only"]);
    let evs = press(&mut core, Vec2::new(10.0 + GUTTER + 200.0, 250.0));
    let start = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("drag"))
        .expect("the drag started");
    assert_eq!(field(start, "line"), Some(Value::Int(0)));
    // Past the end of the line's text: its length.
    assert_eq!(field(start, "byte"), Some(Value::Int(4)));
    release(&mut core);

    // A sink that draws no lines: the payload is as it was.
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let sink = ui.text_in_keyed(
        "plain",
        NodeSpec::column().fill().key_sink().on_drag("d"),
        "no lines here",
        mono(),
    );
    ui.take_key_focus(sink);
    ui.finish();
    let evs = press(&mut core, Vec2::new(50.0, 50.0));
    let start = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("drag"))
        .expect("the drag started");
    assert_eq!(field(start, "line"), None);
    assert_eq!(field(start, "byte"), None);
    assert_eq!(field(start, "clicks"), None);
    release(&mut core);
}

/// A click nothing pressed for — Enter or Space on a focused control, a
/// screen reader's `click` — gains nothing: the cursor is wherever the
/// mouse rests and the last press was about something else.
#[test]
fn a_click_without_a_press_gains_nothing() {
    use kui_core::{AccessAction, AccessRequest};
    let mut core = Core::new();
    let sink = frame(&mut core, &["hello world", "second"]);
    // A real double click on line 2 leaves its count behind.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(
        10.0 + GUTTER + 20.0,
        10.0 + LH + 5.0,
    )));
    core.handle_input(InputEvent::mouse_down(2));
    core.handle_input(InputEvent::mouse_up());
    // Then assistive technology clicks the sink: no press, no fields.
    let evs = core.handle_input(InputEvent::Access(AccessRequest::new(
        sink,
        AccessAction::Click,
    )));
    let click = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("hit"))
        .expect("the click arrived");
    assert_eq!(field(click, "line"), None, "{:?}", click.payload);
    assert_eq!(field(click, "byte"), None);
    assert_eq!(field(click, "clicks"), None);
}

/// A key event on the sink is not a pointer event: nothing is attached.
#[test]
fn a_key_on_the_sink_gains_nothing() {
    use kui_core::{KeyCode, KeyMods, KeyPress};
    let mut core = Core::new();
    frame(&mut core, &["hello"]);
    let evs = core.handle_input(InputEvent::KeyDown(KeyPress::new(
        KeyCode::Char('j'),
        KeyMods::default(),
    )));
    let key = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("key"))
        .expect("the key arrived");
    assert_eq!(field(key, "line"), None);
    assert_eq!(field(key, "clicks"), None);
}

/// AR11: the pass used to decide "pointer-made" from the payload's
/// `kind` against the events table, whose editor row was spelled
/// `"changed / submit"` — so a stock `<edit>` under a sink that draws
/// lines (a shell with a minibuffer) got `line`, `byte` and `clicks` on
/// every keystroke, resolved from wherever the mouse rested, after a
/// walk of the whole sink. An event is pointer-made where it is built
/// now, and a keystroke's `changed` is not.
#[test]
fn an_editors_changed_under_the_sink_gains_nothing() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let mut field = Key::ROOT;
    ui.with_keyed("shell", NodeSpec::column().fill().key_sink(), |ui| {
        ui.text_in(
            NodeSpec::row().height(LH).role(Role::Line),
            "a line",
            mono(),
        );
        field = kui_core::widgets::text_input(ui, "minibuffer", "");
    });
    ui.finish();
    // A click on the line row first — the count it leaves behind is what
    // the old pass read for every event after it — then focus into the
    // field and type, with the mouse still resting on the line.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 10.0 + 5.0)));
    core.handle_input(InputEvent::mouse_down(1));
    core.handle_input(InputEvent::mouse_up());
    core.set_focus(Some(field));
    let evs = core.handle_input(InputEvent::Text("x".into()));
    let changed = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("changed"))
        .expect("the editor changed");
    assert_eq!(changed.key, field);
    assert_eq!(field_of(changed, "line"), None, "{:?}", changed.payload);
    assert_eq!(field_of(changed, "byte"), None);
    assert_eq!(field_of(changed, "clicks"), None);
}

/// The other half of the same defect: a click whose payload spelled
/// `{kind: "click"}` — a name in the events table — counted as the
/// core's and was skipped. The mark is the press's now, whatever the
/// payload says.
#[test]
fn a_click_payload_named_like_a_core_event_still_gains_its_line() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let sink = ui.with_keyed(
        "editor",
        NodeSpec::column()
            .fill()
            .key_sink()
            .on_click(Value::map([("kind", Value::str("click"))])),
        |ui| {
            for line in ["one", "two"] {
                ui.text_in(NodeSpec::row().height(LH).role(Role::Line), line, mono());
            }
        },
    );
    ui.take_key_focus(sink);
    ui.finish();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(12.0, 10.0 + LH + 5.0)));
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
    let click = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("click"))
        .expect("the click arrived");
    assert_eq!(
        field_of(click, "line"),
        Some(Value::Int(1)),
        "{:?}",
        click.payload
    );
    assert_eq!(field_of(click, "clicks"), Some(Value::Int(1)));
}

/// RG75: a `button` event of the core's carries its own `clicks`, so the
/// pass left a payload of that kind without one — and an app's click
/// payload spelled `{kind: "button"}` lost its count with it. The core's
/// is told by its `phase` and `button` fields now.
#[test]
fn a_click_payload_named_button_still_gains_its_count() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let sink = ui.with_keyed(
        "editor",
        NodeSpec::column().fill().key_sink().on_click(Value::map([
            ("kind", Value::str("button")),
            ("id", Value::Int(3)),
        ])),
        |ui| {
            for line in ["one", "two"] {
                ui.text_in(NodeSpec::row().height(LH).role(Role::Line), line, mono());
            }
        },
    );
    ui.take_key_focus(sink);
    ui.finish();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(12.0, 10.0 + LH + 5.0)));
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
    let click = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("button"))
        .expect("the click arrived");
    assert_eq!(
        field_of(click, "line"),
        Some(Value::Int(1)),
        "{:?}",
        click.payload
    );
    assert_eq!(field_of(click, "clicks"), Some(Value::Int(1)));
}

#[test]
fn the_sink_yanks_to_the_clipboard_and_a_paste_comes_back_as_text() {
    let mut core = Core::new();
    let sink = frame(&mut core, &["hello"]);
    assert!(core.take_menu_actions().is_empty());
    // The two doors queue the two actions a menu's Copy and Paste would.
    core.set_clipboard("yanked", None);
    core.request_paste();
    assert_eq!(
        core.take_menu_actions(),
        vec![
            MenuAction::SetClipboard {
                text: "yanked".into(),
                html: None
            },
            MenuAction::Paste
        ]
    );
    assert!(core.take_menu_actions().is_empty());
    // The host answers a paste with a commit, which the focused sink
    // hears as `text` — the way it hears an IME's commit.
    let evs = core.handle_input(InputEvent::Commit("from the clipboard".into()));
    let text = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("text"))
        .expect("the paste arrived");
    assert_eq!(text.key, sink);
    assert_eq!(field(text, "text"), Some(Value::str("from the clipboard")));
    // From a view too, through `Ui`.
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.set_clipboard("from the view", Some("<b>from the view</b>".into()));
    ui.finish();
    assert_eq!(
        core.take_menu_actions(),
        vec![MenuAction::SetClipboard {
            text: "from the view".into(),
            html: Some("<b>from the view</b>".into())
        }]
    );
}

/// With nothing focused, a root sink hears the paste the way it hears an
/// unclaimed key (ADR 0022, decision 8) — a shell that asked for one with
/// nothing focused would otherwise lose it.
#[test]
fn a_root_sink_hears_a_paste_with_nothing_focused() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().on_key("shell"));
    ui.text("nothing focusable", mono());
    ui.finish();
    assert_eq!(core.focus(), None);
    let evs = core.handle_input(InputEvent::Commit("pasted".into()));
    let text = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("text"))
        .expect("the root sink heard it");
    assert_eq!(text.key, Key::ROOT);
    assert_eq!(field(text, "tag"), Some(Value::str("shell")));
}

/// One paste ask at a time (backlog AR34): a view that asks on every frame
/// until the answer lands queues one `Paste`, not one per frame, and the
/// commit that answers — empty for an empty clipboard — lets the next ask
/// through. A menu's Paste row goes through the same gate.
#[test]
fn a_second_paste_ask_is_dropped_until_the_first_is_answered() {
    let mut core = Core::new();
    frame(&mut core, &["hello"]);
    assert!(!core.awaiting_paste());
    core.request_paste();
    core.request_paste();
    core.request_paste();
    assert!(core.awaiting_paste());
    assert_eq!(core.take_menu_actions(), vec![MenuAction::Paste], "one ask");
    // Taken by the driver, still unanswered: an ask now is still a
    // duplicate, since the driver's answer is on its way.
    core.request_paste();
    assert!(core.take_menu_actions().is_empty());
    assert!(core.awaiting_paste());
    // The answer, empty: the clipboard held nothing. The sink hears it
    // as a text of nothing, and the gate opens.
    let evs = core.handle_input(InputEvent::Commit(String::new()));
    assert_eq!(evs.len(), 1);
    assert!(!core.awaiting_paste());
    core.request_paste();
    assert_eq!(
        core.take_menu_actions(),
        vec![MenuAction::Paste],
        "the next ask goes through"
    );
    core.handle_input(InputEvent::Commit("pasted".into()));
    assert!(!core.awaiting_paste());
}

/// A paste's answer says what the pasteboard marked it (backlog F84): a
/// password manager's secret reaches the sink with `concealed` and
/// `transient` beside the text, each only when it is set, so a sink that
/// never heard of them sees the payload it always did. The answer clears
/// the one-ask gate the way a bare commit does, and a secret the app puts
/// on the clipboard itself is its own action for the host to write marked.
#[test]
fn a_paste_carries_the_pasteboards_markers_to_the_sink() {
    let mut core = Core::new();
    let sink = frame(&mut core, &["hello"]);
    core.request_paste();
    assert_eq!(core.take_menu_actions(), vec![MenuAction::Paste]);
    let evs = core.handle_input(InputEvent::Paste {
        text: "hunter2".into(),
        marks: ClipboardMarks::SECRET,
    });
    assert!(!core.awaiting_paste(), "a paste is the answer");
    let text = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("text"))
        .expect("the paste arrived");
    assert_eq!(text.key, sink);
    assert_eq!(field(text, "text"), Some(Value::str("hunter2")));
    assert_eq!(field(text, "concealed"), Some(Value::Bool(true)));
    assert_eq!(field(text, "transient"), Some(Value::Bool(true)));

    // One marker: only it rides along.
    let evs = core.handle_input(InputEvent::Paste {
        text: "brief".into(),
        marks: ClipboardMarks {
            concealed: false,
            transient: true,
        },
    });
    let text = evs
        .iter()
        .find(|e| kind(e).as_deref() == Some("text"))
        .unwrap();
    assert_eq!(field(text, "concealed"), None, "absent, never false");
    assert_eq!(field(text, "transient"), Some(Value::Bool(true)));

    // Unmarked, and a bare commit: the payload it always was.
    for ev in [
        InputEvent::Paste {
            text: "plain".into(),
            marks: ClipboardMarks::default(),
        },
        InputEvent::Commit("plain".into()),
    ] {
        let evs = core.handle_input(ev);
        let text = evs
            .iter()
            .find(|e| kind(e).as_deref() == Some("text"))
            .unwrap();
        assert_eq!(field(text, "concealed"), None);
        assert_eq!(field(text, "transient"), None);
    }

    // The other way: a secret the app copies is its own action.
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.set_clipboard_secret("s3cret");
    ui.finish();
    assert_eq!(
        core.take_menu_actions(),
        vec![MenuAction::SetClipboardSecret {
            text: "s3cret".into()
        }]
    );
    // And the bits the C ABI spells them in round-trip.
    for bits in 0..4 {
        assert_eq!(ClipboardMarks::from_bits(bits).bits(), bits);
    }
    assert_eq!(ClipboardMarks::SECRET.bits(), 3);
    assert!(ClipboardMarks::default().is_empty());
}

/// RG37: what the runner leaves to a sink. Over a selection scope the
/// runner's ⌘V pastes only into a focused editor; a focused sink hears
/// the raw press — its own binding asks for the paste — and never a bare
/// `Text`, which is what the runner sent it before, to nobody.
#[test]
fn a_sink_hears_the_paste_chord_and_never_a_bare_text() {
    let mut core = Core::new();
    let sink = frame(&mut core, &["hello"]);
    let cmd_v = KeyPress::new(KeyCode::Char('v'), KeyMods::NONE.with_super());
    let evs = core.handle_input(InputEvent::KeyDown(cmd_v));
    assert!(
        evs.iter()
            .any(|e| e.key == sink && kind(e).as_deref() == Some("key")),
        "the sink hears the chord: {evs:?}"
    );
    let evs = core.handle_input(InputEvent::Text("hunter2".into()));
    assert!(
        !evs.iter()
            .any(|e| e.key == sink && kind(e).as_deref() == Some("text")),
        "a bare Text is not the sink's: {evs:?}"
    );
}

/// RG75: a `button` event inside a sink that draws lines carries `line`
/// and `byte`, and `UiEvent::button()` reads them, as `drag()` does — it
/// read the cell and not these.
#[test]
fn a_button_press_in_a_sink_reads_its_line_and_byte() {
    use kui_core::{Buttons, MouseButton};
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let sink = ui.with_keyed(
        "editor",
        NodeSpec::column()
            .fill()
            .key_sink()
            .on_button("btn")
            .buttons(Buttons::MIDDLE),
        |ui| {
            for line in ["one", "two"] {
                ui.text_in(NodeSpec::row().height(LH).role(Role::Line), line, mono());
            }
        },
    );
    ui.take_key_focus(sink);
    ui.finish();
    let w = core.measure_text("M", &mono(), None).width;
    core.handle_input(InputEvent::CursorMoved(Vec2::new(
        10.0 + 2.2 * w,
        10.0 + LH + 5.0,
    )));
    let evs = core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Middle,
        clicks: 1,
    });
    let b = evs.iter().find_map(|e| e.button()).expect("the press");
    assert_eq!((b.line, b.byte), (Some(1), Some(2)), "{evs:?}");
}
