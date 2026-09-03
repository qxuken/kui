//! Every accessibility prop in one window, and the fixture the platform
//! audit drives (`scripts/ax-audit.swift` on macOS, or a screen reader by
//! hand). Each control is built so that assistive technology can both
//! read it and change it, and see the change: the button's name counts
//! its presses, the slider's value follows increment and decrement, and
//! both editors report the text they hold.
//!
//! Run: cargo run -p kui --example accessibility
//!
//! With VoiceOver (⌘F5): VO-right walks the controls, VO-space presses
//! the button, and inside either editor the arrow keys read by character
//! and VO-arrows by word and line.
//!
//! With the keyboard alone (`docs/adr/0002-keyboard-focus-as-data.md`):
//! Tab walks every control in order — the button, the icon button, the
//! switch, the slider, the built-in editor, the app-owned editor — with a
//! ring around the focused one; Enter or Space presses a button or flips
//! the switch, the arrows move the slider, Escape lets go. The app-owned
//! editor is a key sink, so it keeps Tab; its declaration below takes
//! focus once, on the first frame, and never clobbers a Tab press.
//!
//! The tab list reads as "General, tab, 1 of 3, selected" and the
//! disclosure below it as "Advanced, collapsed": `selected` says which of
//! a set is the current one (distinct from a switch being on), `expanded`
//! names a disclosure's state so a shut one can say it is shut, and "1 of
//! 3" is not declared at all — the core numbers the tabs a `tabList`
//! holds.
//!
//! The list under them is the other half of `selected`: a picked row
//! reads as selected where a tab reads as on, and macOS spells those two
//! differently (`AXSelected` against `AXValue`), which is what
//! `scripts/ax-audit.swift` is there to check.
//!
//! "Delete…" opens a modal dialog (`docs/adr/0003-modal-surfaces.md`):
//! Tab cannot leave it, nothing behind it clicks, VoiceOver announces a
//! modal dialog and stays inside it, and Escape or a click outside asks
//! it to close — the app decides, and focus returns to the button that
//! opened it.

use kui::widgets;
use kui::{
    Align, App, Color, EditOptions, FloatConfig, Key, NodeSpec, Role, Sizing, TextStyle, Ui,
    UiEvent, Value,
};

const DOC: &str = "hello world\nsecond line";

struct A11y {
    presses: u32,
    volume: f32,
    muted: bool,
    /// The custom editor's document and its caret (line, byte offset):
    /// the app owns the buffer, kui only learns where the caret sits.
    lines: Vec<String>,
    caret: (usize, usize),
    edit: Key,
    /// Whether the confirm dialog is declared this frame. Nothing else:
    /// the modal is the frame that declares it.
    dialog: bool,
    /// Which tab the tab list shows, and whether the disclosure below is
    /// open: `selected` and `expanded` are these two fields, read out.
    tab: usize,
    advanced: bool,
    /// The picked row of the list below — `selected` again, on the other
    /// role that carries it.
    row: usize,
}

impl A11y {
    fn new() -> Self {
        Self {
            presses: 0,
            volume: 3.0,
            muted: false,
            lines: vec!["fn main() {".into(), "    greet()".into(), "}".into()],
            caret: (1, 4),
            edit: Key::ROOT,
            dialog: false,
            tab: 0,
            advanced: false,
            row: 1,
        }
    }

    /// The line the caret is on, clamped into the document.
    fn caret_line(&self) -> usize {
        self.caret.0.min(self.lines.len().saturating_sub(1))
    }
}

impl App for A11y {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let text = TextStyle::new(14.0).color(Color::rgb8(0xd6, 0xd8, 0xe0));
        ui.configure_root(
            NodeSpec::column()
                .fill()
                .bg(Color::rgb8(0x11, 0x13, 0x1a))
                .gap(10.0)
                .pad(14.0),
        );
        widgets::titlebar(ui, "kui — accessibility");

        // A heading: named by the text inside it, which is then read as
        // part of it rather than as a label of its own.
        ui.with(NodeSpec::row().role(Role::Heading), |ui| {
            ui.text("Controls", TextStyle::new(20.0).color(Color::WHITE))
        });

