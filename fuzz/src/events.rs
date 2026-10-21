//! What an event arrives as (ADR 0047), from every handler row at once:
//! a view whose nodes declare messages the fuzzer chose — maps whose keys
//! may be the very field the event writes, strings, numbers, `Null` — and
//! an editor whose `value` the app declares back from the `change` it
//! heard, through a filter the fuzzer chose, the way an app keeps a field
//! in its model.
//!
//! Checked on every event: the core can read it back whole
//! (`UiEvent::read` is never `Unread`); the payload keeps every field of
//! the map its handler declared — a node's row, or the root's
//! `windowEvents` for a resize, a modifier change and the window's other
//! own events (ADR 0048) — but the one the event writes, a string or a
//! number under `tag`, and nothing for a `Null`. A `change` from the
//! editor carries its text. After every frame: the editor holds the value
//! the app declared unless a composition is under way, and its caret and
//! selection sit on character boundaries of that text.
//!
//! Every input gets a new core, built in a session kept from the inputs
//! before it (`crate::reuse`).

use arbitrary::{Arbitrary, Result, Unstructured};
use kui_core::access::{AccessAction, AccessRequest};
use kui_core::input::{EditKey, Mods};
use kui_core::{
    Core, EditOptions, Event, EventKind, InputEvent, Key, MenuItem, NodeSpec, Role, Size,
    TextStyle, UiEvent, Value, Vec2, WindowEvents, widgets,
};

use crate::reuse::{self, Kept};

#[derive(Arbitrary, Debug)]
struct Input {
    /// One message per handler row, by [`Row`].
    messages: [Msg; ROWS],
    /// Whether the editor declares `value`, and the filter the app puts
    /// the `change` it heard through before declaring it back.
    value: bool,
    filter: Filter,
    multiline: bool,
    actions: Vec<Action>,
}

/// The rows the view declares, in [`Input::messages`]' order.
#[derive(Clone, Copy, Debug)]
enum Row {
    Change,
    Submit,
    Cancel,
    Boundary,
    Menu,
    Drag,
    Hover,
    Key,
    Click,
    Slider,
    Scroll,
    Focus,
    ContextMenu,
    Layout,
    Select,
    /// Every key of the root's `windowEvents` group.
    Window,
}

const ROWS: usize = 16;

/// A message as an app might declare one.
#[derive(Arbitrary, Debug, Clone)]
enum Msg {
    Null,
    Bool(bool),
    Int(i8),
    Str(Word),
    Map(Vec<(Word, Leaf)>),
}

#[derive(Arbitrary, Debug, Clone)]
enum Leaf {
    Null,
    Int(i16),
    Str(Word),
    /// One level down, so a field the event writes can hold a map already.
    Map(Vec<(Word, i8)>),
}

/// A key or a string: one of the names that matter — `kind`, `tag` and
/// every event's field — most of the time, anything otherwise.
#[derive(Debug, Clone)]
struct Word(String);

const WORDS: &[&str] = &[
    "kind",
    "tag",
    "id",
    "change",
    "submit",
    "cancel",
    "boundary",
    "menu",
    "drag",
    "hover",
    "key",
    "text",
    "preedit",
    "select",
    "click",
    "scroll",
    "focus",
    "contextmenu",
    "layout",
    "button",
    "resize",
    "window",
    "system",
    "modifiers",
    "open",
];

impl<'a> Arbitrary<'a> for Word {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        Ok(Word(if u.ratio(7u8, 8u8)? {
            WORDS[u.choose_index(WORDS.len())?].to_string()
        } else {
            let s: String = u.arbitrary()?;
            s.chars().take(12).collect()
        }))
    }
}

impl Msg {
    fn value(&self) -> Value {
        match self {
            Msg::Null => Value::Null,
            Msg::Bool(b) => Value::Bool(*b),
            Msg::Int(n) => Value::Int(i64::from(*n)),
            Msg::Str(w) => Value::str(w.0.as_str()),
            Msg::Map(entries) => {
                let mut out: Vec<(String, Value)> = Vec::new();
                for (k, v) in entries.iter().take(6) {
                    // A map has one value per key, as every binding's does.
                    if out.iter().any(|(have, _)| *have == k.0) {
                        continue;
                    }
                    let v = match v {
                        Leaf::Null => Value::Null,
                        Leaf::Int(n) => Value::Int(i64::from(*n)),
                        Leaf::Str(w) => Value::str(w.0.as_str()),
                        Leaf::Map(inner) => Value::Map(
                            inner
                                .iter()
                                .take(3)
                                .map(|(k, n)| (k.0.clone(), Value::Int(i64::from(*n))))
                                .collect(),
                        ),
                    };
                    out.push((k.0.clone(), v));
                }
                Value::Map(out)
            }
        }
    }
}

