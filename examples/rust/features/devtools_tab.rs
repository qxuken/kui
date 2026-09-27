//! A tab of the app's own in the core's devtools panel
//! (`docs/adr/0032-a-devtools-tab-mounts-a-slot.md`), in the shape a
//! tree-sitter inspector has. The page is a "source file" whose tokens
//! are coloured by **highlight group** (`@keyword`, `@function`, …) from
//! a small syntax tree this file writes by hand; the panel gains an
//! **Inspector** tab beside facts, events and tree, drawn by this app from
//! its own view with `Ui::devtools_tab_with`, listing that tree. Hover
//! goes both ways: a row of the tab lights the node's tokens in the
//! source, and a token in the source lights its path in the tab — both
//! are the app's own nodes, so both are the app's own `on_hover`. A click
//! on a row selects the token in the panel's tree tab
//! (`set_devtools_selected`), and the tab can raise the panel's picker
//! (`set_devtools_pick`): the pick lands in `devtools_selected`, which the
//! tab reads back as the node it names, with the tab still up. The page
//! has a button of its own that jumps to the tab (`set_devtools_tab`) —
//! an editor's `:syntax_tree` command — and prints which tab the panel
//! is on (`devtools_current_tab`).
//!
//! The closure runs only while the tab is on show — `built` counts the
//! runs, and the page prints it — so a tab nobody looks at costs the
//! declaration and nothing else. Walk to it with Ctrl+Shift+N (facts →
//! events → tree → Inspector), or click it in the strip.
//!
//! Run: cargo run -p kui-native --example devtools_tab [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::widgets;
use kui_native::{
    Align, App, Color, Core, Key, NodeSpec, Sizing, TextStyle, Theme, Ui, UiEvent, Value,
};

/// One node of the syntax tree: its kind, the highlight group a token
/// carries (`None` for an inner node), the line it starts on, its
/// parent, and — for a token — the text.
struct Node {
    kind: &'static str,
    group: Option<&'static str>,
    line: usize,
    parent: Option<usize>,
    text: Option<&'static str>,
}

const fn tok(
    kind: &'static str,
    group: &'static str,
    line: usize,
    parent: usize,
    text: &'static str,
) -> Node {
    Node {
        kind,
        group: Some(group),
        line,
        parent: Some(parent),
        text: Some(text),
    }
}

const fn inner(kind: &'static str, line: usize, parent: usize) -> Node {
    Node {
        kind,
        group: None,
        line,
        parent: Some(parent),
        text: None,
    }
}

/// The tree, in preorder, over the six lines below — what a parser would
/// have said about them. Tokens appear in source order within a line.
#[rustfmt::skip]
const TREE: &[Node] = &[
    Node { kind: "source_file", group: None, line: 0, parent: None, text: None }, // 0
    inner("function_item", 0, 0),                                                 // 1
    tok("fn", "keyword", 0, 1, "fn"),                                             // 2
    tok("identifier", "function", 0, 1, "main"),                                  // 3
    inner("parameters", 0, 1),                                                    // 4
    tok("(", "punctuation", 0, 4, "("),                                           // 5
    tok(")", "punctuation", 0, 4, ")"),                                           // 6
    inner("block", 0, 1),                                                         // 7
    tok("{", "punctuation", 0, 7, "{"),                                           // 8
    inner("let_declaration", 1, 7),                                               // 9
    tok("let", "keyword", 1, 9, "let"),                                           // 10
    tok("identifier", "variable", 1, 9, "tree"),                                  // 11
    tok("=", "operator", 1, 9, "="),                                              // 12
    inner("call_expression", 1, 9),                                               // 13
    tok("identifier", "function", 1, 13, "parse"),                                // 14
    inner("arguments", 1, 13),                                                    // 15
    tok("(", "punctuation", 1, 15, "("),                                          // 16
    tok("identifier", "variable", 1, 15, "source"),                               // 17
    tok(")", "punctuation", 1, 15, ")"),                                          // 18
    tok(";", "punctuation", 1, 9, ";"),                                           // 19
    inner("for_expression", 2, 7),                                                // 20
    tok("for", "keyword", 2, 20, "for"),                                          // 21
    tok("identifier", "variable", 2, 20, "node"),                                 // 22
    tok("in", "keyword", 2, 20, "in"),                                            // 23
    inner("call_expression", 2, 20),                                              // 24
    inner("field_expression", 2, 24),                                             // 25
    tok("identifier", "variable", 2, 25, "tree"),                                 // 26
    tok(".", "punctuation", 2, 25, "."),                                          // 27
    tok("field_identifier", "function", 2, 25, "walk"),                           // 28
    inner("arguments", 2, 24),                                                    // 29
    tok("(", "punctuation", 2, 29, "("),                                          // 30
    tok(")", "punctuation", 2, 29, ")"),                                          // 31
    inner("block", 2, 20),                                                        // 32
    tok("{", "punctuation", 2, 32, "{"),                                          // 33
    inner("expression_statement", 3, 32),                                         // 34
    inner("macro_invocation", 3, 34),                                             // 35
    tok("identifier", "macro", 3, 35, "println"),                                 // 36
    tok("!", "macro", 3, 35, "!"),                                                // 37
    inner("token_tree", 3, 35),                                                   // 38
    tok("(", "punctuation", 3, 38, "("),                                          // 39
    tok("string_literal", "string", 3, 38, "\"{node:?}\""),                       // 40
    tok(")", "punctuation", 3, 38, ")"),                                          // 41
    tok(";", "punctuation", 3, 34, ";"),                                          // 42
    tok("}", "punctuation", 4, 32, "}"),                                          // 43
    tok("}", "punctuation", 5, 7, "}"),                                           // 44
];