        // A tab list: `selected` is which one the view shows, and every
        // tab reports the state so a reader can say which is on. Nothing
        // here says "1 of 3" — the core counts what the list holds.
        ui.with_keyed("tabs", NodeSpec::row().role(Role::TabList).gap(4.0), |ui| {
            for (i, name) in ["General", "Network", "About"].iter().enumerate() {
                let on = i == self.tab;
                ui.with_keyed(
                    name,
                    NodeSpec::row()
                        .role(Role::Tab)
                        .selected(on)
                        .on_click(Value::Int(i as i64))
                        .pad_xy(10.0, 6.0)
                        .bg(if on {
                            Color::rgb8(0x3b, 0x5b, 0xd4)
                        } else {
                            Color::rgb8(0x1d, 0x20, 0x2b)
                        })
                        .radius(6.0),
                    |ui| {
                        ui.text(
                            name,
                            TextStyle::new(13.0).color(if on {
                                Color::WHITE
                            } else {
                                Color::rgb8(0x8a, 0x8f, 0xa3)
                            }),
                        )
                    },
                );
            }
        });

        // A disclosure: `expanded` names its state, so a reader says
        // "collapsed" rather than nothing at all when it is shut.
        ui.with_keyed(
            "advanced",
            widgets::button_spec()
                .expanded(self.advanced)
                .on_click(Value::str("advanced"))
                .label("Advanced"),
            |ui| {
                ui.text(
                    if self.advanced {
                        "▾ Advanced"
                    } else {
                        "▸ Advanced"
                    },
                    TextStyle::new(widgets::BUTTON_TEXT).color(Color::WHITE),
                )
            },
        );
        if self.advanced {
            ui.with(
                NodeSpec::row()
                    .pad_xy(10.0, 6.0)
                    .bg(Color::rgb8(0x0e, 0x10, 0x16))
                    .radius(6.0),
                |ui| ui.text("Nothing here yet.", text),
            );
        }

        // A list whose rows can be picked. A row is not named by its
        // content the way a button is — it is a container of content, and
        // giving it a label as well would have it read twice — so its
        // text child is what a reader announces. Nothing here says "2 of
        // 3" either: the core numbers the rows it holds.
        ui.with_keyed(
            "mailboxes",
            NodeSpec::column().role(Role::List).gap(2.0),
            |ui| {
                for (i, name) in ["Inbox", "Drafts", "Sent"].iter().enumerate() {
                    let on = i == self.row;
                    ui.with_keyed(
                        name,
                        NodeSpec::row()
                            .role(Role::ListItem)
                            .selected(on)
                            .focusable()
                            .on_click(Value::str(format!("row{i}")))
                            .width(Sizing::Fixed(200.0))
                            .pad_xy(10.0, 5.0)
                            .bg(if on {
                                Color::rgb8(0x2f, 0x54, 0xc4)
                            } else {
                                Color::rgb8(0x1d, 0x20, 0x2b)
                            })
                            .radius(4.0),
                        |ui| {
                            ui.text(
                                name,
                                TextStyle::new(13.0).color(if on {
                                    Color::WHITE
                                } else {
                                    Color::rgb8(0x8a, 0x8f, 0xa3)
                                }),
                            )
                        },
                    );
                }
            },
        );

        // A button named by its content, so a press is visible through
        // the accessibility API alone. Keyed by hand: `widgets::button`
        // keys a node by its text, and this text changes on every press —
        // a re-keyed node is a new node, which drops keyboard focus and
        // leaves a screen reader's cursor on an element that no longer
        // exists.
        ui.with_keyed(
            "count",
            widgets::button_spec().on_click(Value::str("press")),
            |ui| {
                ui.text(
                    &format!("count {}", self.presses),
                    TextStyle::new(widgets::BUTTON_TEXT).color(Color::WHITE),
                )
            },
        );

        // An icon button: nothing to read inside, so it needs a label.
        ui.with_keyed(
            "save",
            widgets::button_spec()
                .on_click(Value::str("save"))
                .label("Save"),
            |ui| ui.text("⌘", TextStyle::new(15.0).color(Color::WHITE)),
        );

        // The button that opens the modal below.
        widgets::button(ui, "Delete…", Value::str("open-confirm"));

