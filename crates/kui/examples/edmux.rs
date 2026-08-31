//! edmux: the shell of an emacs/helix/terminal-multiplexer hybrid, with the
//! app — not kui — owning every document, terminal, and keymap. kui's part is
//! the view: splits are nested Grow containers, panes are data, and the whole
//! keyboard arrives through one `on_key` sink as `{kind="key"}` events, so
//! the same modal dispatch would work verbatim from Lua or C.
//!
//! The document model here is a deliberately dumb Vec<String> — the point is
//! the boundary, not the rope. An IO/LSP/extension layer replaces the model;
//! the view code stays.
//!
//! Run: cargo run -p kui --example edmux
//!
//! Keys: see the welcome buffer (`:help` brings it back).

use std::collections::HashMap;

use kui::widgets;
use kui::{Align, App, Color, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value, WindowCommand};

const FONT: f32 = 13.5;
const LH: f32 = 20.0;
const GUTTER_W: f32 = 52.0;
const STATUS_H: f32 = 24.0;
const TABBAR_H: f32 = 30.0;
const MINIBUF_H: f32 = 26.0;

// ---------------------------------------------------------------- palette

#[derive(Clone, Copy)]
struct Pal {
    bg: Color,
    bg2: Color,
    panel: Color,
    status: Color,
    border: Color,
    border_focus: Color,
    fg: Color,
    dim: Color,
    faint: Color,
    accent: Color,
    select: Color,
    insert: Color,
    command: Color,
    term: Color,
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
            bg2: Color::rgb8(0x13, 0x15, 0x1d),
            panel: Color::rgb8(0x14, 0x16, 0x1e),
            status: Color::rgb8(0x1a, 0x1d, 0x27),
            border: Color::rgb8(0x22, 0x25, 0x31),
            border_focus: Color::rgb8(0x3b, 0x5b, 0xd4),
            fg: Color::rgb8(0xd6, 0xd8, 0xe0),
            dim: Color::rgb8(0x8a, 0x8f, 0xa3),
            faint: Color::rgb8(0x50, 0x55, 0x66),
            accent: Color::rgb8(0x6a, 0x8b, 0xff),
            select: Color::rgba8(0x3b, 0x5b, 0xd4, 0x55),
            insert: Color::rgb8(0x5f, 0xc1, 0x7e),
            command: Color::rgb8(0xd9, 0xa1, 0x4d),
            term: Color::rgb8(0xb1, 0x6a, 0xd4),
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
enum Mode {
    Normal,
    Insert,
    Command,
}

#[derive(Clone, Copy, PartialEq)]
enum Lang {
    Rust,
    Text,
}

impl Lang {
    fn from_name(name: &str) -> Self {
        if name.ends_with(".rs") { Lang::Rust } else { Lang::Text }
    }
}

struct Doc {
    name: String,
    lines: Vec<String>,
    lang: Lang,
    modified: bool,
}

impl Doc {
    fn new(name: &str, text: &str, lang: Lang) -> Self {
        let lines = text.split('\n').map(|l| l.replace('\t', "    ")).collect();
        Self { name: name.into(), lines, lang, modified: false }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Pos {
    line: usize,
    col: usize,
}

/// Per-pane view of a document: cursor, selection anchor, scroll. `rows` is
/// written during render (the view fn knows the pane's height) and read by
/// the keymap for paging; scroll-into-view happens in render too, so the
/// caret can never leave the screen.
#[derive(Clone)]
struct View {
    cur: Pos,
    anchor: Option<Pos>,
    top: usize,
    rows: usize,
}

impl Default for View {
    fn default() -> Self {
        Self { cur: Pos::default(), anchor: None, top: 0, rows: 24 }
    }
}

#[derive(Clone, Default)]
struct Term {
    lines: Vec<String>,
    input: String,
}

#[derive(Clone)]
enum Content {
    Edit { doc: usize, view: View },
    Term(Term),
}

#[derive(Clone)]
struct Pane {
    content: Content,
}

#[derive(Clone, Copy, PartialEq)]
enum SplitDir {
    /// Side by side.
    H,
    /// Stacked.
    V,
}

#[derive(Clone)]
enum Node {
    Pane(u64),
    Split { dir: SplitDir, a: Box<Node>, b: Box<Node> },
}

impl Node {
    fn panes(&self, out: &mut Vec<u64>) {
        match self {
            Node::Pane(id) => out.push(*id),
            Node::Split { a, b, .. } => {
                a.panes(out);
                b.panes(out);
            }
        }
    }

    /// Replaces the leaf `target` with a split of it and `new_id`.
    fn split(&mut self, target: u64, dir: SplitDir, new_id: u64) -> bool {
        match self {
            Node::Pane(id) if *id == target => {
                let old = Node::Pane(*id);
                *self = Node::Split { dir, a: Box::new(old), b: Box::new(Node::Pane(new_id)) };
                true
            }
            Node::Pane(_) => false,
            Node::Split { a, b, .. } => a.split(target, dir, new_id) || b.split(target, dir, new_id),
        }
    }
}

/// Removes a leaf, collapsing its split; None if the tree became empty.
fn without(node: Node, target: u64) -> Option<Node> {
    match node {
        Node::Pane(id) if id == target => None,
        Node::Pane(id) => Some(Node::Pane(id)),
        Node::Split { dir, a, b } => match (without(*a, target), without(*b, target)) {
            (Some(a), Some(b)) => Some(Node::Split { dir, a: Box::new(a), b: Box::new(b) }),
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        },
    }
}

struct Tab {
    root: Node,
}

// ---------------------------------------------------------------- app

struct Edmux {
    pal: Pal,
    docs: Vec<Doc>,
    panes: HashMap<u64, Pane>,
    next_pane: u64,
    tabs: Vec<Tab>,
    tab: usize,
    focused: u64,
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
    ch: Option<char>,
    text: Option<String>,
    ctrl: bool,
    alt: bool,
}

impl KeyEv {
    fn from_payload(p: &Value) -> Option<Self> {
        let code = p.get("code")?.as_str()?.to_string();
        let mut chars = code.chars();
        let ch = match (chars.next(), chars.next()) {
            (Some(c), None) => Some(c),
            _ => None,
        };
        Some(Self {
            ch,
            text: p.get("text").and_then(Value::as_str).map(str::to_string),
            ctrl: p.get("ctrl").and_then(Value::as_bool).unwrap_or(false),
            alt: p.get("alt").and_then(Value::as_bool).unwrap_or(false),
            code,
        })
    }
}

impl Edmux {
    fn new() -> Self {
        let docs = vec![
            Doc::new("*welcome*", WELCOME, Lang::Text),
            Doc::new("main.rs", SAMPLE_RS, Lang::Rust),
        ];
        let mut panes = HashMap::new();
        panes.insert(0, Pane { content: Content::Edit { doc: 0, view: View::default() } });
        panes.insert(1, Pane { content: Content::Edit { doc: 1, view: View::default() } });
        panes.insert(2, Pane { content: Content::Term(fresh_term()) });
        // Launch looking like the app it wants to be: editor | (editor / term).
        let root = Node::Split {
            dir: SplitDir::H,
            a: Box::new(Node::Pane(0)),
            b: Box::new(Node::Split {
                dir: SplitDir::V,
                a: Box::new(Node::Pane(1)),
                b: Box::new(Node::Pane(2)),
            }),
        };
        Self {
            pal: Pal::default(),
            docs,
            panes,
            next_pane: 3,
            tabs: vec![Tab { root }],
            tab: 0,
            focused: 0,
            mode: Mode::Normal,
            cmd: String::new(),
            message: "edmux — modal shell demo. :help brings the keymap back".into(),
            pending: None,
            yank: Vec::new(),
            quit: false,
        }
    }

    fn pane_label(&self, id: u64) -> String {
        match &self.panes[&id].content {
            Content::Edit { doc, .. } => self.docs[*doc].name.clone(),
            Content::Term(_) => "*term*".into(),
        }
    }

    fn tab_label(&self, i: usize) -> String {
        let mut ids = Vec::new();
        self.tabs[i].root.panes(&mut ids);
        ids.first().map(|id| self.pane_label(*id)).unwrap_or_else(|| "empty".into())
    }

    fn pane_ids(&self) -> Vec<u64> {
        let mut ids = Vec::new();
        self.tabs[self.tab].root.panes(&mut ids);
        ids
    }

    fn refocus(&mut self) {
        self.focused = self.pane_ids().first().copied().unwrap_or(0);
    }

    fn cycle_pane(&mut self) {
        let ids = self.pane_ids();
        if let Some(i) = ids.iter().position(|id| *id == self.focused) {
            self.focused = ids[(i + 1) % ids.len()];
        } else if let Some(id) = ids.first() {
            self.focused = *id;
        }
    }

    fn split(&mut self, dir: SplitDir, content: Option<Content>) {
        let content = content.unwrap_or_else(|| self.panes[&self.focused].content.clone());
        let id = self.next_pane;
        self.next_pane += 1;
        self.panes.insert(id, Pane { content });
        self.tabs[self.tab].root.split(self.focused, dir, id);
        self.focused = id;
    }

    fn close_pane(&mut self) {
        let target = self.focused;
        let root = std::mem::replace(&mut self.tabs[self.tab].root, Node::Pane(u64::MAX));
        self.panes.remove(&target);
        match without(root, target) {
            Some(root) => {
                self.tabs[self.tab].root = root;
                self.refocus();
            }
            None => {
                self.tabs.remove(self.tab);
                if self.tabs.is_empty() {
                    self.quit = true;
                    return;
                }
                self.tab = self.tab.min(self.tabs.len() - 1);
                self.refocus();
            }
        }
    }

    fn new_tab(&mut self) {
        let id = self.next_pane;
        self.next_pane += 1;
        self.panes.insert(id, Pane { content: Content::Edit { doc: 0, view: View::default() } });
        self.tabs.push(Tab { root: Node::Pane(id) });
        self.tab = self.tabs.len() - 1;
        self.focused = id;
    }

    fn open_doc(&mut self, name: &str) {
        let idx = match self.docs.iter().position(|d| d.name == name) {
            Some(i) => i,
            None => {
                self.docs.push(Doc::new(name, "", Lang::from_name(name)));
                self.docs.len() - 1
            }
        };
        if let Some(Pane { content: Content::Edit { doc, view } }) =
            self.panes.get_mut(&self.focused)
        {
            *doc = idx;
            *view = View::default();
        }
        self.message = format!("opened {name}");
    }

    // ------------------------------------------------------------ keymap

    fn on_key(&mut self, k: KeyEv) {
        // Multiplexer chords work everywhere, modes be damned.
        if k.alt && !k.ctrl {
            match k.ch {
                Some('o') => return self.cycle_pane(),
                Some('v') => return self.split(SplitDir::H, None),
                Some('s') => return self.split(SplitDir::V, None),
                Some('w') => return self.close_pane(),
                Some('t') => return self.split(SplitDir::V, Some(Content::Term(fresh_term()))),
                Some(c @ '1'..='9') => {
                    let i = c as usize - '1' as usize;
                    if i < self.tabs.len() {
                        self.tab = i;
                        self.refocus();
                    }
                    return;
                }
                _ => {}
            }
        }
        match self.mode {
            Mode::Command => self.key_command(k),
            _ => {
                match self.panes.get(&self.focused).map(|p| matches!(p.content, Content::Term(_)))
                {
                    Some(true) => self.key_term(k),
                    Some(false) if self.mode == Mode::Insert => self.key_insert(k),
                    Some(false) => self.key_normal(k),
                    None => {}
                }
            }
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
        // Tab hops out of a pending `g` never touch the document.
        if pending == Some('g') {
            match k.code.as_str() {
                "t" => {
                    self.tab = (self.tab + 1) % self.tabs.len();
                    self.refocus();
                    return;
                }
                "T" => {
                    self.tab = (self.tab + self.tabs.len() - 1) % self.tabs.len();
                    self.refocus();
                    return;
                }
                _ => {}
            }
        }

        let id = self.focused;
        let Some(Pane { content: Content::Edit { doc: di, view } }) = self.panes.get_mut(&id)
        else {
            return;
        };
        let doc = &mut self.docs[*di];

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
            "v" => view.anchor = if view.anchor.is_some() { None } else { Some(view.cur) },
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
        let id = self.focused;
        let Some(Pane { content: Content::Edit { doc: di, view } }) = self.panes.get_mut(&id)
        else {
            return;
        };
        let doc = &mut self.docs[*di];
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

    fn key_term(&mut self, k: KeyEv) {
        let id = self.focused;
        let Some(Pane { content: Content::Term(term) }) = self.panes.get_mut(&id) else { return };
        match k.code.as_str() {
            "enter" => {
                let input = std::mem::take(&mut term.input);
                term_exec(term, &self.docs, &input);
            }
            "backspace" => {
                term.input.pop();
            }
            "escape" => self.message = "terminal is modeless — Alt-o hops panes".into(),
            _ => {
                if let Some(t) = &k.text {
                    term.input.push_str(t);
                }
            }
        }
    }

    fn exec_command(&mut self, cmd: &str) {
        let mut parts = cmd.split_whitespace();
        let head = parts.next().unwrap_or("");
        let rest = parts.collect::<Vec<_>>().join(" ");
        match head {
            "" => {}
            "q" | "q!" => self.close_pane(),
            "qa" | "qa!" => self.quit = true,
            "w" | "write" => self.write_doc(),
            "wq" => {
                self.write_doc();
                self.close_pane();
            }
            "vs" | "vsplit" => self.split(SplitDir::H, None),
            "sp" | "split" | "hsplit" => self.split(SplitDir::V, None),
            "term" => self.split(SplitDir::V, Some(Content::Term(fresh_term()))),
            "tabnew" => self.new_tab(),
            "e" | "o" | "open" | "b" | "buffer" if !rest.is_empty() => self.open_doc(&rest),
            "bn" | "bnext" => self.cycle_doc(1),
            "bp" | "bprev" => self.cycle_doc(-1),
            "ls" | "buffers" => {
                let names: Vec<&str> = self.docs.iter().map(|d| d.name.as_str()).collect();
                self.message = format!("buffers: {}", names.join("  "));
            }
            "help" => self.open_doc("*welcome*"),
            other => self.message = format!("not a command: {other} (:help for the keymap)"),
        }
    }

    fn write_doc(&mut self) {
        if let Some(Pane { content: Content::Edit { doc, .. } }) = self.panes.get(&self.focused) {
            let doc = &mut self.docs[*doc];
            doc.modified = false;
            self.message =
                format!("wrote {} — well, pretended to; IO belongs to your app", doc.name);
        }
    }

    fn cycle_doc(&mut self, delta: i64) {
        let n = self.docs.len() as i64;
        if let Some(Pane { content: Content::Edit { doc, view } }) =
            self.panes.get_mut(&self.focused)
        {
            *doc = ((*doc as i64 + delta + n) % n) as usize;
            *view = View::default();
        }
    }

    // ------------------------------------------------------------ view

    fn tab_bar(&self, ui: &mut Ui<'_>) {
        let pal = self.pal;
        ui.with(
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Fixed(TABBAR_H))
                .bg(pal.bg2)
                .pad_xy(8.0, 0.0)
                .gap(4.0)
                .cross_align(Align::Center),
            |ui| {
                for i in 0..self.tabs.len() {
                    let active = i == self.tab;
                    let (bg, fg) =
                        if active { (pal.panel, pal.fg) } else { (Color::TRANSPARENT, pal.dim) };
                    ui.with_keyed(
                        &format!("tab{i}"),
                        NodeSpec::row().pad_xy(10.0, 4.0).radius(6.0).bg(bg).on_click(Value::map([
                            ("kind", "tab".into()),
                            ("tab", Value::Int(i as i64)),
                        ])),
                        |ui| {
                            ui.text(
                                &format!("{}  {}", i + 1, self.tab_label(i)),
                                TextStyle::new(12.0).color(fg),
                            );
                        },
                    );
                }
                ui.with_keyed(
                    "tab+",
                    NodeSpec::row()
                        .pad_xy(8.0, 4.0)
                        .radius(6.0)
                        .on_click(Value::map([("kind", "tabnew".into())])),
                    |ui| ui.text("+", TextStyle::new(12.0).color(pal.faint)),
                );
                ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                ui.text(
                    &format!(
                        "{} panes — Alt-v/s split · Alt-o hop · Alt-t term",
                        self.pane_ids().len()
                    ),
                    TextStyle::new(11.0).color(pal.faint),
                );
            },
        );
    }

    fn render_node(&mut self, ui: &mut Ui<'_>, node: &Node, h: f32) {
        match node {
            Node::Pane(id) => self.render_pane(ui, *id, h),
            Node::Split { dir: SplitDir::H, a, b } => {
                ui.with(NodeSpec::row().fill().gap(1.0), |ui| {
                    ui.with(NodeSpec::column().fill(), |ui| self.render_node(ui, a, h));
                    ui.with(NodeSpec::column().fill(), |ui| self.render_node(ui, b, h));
                });
            }
            Node::Split { dir: SplitDir::V, a, b } => {
                let child_h = (h - 1.0) / 2.0;
                ui.with(NodeSpec::column().fill().gap(1.0), |ui| {
                    ui.with(NodeSpec::column().fill(), |ui| self.render_node(ui, a, child_h));
                    ui.with(NodeSpec::column().fill(), |ui| self.render_node(ui, b, child_h));
                });
            }
        }
    }

    fn render_pane(&mut self, ui: &mut Ui<'_>, id: u64, h: f32) {
        let pal = self.pal;
        let focused = self.focused == id;
        let mode = self.mode;
        let border = if focused { pal.border_focus } else { pal.border };
        let Some(pane) = self.panes.get_mut(&id) else { return };
        let docs = &self.docs;
        ui.with_keyed(
            &format!("pane{id}"),
            NodeSpec::column()
                .fill()
                .bg(pal.panel)
                .border(1.0, border)
                .clip()
                .on_click(Value::map([
                    ("kind", "focus".into()),
                    ("pane", Value::Int(id as i64)),
                ])),
            |ui| match &mut pane.content {
                Content::Edit { doc, view } => {
                    render_editor(ui, &pal, &docs[*doc], view, focused, mode, h)
                }
                Content::Term(term) => render_term(ui, &pal, term, focused, h),
            },
        );
    }

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

impl App for Edmux {
    fn view(&mut self, ui: &mut Ui<'_>) {
        if self.quit {
            ui.window_command(WindowCommand::Close);
        }
        let pal = self.pal;
        ui.window_title("edmux");
        ui.configure_root(NodeSpec::column().fill().bg(pal.bg));
        widgets::titlebar(ui, "edmux — the app owns the model, kui owns the pixels");
        self.tab_bar(ui);

        let vp = ui.viewport();
        let content_h = (vp.h - widgets::TITLEBAR_H - TABBAR_H - MINIBUF_H).max(LH);
        let root = self.tabs[self.tab].root.clone();
        let sink = ui.with_keyed(
            "main",
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .on_key(Value::Null),
            |ui| self.render_node(ui, &root, content_h),
        );
        ui.take_key_focus(sink);

        self.minibuffer(ui);
        widgets::latency_hud(ui);
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("key") => {
                if let Some(k) = KeyEv::from_payload(&ev.payload) {
                    self.on_key(k);
                }
            }
            Some("focus") => {
                if let Some(id) = ev.payload.get("pane").and_then(Value::as_int) {
                    self.focused = id as u64;
                }
            }
            Some("tab") => {
                if let Some(i) = ev.payload.get("tab").and_then(Value::as_int) {
                    self.tab = i as usize;
                    self.refocus();
                }
            }
            Some("tabnew") => self.new_tab(),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- editor rendering

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
        NodeSpec::column().width(Sizing::Fixed(2.0)).height(Sizing::Fixed(LH - 4.0)).bg(color),
        |_| {},
    );
}

#[derive(Clone, Copy, PartialEq)]
enum Caret {
    Bar,
    Block,
}

fn render_editor(
    ui: &mut Ui<'_>,
    pal: &Pal,
    doc: &Doc,
    view: &mut View,
    focused: bool,
    mode: Mode,
    h: f32,
) {
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
        NodeSpec::row().width(Sizing::Grow(1.0)).height(Sizing::Grow(1.0)).pad_xy(0.0, 4.0),
        |ui| {
            // Gutter.
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Fixed(GUTTER_W))
                    .height(Sizing::Grow(1.0))
                    .pad_xy(12.0, 0.0),
                |ui| {
                    for ln in view.top..last {
                        let current = ln == view.cur.line;
                        let color = if current && focused { pal.dim } else { pal.faint };
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
                NodeSpec::column().width(Sizing::Grow(1.0)).height(Sizing::Grow(1.0)).clip(),
                |ui| {
                    for ln in view.top..last {
                        let caret = (focused && ln == view.cur.line).then(|| {
                            (
                                view.cur.col,
                                if mode == Mode::Insert { Caret::Bar } else { Caret::Block },
                            )
                        });
                        emit_line(
                            ui,
                            pal,
                            &doc.lines[ln],
                            doc.lang,
                            sel_on_line(view, doc, ln),
                            caret,
                        );
                    }
                },
            );
        },
    );

    status_line(ui, pal, focused, mode, &doc.name, doc.modified, view, doc.lines.len());
}

/// One line as a row of coalesced style runs: syntax color from the app's
/// highlighter, selection as background segments, the caret as an inline
/// node. No text measurement needed anywhere — the row *is* the layout.
fn emit_line(
    ui: &mut Ui<'_>,
    pal: &Pal,
    text: &str,
    lang: Lang,
    sel: Option<(usize, usize)>,
    caret: Option<(usize, Caret)>,
) {
    let chars: Vec<char> = text.chars().collect();
    let colors = highlight(pal, &chars, lang);
    let (caret_col, caret_kind) = match caret {
        Some((c, k)) => (Some(c), Some(k)),
        None => (None, None),
    };
    let at_sel = |i: usize| sel.is_some_and(|(a, b)| i >= a && i < b);

    ui.with(
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(LH))
            .cross_align(Align::Center),
        |ui| {
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
                // Extend a run of chars sharing color + selection, breaking
                // at the caret cell so it can be emitted inline.
                let selected = at_sel(i);
                let start = i;
                let color = colors[i];
                i += 1;
                while i < chars.len()
                    && colors[i] == color
                    && at_sel(i) == selected
                    && caret_col != Some(i)
                {
                    i += 1;
                }
                let run: String = chars[start..i].iter().collect();
                let style = mono(pal).color(color);
                if selected {
                    ui.with(
                        NodeSpec::row()
                            .height(Sizing::Fixed(LH))
                            .cross_align(Align::Center)
                            .bg(pal.select),
                        |ui| ui.text(&nbsp(&run), style),
                    );
                } else {
                    ui.text(&nbsp(&run), style);
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
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn status_line(
    ui: &mut Ui<'_>,
    pal: &Pal,
    focused: bool,
    mode: Mode,
    name: &str,
    modified: bool,
    view: &View,
    total: usize,
) {
    ui.with(
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(STATUS_H))
            .bg(pal.status)
            .pad_xy(8.0, 0.0)
            .gap(8.0)
            .cross_align(Align::Center),
        |ui| {
            if focused {
                let (label, color) = match mode {
                    Mode::Normal => ("NOR", pal.accent),
                    Mode::Insert => ("INS", pal.insert),
                    Mode::Command => ("CMD", pal.command),
                };
                mode_chip(ui, label, color, pal);
            }
            let name_color = if focused { pal.fg } else { pal.dim };
            ui.text(name, TextStyle::new(12.0).color(name_color));
            if modified {
                ui.text("●", TextStyle::new(10.0).color(pal.command));
            }
            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
            let pct = if total <= 1 { 100 } else { (view.cur.line * 100) / (total - 1) };
            ui.text(
                &format!("{}:{}  {pct}%", view.cur.line + 1, view.cur.col + 1),
                TextStyle::new(11.0).mono().color(pal.dim),
            );
        },
    );
}

fn mode_chip(ui: &mut Ui<'_>, label: &str, color: Color, pal: &Pal) {
    ui.with(NodeSpec::row().pad_xy(8.0, 2.0).radius(4.0).bg(color), |ui| {
        ui.text(label, TextStyle::new(10.0).mono().color(pal.bg));
    });
}

// ---------------------------------------------------------------- terminal rendering

fn fresh_term() -> Term {
    Term {
        lines: vec![
            "edmux pseudo-terminal — a pane kind, not a shell.".into(),
            "The real app would put a PTY behind this exact view.".into(),
            "try: help · ls · echo hi · clear".into(),
        ],
        input: String::new(),
    }
}

fn render_term(ui: &mut Ui<'_>, pal: &Pal, term: &Term, focused: bool, h: f32) {
    let rows = (((h - STATUS_H - 12.0) / LH).max(1.0)) as usize;
    let visible = rows.saturating_sub(1);
    let start = term.lines.len().saturating_sub(visible);
    ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .pad_xy(10.0, 6.0)
            .clip(),
        |ui| {
            for line in &term.lines[start..] {
                ui.with(
                    NodeSpec::row()
                        .width(Sizing::Grow(1.0))
                        .height(Sizing::Fixed(LH))
                        .cross_align(Align::Center),
                    |ui| {
                        let color = if line.starts_with('❯') { pal.fg } else { pal.dim };
                        ui.text(&nbsp(line), mono(pal).color(color));
                    },
                );
            }
            ui.with(
                NodeSpec::row()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(LH))
                    .cross_align(Align::Center)
                    .gap(2.0),
                |ui| {
                    ui.text("❯\u{a0}", mono(pal).color(pal.term));
                    ui.text(&nbsp(&term.input), mono(pal));
                    if focused {
                        caret_bar(ui, pal.term);
                    }
                },
            );
        },
    );
    ui.with(
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(STATUS_H))
            .bg(pal.status)
            .pad_xy(8.0, 0.0)
            .gap(8.0)
            .cross_align(Align::Center),
        |ui| {
            if focused {
                mode_chip(ui, "TERM", pal.term, pal);
            }
            ui.text("*term*", TextStyle::new(12.0).color(if focused { pal.fg } else { pal.dim }));
            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
            ui.text("modeless — Alt-o hops out", TextStyle::new(11.0).color(pal.faint));
        },
    );
}

fn term_exec(term: &mut Term, docs: &[Doc], input: &str) {
    let input = input.trim();
    term.lines.push(format!("❯ {input}"));
    let mut parts = input.split_whitespace();
    match parts.next().unwrap_or("") {
        "" => {}
        "help" => {
            term.lines.push("this terminal is a prop: help · ls · echo <text> · clear".into());
            term.lines.push("wire a PTY into a pane and this view draws it.".into());
        }
        "ls" => {
            let names: Vec<&str> = docs.iter().map(|d| d.name.as_str()).collect();
            term.lines.push(names.join("  "));
        }
        "clear" => term.lines.clear(),
        "echo" => term.lines.push(parts.collect::<Vec<_>>().join(" ")),
        other => term.lines.push(format!("edmux: {other}: not found (the shell is a prop)")),
    }
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
    Some(if a <= view.cur { (a, view.cur) } else { (view.cur, a) })
}

/// The selection's intersection with one display line, as a half-open char
/// range; the end can run one past the line for the picked-up newline.
fn sel_on_line(view: &View, doc: &Doc, line: usize) -> Option<(usize, usize)> {
    let (s, e) = sel_range(view)?;
    if line < s.line || line > e.line {
        return None;
    }
    let a = if line == s.line { s.col } else { 0 };
    let b = if line == e.line { e.col + 1 } else { line_len(doc, line) + 1 };
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
    let Some((s, e)) = sel_range(view) else { return false };
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

// ---------------------------------------------------------------- highlighter (a stand-in)

const RUST_KW: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
    "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
    "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true",
    "type", "unsafe", "use", "where", "while",
];

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

const WELCOME: &str = "\
# edmux

The shell of an editor that isn't kui's business.
This app owns the documents, the keymap, the splits,
the terminal, and (in real life) IO, LSP, and
extensions. kui turns that state into pixels.

## normal mode        ## panes & tabs
h j k l   move        Alt-v     split side-by-side
w b       words       Alt-s     split stacked
0 $       line ends   Alt-o     hop between panes
gg ge G   document    Alt-w     close pane
i a I A   insert      Alt-t     open a terminal
o O       open line   Alt-1..9  jump to tab
v         select      gt gT     cycle tabs
x d dd    delete
y p       yank/paste  ## commands (:)
:         command     :vs :sp :term :tabnew
esc       back out    :e <name> :bn :ls
                      :w :q :qa :help

Click any pane to focus it. The caret block is
normal mode; the bar is insert. Try dd on this
line, then p a few times.

— events are data: every key you press arrives as
  {kind=key, code=..} on one on_key sink. The
  same dispatch would run from Lua or C.";

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
    kui::app("edmux").custom_titlebar().size(1280.0, 800.0).run(Edmux::new()).unwrap();
}
