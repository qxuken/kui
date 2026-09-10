//! A helix-flavored modal editor with the app — not kui — owning the
//! document, the keymap, and the modes. The whole keyboard arrives through
//! one `on_key` sink as `{kind="key"}` events, so the same modal dispatch
//! would work verbatim from Lua or C. kui's part is turning the state into
//! rows of text runs: selection as background segments, the caret as an
//! inline node, no text measurement anywhere.
//!
//! The document model is a deliberately dumb Vec<String> — the point is the
//! boundary, not the rope. An IO/LSP/undo layer replaces the model; the view
//! code stays.
//!
//! Run: cargo run -p kui --example modal_editor
//!
//! Keys: see the buffer text (`:help` puts a summary in the minibuffer).

use kui::widgets;
use kui::{
    Align, App, Color, NodeSpec, Role, Sizing, TextStyle, Theme, Ui, UiEvent, Value, WindowCommand,
};
use kui_devtools::Example;

const FONT: f32 = 13.5;
const LH: f32 = 20.0;
const GUTTER_W: f32 = 52.0;
const STATUS_H: f32 = 24.0;
const MINIBUF_H: f32 = 26.0;

// ---------------------------------------------------------------- palette

#[derive(Clone, Copy)]
struct Pal {
    bg: Color,
    bg2: Color,
    panel: Color,
    status: Color,
    fg: Color,
    dim: Color,
    faint: Color,
    accent: Color,
    select: Color,
    insert: Color,
    command: Color,
}

impl From<Theme> for Pal {
    /// Every field is a theme role, including the three that look like
    /// app colours: a mode indicator is a status, and the theme already
    /// names the three statuses an editor has anything to say with.
    fn from(t: Theme) -> Self {
        Self {
            bg: t.bg,
            bg2: t.sunken,
            panel: t.surface,
            status: t.sunken,
            fg: t.fg,
            dim: t.muted,
            faint: t.faint,
            accent: t.focus_ring,
            select: t.selection,
            insert: t.success,
            command: t.warning,
        }
    }
}

// ---------------------------------------------------------------- model

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Normal,
    Insert,
    Command,
}

struct Doc {
    name: String,
    lines: Vec<String>,
    modified: bool,
}

