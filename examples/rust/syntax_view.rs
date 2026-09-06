//! A read-only syntax-highlighted code view: the app's highlighter assigns
//! each char a color, and every visible line becomes a row of coalesced
//! style runs — one text node per token run, no text measurement anywhere;
//! the row *is* the layout. Tree-sitter goes where `highlight()` sits in the
//! real app; `emit_line` wouldn't notice.
//!
//! This is the frame shape the kui-core `highlight` bench measures.
//!
//! Run: cargo run -p kui --example syntax_view
//!
//! Keys: j/k or arrows move · pageup/pagedown · g/G ends · tab next buffer.

use kui::widgets;
use kui::{Align, App, Color, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};

const FONT: f32 = 13.5;
const LH: f32 = 20.0;
const GUTTER_W: f32 = 52.0;
const STATUS_H: f32 = 24.0;

// ---------------------------------------------------------------- palette

#[derive(Clone, Copy)]
struct Pal {
    bg: Color,
    panel: Color,
    line: Color,
    status: Color,
    fg: Color,
    dim: Color,
    faint: Color,
    accent: Color,
    kw: Color,
    string: Color,
    number: Color,
    comment: Color,
    ty: Color,
    mac: Color,
}

impl Default for Pal {
    fn default() -> Self {
        Self {
            bg: Color::rgb8(0x0f, 0x11, 0x17),
            panel: Color::rgb8(0x14, 0x16, 0x1e),
            line: Color::rgb8(0x1a, 0x1d, 0x29),
            status: Color::rgb8(0x1a, 0x1d, 0x27),
            fg: Color::rgb8(0xd6, 0xd8, 0xe0),
            dim: Color::rgb8(0x8a, 0x8f, 0xa3),
            faint: Color::rgb8(0x50, 0x55, 0x66),
            accent: Color::rgb8(0x6a, 0x8b, 0xff),
            kw: Color::rgb8(0xc7, 0x8f, 0xe8),
            string: Color::rgb8(0x9c, 0xc8, 0x7a),
            number: Color::rgb8(0xd9, 0xa1, 0x4d),
            comment: Color::rgb8(0x5c, 0x66, 0x79),
            ty: Color::rgb8(0x6f, 0xc3, 0xd6),
            mac: Color::rgb8(0xe0, 0x9a, 0x6a),
        }
    }
}

// ---------------------------------------------------------------- model

#[derive(Clone, Copy, PartialEq)]
enum Lang {
    Rust,
    Text,
}

struct Doc {
    name: String,
    lines: Vec<String>,
    lang: Lang,
}

impl Doc {
    fn new(name: &str, text: &str, lang: Lang) -> Self {
        let lines = text.split('\n').map(|l| l.replace('\t', "    ")).collect();
        Self {
            name: name.into(),
            lines,
            lang,
        }
    }
}

// ---------------------------------------------------------------- app

struct SyntaxView {
    pal: Pal,
    docs: Vec<Doc>,
    cur: usize,
    line: usize,
    top: usize,
    rows: usize,
}

impl SyntaxView {
    fn new() -> Self {
        Self {
            pal: Pal::default(),
            docs: vec![
                Doc::new("main.rs", SAMPLE_RS, Lang::Rust),
                Doc::new("NOTES", NOTES, Lang::Text),
            ],
            cur: 0,
            line: 0,
            top: 0,
            rows: 24,
        }
    }

    fn move_line(&mut self, dy: i64) {
        let total = self.docs[self.cur].lines.len() as i64;
        self.line = (self.line as i64 + dy).clamp(0, total - 1) as usize;
    }

    fn on_key(&mut self, code: &str) {
        match code {
            "j" | "down" => self.move_line(1),
            "k" | "up" => self.move_line(-1),
            "pagedown" => self.move_line(self.rows as i64 - 1),
            "pageup" => self.move_line(-(self.rows as i64 - 1)),
            "g" | "home" => self.line = 0,
            "G" | "end" => self.line = self.docs[self.cur].lines.len() - 1,
            "tab" => {
                self.cur = (self.cur + 1) % self.docs.len();
                self.line = 0;
                self.top = 0;
            }
            _ => {}
        }
    }
}

impl App for SyntaxView {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let pal = self.pal;
        ui.configure_root(NodeSpec::column().fill().bg(pal.bg));
        widgets::titlebar(
            ui,
            "kui — syntax view (highlighting is coalesced style runs)",
        );