/// What the app does to the editor's text before declaring it back.
#[derive(Arbitrary, Debug, Clone, Copy)]
enum Filter {
    Same,
    Upper,
    Digits,
    /// The first n characters.
    Cap(u8),
    /// A suffix the app appends, as a formatter might.
    Suffix,
    /// The model left where it was: every keystroke reverted.
    Frozen,
}

impl Filter {
    fn apply(self, text: &str, model: &str) -> String {
        match self {
            Filter::Same => text.to_string(),
            Filter::Upper => text.to_uppercase(),
            Filter::Digits => text.chars().filter(char::is_ascii_digit).collect(),
            Filter::Cap(n) => text.chars().take(usize::from(n % 24)).collect(),
            Filter::Suffix => {
                if text.ends_with('!') {
                    text.to_string()
                } else {
                    format!("{text}!")
                }
            }
            Filter::Frozen => model.to_string(),
        }
    }
}

#[derive(Arbitrary, Debug)]
enum Action {
    /// The pointer to a node of the last frame, by index.
    Aim(u8),
    Move(i8, i8),
    Down(bool, u8),
    Up(bool),
    Scroll(i8, i8),
    Text(String),
    Commit(String),
    Preedit(String, Option<(u8, u8)>),
    /// An editing key by index into `EditKey::ALL`, the modifiers as bits.
    Key(u8, u8),
    /// The keyboard to a node of the last frame.
    Focus(u8),
    /// An assistive request on a node of the last frame.
    Access(u8, u8, Option<String>),
    /// A row of the open menu chosen.
    Choose(u8),
    /// The window's own events (ADR 0048): the held modifiers as bits,
    /// the window gaining or losing the keyboard, `n` documents from the
    /// OS, and the frames from here on this much smaller than 400 x 320.
    Modifiers(u8),
    WindowFocus(bool),
    Open(u8),
    Resize(u8, u8),
    Frame,
}

pub fn run(data: &[u8]) {
    drive(data, None);
}

/// What [`run`] saw of `data`, line by line — the events, the warnings,
/// the commands and the editor after every step — for `tests/reuse.rs`
/// to hold a kept session to a new one. Tracing only reads what `run`
/// reads.
pub fn trace(data: &[u8]) -> Vec<String> {
    let mut lines = Vec::new();
    drive(data, Some(&mut lines));
    lines
}

thread_local! {
    static KEPT: Kept<()> = const { Kept::new() };
}

fn drive(data: &[u8], trace: Option<&mut Vec<String>>) {
    let Ok(input) = Input::arbitrary_take_rest(Unstructured::new(data)) else {
        return;
    };
    KEPT.with(|kept| {
        let (core, ()) = kept.core(|_| ());
        let mut d = Driver {
            core,
            messages: input.messages.iter().map(Msg::value).collect(),
            value: input.value,
            filter: input.filter,
            multiline: input.multiline,
            model: String::new(),
            keys: Vec::new(),
            cursor: Vec2::new(10.0, 10.0),
            size: Size::new(400.0, 320.0),
            dirty: false,
            trace,
        };
        d.core.set_diagnostics(true);
        d.core.set_inspect(true);
        d.frame();
        for action in input.actions.iter().take(64) {
            let events = d.apply(action);
            d.note(|| format!("{action:?} -> {events:?}"));
            d.check_events(&events);
            d.frame();
        }
        kept.done(d.core, (), d.dirty);
    });
}

struct Driver<'a> {
    core: Core,
    messages: Vec<Value>,
    value: bool,
    filter: Filter,
    multiline: bool,
    /// The app's model of the editor's text.
    model: String,
    /// The key each row was declared on in the last frame, by [`Row`].
    keys: Vec<(Row, Key)>,
    cursor: Vec2,
    /// The window's viewport: what every frame is built at.
    size: Size,
    /// A window opened or a sound was queued: the session is not handed
    /// back (see [`crate::reuse`]).
    dirty: bool,
    trace: Option<&'a mut Vec<String>>,
}