impl Doc {
    fn new(name: &str, text: &str) -> Self {
        let lines = text.split('\n').map(|l| l.replace('\t', "    ")).collect();
        Self {
            name: name.into(),
            lines,
            modified: false,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Pos {
    line: usize,
    col: usize,
}

/// The view of the document: cursor, selection anchor, scroll. `rows` is
/// written during render (the view fn knows the editor's height) and read by
/// the keymap for paging; scroll-into-view happens in render too, so the
/// caret can never leave the screen.
struct View {
    cur: Pos,
    anchor: Option<Pos>,
    top: usize,
    rows: usize,
}

impl Default for View {
    fn default() -> Self {
        Self {
            cur: Pos::default(),
            anchor: None,
            top: 0,
            rows: 24,
        }
    }
}

// ---------------------------------------------------------------- app

struct ModalEditor {
    pal: Pal,
    doc: Doc,
    view: View,
    mode: Mode,
    cmd: String,
    message: String,
    pending: Option<char>,
    yank: Vec<String>,
    quit: bool,
}

/// The key event, back out of `Value` form.
struct KeyEv {
    code: String,
    text: Option<String>,
}

impl KeyEv {
    /// A press, from a `{kind="key"}` payload. Modal editing is a keymap,
    /// not a held-key interaction: the releases the same sink delivers
    /// (`phase="up"`) are not commands, so they stop here.
    /// Every key event is a press: the sink never asked for releases
    /// (`key_up`), so a keymap needs no phase check.
    fn from_payload(p: &Value) -> Option<Self> {
        Some(Self {
            code: p.get("code")?.as_str()?.to_string(),
            text: p.get("text").and_then(Value::as_str).map(str::to_string),
        })
    }
}

impl ModalEditor {
    fn new() -> Self {
        Self {
            pal: Theme::default().into(),
            doc: Doc::new("*scratch*", SAMPLE),
            view: View::default(),
            mode: Mode::Normal,
            cmd: String::new(),
            message: "modal editing demo — the buffer text is the keymap".into(),
            pending: None,
            yank: Vec::new(),
            quit: false,
        }
    }

    // ------------------------------------------------------------ keymap

    fn on_key(&mut self, k: KeyEv) {
        match self.mode {
            Mode::Command => self.key_command(k),
            Mode::Insert => self.key_insert(k),
            Mode::Normal => self.key_normal(k),
        }
    }

    fn key_command(&mut self, k: KeyEv) {
        match k.code.as_str() {
            "escape" => {
                self.mode = Mode::Normal;
                self.cmd.clear();
            }
            "enter" => {
                self.mode = Mode::Normal;
                let cmd = std::mem::take(&mut self.cmd);
                self.exec_command(&cmd);
            }
            "backspace" => {
                if self.cmd.pop().is_none() {
                    self.mode = Mode::Normal;
                }
            }
            _ => {
                if let Some(t) = &k.text {
                    self.cmd.push_str(t);
                }
            }
        }
    }

    fn key_normal(&mut self, k: KeyEv) {
        let pending = self.pending.take();
        let (doc, view) = (&mut self.doc, &mut self.view);

        if let Some(p) = pending {
            match (p, k.code.as_str()) {
                ('g', "g") => {
                    view.cur = Pos::default();
                    view.anchor = None;
                }
                ('g', "e") => {
                    view.cur.line = doc.lines.len().saturating_sub(1);
                    clamp_col(doc, view);
                }
                ('d', "d") => {
                    self.yank = vec![doc.lines[view.cur.line].clone()];
                    delete_line(doc, view);
                    self.message = "deleted line (p puts it back)".into();
                }
                _ => self.message = format!("{p}{} is not a thing here", k.code),
            }
            return;
        }

        match k.code.as_str() {
            "h" | "left" => move_h(doc, view, -1),
            "l" | "right" => move_h(doc, view, 1),
            "j" | "down" => move_v(doc, view, 1),
            "k" | "up" => move_v(doc, view, -1),
            "w" => word_fwd(doc, view),
            "b" => word_back(doc, view),
            "0" | "home" => view.cur.col = 0,
            "$" | "end" => view.cur.col = line_len(doc, view.cur.line),
            "pageup" => move_v(doc, view, -(view.rows as i64 - 1)),
            "pagedown" => move_v(doc, view, view.rows as i64 - 1),
            "G" => {
                view.cur.line = doc.lines.len().saturating_sub(1);
                clamp_col(doc, view);
            }
            "g" => self.pending = Some('g'),
            "d" => {
                if view.anchor.is_some() {
                    self.yank = sel_lines(doc, view);
                    delete_sel(doc, view);
                } else {
                    self.pending = Some('d');
                }
            }
            "i" => self.mode = Mode::Insert,
            "a" => {
                view.cur.col = (view.cur.col + 1).min(line_len(doc, view.cur.line));
                self.mode = Mode::Insert;
            }
            "I" => {
                view.cur.col = 0;
                self.mode = Mode::Insert;
            }
            "A" => {
                view.cur.col = line_len(doc, view.cur.line);
                self.mode = Mode::Insert;
            }
            "o" => {
                open_line(doc, view, 1);
                self.mode = Mode::Insert;
            }
            "O" => {
                open_line(doc, view, 0);
                self.mode = Mode::Insert;
            }
            "v" => {
                view.anchor = if view.anchor.is_some() {
                    None
                } else {
                    Some(view.cur)
                }
            }
            "x" => {
                if !delete_sel(doc, view) {
                    delete_char(doc, view);
                }
            }
            "y" => {
                self.yank = sel_lines(doc, view);
                view.anchor = None;
                self.message = format!("yanked {} line(s)", self.yank.len());
            }
            "p" => paste(doc, view, &self.yank),
            "u" => self.message = "undo is where the demo ends and your app begins".into(),
            ":" => {
                self.mode = Mode::Command;
                self.cmd.clear();
            }
            "escape" => view.anchor = None,
            _ => {}
        }
    }

    fn key_insert(&mut self, k: KeyEv) {
        let (doc, view) = (&mut self.doc, &mut self.view);
        match k.code.as_str() {
            "escape" => {
                self.mode = Mode::Normal;
                clamp_col(doc, view);
            }
            "enter" => insert_text(doc, view, "\n"),
            "backspace" => backspace(doc, view),
            "delete" => delete_char(doc, view),
            "tab" => insert_text(doc, view, "    "),
            "left" => move_h(doc, view, -1),
            "right" => move_h(doc, view, 1),
            "up" => move_v(doc, view, -1),
            "down" => move_v(doc, view, 1),
            "home" => view.cur.col = 0,
            "end" => view.cur.col = line_len(doc, view.cur.line),
            _ => {
                if let Some(t) = k.text.clone() {
                    insert_text(doc, view, &t);
                }
            }
        }
    }

    fn exec_command(&mut self, cmd: &str) {
        match cmd.trim() {
            "" => {}
            "q" | "q!" | "qa" | "qa!" => self.quit = true,
            "w" | "write" => self.write_doc(),
            "wq" => {
                self.write_doc();
                self.quit = true;
            }
            "help" => {
                self.message =
                    "h j k l · w b · gg G · i a o · v x d dd y p · :w :q — see the buffer".into()
            }
            other => self.message = format!("not a command: {other} (:help for the keymap)"),
        }
    }

    fn write_doc(&mut self) {
        self.doc.modified = false;
        self.message = format!(
            "wrote {} — well, pretended to; IO belongs to your app",
            self.doc.name
        );
    }

    // ------------------------------------------------------------ view

    fn minibuffer(&self, ui: &mut Ui<'_>) {
        let pal = self.pal;
        ui.with(
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Fixed(MINIBUF_H))
                .bg(pal.bg2)
                .pad_xy(10.0, 0.0)
                .gap(2.0)
                .cross_align(Align::Center),
            |ui| {
                if self.mode == Mode::Command {
                    ui.text(":", TextStyle::new(FONT).mono().color(pal.command));
                    ui.text(&nbsp(&self.cmd), TextStyle::new(FONT).mono().color(pal.fg));
                    caret_bar(ui, pal.command);
                } else {
                    ui.text(&self.message, TextStyle::new(12.0).color(pal.dim));
                }
                ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                let hint = match self.mode {
                    Mode::Insert => "esc → normal",
                    Mode::Command => "enter run · esc cancel",
                    Mode::Normal => "i insert · v select · : command · :help",
                };
                ui.text(hint, TextStyle::new(11.0).color(pal.faint));
            },
        );
    }
}

impl App for ModalEditor {
    fn view(&mut self, ui: &mut Ui<'_>) {
        if self.quit {
            ui.window_command(WindowCommand::Close(ui.env().window.id));
        }
        // Rebuilt from the theme each frame, so the window follows the OS.
        self.pal = ui.theme().into();
        let pal = self.pal;
        ui.with(NodeSpec::column().fill().bg(pal.bg), |ui| {
            widgets::titlebar(ui, "kui — modal editor (the app owns the keymap)");

            let vp = ui.viewport();
            let editor_h = (vp.h - widgets::TITLEBAR_H - MINIBUF_H).max(LH);
            let mode = self.mode;
            let (doc, view) = (&self.doc, &mut self.view);
            let sink = ui.with_keyed(
                "editor",
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0))
                    .bg(pal.panel)
                    .clip()
                    .on_key(Value::Null)
                    // The app owns the text, so it says what the sink is; the
                    // lines it draws (`Role::Line` rows) are the editor's text
                    // to a screen reader, and selection requests come back as
                    // `{kind="access"}` events (see `on_access`).
                    .role(Role::MultilineTextInput)
                    .label(doc.name.as_str()),
                |ui| render_editor(ui, &pal, doc, view, mode, editor_h),
            );
            ui.take_key_focus(sink);

            self.minibuffer(ui);
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("key") => {
                if let Some(k) = KeyEv::from_payload(&ev.payload) {
                    self.on_key(k);
                }
            }
            Some("access") => self.on_access(&ev.payload),
            _ => {}
        }
    }
}

impl ModalEditor {
    /// A screen reader's text requests, in the app's own terms: lines are
    /// ordinals among the drawn lines (so `view.top` maps them back into
    /// the document), offsets are bytes.
    fn on_access(&mut self, p: &Value) {
        let text = p.get("text").and_then(Value::as_str).unwrap_or("");
        match p.get("action").and_then(Value::as_str) {
            Some("setTextSelection") => {
                let (Some(anchor), Some(focus)) = (
                    p.get("anchor").and_then(|v| self.access_pos(v)),
                    p.get("focus").and_then(|v| self.access_pos(v)),
                ) else {
                    return;
                };
                self.view.cur = focus;
                self.view.anchor = (anchor != focus).then_some(anchor);
            }
            Some("replaceSelectedText") => {
                delete_sel(&mut self.doc, &mut self.view);
                insert_text(&mut self.doc, &mut self.view, text);
            }
            Some("setValue") => {
                self.doc.lines = text.split('\n').map(String::from).collect();
                self.doc.modified = true;
                self.view.cur = Pos::default();
                self.view.anchor = None;
            }
            _ => {}
        }
    }