/// How far each line is indented, in spaces.
const INDENT: [usize; 6] = [0, 4, 4, 8, 4, 0];

/// The colour a highlight group paints with, from the theme's roles.
fn group_color(t: &Theme, group: &str) -> Color {
    match group {
        "keyword" => t.accent,
        "function" => t.success,
        "string" => t.warning,
        "macro" => t.danger,
        "punctuation" | "operator" => t.muted,
        _ => t.fg,
    }
}

/// Whether `node` is `ancestor` or under it.
fn under(node: usize, ancestor: usize) -> bool {
    let mut n = Some(node);
    while let Some(i) = n {
        if i == ancestor {
            return true;
        }
        n = TREE[i].parent;
    }
    false
}

fn depth(i: usize) -> usize {
    let mut d = 0;
    let mut n = TREE[i].parent;
    while let Some(p) = n {
        d += 1;
        n = TREE[p].parent;
    }
    d
}

/// The node's path from the root, `source_file › function_item › …`.
fn path(i: usize) -> String {
    let mut names = vec![TREE[i].kind];
    let mut n = TREE[i].parent;
    while let Some(p) = n {
        names.push(TREE[p].kind);
        n = TREE[p].parent;
    }
    names.reverse();
    names.join(" › ")
}

#[derive(Default)]
struct Page {
    /// How many times the tab's closure ran: the laziness, on screen.
    built: u32,
    /// The tab row under the pointer: its node's tokens light up in the
    /// source.
    hover_row: Option<usize>,
    /// The source token under the pointer: its path lights up in the tab.
    hover_tok: Option<usize>,
    /// The token the tab last scrolled to, so a hover reveals its row once
    /// and the list stays where the user put it afterwards.
    revealed: Option<usize>,
    /// A row the tab clicked: select its token in the panel's tree on the
    /// next view (the door is the core's, reached from the view).
    reveal: Option<usize>,
    /// The tab asked for the picker, applied the same way.
    pick: bool,
    /// The page's button asked for the tab, applied the same way.
    show_tab: bool,
}

impl Page {
    /// The token a source-side key names, back from the panel: what
    /// `devtools_selected` / `devtools_picked` mean in the tree's own
    /// terms.
    fn node_of(ui: &mut Ui<'_>, key: Option<Key>) -> Option<usize> {
        let key = key?;
        (0..TREE.len())
            .find(|i| TREE[*i].text.is_some() && ui.key_of(&format!("tok:{i}")) == Some(key))
    }
}

impl App for Page {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        if let Some(i) = self.reveal.take()
            && let Some(k) = ui.key_of(&format!("tok:{i}"))
        {
            ui.core().set_devtools_selected(Some(k));
        }
        if std::mem::take(&mut self.pick) {
            ui.core().set_devtools_pick(true);
        }
        if std::mem::take(&mut self.show_tab) {
            ui.core().set_devtools_tab("inspector");
        }
        let current_tab = ui.devtools_current_tab();
        // What the panel holds, in the tree's terms — read before the
        // page is built, so the tab and the source agree on it.
        let selected = Self::node_of(ui, ui.devtools_selected());
        let picked = Self::node_of(ui, ui.devtools_picked());
        let hover_row = self.hover_row;
        let hover_tok = self.hover_tok;
        // A token lights up when the tab row over it is the token itself
        // or an ancestor, or when the panel selected or is picking it.
        let lit = |i: usize| {
            hover_row.is_some_and(|r| under(i, r)) || selected == Some(i) || picked == Some(i)
        };

