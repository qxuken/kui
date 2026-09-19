//! The clipboard: every way onto it, and the two ways a paste lands. The
//! clipboard is the host's — the core never reads it — so a copy is the
//! core, or the app, working out *what* and handing the host the text,
//! and a paste is the host reading the clipboard and handing the text
//! back as input. Four things to copy from, each a different rule:
//!
//!   * **The field.** An `edit`: Cmd/Ctrl-C/X/V are the runner's own —
//!     it performs the chords while an editor has focus and writes the
//!     clipboard itself, no queue — and its context menu's Cut / Copy /
//!     Paste are the same three as `MenuAction`s the host drains.
//!   * **The card.** A `selectable` scope: the same chord copies its runs
//!     in reading order, the bold carried beside the plain text as HTML;
//!     its menu's Copy queues the same.
//!   * **The log.** A selectable virtual list: a copy whose selection
//!     reaches rows the frame never built cannot be answered by the core,
//!     so it is a `selectionrange` ask to the app — the rows are the
//!     app's — and `Ui::answer_selection_range` is what reaches the
//!     clipboard (ADR 0017, tier 3).
//!   * **The register.** An `on_key` sink that owns its lines: the runner
//!     leaves the chords to it, and it binds `y` to `Ui::set_clipboard`
//!     and `p` to `Ui::request_paste` (backlog C33). The paste comes back
//!     as the `{kind="text"}` event an IME's commit arrives on, and the
//!     sink appends it as a line.
//!
//! And two things every selection does on the way to a copy (ADR 0029):
//! a Shift-click extends it from its anchor instead of starting over, and
//! a drag held past the log's edge scrolls the log toward the pointer —
//! the wheel under a held press moves the live end too.
//!
//! Every path but the first ends in one queue — `MenuAction::SetClipboard`
//! and `MenuAction::Paste` — which the runner drains after every input and
//! every frame, and a headless drive reads with `take_menu_actions`. The
//! readout at the bottom is the app's side of it: what it last handed
//! over, and what last came back.
//!
//! Run: cargo run -p kui --example clipboard [-- --headless]

use kui::widgets;
use kui::{
    Align, App, Core, EditKey, EditOptions, MenuAction, Mods, NodeSpec, Role, Sizing, Span,
    TextStyle, Theme, Ui, UiEvent, Value,
};
use kui_devtools::{Drive, Example};

const ROWS: usize = 2000;
const ROW_H: f32 = 22.0;

struct Clipboard {
    /// The register's lines and its highlighted one.
    lines: Vec<String>,
    cursor: usize,
    /// What `y` chose, handed to `ui.set_clipboard` by the next view —
    /// `on_event` has no `Ui`, and the runner draws right after an event.
    clip_out: Option<String>,
    /// `p` asked: the next `text` event is the paste.
    awaiting_paste: bool,
    /// A `selectionrange` ask the log has to answer, as the rows it named.
    answer: Option<String>,
    /// The readout.
    sent: String,
    received: String,
}

impl Clipboard {
    fn new() -> Self {
        Self {
            lines: vec![
                "j / k move, y copies the line, p pastes a new one".into(),
                "the sink hears Cmd-C raw and binds nothing to it".into(),
                "so the clipboard is two calls on Ui".into(),
            ],
            cursor: 0,
            clip_out: None,
            awaiting_paste: false,
            answer: None,
            sent: "nothing yet".into(),
            received: "nothing yet".into(),
        }
    }

    /// The log's row `i`, the app's own data — what a `selectionrange`
    /// ask is answered from.
    fn row(i: usize) -> String {
        format!("{i:>4}  log line {i}")
    }