    fn access_pos(&self, v: &Value) -> Option<Pos> {
        let line = v.get("line")?.as_int()? as usize + self.view.top;
        let line = line.min(self.doc.lines.len().saturating_sub(1));
        let offset = v.get("offset")?.as_int()? as usize;
        Some(Pos {
            line,
            col: col_at(&self.doc.lines[line], offset),
        })
    }
}

/// Byte offset of character column `col` in `text` (its length past the end).
fn byte_at(text: &str, col: usize) -> u32 {
    text.char_indices().nth(col).map_or(text.len(), |(b, _)| b) as u32
}

/// Character column of byte offset `offset` in `text`.
fn col_at(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].chars().count()
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

fn caret_bar(ui: &mut Ui<'_>, color: Color) {
    ui.with(
        NodeSpec::column()
            .width(Sizing::Fixed(2.0))
            .height(Sizing::Fixed(LH - 4.0))
            .bg(color),
        |_| {},
    );
}

#[derive(Clone, Copy, PartialEq)]
enum Caret {
    Bar,
    Block,
}

fn render_editor(ui: &mut Ui<'_>, pal: &Pal, doc: &Doc, view: &mut View, mode: Mode, h: f32) {
    let rows = (((h - STATUS_H - 8.0) / LH).max(1.0)) as usize;
    view.rows = rows;
    // Scroll the caret into view — the app's job, and two lines of it.
    if view.cur.line < view.top {
        view.top = view.cur.line;
    }
    if view.cur.line >= view.top + rows {
        view.top = view.cur.line + 1 - rows;
    }
    let last = (view.top + rows).min(doc.lines.len());

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
                    .pad_xy(12.0, 0.0)
                    // Decoration: not part of the editor's text.
                    .role(Role::None),
                |ui| {
                    for ln in view.top..last {
                        let color = if ln == view.cur.line {
                            pal.dim
                        } else {
                            pal.faint
                        };
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
                    for ln in view.top..last {
                        let caret = (ln == view.cur.line).then(|| {
                            (
                                view.cur.col,
                                if mode == Mode::Insert {
                                    Caret::Bar
                                } else {
                                    Caret::Block
                                },
                            )
                        });
                        // Where the caret and the selection's other end
                        // sit on this line, as byte offsets, for the
                        // access tree.
                        let access = (
                            (ln == view.cur.line).then(|| byte_at(&doc.lines[ln], view.cur.col)),
                            view.anchor
                                .filter(|a| a.line == ln)
                                .map(|a| byte_at(&doc.lines[ln], a.col)),
                        );
                        emit_line(
                            ui,
                            pal,
                            &doc.lines[ln],
                            sel_on_line(view, doc, ln),
                            caret,
                            access,
                        );
                    }
                },
            );
        },
    );

    status_line(ui, pal, mode, doc, view);
}