        ui.with(
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .pad(16.0)
                .gap(4.0),
            |ui| {
                ui.text(
                    "the source, coloured by highlight group — hover a token to see its node in the Inspector",
                    TextStyle::new(12.0).color(t.muted),
                );
                for (line, indent) in INDENT.iter().enumerate() {
                    ui.with(
                        NodeSpec::row()
                            .width(Sizing::Grow(1.0))
                            .cross_align(Align::Center),
                        |ui| {
                            ui.leaf(
                                NodeSpec::row().width(Sizing::Fixed(8.0 * *indent as f32)));
                            let mut first = true;
                            for (i, n) in TREE.iter().enumerate() {
                                let Some(text) = n.text else { continue };
                                if n.line != line {
                                    continue;
                                }
                                // A space before a token that is a word, or
                                // before `=` and `{`: the spacing a formatter
                                // leaves.
                                let word = text.chars().next().is_some_and(|c| c.is_alphanumeric());
                                if !first && (word || matches!(text, "=" | "{")) {
                                    ui.leaf(NodeSpec::row().width(Sizing::Fixed(7.0)));
                                }
                                first = false;
                                let on = lit(i);
                                let tag = Value::map([
                                    ("kind", Value::str("tok")),
                                    ("id", Value::Int(i as i64)),
                                ]);
                                ui.with_keyed(
                                    &format!("tok:{i}"),
                                    NodeSpec::row()
                                        .pad_xy(1.0, 2.0)
                                        .radius(3.0)
                                        .bg(if on { t.accent_soft } else { Color::TRANSPARENT })
                                        .hover_bg(t.hover)
                                        .on_hover(tag.clone())
                                        .on_click(tag)
                                        .label(n.kind),
                                    |ui| {
                                        ui.text(
                                            text,
                                            TextStyle::new(14.0)
                                                .mono()
                                                .color(group_color(&t, n.group.unwrap_or(""))),
                                        );
                                    },
                                );
                            }
                        },
                    );
                }
                ui.text(
                    &match hover_tok.or(selected) {
                        Some(i) => {
                            format!("{} · @{}", path(i), TREE[i].group.unwrap_or("none"))
                        }
                        None => "hover a token, or pick one from the Inspector".into(),
                    },
                    TextStyle::new(12.0).color(t.muted),
                );
                ui.with(
                    NodeSpec::row().gap(8.0).cross_align(Align::Center),
                    |ui| {
                        widgets::button(
                            ui,
                            "open the Inspector",
                            Value::map([("kind", Value::str("show-tab"))]),
                        );
                        ui.text(
                            &format!(
                                "the panel is on `{current_tab}` · Inspector built {} time(s) — only while its tab is on show",
                                self.built
                            ),
                            TextStyle::new(11.0).color(t.faint),
                        );
                    },
                );
            },
        );