    /// The text of the rows an ask named: `from.index`..=`to.index`,
    /// joined by newlines, cut to the bytes at each end.
    fn range_text(p: &Value) -> Option<String> {
        let end = |name: &str| {
            let e = p.get(name)?;
            Some((
                e.get("index")?.as_int()? as usize,
                e.get("byte")?.as_int()? as usize,
            ))
        };
        let (from, to) = (end("from")?, end("to")?);
        let mut text: String = (from.0..=to.0.min(ROWS - 1))
            .map(Self::row)
            .collect::<Vec<_>>()
            .join("\n");
        // The last row is cut at `to.byte`, then the first at `from.byte`.
        if to.0 < ROWS {
            let last_start = text.len() - Self::row(to.0).len();
            text.truncate(last_start + to.1.min(Self::row(to.0).len()));
        }
        Some(text[from.1.min(text.len())..].to_string())
    }
}

fn card(t: &Theme) -> NodeSpec {
    NodeSpec::column()
        .width(Sizing::Grow(1.0))
        .max_width(600.0)
        .pad(14.0)
        .gap(8.0)
        .bg(t.surface)
        .radius(10.0)
        .border(1.0, t.border)
}

fn caption(ui: &mut Ui<'_>, t: &Theme, s: &str) {
    ui.text(s, TextStyle::new(12.0).color(t.muted));
}