        let vp = ui.viewport();
        let h = (vp.h - widgets::TITLEBAR_H).max(LH);
        self.rows = (((h - STATUS_H - 8.0) / LH).max(1.0)) as usize;
        // Scroll the cursor line into view.
        if self.line < self.top {
            self.top = self.line;
        }
        if self.line >= self.top + self.rows {
            self.top = self.line + 1 - self.rows;
        }
        let doc = &self.docs[self.cur];
        let last = (self.top + self.rows).min(doc.lines.len());
        let (top, cur_line) = (self.top, self.line);

        let sink = ui.with_keyed(
            "view",
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .bg(pal.panel)
                .clip()
                .on_key(Value::Null),
            |ui| {
                ui.with(
                    NodeSpec::row()
                        .width(Sizing::Grow(1.0))
                        .height(Sizing::Grow(1.0))
                        .pad_xy(0.0, 4.0),
                    |ui| {
                        // Gutter.
                        ui.with(
                            NodeSpec::column()
                                .width(Sizing::Fixed(GUTTER_W))
                                .height(Sizing::Grow(1.0))
                                .pad_xy(12.0, 0.0),
                            |ui| {
                                for ln in top..last {
                                    let color = if ln == cur_line { pal.dim } else { pal.faint };
                                    ui.with(
                                        NodeSpec::row()
                                            .width(Sizing::Grow(1.0))
                                            .height(Sizing::Fixed(LH))
                                            .main_align(Align::End)
                                            .cross_align(Align::Center),
                                        |ui| {
                                            ui.text(
                                                &format!("{}", ln + 1),
                                                TextStyle::new(11.0).mono().color(color),
                                            );
                                        },
                                    );
                                }
                            },
                        );
                        // Text.
                        ui.with(
                            NodeSpec::column()
                                .width(Sizing::Grow(1.0))
                                .height(Sizing::Grow(1.0))
                                .clip(),
                            |ui| {
                                for ln in top..last {
                                    emit_line(ui, &pal, &doc.lines[ln], doc.lang, ln == cur_line);
                                }
                            },
                        );
                    },
                );
                status_line(ui, &pal, doc, cur_line);
            },
        );
        ui.take_key_focus(sink);

        widgets::latency_hud(ui);
    }

    fn on_event(&mut self, ev: UiEvent) {
        // Presses only, because the sink never asked for releases (no
        // `key_up`): this pane scrolls on a chord, and nothing arrives on
        // the way up to be filtered out.
        if ev.payload.get("kind").and_then(Value::as_str) == Some("key")
            && let Some(code) = ev.payload.get("code").and_then(Value::as_str)
        {
            let code = code.to_string();
            self.on_key(&code);
        }
    }
}

// ---------------------------------------------------------------- rendering

fn mono(pal: &Pal) -> TextStyle {
    TextStyle::new(FONT).mono().line_height(LH).color(pal.fg)
}

/// Trailing spaces in a measured text run are unreliable across fonts; NBSP
/// has the same advance in monospace and always measures.
fn nbsp(s: &str) -> String {
    s.replace(' ', "\u{a0}")
}

/// One line as a row of coalesced color runs: adjacent chars sharing a color
/// become one text node. The cache in the core is keyed by (content, style,
/// scale) — color excluded — so token runs dedupe across lines and colors.
fn emit_line(ui: &mut Ui<'_>, pal: &Pal, text: &str, lang: Lang, current: bool) {
    let chars: Vec<char> = text.chars().collect();
    let colors = highlight(pal, &chars, lang);
    let mut row = NodeSpec::row()
        .width(Sizing::Grow(1.0))
        .height(Sizing::Fixed(LH))
        .cross_align(Align::Center);
    if current {
        row = row.bg(pal.line);
    }
    ui.with(row, |ui| {
        let mut i = 0;
        while i < chars.len() {
            let start = i;
            let color = colors[i];
            i += 1;
            while i < chars.len() && colors[i] == color {
                i += 1;
            }
            let run: String = chars[start..i].iter().collect();
            ui.text(&nbsp(&run), mono(pal).color(color));
        }
    });
}

