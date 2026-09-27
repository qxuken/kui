//! A read-only syntax-highlighted code view: the app's highlighter assigns
//! each char a color, and every visible line becomes a row of coalesced
//! style runs — one text node per token run, no text measurement anywhere;
//! the row *is* the layout. Tree-sitter goes where `highlight()` sits in the
//! real app; `emit_line` wouldn't notice.
//!
//! This is the frame shape the kui-core `highlight` bench measures.
//!
//! One line carries a diagnostic: the runs under it are underlined by a
//! red wave in their own colour (`underline_color` + `underline_style`,
//! backlog K4), the way an editor marks an unused field — no `line` float
//! under the run, no rect arithmetic.
//!
//! Run: cargo run -p kui-native --example syntax_view
//!
//! Keys: j/k or arrows move · pageup/pagedown · g/G ends · tab next buffer.

use kui_devtools::Example;
use kui_native::widgets;
use kui_native::{
    Align, App, Color, Core, NodeSpec, TextStyle, Theme, Ui, UiEvent, UnderlineStyle,
};

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
    /// The diagnostic wave: the theme's danger role.
    error: Color,
}

impl From<Theme> for Pal {
    /// The chrome comes off the theme; the six syntax hues are the app's
    /// own, which is the honest split — a keyword's purple is authored,
    /// the way ADR 0017 says a span's colour is, and no UI role names it.
    /// They still come in two sets, because a hue picked to read on
    /// `#0f1117` does not read on white: same families, darkened for the
    /// light base, each checked past 4.5:1 on the surface it lands on.
    fn from(t: Theme) -> Self {
        let dark = t.is_dark();
        let hue = |d: u32, l: u32| Color::hex(if dark { d } else { l });
        Self {
            bg: t.bg,
            panel: t.surface,
            line: t.sunken,
            status: t.sunken,
            fg: t.fg,
            dim: t.muted,
            faint: t.faint,
            accent: t.focus_ring,
            kw: hue(0xc78fe8ff, 0x7c3aabff),
            string: hue(0x9cc87aff, 0x35701cff),
            number: hue(0xd9a14dff, 0x8a5c08ff),
            // A comment is meant to recede, so it is the theme's own
            // "barely there" tier rather than a seventh hue.
            comment: t.faint,
            ty: hue(0x6fc3d6ff, 0x17697dff),
            mac: hue(0xe09a6aff, 0xa1541cff),
            error: t.danger,
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

    /// The columns a diagnostic marks on `line`, if any: the Rust sample
    /// has one, on the field its counter never reads — the columns of
    /// `count` on the line that declares it.
    fn diagnostic_on(&self, line: usize) -> Option<std::ops::Range<usize>> {
        if self.lang != Lang::Rust {
            return None;
        }
        let text = self.lines.get(line)?;
        let at = text.find("count: i64")?;
        Some(at..at + "count".len())
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
            pal: Theme::default().into(),
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
        // Rebuilt from the theme each frame, so the window follows the OS.
        self.pal = ui.theme().into();
        let pal = self.pal;
        ui.with(NodeSpec::column().fill().bg(pal.bg), |ui| {
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
                NodeSpec::column().fill().bg(pal.panel).clip().key_sink(),
                |ui| {
                    ui.with(NodeSpec::row().fill().pad_xy(0.0, 4.0), |ui| {
                        // Gutter.
                        ui.with(
                            NodeSpec::column()
                                .width(GUTTER_W)
                                .grow_height()
                                .pad_xy(12.0, 0.0),
                            |ui| {
                                for ln in top..last {
                                    let color = if ln == cur_line { pal.dim } else { pal.faint };
                                    ui.text_in(
                                        NodeSpec::row()
                                            .grow_width()
                                            .height(LH)
                                            .main_align(Align::End)
                                            .cross_align(Align::Center),
                                        &format!("{}", ln + 1),
                                        TextStyle::new(11.0).mono().color(color),
                                    );
                                }
                            },
                        );
                        // Text.
                        ui.with(NodeSpec::column().fill().clip(), |ui| {
                            for ln in top..last {
                                emit_line(
                                    ui,
                                    &pal,
                                    &doc.lines[ln],
                                    doc.lang,
                                    ln == cur_line,
                                    doc.diagnostic_on(ln),
                                );
                            }
                        });
                    });
                    status_line(ui, &pal, doc, cur_line);
                },
            );
            ui.take_key_focus(sink);
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        // Presses only, because the sink never asked for releases (no
        // `key_up`): this pane scrolls on a chord, and nothing arrives on
        // the way up to be filtered out.
        if ev.kind() == Some("key")
            && let Some(code) = ev.payload.get_str("code")
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

/// One line as a row of coalesced color runs: adjacent chars sharing a color
/// become one text node. The cache in the core is keyed by (content, style,
/// scale) — color excluded — so token runs dedupe across lines and colors.
fn emit_line(
    ui: &mut Ui<'_>,
    pal: &Pal,
    text: &str,
    lang: Lang,
    current: bool,
    diagnostic: Option<std::ops::Range<usize>>,
) {
    let chars: Vec<char> = text.chars().collect();
    let colors = highlight(pal, &chars, lang);
    // A run breaks where the diagnostic starts and ends, so the wave
    // covers the marked columns and nothing beside them.
    let marked = |i: usize| diagnostic.as_ref().is_some_and(|d| d.contains(&i));
    let mut row = NodeSpec::row()
        .grow_width()
        .height(LH)
        .cross_align(Align::Center);
    if current {
        row = row.bg(pal.line);
    }
    ui.with(row, |ui| {
        let mut i = 0;
        while i < chars.len() {
            let start = i;
            let color = colors[i];
            let mark = marked(i);
            i += 1;
            while i < chars.len() && colors[i] == color && marked(i) == mark {
                i += 1;
            }
            let run: String = chars[start..i].iter().collect();
            let mut style = mono(pal).color(color);
            if mark {
                style = style
                    .underline_color(pal.error)
                    .underline_style(UnderlineStyle::Wavy);
            }
            ui.text(&run, style);
        }
    });
}

fn status_line(ui: &mut Ui<'_>, pal: &Pal, doc: &Doc, line: usize) {
    ui.with(
        NodeSpec::row()
            .grow_width()
            .height(STATUS_H)
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
            ui.leaf(NodeSpec::row().grow_width());
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

use kui_native::widgets;
use kui_native::{App, NodeSpec, TextStyle, Ui, UiEvent, Value};

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
    kui_native::run(\"kui — counter\", Counter::default(), vec![]).unwrap();
}";

impl Example for SyntaxView {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        ("j / k, arrows", "move"),
        ("PgUp / PgDn", "page"),
        ("g / G", "ends"),
        ("Tab", "next buffer"),
    ];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default()
            .size(900.0, 700.0)
            .custom_titlebar()
    }

    fn dock(&self) -> kui_devtools::Dock {
        kui_devtools::Dock::Bottom
    }

    /// The keys, driven (backlog C36): `j`, `G` and `tab` move the line,
    /// the view and the buffer — and the view keeps the line on screen.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        use kui_devtools::Drive;
        use kui_native::KeyMods;
        let mut d = Drive::new(core, 900.0, 700.0);
        d.frame(self);
        d.check(
            self.cur == 0 && self.line == 0 && self.top == 0,
            "the first buffer, at its top",
        )?;
        d.key(self, "j", KeyMods::default());
        d.frame(self);
        d.check(self.line == 1 && self.top == 0, "j moves down a line")?;
        d.key(self, "G", KeyMods::default());
        d.frame(self);
        let last = self.docs[0].lines.len() - 1;
        d.check(self.line == last, "G goes to the last line")?;
        d.check(
            self.top > 0 && self.top + self.rows > last,
            "and the view scrolled so the line is on screen",
        )?;
        d.key(self, "k", KeyMods::default());
        d.frame(self);
        d.check(self.line == last - 1, "k moves up")?;
        // Tab is the sink's, not the ring's: a sink that holds focus keeps
        // every key (ADR 0002, decision 3).
        d.key(self, "tab", KeyMods::default());
        d.frame(self);
        d.check(
            self.cur == 1 && self.line == 0 && self.top == 0,
            "tab switches to the next buffer, at its top",
        )?;
        d.key(self, "tab", KeyMods::default());
        d.frame(self);
        d.check(self.cur == 0, "and wraps around")
    }
}

kui_devtools::main!(SyntaxView::new());