        // The tab: declared every frame, drawn only while it is on show.
        let built = &mut self.built;
        let revealed = &mut self.revealed;
        ui.devtools_tab_with("inspector", "Inspector", |ui| {
            *built += 1;
            let picking = ui.devtools_picking();
            // A token hovered in the source scrolls the tab to its row,
            // once per token: `reveal` reads last frame's layout, which
            // is why it is asked here and not where the hover arrived.
            if let Some(i) = hover_tok
                && *revealed != Some(i)
                && let Some(k) = ui.key_of(&format!("node:{i}"))
            {
                ui.reveal(k);
                *revealed = Some(i);
            }
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0))
                    .gap(4.0),
                |ui| {
                    ui.with(
                        NodeSpec::row().gap(6.0).cross_align(Align::Center),
                        |ui| {
                            widgets::button(
                                ui,
                                if picking {
                                    "picking… (Escape leaves)"
                                } else {
                                    "pick a node"
                                },
                                Value::map([("kind", Value::str("pick"))]),
                            );
                            ui.text(
                                &match (selected, picked) {
                                    (_, Some(p)) => format!("picking {}", TREE[p].kind),
                                    (Some(s), None) => format!("selected {}", TREE[s].kind),
                                    (None, None) => "nothing selected in the panel".into(),
                                },
                                TextStyle::new(11.0).color(t.muted),
                            );
                        },
                    );
                    ui.text(
                        "the syntax tree — hover a row to light its tokens, click to select it in the panel's tree",
                        TextStyle::new(11.0).color(t.muted),
                    );
                    ui.with(
                        NodeSpec::column()
                            .width(Sizing::Grow(1.0))
                            .height(Sizing::Grow(1.0))
                            .scroll_y(),
                        |ui| {
                            for (i, n) in TREE.iter().enumerate() {
                                // A row is lit when the source token under the
                                // pointer is it or under it, or when the panel
                                // selected or is picking it.
                                let on_path = hover_tok.is_some_and(|k| under(k, i));
                                let on = on_path || selected == Some(i) || picked == Some(i);
                                let tag = Value::map([
                                    ("kind", Value::str("row")),
                                    ("id", Value::Int(i as i64)),
                                ]);
                                ui.with_keyed(
                                    &format!("node:{i}"),
                                    NodeSpec::row()
                                        .width(Sizing::Grow(1.0))
                                        .pad_xy(6.0, 2.0)
                                        .gap(6.0)
                                        .radius(3.0)
                                        .cross_align(Align::Center)
                                        .bg(if on { t.accent_soft } else { Color::TRANSPARENT })
                                        .hover_bg(t.hover)
                                        .on_hover(tag.clone())
                                        .on_click(tag)
                                        .label(n.kind),
                                    |ui| {
                                        ui.leaf(
                                            NodeSpec::row()
                                                .width(Sizing::Fixed(10.0 * depth(i) as f32)));
                                        let one_line = |size: f32| {
                                            TextStyle::new(size).mono().wrap(kui_native::TextWrap::None)
                                        };
                                        ui.text(
                                            n.kind,
                                            one_line(12.0).color(if n.text.is_some() { t.fg } else { t.muted }),
                                        );
                                        if let Some(g) = n.group {
                                            ui.text(&format!("@{g}"), one_line(11.0).color(group_color(&t, g)));
                                        }
                                        if let Some(text) = n.text {
                                            ui.text(text, one_line(11.0).color(t.faint));
                                        }
                                        ui.leaf(NodeSpec::row().width(Sizing::Grow(1.0)));
                                        ui.text(
                                            &format!("{}", n.line + 1),
                                            TextStyle::new(10.0).color(t.faint),
                                        );
                                    },
                                );
                            }
                        },
                    );
                },
            );
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        let phase = ev.payload.get("phase").and_then(Value::as_str);
        // A hover carries its node's tag under `tag`; a click is the tag.
        let tag = ev.payload.get("tag").unwrap_or(&ev.payload);
        let id = tag.get("id").and_then(Value::as_int).map(|i| i as usize);
        match (tag.get("kind").and_then(Value::as_str), phase) {
            (Some("tok"), Some("enter")) => self.hover_tok = id,
            (Some("tok"), Some("leave")) => {
                if self.hover_tok == id {
                    self.hover_tok = None;
                }
            }
            (Some("row"), Some("enter")) => self.hover_row = id,
            (Some("row"), Some("leave")) => {
                if self.hover_row == id {
                    self.hover_row = None;
                }
            }
            // A click on a row or a token: select it in the panel's tree.
            (Some("row") | Some("tok"), None) => self.reveal = id,
            (Some("pick"), None) => self.pick = true,
            (Some("show-tab"), None) => self.show_tab = true,
            _ => {}
        }
    }
}