impl App for Clipboard {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        // The app's half of the queue: what the keymap chose, the paste it
        // asked for, and the answer the log owes — all through `Ui`.
        if let Some(text) = self.clip_out.take() {
            ui.set_clipboard(text, None);
        }
        // Asked on every frame the answer is outstanding: the core queues
        // one ask at a time, so this is one paste and not one per frame
        // (backlog AR34).
        if self.awaiting_paste {
            ui.request_paste();
        }
        if let Some(text) = self.answer.take() {
            self.sent = format!("the log's rows: {} bytes", text.len());
            ui.answer_selection_range(&text);
        }
        ui.with(
            NodeSpec::column()
                .fill()
                .cross_align(Align::Center)
                .pad(20.0)
                .gap(10.0)
                .scroll_y(),
            |ui| {
                // The field: the runner's chords, and the stock menu.
                ui.with(card(&t), |ui| {
                    caption(
                        ui,
                        &t,
                        "an editor: ⌘C ⌘X ⌘V are the runner's, its menu queues",
                    );
                    ui.text_edit(
                        "note",
                        "Select some of this and copy it; paste lands as typing.",
                        &EditOptions::default(),
                        NodeSpec::column()
                            .width(Sizing::Grow(1.0))
                            .pad(8.0)
                            .radius(6.0)
                            .bg(t.bg)
                            .border(1.0, t.border)
                            .label("note"),
                    );
                });

                // The card: a scope's runs, with the formatting beside.
                ui.with_keyed("article", card(&t).selectable(), |ui| {
                    caption(
                        ui,
                        &t,
                        "a selectable scope: ⌘C copies the runs, the bold as HTML beside",
                    );
                    ui.rich_text(
                        &[
                            Span::new("Drag across this and the "),
                            Span::new("bold").bold(),
                            Span::new(" comes along as a second flavour the host may offer."),
                        ],
                        TextStyle::new(14.0).line_height(22.0),
                    );
                });

                // The log: a copy over rows the frame never built is
                // asked of the app.
                ui.with(card(&t).pad(8.0), |ui| {
                    caption(
                        ui,
                        &t,
                        "a virtual list: select, scroll away, ⌘C — the app answers",
                    );
                    widgets::virtual_column(
                        ui,
                        "log",
                        NodeSpec::column()
                            .width(Sizing::Grow(1.0))
                            .height(Sizing::Fixed(4.0 * ROW_H))
                            .bg(t.sunken)
                            .radius(6.0)
                            .selectable()
                            .role(Role::List)
                            .label("log"),
                        ROWS,
                        ROW_H,
                        |ui, i| {
                            ui.with(
                                NodeSpec::row()
                                    .width(Sizing::Grow(1.0))
                                    .height(Sizing::Grow(1.0))
                                    .pad_xy(8.0, 0.0)
                                    .cross_align(Align::Center),
                                |ui| {
                                    ui.text(&Self::row(i), TextStyle::new(12.0).mono().color(t.fg));
                                },
                            );
                        },
                    );
                });

                // The register: a sink with its own bindings.
                let cursor = self.cursor;
                let lines = &self.lines;
                let sink = ui.with_keyed(
                    "register",
                    card(&t)
                        .on_key(Value::Null)
                        .focusable()
                        .role(Role::Group)
                        .label("register"),
                    |ui| {
                        caption(
                            ui,
                            &t,
                            "an on_key sink: y → set_clipboard, p → request_paste",
                        );
                        for (i, line) in lines.iter().enumerate() {
                            ui.with(
                                NodeSpec::row()
                                    .width(Sizing::Grow(1.0))
                                    .pad_xy(8.0, 3.0)
                                    .radius(4.0)
                                    .bg(if i == cursor { t.selection } else { t.surface }),
                                |ui| {
                                    ui.text(line, TextStyle::new(13.0).mono().color(t.fg));
                                },
                            );
                        }
                    },
                );
                // Edge-triggered: focus lands once, when the sink starts
                // being declared, and a click in the field moves it away.
                ui.take_key_focus(sink);

                // The readout: the app's side of the queue.
                ui.with(
                    NodeSpec::column()
                        .width(Sizing::Grow(1.0))
                        .max_width(600.0)
                        .gap(2.0),
                    |ui| {
                        ui.text(
                            &format!("→ clipboard: {}", self.sent),
                            TextStyle::new(12.0).color(t.accent),
                        );
                        ui.text(
                            &format!("← paste: {}", self.received),
                            TextStyle::new(12.0).color(t.accent),
                        );
                    },
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("key") => match ev.payload.get("code").and_then(Value::as_str) {
                Some("j") | Some("down") => {
                    self.cursor = (self.cursor + 1).min(self.lines.len() - 1);
                }
                Some("k") | Some("up") => self.cursor = self.cursor.saturating_sub(1),
                Some("y") => {
                    let line = self.lines[self.cursor].clone();
                    self.sent = format!("the register's line: {line:?}");
                    self.clip_out = Some(line);
                }
                Some("p") => {
                    self.awaiting_paste = true;
                }
                _ => {}
            },
            // The paste `p` asked for — or an IME's commit, which this
            // sink has no use for.
            Some("text") if self.awaiting_paste => {
                self.awaiting_paste = false;
                let text = ev.payload.get("text").and_then(Value::as_str).unwrap_or("");
                self.received = format!("{text:?}");
                self.lines.push(text.to_string());
                self.cursor = self.lines.len() - 1;
            }
            // The log's copy reached rows no frame built: the app knows
            // its own rows, and answers from them in the next view.
            Some("selectionrange") => self.answer = Self::range_text(&ev.payload),
            _ => {}
        }
    }
}

impl Example for Clipboard {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        ("⌘C ⌘X ⌘V", "in the field and the card"),
        ("⇧-click", "extend a selection"),
        ("drag past the edge", "scroll the log"),
        ("right-click", "the stock menu's Copy / Paste"),
        ("j k y p", "in the register"),
    ];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(640.0, 640.0)
    }

    /// Each path onto the queue, and what it leaves there.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        use kui::{CopyRequest, InputEvent, KeyMods};
        let mut d = Drive::new(core, 640.0, 640.0);
        d.core.set_native_menus(false);
        d.frame(self);
        let note = d.key_of("note").ok_or("no field")?;
        let article = d.key_of("article").ok_or("no card")?;
        let log = d.key_of("log").ok_or("no log")?;
        let register = d.key_of("register").ok_or("no register")?;

        // The field: the chord's copy never enters the queue — the core
        // answers `request_copy` at once and the runner writes the
        // clipboard itself; a paste is typing.
        d.core.set_focus(Some(note));
        d.input(self, InputEvent::Key(EditKey::SelectAll, Mods::default()));
        let ready = matches!(d.core.request_copy(), CopyRequest::Ready(t) if t.contains("copy it"));
        d.check(
            ready,
            "the field's copy is answered at once, from the editor's selection",
        )?;
        let queued = d.core.take_menu_actions();
        d.check(
            queued.is_empty(),
            "and nothing is queued: the runner writes the clipboard itself",
        )?;
        d.input(self, InputEvent::Text("pasted".into()));
        d.check(
            d.core.edit_text(note).as_deref() == Some("pasted"),
            "a paste into the field is typing over the selection",
        )?;

        // The card: a drag across the scope selects its runs as one, the
        // copy is answered at once, with the flavour, and its menu's Copy
        // queues both for the host.
        let r = d.rect_of(article).ok_or("the card has no rect")?;
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 30.0, r.y + 40.0)),
        );
        d.input(self, InputEvent::mouse_down(1));
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 300.0, r.y + 44.0)),
        );
        d.input(self, InputEvent::mouse_up());
        d.check(
            d.core.selection_text().is_some_and(|s| s.contains("bold")),
            "a drag across the card selects its runs as one",
        )?;
        d.check(
            d.core.selection_html().is_some_and(|h| h.contains("<b>")),
            "and the copy carries the bold as HTML",
        )?;
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 30.0, r.y + 40.0)),
        );
        d.input(
            self,
            InputEvent::MouseDown {
                button: kui::MouseButton::Secondary,
                clicks: 1,
            },
        );
        d.input(
            self,
            InputEvent::MouseUp {
                button: kui::MouseButton::Secondary,
            },
        );
        let menu = d.core.menu().cloned().ok_or("no menu over the card")?;
        let copy = menu
            .items
            .iter()
            .position(|i| i.role == kui::MenuRole::Copy)
            .ok_or("no Copy row")?;
        d.core.activate_menu_item(copy).ok_or("Copy refused")?;
        let queued = d.core.take_menu_actions();
        d.check(
            matches!(&queued[..], [MenuAction::SetClipboard { text, html: Some(h) }] if text.contains("bold") && h.contains("<b>")),
            "the menu's Copy queues the text and the HTML for the host",
        )?;

        // The log: select across rows, scroll them out of the frame, and
        // the copy is a question for the app.
        let r = d.rect_of(log).ok_or("the log has no rect")?;
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 20.0, r.y + 6.0)),
        );
        d.input(self, InputEvent::mouse_down(1));
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 200.0, r.y + 2.5 * ROW_H)),
        );
        d.input(self, InputEvent::mouse_up());
        d.frame(self);
        // Held past the log's bottom edge, the log scrolls toward the
        // pointer a frame at a time and the live end follows (ADR 0029):
        // half a second 60 px past is 600 px/s, so rows 0..2 became rows
        // 0..~13. The wheel under the held press moves it too.
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 20.0, r.y + 6.0)),
        );
        d.input(self, InputEvent::mouse_down(1));
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 200.0, r.y + r.h + 60.0)),
        );
        for _ in 0..30 {
            d.advance(1.0 / 60.0);
            d.frame(self);
        }
        let scrolled = d.core.scroll_offset(log).y;
        d.check(
            scrolled > 10.0 * ROW_H,
            "a press held past the edge scrolls the log toward the pointer",
        )?;
        d.check(
            d.core.selection().is_some_and(|s| s.focus.row >= Some(10)),
            "and the live end followed onto the rows that scrolled in",
        )?;
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 200.0, r.y + 2.5 * ROW_H)),
        );
        d.wheel(self, r.x + 200.0, r.y + 2.5 * ROW_H, 0.0, -20.0 * ROW_H);
        d.frame(self);
        d.frame(self);
        d.check(
            d.core.selection().is_some_and(|s| s.focus.row >= Some(30)),
            "the wheel under a held press moves the live end with the rows",
        )?;
        d.input(self, InputEvent::mouse_up());
        d.frame(self);
        // A Shift-click keeps the anchor: the selection still starts on
        // row 0 and now ends where the click landed.
        d.input(
            self,
            InputEvent::Modifiers(KeyMods {
                shift: true,
                ..KeyMods::default()
            }),
        );
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 100.0, r.y + 1.5 * ROW_H)),
        );
        d.input(self, InputEvent::mouse_down(1));
        d.input(self, InputEvent::mouse_up());
        d.input(self, InputEvent::Modifiers(KeyMods::default()));
        d.frame(self);
        let sel = d
            .core
            .selection()
            .ok_or("the Shift-click lost the selection")?;
        d.check(
            sel.anchor.row == Some(0) && sel.focus.row.is_some_and(|f| f > 25),
            "a Shift-click extends from the anchor instead of starting over",
        )?;
        // Back to the top for the copy below: a plain drag over rows 0..2.
        d.core.set_scroll(log, kui::Vec2::ZERO);
        d.frame(self);
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 20.0, r.y + 6.0)),
        );
        d.input(self, InputEvent::mouse_down(1));
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 200.0, r.y + 2.5 * ROW_H)),
        );
        d.input(self, InputEvent::mouse_up());
        d.frame(self);
        d.wheel(self, r.x + 100.0, r.y + 40.0, 0.0, -40.0 * ROW_H);
        d.frame(self);
        let asked = d.core.request_copy() == CopyRequest::Asked;
        d.check(
            asked,
            "a copy over rows the frame never built is asked of the app",
        )?;
        d.frame(self); // the ask reaches `on_event`
        d.check(
            self.answer.is_some(),
            "as a `selectionrange` the log can answer from its rows",
        )?;
        d.frame(self); // the view answers
        let queued = d.core.take_menu_actions();
        // The press landed a few bytes into row 0, so the answer starts
        // mid-row; the drag ended on row 2.
        let forwards = match &queued[..] {
            [MenuAction::SetClipboard { text, html: None }]
                if text.contains("log line 0\n") && text.contains("log line 2") =>
            {
                text.clone()
            }
            _ => String::new(),
        };
        d.check(
            !forwards.is_empty(),
            "and the answer is what reaches the clipboard",
        )?;
        // The same drag made backwards — pressed on row 2, released on
        // row 0 — is asked for as the same range: `from` precedes `to`
        // whichever end the press was, so the app's `from..=to` answers
        // the same rows.
        d.core.set_scroll(log, kui::Vec2::ZERO);
        d.frame(self);
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 200.0, r.y + 2.5 * ROW_H)),
        );
        d.input(self, InputEvent::mouse_down(1));
        d.input(
            self,
            InputEvent::CursorMoved(kui::Vec2::new(r.x + 20.0, r.y + 6.0)),
        );
        d.input(self, InputEvent::mouse_up());
        d.frame(self);
        d.wheel(self, r.x + 100.0, r.y + 40.0, 0.0, -40.0 * ROW_H);
        d.frame(self);
        let asked = d.core.request_copy() == CopyRequest::Asked;
        d.check(
            asked,
            "a backwards drag over unbuilt rows is asked the same way",
        )?;
        d.frame(self);
        d.frame(self);
        let queued = d.core.take_menu_actions();
        d.check(
            matches!(&queued[..], [MenuAction::SetClipboard { text, html: None }] if *text == forwards),
            "and asks for the same rows in reading order, so the answer is the same text",
        )?;

        // The register: the sink's own bindings, through `Ui`.
        d.core.set_focus(Some(register));
        d.key(self, "j", KeyMods::default());
        d.key(self, "y", KeyMods::default());
        d.frame(self);
        let queued = d.core.take_menu_actions();
        d.check(
            matches!(&queued[..], [MenuAction::SetClipboard { text, .. }] if text.starts_with("the sink hears")),
            "y hands the sink's line to the host through set_clipboard",
        )?;
        d.key(self, "p", KeyMods::default());
        d.frame(self);
        let queued = d.core.take_menu_actions();
        d.check(
            queued == vec![MenuAction::Paste],
            "p asks the host for the clipboard",
        )?;
        // The host reads it and commits; the sink hears the text.
        d.input(self, InputEvent::Commit("from another app".into()));
        d.check(
            self.lines.last().map(String::as_str) == Some("from another app"),
            "and the paste comes back as the sink's text event",
        )?;
        d.check(!self.awaiting_paste, "once")
    }
}

kui_devtools::main!(Clipboard::new());