        // A switch: `checked` is the state assistive technology reads.
        ui.with_keyed(
            "mute",
            NodeSpec::row()
                .role(Role::Switch)
                .checked(self.muted)
                .on_click(Value::str("mute"))
                .pad_xy(10.0, 6.0)
                .bg(Color::rgb8(0x1d, 0x20, 0x2b))
                .radius(6.0)
                .label("Mute"),
            |ui| {
                ui.text(
                    if self.muted { "on" } else { "off" },
                    TextStyle::new(13.0).color(Color::rgb8(0x8a, 0x8f, 0xa3)),
                )
            },
        );

        // A slider the app draws: the value and range are data, and the
        // increment / decrement requests arrive as `access` events.
        ui.with_keyed(
            "volume",
            NodeSpec::row()
                .role(Role::Slider)
                .label("Volume")
                .value_now(self.volume)
                .value_min(0.0)
                .value_max(10.0)
                .on_drag(Value::str("volume"))
                .width(Sizing::Fixed(200.0))
                .height(Sizing::Fixed(16.0))
                .bg(Color::rgb8(0x1d, 0x20, 0x2b))
                .radius(8.0),
            |ui| {
                ui.with(
                    NodeSpec::row()
                        .width(Sizing::Percent(self.volume / 10.0))
                        .height(Sizing::Grow(1.0))
                        .bg(Color::rgb8(0x3b, 0x5b, 0xd4))
                        .radius(8.0),
                    |_| {},
                );
            },
        );

        // A built-in editor: the core owns the buffer, so its runs, caret
        // and selection come out of the edit store, and a screen reader's
        // selection and text requests are applied for you.
        ui.with(NodeSpec::row().role(Role::Heading), |ui| {
            ui.text("Built-in editor", TextStyle::new(15.0).color(Color::WHITE))
        });
        self.edit = ui.text_edit(
            "doc",
            DOC,
            &EditOptions {
                multiline: true,
                style: text,
                ..Default::default()
            },
            NodeSpec::column()
                .width(Sizing::Fixed(280.0))
                .height(Sizing::Fixed(56.0))
                .pad(8.0)
                .bg(Color::rgb8(0x0e, 0x10, 0x16))
                .radius(6.0)
                .clip()
                .label("Notes"),
        );

        // An editor the app owns: the sink says what it is, each drawn
        // row is a line of its text, and the caret rides along as a byte
        // offset. Text requests come back as `access` events.
        ui.with(NodeSpec::row().role(Role::Heading), |ui| {
            ui.text("App-owned editor", TextStyle::new(15.0).color(Color::WHITE))
        });
        let (lines, caret) = (&self.lines, self.caret);
        let sink = ui.with_keyed(
            "code",
            NodeSpec::column()
                .width(Sizing::Fixed(280.0))
                .pad(8.0)
                .bg(Color::rgb8(0x0e, 0x10, 0x16))
                .radius(6.0)
                .on_key(Value::str("code"))
                .role(Role::MultilineTextInput)
                .label("Source"),
            |ui| {
                ui.with(NodeSpec::row().gap(8.0), |ui| {
                    // Decoration: line numbers are not part of the text.
                    ui.with(NodeSpec::column().role(Role::None), |ui| {
                        for i in 0..lines.len() {
                            ui.text(
                                &format!("{}", i + 1),
                                TextStyle::new(12.0)
                                    .mono()
                                    .color(Color::rgb8(0x50, 0x55, 0x66)),
                            );
                        }
                    });
                    ui.with(NodeSpec::column(), |ui| {
                        for (i, line) in lines.iter().enumerate() {
                            let mut row = NodeSpec::row().role(Role::Line);
                            if caret.0 == i {
                                row = row.caret(caret.1.min(line.len()) as u32);
                            }
                            ui.with_keyed(&format!("l{i}"), row, |ui| {
                                ui.text(line, TextStyle::new(13.0).mono().color(text.color));
                            });
                        }
                    });
                });
            },
        );
        ui.take_key_focus(sink);

        widgets::latency_hud_at(ui, Align::End, Align::Start);

