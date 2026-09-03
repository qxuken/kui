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

use kui::widgets;
use kui::{
    Align, App, Color, EditOptions, Key, NodeSpec, Role, Sizing, TextStyle, Ui, UiEvent, Value,
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

        // A button named by its content, so a press is visible through
        // the accessibility API alone.
        widgets::button(ui, &format!("count {}", self.presses), Value::str("press"));

        // An icon button: nothing to read inside, so it needs a label.
        ui.with_keyed(
            "save",
            widgets::button_spec()
                .on_click(Value::str("save"))
                .label("Save"),
            |ui| ui.text("⌘", TextStyle::new(15.0).color(Color::WHITE)),
        );

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
            Some("mute") => {
                self.muted = !self.muted;
                println!("mute -> {}", self.muted);
                return;
            }
            _ => {}
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
        .size(520.0, 620.0)
        .run(A11y::new())
}
