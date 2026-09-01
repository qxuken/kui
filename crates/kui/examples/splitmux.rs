//! A pane multiplexer: nested splits, tabs, and focus hopping, with the app
//! — not kui — owning the pane tree and the chord keymap. Splits are nested
//! Grow containers rebuilt from a plain enum tree each frame; a pane is just
//! its number here (the modal_editor example shows what real pane content
//! looks like). The whole keyboard arrives through one `on_key` sink as
//! `{kind="key"}` events, so the same chord dispatch would work verbatim
//! from Lua or C.
//!
//! Run: cargo run -p kui --example splitmux
//!
//! Keys — Alt is ⌥ Option on macOS: Alt-v/s split · Alt-o hop panes ·
//! Alt-w close · Alt-t new tab · Alt-1..9 jump to tab. Click a pane to
//! focus it.

use kui::widgets;
use kui::{Align, App, Color, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value, WindowCommand};

const TABBAR_H: f32 = 30.0;

#[cfg(target_os = "macos")]
const ALT: &str = "⌥";
#[cfg(not(target_os = "macos"))]
const ALT: &str = "Alt-";

// ---------------------------------------------------------------- palette

#[derive(Clone, Copy)]
struct Pal {
    bg: Color,
    bg2: Color,
    panel: Color,
    border: Color,
    border_focus: Color,
    fg: Color,
    dim: Color,
    faint: Color,
    accent: Color,
}

impl Default for Pal {
    fn default() -> Self {
        Self {
            bg: Color::rgb8(0x0f, 0x11, 0x17),
            bg2: Color::rgb8(0x13, 0x15, 0x1d),
            panel: Color::rgb8(0x14, 0x16, 0x1e),
            border: Color::rgb8(0x22, 0x25, 0x31),
            border_focus: Color::rgb8(0x3b, 0x5b, 0xd4),
            fg: Color::rgb8(0xd6, 0xd8, 0xe0),
            dim: Color::rgb8(0x8a, 0x8f, 0xa3),
            faint: Color::rgb8(0x50, 0x55, 0x66),
            accent: Color::rgb8(0x6a, 0x8b, 0xff),
        }
    }
}

// ---------------------------------------------------------------- model

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
    Split { dir: SplitDir, ratio: f32, a: Box<Node>, b: Box<Node> },
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
                *self = Node::Split {
                    dir,
                    ratio: 0.5,
                    a: Box::new(old),
                    b: Box::new(Node::Pane(new_id)),
                };
                true
            }
            Node::Pane(_) => false,
            Node::Split { a, b, .. } => a.split(target, dir, new_id) || b.split(target, dir, new_id),
        }
    }

    /// The split at `path` ("a"/"b" steps from the root), for divider drags.
    fn ratio_mut(&mut self, path: &str) -> Option<&mut f32> {
        match self {
            Node::Pane(_) => None,
            Node::Split { ratio, a, b, .. } => match path.split_at_checked(1) {
                None => Some(ratio),
                Some(("a", rest)) => a.ratio_mut(rest),
                Some((_, rest)) => b.ratio_mut(rest),
            },
        }
    }
}

/// Removes a leaf, collapsing its split; None if the tree became empty.
fn without(node: Node, target: u64) -> Option<Node> {
    match node {
        Node::Pane(id) if id == target => None,
        Node::Pane(id) => Some(Node::Pane(id)),
        Node::Split { dir, ratio, a, b } => match (without(*a, target), without(*b, target)) {
            (Some(a), Some(b)) => Some(Node::Split { dir, ratio, a: Box::new(a), b: Box::new(b) }),
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        },
    }
}

// ---------------------------------------------------------------- app

struct Splitmux {
    pal: Pal,
    next_pane: u64,
    /// One split tree per tab — the whole layout is this data.
    tabs: Vec<Node>,
    tab: usize,
    focused: u64,
    /// Path of the divider being dragged, for active styling while the
    /// cursor is off the strip.
    dragging: Option<String>,
    quit: bool,
}