        // The modal, declared last so it floats over everything (and over
        // the latency HUD, which is a float too). One row makes it modal:
        // focus enters it, Tab cannot leave, nothing behind it takes
        // input, and a screen reader announces a dialog and stays inside.
        if self.dialog {
            ui.with_keyed(
                "confirm",
                NodeSpec::column()
                    .float(
                        FloatConfig::viewport()
                            .at(Align::Center, Align::Center)
                            .self_at(Align::Center, Align::Center),
                    )
                    .modal(Value::str("confirm"))
                    .label("Delete note")
                    .width(Sizing::Fixed(260.0))
                    .gap(10.0)
                    .pad(14.0)
                    .bg(Color::rgb8(0x1d, 0x20, 0x2b))
                    .border(1.0, Color::rgb8(0x3b, 0x5b, 0xd4))
                    .radius(8.0),
                |ui| {
                    ui.text(
                        "Delete this note?",
                        TextStyle::new(15.0).color(Color::WHITE),
                    );
                    ui.with(NodeSpec::row().gap(8.0), |ui| {
                        widgets::button(ui, "Cancel", Value::str("cancel"));
                        widgets::button(ui, "Delete", Value::str("delete"));
                    });
                },
            );
        }
    }

    fn on_event(&mut self, ev: UiEvent) {
        let payload = &ev.payload;
        match payload.as_str() {
            Some("press") => {
                self.presses += 1;
                println!("press -> {}", self.presses);
                return;
            }
            Some("save") => {
                println!("save");
                return;
            }
            // Opening the dialog is a field the view reads; closing it is
            // the same field. The core moves focus into it and hands
            // focus back to this button when it goes away.
            Some("open-confirm") => {
                self.dialog = true;
                return;
            }
            Some("cancel") => {
                self.dialog = false;
                println!("cancelled");
                return;
            }
            Some("delete") => {
                self.dialog = false;
                println!("deleted");
                return;
            }
            Some("advanced") => {
                self.advanced = !self.advanced;
                println!("advanced -> {}", self.advanced);
                return;
            }
            Some("mute") => {
                self.muted = !self.muted;
                println!("mute -> {}", self.muted);
                return;
            }
            _ => {}
        }
        // The rows carry their ordinal in a tagged string, so they do not
        // collide with the tabs' plain indices.
        if let Some(i) = payload
            .as_str()
            .and_then(|s| s.strip_prefix("row"))
            .and_then(|s| s.parse::<usize>().ok())
        {
            self.row = i;
            println!("row -> {}", self.row);
            return;
        }
        // The tabs carry their index as the payload.
        if let Some(i) = payload.as_int() {
            self.tab = i as usize;
            println!("tab -> {}", self.tab);
            return;
        }
        // Escape, or a click outside the dialog: the core asks, the app
        // decides. A dialog holding unsaved work could ask again here.
        if payload.get("kind").and_then(Value::as_str) == Some("dismiss") {
            let reason = payload.get("reason").and_then(Value::as_str).unwrap_or("");
            println!("dismiss ({reason})");
            self.dialog = false;
            return;
        }
        if payload.get("kind").and_then(Value::as_str) != Some("access") {
            return;
        }
        let action = payload.get("action").and_then(Value::as_str).unwrap_or("");
        match action {
            // The slider: the app decides what a step means.
            "increment" | "decrement" => {
                let step = if action == "increment" { 1.0 } else { -1.0 };
                self.volume = (self.volume + step).clamp(0.0, 10.0);
                println!("volume -> {}", self.volume);
            }
            // The app-owned editor: line ordinals among the rows it drew
            // (all of them here), byte offsets into their text.
            "setTextSelection" => {
                if let Some(f) = payload.get("focus") {
                    let line = f.get("line").and_then(Value::as_int).unwrap_or(0) as usize;
                    let offset = f.get("offset").and_then(Value::as_int).unwrap_or(0) as usize;
                    self.caret = (line, offset);
                    println!("caret -> {line}:{offset}");
                }
            }
            "replaceSelectedText" | "setValue" => {
                let text = payload.get("text").and_then(Value::as_str).unwrap_or("");
                if action == "setValue" {
                    self.lines = text.split('\n').map(String::from).collect();
                    self.caret = (0, 0);
                } else {
                    let line = self.caret_line();
                    let at = self.caret.1.min(self.lines[line].len());
                    self.lines[line].insert_str(at, text);
                    self.caret = (line, at + text.len());
                }
                println!("text -> {:?}", self.lines);
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui::app("kui — accessibility")
        // Every control in one column, so the window has to be tall
        // enough for all of them: a column that overflows squeezes its
        // children, and squeezed rows are exactly what a fixture must not
        // show when the point of it is that the rows read correctly.
        .size(560.0, 820.0)
        .run(A11y::new())
}