/// One line as a row of runs: selection as background segments, the caret as
/// an inline node. No text measurement needed anywhere — the row *is* the
/// layout. (The syntax_view example adds per-char colors to this same shape.)
fn emit_line(
    ui: &mut Ui<'_>,
    pal: &Pal,
    text: &str,
    sel: Option<(usize, usize)>,
    caret: Option<(usize, Caret)>,
    access: (Option<u32>, Option<u32>),
) {
    let chars: Vec<char> = text.chars().collect();
    let (caret_col, caret_kind) = match caret {
        Some((c, k)) => (Some(c), Some(k)),
        None => (None, None),
    };
    let at_sel = |i: usize| sel.is_some_and(|(a, b)| i >= a && i < b);

    // One line of the editor's text to assistive technology: the text
    // nodes inside this row, whatever they are split into for drawing.
    let mut row = NodeSpec::row()
        .width(Sizing::Grow(1.0))
        .height(Sizing::Fixed(LH))
        .cross_align(Align::Center)
        .role(Role::Line);
    if let Some(c) = access.0 {
        row = row.caret(c);
    }
    if let Some(a) = access.1 {
        row = row.selection_anchor(a);
    }
    ui.with(row, |ui| {
        let mut i = 0;
        let mut bar_done = false;
        while i < chars.len() {
            if !bar_done && caret_col == Some(i) && caret_kind == Some(Caret::Bar) {
                caret_bar(ui, pal.accent);
                bar_done = true;
            }
            if caret_col == Some(i) && caret_kind == Some(Caret::Block) {
                // Block caret: one inverted cell.
                ui.with(
                    NodeSpec::row()
                        .height(Sizing::Fixed(LH))
                        .cross_align(Align::Center)
                        .bg(pal.accent),
                    |ui| ui.text(&nbsp(&chars[i].to_string()), mono(pal).color(pal.bg)),
                );
                i += 1;
                continue;
            }
            // Extend a run of chars sharing selection state, breaking at
            // the caret cell so it can be emitted inline.
            let selected = at_sel(i);
            let start = i;
            i += 1;
            while i < chars.len() && at_sel(i) == selected && caret_col != Some(i) {
                i += 1;
            }
            let run: String = chars[start..i].iter().collect();
            if selected {
                ui.with(
                    NodeSpec::row()
                        .height(Sizing::Fixed(LH))
                        .cross_align(Align::Center)
                        .bg(pal.select),
                    |ui| ui.text(&nbsp(&run), mono(pal)),
                );
            } else {
                ui.text(&nbsp(&run), mono(pal));
            }
        }
        // Caret at end of line.
        if caret_col == Some(chars.len()) {
            match caret_kind {
                Some(Caret::Bar) => caret_bar(ui, pal.accent),
                Some(Caret::Block) => {
                    ui.with(
                        NodeSpec::column()
                            .width(Sizing::Fixed(8.0))
                            .height(Sizing::Fixed(LH - 4.0))
                            .bg(pal.accent),
                        |_| {},
                    );
                }
                None => {}
            }
        }
        // Selection running past the newline.
        if sel.is_some_and(|(_, b)| b > chars.len()) && caret_col != Some(chars.len()) {
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Fixed(8.0))
                    .height(Sizing::Fixed(LH))
                    .bg(pal.select),
                |_| {},
            );
        }
    });
}