impl Driver<'_> {
    fn note(&mut self, line: impl FnOnce() -> String) {
        if let Some(t) = self.trace.as_deref_mut() {
            t.push(line());
        }
    }

    fn message(&self, row: Row) -> Value {
        self.messages[row as usize].clone()
    }

    fn frame(&mut self) {
        let messages = self.messages.clone();
        let msg = |row: Row| messages[row as usize].clone();
        let (value, multiline, model) = (self.value, self.multiline, self.model.clone());
        let mut ui = self.core.frame(self.size, 1.0);
        ui.configure_root(NodeSpec::column().fill().pad(8.0).gap(6.0));
        let window = msg(Row::Window);
        ui.window_events(WindowEvents {
            on_resize: Some(window.clone()),
            on_focus: Some(window.clone()),
            on_system_settings_change: Some(window.clone()),
            on_fonts: Some(window.clone()),
            on_modifiers: Some(window.clone()),
            on_open: Some(window),
        });
        let mut keys = Vec::new();
        let opts = EditOptions {
            style: TextStyle::new(14.0),
            multiline,
            autofocus: true,
            ..Default::default()
        };
        let spec = NodeSpec::row()
            .grow_width()
            .on_change(msg(Row::Change))
            .on_submit(msg(Row::Submit))
            .on_cancel(msg(Row::Cancel))
            .on_boundary(msg(Row::Boundary))
            .on_menu(msg(Row::Menu));
        let field = if value {
            ui.text_edit_value("field", &model, &opts, spec)
        } else {
            ui.text_edit("field", "", &opts, spec)
        };
        for row in [
            Row::Change,
            Row::Submit,
            Row::Cancel,
            Row::Boundary,
            Row::Menu,
        ] {
            keys.push((row, field));
        }
        let pad = ui.leaf_keyed(
            "pad",
            NodeSpec::row()
                .width(120.0)
                .height(40.0)
                .on_drag(msg(Row::Drag))
                .on_hover(msg(Row::Hover))
                .on_click(msg(Row::Click))
                .on_scroll(msg(Row::Scroll))
                .on_context_menu(msg(Row::ContextMenu))
                .on_layout(msg(Row::Layout)),
        );
        for row in [
            Row::Drag,
            Row::Hover,
            Row::Click,
            Row::Scroll,
            Row::ContextMenu,
            Row::Layout,
        ] {
            keys.push((row, pad));
        }
        // A key sink that is a custom editor: what a reader's requests and
        // a sink's text and composition reach.
        let sink = ui.with_keyed(
            "sink",
            NodeSpec::column()
                .width(160.0)
                .height(40.0)
                .on_key(msg(Row::Key))
                .on_focus(msg(Row::Focus))
                .role(Role::MultilineTextInput)
                .label("Sink"),
            |ui| {
                ui.with(NodeSpec::row().role(Role::Line), |ui| {
                    ui.text("one two", TextStyle::new(14.0));
                });
            },
        );
        keys.push((Row::Key, sink));
        keys.push((Row::Focus, sink));
        let slider = ui.leaf_keyed(
            "slider",
            NodeSpec::row()
                .width(120.0)
                .height(20.0)
                .role(Role::Slider)
                .value_min(0.0)
                .value_max(10.0)
                .value_now(3.0)
                .on_change(msg(Row::Slider)),
        );
        keys.push((Row::Slider, slider));
        let items: Vec<MenuItem> = ["One", "Two", "Three"]
            .iter()
            .map(|s| MenuItem::new(*s))
            .collect();
        let select =
            widgets::select_items_with(&mut ui, "pick", &items, Some(0), Some(msg(Row::Select)));
        keys.push((Row::Select, select));
        ui.finish();
        self.keys = keys;
        let pending = self.core.take_pending_events();
        self.check_events(&pending);
        let warnings = self.core.take_warnings();
        let windows = self.core.take_window_commands();
        let audio = self.core.take_audio_commands();
        self.dirty |= reuse::opens(&windows) || !audio.is_empty();
        if self.trace.is_some() {
            let tree = format!("{:?}", self.core.access_tree());
            let text = self.core.edit_text(field);
            let caret = self.core.edit.caret_and_selection(field);
            let focus = self.core.focus();
            let model = self.model.clone();
            self.note(|| {
                format!(
                    "frame: {pending:?}\n{warnings:?}\n{windows:?}\n{audio:?}\n\
                     {text:?} {caret:?} focus {focus:?} model {model:?}\n{tree}"
                )
            });
        } else {
            let _ = self.core.access_tree();
        }
        self.check_editor(field);
    }

    /// The editor holds what the app declared, and its caret and selection
    /// are places in that text.
    fn check_editor(&self, field: Key) {
        let Some(text) = self.core.edit_text(field) else {
            panic!("the editor lost its text");
        };
        let composing = self.core.edit.preedit(field).is_some_and(|p| !p.is_empty());
        if self.value && !composing {
            let declared: String = if self.multiline {
                self.model.replace("\r\n", "\n").replace('\r', "\n")
            } else {
                self.model.replace(['\r', '\n'], "")
            };
            assert_eq!(text, declared, "the editor holds the declared value");
        }
        if let Some((caret, selection)) = self.core.edit.caret_and_selection(field) {
            // Offsets into the buffer's text, whose lines may be split on
            // `\n` where the value said `\r\n`; the boundary is what counts.
            let flat = text.replace("\r\n", "\n");
            for at in std::iter::once(caret).chain(selection.into_iter().flat_map(|(a, b)| [a, b]))
            {
                assert!(
                    at <= flat.len() && flat.is_char_boundary(at),
                    "a caret at {at} in {flat:?}"
                );
            }
        }
    }

    fn row_of(&self, key: Key, kind: EventKind) -> Option<Row> {
        let wanted: &[Row] = match kind {
            EventKind::Change => &[Row::Change, Row::Slider],
            EventKind::Submit => &[Row::Submit],
            EventKind::Cancel => &[Row::Cancel],
            EventKind::Boundary => &[Row::Boundary],
            EventKind::Menu => &[Row::Menu, Row::Select],
            EventKind::Drag => &[Row::Drag],
            EventKind::Hover => &[Row::Hover],
            EventKind::Key | EventKind::Text | EventKind::Preedit | EventKind::Select => {
                &[Row::Key]
            }
            EventKind::Click => &[Row::Click],
            EventKind::Scroll => &[Row::Scroll],
            EventKind::Focus => &[Row::Focus],
            EventKind::ContextMenu => &[Row::ContextMenu],
            EventKind::Layout => &[Row::Layout],
            _ => &[],
        };
        self.keys
            .iter()
            .find(|(row, k)| *k == key && wanted.iter().any(|w| *w as usize == *row as usize))
            .map(|(row, _)| *row)
    }

    fn check_events(&mut self, events: &[UiEvent]) {
        for ev in events {
            let read = ev.read();
            assert!(
                !matches!(read, Event::Unread(_)),
                "the core could not read its own event: {ev:?}"
            );
            // The window's own: the root's `windowEvents` message, which
            // every key of the group shares here (no declared windows, so
            // no `window` is a lifecycle one).
            let window_kind = matches!(
                ev.event,
                EventKind::Resize
                    | EventKind::Window
                    | EventKind::System
                    | EventKind::Fonts
                    | EventKind::Modifiers
                    | EventKind::Open
            );
            if window_kind {
                check_payload(ev, &self.message(Row::Window));
                continue;
            }
            // A node's event: is it one of this view's rows, the payload
            // is that row's message with the event's data in it.
            if let Some(row) = self.row_of(ev.key, ev.event) {
                let message = self.message(row);
                check_payload(ev, &message);
            }
            if ev.event == EventKind::Change
                && let Event::EditChange(change) = &read
                && let Some(field) = self.keys.iter().find(|(r, _)| matches!(r, Row::Change))
                && ev.key == field.1
            {
                // The app's loop: the change through the filter, declared
                // back next frame.
                self.model = self.filter.apply(&change.text, &self.model);
            }
        }
        // The last change of a batch is the editor's text now.
        if let Some(field) = self
            .keys
            .iter()
            .find(|(r, _)| matches!(r, Row::Change))
            .map(|f| f.1)
            && let Some(last) = events
                .iter()
                .rev()
                .find(|e| e.key == field && e.event == EventKind::Change)
            && let Event::EditChange(change) = last.read()
            && let Some(now) = self.core.edit_text(field)
        {
            assert_eq!(change.text, now, "a change carries the editor's text");
        }
    }

    fn node_key(&self, i: u8) -> Option<Key> {
        let nodes = self.core.nodes();
        let n = nodes.len();
        nodes
            .into_iter()
            .nth(usize::from(i) % n.max(1))
            .map(|n| n.key)
    }

    fn input(&mut self, ev: InputEvent) -> Vec<UiEvent> {
        self.core.handle_input(ev)
    }

    fn apply(&mut self, action: &Action) -> Vec<UiEvent> {
        match action {
            Action::Aim(i) => {
                let nodes = self.core.nodes();
                let n = nodes.len();
                let Some(node) = nodes.into_iter().nth(usize::from(*i) % n.max(1)) else {
                    return Vec::new();
                };
                self.cursor = node.rect.center();
                self.input(InputEvent::CursorMoved(self.cursor))
            }
            Action::Move(dx, dy) => {
                self.cursor = Vec2::new(
                    self.cursor.x + f32::from(*dx),
                    self.cursor.y + f32::from(*dy),
                );
                self.input(InputEvent::CursorMoved(self.cursor))
            }
            Action::Down(secondary, clicks) => self.input(InputEvent::MouseDown {
                button: if *secondary {
                    kui_core::MouseButton::Secondary
                } else {
                    kui_core::MouseButton::Primary
                },
                clicks: clicks % 4,
            }),
            Action::Up(secondary) => self.input(InputEvent::MouseUp {
                button: if *secondary {
                    kui_core::MouseButton::Secondary
                } else {
                    kui_core::MouseButton::Primary
                },
            }),
            Action::Scroll(dx, dy) => self.input(InputEvent::Scroll(Vec2::new(
                f32::from(*dx),
                f32::from(*dy),
            ))),
            Action::Text(s) => self.input(InputEvent::Text(s.chars().take(16).collect())),
            Action::Commit(s) => self.input(InputEvent::Commit(s.chars().take(16).collect())),
            Action::Preedit(s, range) => self.input(InputEvent::Preedit(
                s.chars().take(8).collect(),
                range.map(|(a, b)| (usize::from(a), usize::from(b))),
            )),
            Action::Key(k, m) => self.input(InputEvent::Key(
                EditKey::ALL[usize::from(*k) % EditKey::ALL.len()],
                Mods {
                    shift: m & 1 != 0,
                    word: m & 2 != 0,
                    doc: m & 4 != 0,
                },
            )),
            Action::Modifiers(bits) => self.input(InputEvent::Modifiers(
                kui_core::KeyMods::from_bits(u32::from(*bits)),
            )),
            Action::WindowFocus(on) => {
                self.core.set_focused(*on);
                self.core.take_pending_events()
            }
            Action::Open(n) => self.input(InputEvent::Open(
                (0..n % 4).map(|i| format!("/doc/{i}.txt")).collect(),
            )),
            Action::Resize(dw, dh) => {
                self.size = Size::new(400.0 - f32::from(*dw), 320.0 - f32::from(*dh));
                Vec::new()
            }
            Action::Focus(i) => {
                let key = self.node_key(*i);
                self.core.set_focus(key);
                Vec::new()
            }
            Action::Access(i, action, value) => {
                let Some(key) = self.node_key(*i) else {
                    return Vec::new();
                };
                let mut req = AccessRequest::new(
                    key,
                    AccessAction::ALL[usize::from(*action) % AccessAction::ALL.len()],
                );
                req.value = value.clone();
                self.input(InputEvent::Access(req))
            }
            Action::Choose(row) => {
                if self.core.menu().is_none() {
                    return Vec::new();
                }
                self.core
                    .activate_menu_item(usize::from(row % 8))
                    .unwrap_or_default()
            }
            Action::Frame => Vec::new(),
        }
    }
}

/// The payload is a map: the declared message with the event's data in
/// it — a map's own fields kept but the one the event writes, a non-map's
/// value under `tag`, and nothing more for a `Null`.
fn check_payload(ev: &UiEvent, message: &Value) {
    let field = ev.event.name();
    let data = ev.data();
    let Value::Map(entries) = &ev.payload else {
        panic!("a node's payload is a map: {ev:?}");
    };
    match message {
        Value::Map(own) => {
            for (k, v) in own {
                if k == field {
                    continue;
                }
                assert_eq!(
                    ev.payload.get(k),
                    Some(v),
                    "the message's own `{k}` kept: {ev:?}"
                );
            }
        }
        Value::Null => {
            let extra = usize::from(data.is_some());
            assert_eq!(
                entries.len(),
                extra,
                "a Null message adds nothing but the data: {ev:?}"
            );
        }
        other => assert_eq!(
            ev.payload.get("tag"),
            Some(other),
            "a non-map message under `tag`: {ev:?}"
        ),
    }
    if let Some(d) = data {
        assert_eq!(
            ev.payload.get(field),
            Some(d),
            "the data rides under `{field}`"
        );
    }
}