fn status_line(ui: &mut Ui<'_>, pal: &Pal, doc: &Doc, line: usize) {
    ui.with(
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(STATUS_H))
            .bg(pal.status)
            .pad_xy(8.0, 0.0)
            .gap(8.0)
            .cross_align(Align::Center),
        |ui| {
            ui.with(
                NodeSpec::row().pad_xy(8.0, 2.0).radius(4.0).bg(pal.accent),
                |ui| {
                    let label = match doc.lang {
                        Lang::Rust => "RUST",
                        Lang::Text => "TEXT",
                    };
                    ui.text(label, TextStyle::new(10.0).mono().color(pal.bg));
                },
            );
            ui.text(&doc.name, TextStyle::new(12.0).color(pal.fg));
            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
            ui.text("tab switches buffer", TextStyle::new(11.0).color(pal.faint));
            let total = doc.lines.len();
            let pct = if total <= 1 {
                100
            } else {
                (line * 100) / (total - 1)
            };
            ui.text(
                &format!("{}/{total}  {pct}%", line + 1),
                TextStyle::new(11.0).mono().color(pal.dim),
            );
        },
    );
}

// ---------------------------------------------------------------- highlighter (a stand-in)

const RUST_KW: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Per-char colors for one line. Tree-sitter goes here in the real app; the
/// view code (`emit_line`) doesn't care who colored the chars.
fn highlight(pal: &Pal, chars: &[char], lang: Lang) -> Vec<Color> {
    let mut out = vec![pal.fg; chars.len()];
    match lang {
        Lang::Text => {
            if chars.first() == Some(&'#') {
                out.fill(pal.accent);
            } else if chars.first() == Some(&'—') {
                out.fill(pal.faint);
            }
        }
        Lang::Rust => {
            let mut i = 0;
            while i < chars.len() {
                let c = chars[i];
                if c == '/' && chars.get(i + 1) == Some(&'/') {
                    out[i..].fill(pal.comment);
                    break;
                }
                if c == '"' {
                    let start = i;
                    i += 1;
                    while i < chars.len() && chars[i] != '"' {
                        i += if chars[i] == '\\' { 2 } else { 1 };
                    }
                    i = (i + 1).min(chars.len());
                    out[start..i].fill(pal.string);
                    continue;
                }
                if c.is_ascii_digit() && (i == 0 || !is_word(chars[i - 1])) {
                    let start = i;
                    while i < chars.len()
                        && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
                    {
                        i += 1;
                    }
                    out[start..i].fill(pal.number);
                    continue;
                }
                if is_word(c) && (i == 0 || !is_word(chars[i - 1])) {
                    let start = i;
                    while i < chars.len() && is_word(chars[i]) {
                        i += 1;
                    }
                    let word: String = chars[start..i].iter().collect();
                    let color = if chars.get(i) == Some(&'!') {
                        Some(pal.mac)
                    } else if RUST_KW.contains(&word.as_str()) {
                        Some(pal.kw)
                    } else if word.chars().next().is_some_and(char::is_uppercase) {
                        Some(pal.ty)
                    } else {
                        None
                    };
                    if let Some(color) = color {
                        out[start..i].fill(color);
                    }
                    continue;
                }
                i += 1;
            }
        }
    }
    out
}

// ---------------------------------------------------------------- content

const NOTES: &str = "\
# syntax_view

Per-char colors come from the app's highlighter;
kui just draws coalesced runs of styled text.

Every visible line is one row node; every token
run is one text node. No measurement, no spans
API — the row is the layout.

— swap highlight() for tree-sitter and emit_line
  never knows the difference.
— the kui-core `highlight` bench builds frames of
  exactly this shape to keep the cost honest.";

const SAMPLE_RS: &str = "\
//! Minimal Elm-ish counter: view() rebuilds the tree
//! from state, clicks arrive as data in on_event.

use kui::widgets;
use kui::{App, NodeSpec, TextStyle, Ui, UiEvent, Value};

#[derive(Default)]
struct Counter {
    count: i64,
}

impl App for Counter {
    fn view(&mut self, ui: &mut Ui<'_>) {
        ui.configure_root(NodeSpec::column().fill().center().gap(24.0));
        ui.with(
            NodeSpec::column().pad(32.0).gap(20.0).radius(12.0),
            |ui| {
                ui.text(\"kui counter\", TextStyle::new(14.0));
                ui.text(&self.count.to_string(), TextStyle::new(56.0));
                widgets::button(ui, \"+1\", Value::map([(\"kind\", \"inc\".into())]));
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get(\"kind\").and_then(Value::as_str) {
            Some(\"inc\") => self.count += 1,
            Some(\"dec\") => self.count -= 1,
            _ => {}
        }
    }
}

fn main() {
    kui::run(\"kui — counter\", Counter::default(), vec![]).unwrap();
}";

fn main() {
    kui::app("kui — syntax view")
        .custom_titlebar()
        .size(900.0, 700.0)
        .run(SyntaxView::new())
        .unwrap();
}