impl Example for Page {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        (
            "Ctrl+Shift+N",
            "the next tab — facts, events, tree, Inspector",
        ),
        (
            "Ctrl+Shift+P",
            "pick a token; the Inspector reads it back as its node",
        ),
    ];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(760.0, 480.0)
    }

    /// The tab is declared and not built while another is up; on show it
    /// is built once a frame, over the panel's body; hovering a row lights
    /// the node's tokens in the source and hovering a token lights its
    /// path in the tab; a row's click selects the token in the panel's
    /// tree; the tab's picker lands a pick in `selected` with the tab up;
    /// the page's button jumps to the tab from the app's side.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        // The bare core the drive gets has no panel: on, docked right, as
        // the harness's window would have it.
        core.set_devtools(true);
        core.set_devtools_dock(kui_native::DevtoolsDock::Right);
        core.set_inspect(true);
        let mut d = Drive::new(core, 760.0, 480.0);
        d.frame(self);
        d.frame(self);
        d.check(
            self.built == 0,
            "the tab is declared, not built, while events is up",
        )?;
        let listed = d.key_of("kui-devtools/tab-custom:inspector").is_some();
        d.check(listed, "and the strip lists it")?;
        let chord = |d: &mut Drive<'_>, app: &mut Page| {
            d.key(
                app,
                "n",
                kui_native::KeyMods {
                    ctrl: true,
                    shift: true,
                    ..Default::default()
                },
            );
        };
        chord(&mut d, self); // tree
        chord(&mut d, self); // Inspector
        d.frame(self);
        d.check(self.built == 1, "on show, the closure ran once")?;
        d.frame(self);
        d.check(self.built == 2, "and once a frame")?;
        // The body is not interactive, so `rect_of` (the hit list) has no
        // rect for it: the inspect snapshot has every node's.
        let body_key = d
            .key_of("kui-devtools/tab/inspector")
            .ok_or("no tab body")?;
        let body = d
            .core
            .nodes()
            .iter()
            .find(|n| n.key == body_key)
            .map(|n| n.rect)
            .ok_or("no tab body rect")?;
        let row = d.key_of("node:9").ok_or("no let_declaration row")?;
        let r = d.rect_of(row).ok_or("no row rect")?;
        d.check(
            r.x >= body.x && r.x + r.w <= body.x + body.w && r.y >= body.y,
            "the tab's content is laid out over the panel's body",
        )?;
        let lit_of = |d: &mut Drive<'_>, prefix: &str| -> Vec<usize> {
            let nodes = d.core.nodes();
            (0..TREE.len())
                .filter(|i| {
                    d.core.key_of(&format!("{prefix}:{i}")).is_some_and(|k| {
                        nodes
                            .iter()
                            .any(|n| n.key == k && n.bg != Color::TRANSPARENT)
                    })
                })
                .collect()
        };

        // Hover the `let_declaration` row: its tokens light up on line 2,
        // nothing else does.
        d.hover(self, row);
        d.frame(self);
        d.check(self.hover_row == Some(9), "hovering a row names its node")?;
        let lit = lit_of(&mut d, "tok");
        d.check(
            lit == [10, 11, 12, 14, 16, 17, 18, 19],
            "and the node's tokens are lit in the source, no others",
        )?;

        // Hover a token in the source: its path lights up in the tab.
        let walk = d.key_of("tok:28").ok_or("no walk token")?;
        d.hover(self, walk);
        d.frame(self);
        d.check(
            self.hover_tok == Some(28) && self.hover_row.is_none(),
            "hovering a token names it, and the row's hover ended",
        )?;
        let lit_rows = lit_of(&mut d, "node");
        d.check(
            lit_rows == [0, 1, 7, 20, 24, 25, 28],
            "and its path is lit in the tab, root to leaf",
        )?;

        // The hover scrolled the tab to the token's row; a frame settles
        // that, and a click on the row selects its token in the panel's
        // tree.
        d.frame(self);
        let walk_row = d.key_of("node:28").ok_or("no walk row")?;
        d.click_key(self, walk_row);
        d.frame(self);
        d.frame(self);
        d.check(
            d.core.devtools_selected() == Some(walk),
            "a row's click selected its token in the panel's tree",
        )?;

        // The picker, raised from the tab: over the app, the tab stays,
        // and the pick reads back as the node it names.
        let pick = d.key_of("pick a node").ok_or("no pick button")?;
        d.click_key(self, pick);
        d.frame(self);
        d.check(d.core.devtools_picking(), "the tab raised the picker")?;
        d.frame(self);
        let up = d.key_of("kui-devtools/tab/inspector").is_some();
        d.check(up, "and stayed up while picking")?;
        let string = d.key_of("tok:40").ok_or("no string token")?;
        d.hover(self, string);
        d.frame(self);
        d.check(
            d.core.devtools_picked() == Some(string),
            "the token is under the picker",
        )?;
        let r = d.rect_of(string).ok_or("no rect")?;
        d.click(self, r.x + 4.0, r.y + r.h / 2.0);
        d.frame(self);
        d.check(
            !d.core.devtools_picking() && d.core.devtools_selected() == Some(string),
            "the press picked the token into `selected`",
        )?;
        let up = d.key_of("kui-devtools/tab/inspector").is_some();
        d.check(up, "and the tab is still the one on show")?;
        chord(&mut d, self); // facts
        let runs = self.built;
        d.frame(self);
        d.check(self.built == runs, "another tab up: the closure rests")?;
        d.check(
            d.core.devtools_current_tab() == "facts",
            "and the panel says which it is on",
        )?;
        // The page's own button jumps to the tab: the app's command, not
        // the strip's click.
        let open = d.key_of("open the Inspector").ok_or("no open button")?;
        d.click_key(self, open);
        d.frame(self);
        d.check(
            d.core.devtools_current_tab() == "inspector",
            "the button selected the tab from the app's side",
        )?;
        d.check(
            self.built == runs + 1,
            "and the same view built it, the door being read before the tab",
        )?;
        let up = d.key_of("kui-devtools/tab/inspector").is_some();
        d.check(up, "over the panel's body")?;
        Ok(())
    }
}

kui_devtools::main!(Page::default());