impl Splitmux {
    fn new() -> Self {
        // Launch looking like the app it wants to be: pane | (pane / pane).
        let root = Node::Split {
            dir: SplitDir::H,
            ratio: 0.5,
            a: Box::new(Node::Pane(1)),
            b: Box::new(Node::Split {
                dir: SplitDir::V,
                ratio: 0.5,
                a: Box::new(Node::Pane(2)),
                b: Box::new(Node::Pane(3)),
            }),
        };
        Self {
            pal: Pal::default(),
            next_pane: 4,
            tabs: vec![root],
            tab: 0,
            focused: 1,
            dragging: None,
            quit: false,
        }
    }

    fn pane_ids(&self) -> Vec<u64> {
        let mut ids = Vec::new();
        self.tabs[self.tab].panes(&mut ids);
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

    fn split(&mut self, dir: SplitDir) {
        let id = self.next_pane;
        self.next_pane += 1;
        self.tabs[self.tab].split(self.focused, dir, id);
        self.focused = id;
    }

    fn close_pane(&mut self) {
        let root = std::mem::replace(&mut self.tabs[self.tab], Node::Pane(u64::MAX));
        match without(root, self.focused) {
            Some(root) => {
                self.tabs[self.tab] = root;
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
        self.tabs.push(Node::Pane(id));
        self.tab = self.tabs.len() - 1;
        self.focused = id;
    }

    // ------------------------------------------------------------ keymap

    /// Alt chords, straight off the `code` string. Anything else would go to
    /// the focused pane's content — here panes have none, so it's dropped.
    fn chord(&mut self, code: &str) {
        match code {
            "o" => self.cycle_pane(),
            "v" => self.split(SplitDir::H),
            "s" => self.split(SplitDir::V),
            "w" => self.close_pane(),
            "t" => self.new_tab(),
            _ => {
                if let Ok(n @ 1..=9) = code.parse::<usize>()
                    && n <= self.tabs.len()
                {
                    self.tab = n - 1;
                    self.refocus();
                }
            }
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
                    let mut ids = Vec::new();
                    self.tabs[i].panes(&mut ids);
                    ui.with_keyed(
                        &format!("tab{i}"),
                        NodeSpec::row().pad_xy(10.0, 4.0).radius(6.0).bg(bg).on_click(Value::map([
                            ("kind", "tab".into()),
                            ("tab", Value::Int(i as i64)),
                        ])),
                        |ui| {
                            ui.text(
                                &format!("{}  {} pane(s)", i + 1, ids.len()),
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
                    &format!("{ALT}v/{ALT}s split · {ALT}o hop · {ALT}w close · {ALT}t tab"),
                    TextStyle::new(11.0).color(pal.faint),
                );
            },
        );
    }

    fn render_node(&self, ui: &mut Ui<'_>, node: &Node, path: &str) {
        match node {
            Node::Pane(id) => self.render_pane(ui, *id),
            Node::Split { dir, ratio, a, b } => {
                let pal = self.pal;
                let spec = match dir {
                    SplitDir::H => NodeSpec::row(),
                    SplitDir::V => NodeSpec::column(),
                };
                ui.with(spec.fill(), |ui| {
                    let (wa, wb) = (ratio.clamp(0.05, 0.95), 1.0 - ratio.clamp(0.05, 0.95));
                    let grow = |f: f32| match dir {
                        SplitDir::H => NodeSpec::column().width(Sizing::Grow(f)).height(Sizing::Grow(1.0)),
                        SplitDir::V => NodeSpec::column().width(Sizing::Grow(1.0)).height(Sizing::Grow(f)),
                    };
                    ui.with(grow(wa), |ui| self.render_node(ui, a, &format!("{path}a")));
                    // The divider: a grabbable strip that drags the ratio.
                    // Its drag events carry the parent (this split) rect, so
                    // the handler turns absolute x/y into a ratio directly.
                    let divider = ui.child_key("divider");
                    let active = ui.is_hovered(divider)
                        || ui.is_pressed(divider)
                        || self.dragging.as_deref() == Some(path);
                    let bar = match dir {
                        SplitDir::H => NodeSpec::column().width(Sizing::Fixed(5.0)).height(Sizing::Grow(1.0)),
                        SplitDir::V => NodeSpec::column().width(Sizing::Grow(1.0)).height(Sizing::Fixed(5.0)),
                    };
                    ui.with_keyed(
                        "divider",
                        bar.bg(if active { pal.border_focus } else { pal.bg2 }).on_drag(Value::map([
                            ("kind", "split".into()),
                            ("path", Value::str(path)),
                            ("dir", Value::str(match dir {
                                SplitDir::H => "h",
                                SplitDir::V => "v",
                            })),
                        ])),
                        |_| {},
                    );
                    ui.with(grow(wb), |ui| self.render_node(ui, b, &format!("{path}b")));
                });
            }
        }
    }

    /// A pane is just its number — swap this fn for an editor, a terminal,
    /// whatever; the tree around it doesn't change.
    fn render_pane(&self, ui: &mut Ui<'_>, id: u64) {
        let pal = self.pal;
        let focused = self.focused == id;
        let border = if focused { pal.border_focus } else { pal.border };
        ui.with_keyed(
            &format!("pane{id}"),
            NodeSpec::column()
                .fill()
                .center()
                .gap(8.0)
                .bg(pal.panel)
                .border(1.0, border)
                .clip()
                .on_click(Value::map([
                    ("kind", "focus".into()),
                    ("pane", Value::Int(id as i64)),
                ])),
            |ui| {
                let color = if focused { pal.accent } else { pal.faint };
                ui.text(&format!("{id}"), TextStyle::new(48.0).color(color));
                ui.text(
                    if focused { "focused" } else { "click to focus" },
                    TextStyle::new(12.0).color(if focused { pal.fg } else { pal.dim }),
                );
                ui.text(
                    &format!("{ALT}v splits me · {ALT}w closes me"),
                    TextStyle::new(11.0).color(pal.faint),
                );
            },
        );
    }
}

impl App for Splitmux {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // Closing the last pane empties `tabs`, so don't build a frame from
        // them — just ask the runner to close and emit nothing.
        if self.quit {
            ui.window_command(WindowCommand::Close);
            return;
        }
        let pal = self.pal;
        ui.configure_root(NodeSpec::column().fill().bg(pal.bg));
        widgets::titlebar(ui, "splitmux — the app owns the pane tree, kui owns the pixels");
        self.tab_bar(ui);

        let root = self.tabs[self.tab].clone();
        let sink = ui.with_keyed(
            "main",
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .on_key(Value::Null),
            |ui| self.render_node(ui, &root, ""),
        );
        ui.take_key_focus(sink);

        widgets::latency_hud(ui);
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("key") => {
                // The chord map: Alt (⌥ Option on macOS) + a letter or digit.
                let alt = ev.payload.get("alt").and_then(Value::as_bool).unwrap_or(false);
                let ctrl = ev.payload.get("ctrl").and_then(Value::as_bool).unwrap_or(false);
                if alt && !ctrl
                    && let Some(code) = ev.payload.get("code").and_then(Value::as_str)
                {
                    let code = code.to_string();
                    self.chord(&code);
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
            Some("drag") => {
                let tag = ev.payload.get("tag");
                if tag.and_then(|t| t.get("kind")).and_then(Value::as_str) != Some("split") {
                    return;
                }
                let Some(path) = tag.and_then(|t| t.get("path")).and_then(Value::as_str) else {
                    return;
                };
                match ev.payload.get("phase").and_then(Value::as_str) {
                    Some("end") => self.dragging = None,
                    Some(_) => {
                        // Absolute cursor position over the split's own rect
                        // (carried in the payload) is the new ratio directly.
                        let horizontal =
                            tag.and_then(|t| t.get("dir")).and_then(Value::as_str) == Some("h");
                        let parent = ev.payload.get("parent");
                        let get = |m: Option<&Value>, k| {
                            m.and_then(|v| v.get(k)).and_then(Value::as_float).unwrap_or(0.0)
                        };
                        let ratio = if horizontal {
                            let w = get(parent, "w").max(1.0);
                            (get(Some(&ev.payload), "x") - get(parent, "x")) / w
                        } else {
                            let h = get(parent, "h").max(1.0);
                            (get(Some(&ev.payload), "y") - get(parent, "y")) / h
                        };
                        let path = path.to_string();
                        self.dragging = Some(path.clone());
                        if let Some(r) = self.tabs[self.tab].ratio_mut(&path) {
                            *r = (ratio as f32).clamp(0.05, 0.95);
                        }
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }
}

fn main() {
    kui::app("splitmux").custom_titlebar().size(1100.0, 720.0).run(Splitmux::new()).unwrap();
}