fn status_line(ui: &mut Ui<'_>, pal: &Pal, mode: Mode, doc: &Doc, view: &View) {
    ui.with(
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(STATUS_H))
            .bg(pal.status)
            .pad_xy(8.0, 0.0)
            .gap(8.0)
            .cross_align(Align::Center),
        |ui| {
            let (label, color) = match mode {
                Mode::Normal => ("NOR", pal.accent),
                Mode::Insert => ("INS", pal.insert),
                Mode::Command => ("CMD", pal.command),
            };
            ui.with(
                NodeSpec::row().pad_xy(8.0, 2.0).radius(4.0).bg(color),
                |ui| {
                    ui.text(label, TextStyle::new(10.0).mono().color(pal.bg));
                },
            );
            ui.text(&doc.name, TextStyle::new(12.0).color(pal.fg));
            if doc.modified {
                ui.text("●", TextStyle::new(10.0).color(pal.command));
            }
            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
            let total = doc.lines.len();
            let pct = if total <= 1 {
                100
            } else {
                (view.cur.line * 100) / (total - 1)
            };
            ui.text(
                &format!("{}:{}  {pct}%", view.cur.line + 1, view.cur.col + 1),
                TextStyle::new(11.0).mono().color(pal.dim),
            );
        },
    );
}

// ---------------------------------------------------------------- text ops (the toy model)

fn line_len(doc: &Doc, line: usize) -> usize {
    doc.lines[line].chars().count()
}

fn clamp_col(doc: &Doc, view: &mut View) {
    view.cur.col = view.cur.col.min(line_len(doc, view.cur.line));
}

fn move_h(doc: &Doc, view: &mut View, dx: i64) {
    if dx < 0 {
        if view.cur.col > 0 {
            view.cur.col -= 1;
        } else if view.cur.line > 0 {
            view.cur.line -= 1;
            view.cur.col = line_len(doc, view.cur.line);
        }
    } else if view.cur.col < line_len(doc, view.cur.line) {
        view.cur.col += 1;
    } else if view.cur.line + 1 < doc.lines.len() {
        view.cur.line += 1;
        view.cur.col = 0;
    }
}

fn move_v(doc: &Doc, view: &mut View, dy: i64) {
    let line = view.cur.line as i64 + dy;
    view.cur.line = line.clamp(0, doc.lines.len() as i64 - 1) as usize;
    clamp_col(doc, view);
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn word_fwd(doc: &Doc, view: &mut View) {
    let chars: Vec<char> = doc.lines[view.cur.line].chars().collect();
    let mut c = view.cur.col;
    while c < chars.len() && is_word(chars[c]) {
        c += 1;
    }
    while c < chars.len() && !is_word(chars[c]) {
        c += 1;
    }
    if c == view.cur.col && view.cur.line + 1 < doc.lines.len() {
        view.cur.line += 1;
        view.cur.col = 0;
    } else {
        view.cur.col = c;
    }
}

fn word_back(doc: &Doc, view: &mut View) {
    if view.cur.col == 0 {
        if view.cur.line > 0 {
            view.cur.line -= 1;
            view.cur.col = line_len(doc, view.cur.line);
        }
        return;
    }
    let chars: Vec<char> = doc.lines[view.cur.line].chars().collect();
    let mut c = view.cur.col;
    while c > 0 && !is_word(chars[c - 1]) {
        c -= 1;
    }
    while c > 0 && is_word(chars[c - 1]) {
        c -= 1;
    }
    view.cur.col = c;
}

fn char_to_byte(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map(|(b, _)| b).unwrap_or(s.len())
}

fn insert_text(doc: &mut Doc, view: &mut View, s: &str) {
    doc.modified = true;
    for part in s.split_inclusive('\n') {
        let (text, newline) = match part.strip_suffix('\n') {
            Some(t) => (t, true),
            None => (part, false),
        };
        if !text.is_empty() {
            let line = &mut doc.lines[view.cur.line];
            let byte = char_to_byte(line, view.cur.col);
            line.insert_str(byte, text);
            view.cur.col += text.chars().count();
        }
        if newline {
            let line = doc.lines[view.cur.line].clone();
            let byte = char_to_byte(&line, view.cur.col);
            let (a, b) = line.split_at(byte);
            doc.lines[view.cur.line] = a.to_string();
            doc.lines.insert(view.cur.line + 1, b.to_string());
            view.cur.line += 1;
            view.cur.col = 0;
        }
    }
}

fn backspace(doc: &mut Doc, view: &mut View) {
    doc.modified = true;
    if view.cur.col > 0 {
        let byte = char_to_byte(&doc.lines[view.cur.line], view.cur.col - 1);
        doc.lines[view.cur.line].remove(byte);
        view.cur.col -= 1;
    } else if view.cur.line > 0 {
        let tail = doc.lines.remove(view.cur.line);
        view.cur.line -= 1;
        view.cur.col = line_len(doc, view.cur.line);
        doc.lines[view.cur.line].push_str(&tail);
    }
}

fn delete_char(doc: &mut Doc, view: &mut View) {
    doc.modified = true;
    if view.cur.col < line_len(doc, view.cur.line) {
        let byte = char_to_byte(&doc.lines[view.cur.line], view.cur.col);
        doc.lines[view.cur.line].remove(byte);
    } else if view.cur.line + 1 < doc.lines.len() {
        let tail = doc.lines.remove(view.cur.line + 1);
        doc.lines[view.cur.line].push_str(&tail);
    }
}

fn delete_line(doc: &mut Doc, view: &mut View) {
    doc.modified = true;
    if doc.lines.len() > 1 {
        doc.lines.remove(view.cur.line);
        view.cur.line = view.cur.line.min(doc.lines.len() - 1);
    } else {
        doc.lines[0].clear();
    }
    clamp_col(doc, view);
    view.anchor = None;
}

fn sel_range(view: &View) -> Option<(Pos, Pos)> {
    let a = view.anchor?;
    Some(if a <= view.cur {
        (a, view.cur)
    } else {
        (view.cur, a)
    })
}

/// The selection's intersection with one display line, as a half-open char
/// range; the end can run one past the line for the picked-up newline.
fn sel_on_line(view: &View, doc: &Doc, line: usize) -> Option<(usize, usize)> {
    let (s, e) = sel_range(view)?;
    if line < s.line || line > e.line {
        return None;
    }
    let a = if line == s.line { s.col } else { 0 };
    let b = if line == e.line {
        e.col + 1
    } else {
        line_len(doc, line) + 1
    };
    Some((a, b.min(line_len(doc, line) + 1)))
}

fn sel_lines(doc: &Doc, view: &View) -> Vec<String> {
    match sel_range(view) {
        None => vec![doc.lines[view.cur.line].clone()],
        Some((s, e)) if s.line == e.line => {
            let chars: Vec<char> = doc.lines[s.line].chars().collect();
            let hi = (e.col + 1).min(chars.len());
            vec![chars[s.col.min(hi)..hi].iter().collect()]
        }
        Some((s, e)) => {
            let mut out = Vec::new();
            let first: Vec<char> = doc.lines[s.line].chars().collect();
            out.push(first[s.col.min(first.len())..].iter().collect());
            for l in s.line + 1..e.line {
                out.push(doc.lines[l].clone());
            }
            let last: Vec<char> = doc.lines[e.line].chars().collect();
            out.push(last[..(e.col + 1).min(last.len())].iter().collect());
            out
        }
    }
}

fn delete_sel(doc: &mut Doc, view: &mut View) -> bool {
    let Some((s, e)) = sel_range(view) else {
        return false;
    };
    doc.modified = true;
    let end_tail: String = {
        let chars: Vec<char> = doc.lines[e.line].chars().collect();
        chars[(e.col + 1).min(chars.len())..].iter().collect()
    };
    let start_keep: String = {
        let chars: Vec<char> = doc.lines[s.line].chars().collect();
        chars[..s.col.min(chars.len())].iter().collect()
    };
    doc.lines.splice(s.line..=e.line, [start_keep + &end_tail]);
    view.cur = s;
    view.anchor = None;
    clamp_col(doc, view);
    true
}

fn open_line(doc: &mut Doc, view: &mut View, offset: usize) {
    doc.modified = true;
    doc.lines.insert(view.cur.line + offset, String::new());
    view.cur.line += offset;
    view.cur.col = 0;
    view.anchor = None;
}

fn paste(doc: &mut Doc, view: &mut View, yank: &[String]) {
    match yank {
        [] => {}
        [one] => insert_text(doc, view, one),
        many => {
            doc.modified = true;
            for (i, line) in many.iter().enumerate() {
                doc.lines.insert(view.cur.line + 1 + i, line.clone());
            }
            view.cur.line += 1;
            view.cur.col = 0;
        }
    }
}

// ---------------------------------------------------------------- content

const SAMPLE: &str = "\
# modal editor

An editor keymap that isn't kui's business. This app
owns the document, the modes, and (in real life)
IO, LSP, and undo. kui turns that state into pixels.

## normal mode
h j k l   move        i a I A   insert
w b       words       o O       open line
0 $       line ends   v         select
gg ge G   document    x d dd    delete
                      y p       yank/paste

## commands (:)
:w   write (pretend)  :q :qa   quit
:help                 esc      back out

The caret block is normal mode; the bar is insert.
Try dd on this line, then p a few times.

— events are data: every key you press arrives as
  {kind=key, phase=down, code=..} on one on_key sink;
  a sink that also says key_up hears the release.
  The same dispatch would run from Lua or C.";

impl Example for ModalEditor {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        ("i / Esc", "insert / normal"),
        ("h j k l", "move"),
        ("w b", "words"),
        (":help", "the rest, in the minibuffer"),
    ];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default()
            .size(900.0, 700.0)
            .custom_titlebar()
    }

    fn dock(&self) -> kui_devtools::Dock {
        kui_devtools::Dock::Bottom
    }
}

kui_devtools::main!(ModalEditor::new());
