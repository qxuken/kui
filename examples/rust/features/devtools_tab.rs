//! A tab of the app's own in the core's devtools panel
//! (`docs/adr/0032-a-devtools-tab-mounts-a-slot.md`). The page is a
//! "source file" of lines; the panel gains an **Inspector** tab beside
//! facts, events and tree, drawn by this app from its own view with
//! `Ui::devtools_tab_with` — so its nodes are the app's (its keys, its
//! events), laid out over the panel's tab body, in the dock's focus
//! region. The tab reads the panel's facts through the doors
//! (`devtools_selected` / `hovered` / `picked`) and drives the tree tab's
//! highlight back through `set_devtools_selected`, which is the shape a
//! tree-sitter inspector has: the app knows the structure, the panel
//! knows the node.
//!
//! The closure runs only while the tab is on show — `built` counts the
//! runs, and the page prints it — so a tab nobody looks at costs the
//! declaration and nothing else. Walk to it with Ctrl+Shift+N (facts →
//! events → tree → Inspector), or click it in the strip.
//!
//! Run: cargo run -p kui --example devtools_tab [-- --headless]

use kui::widgets;
use kui::{App, Core, Key, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};
use kui_devtools::{Drive, Example};

const LINES: &[&str] = &[
    "fn main() {",
    "    let tree = parse(source);",
    "    for node in tree.walk() {",
    "        println!(\"{node:?}\");",
    "    }",
    "}",
];

#[derive(Default)]
struct Page {
    /// How many times the tab's closure ran: the laziness, on screen.
    built: u32,
    /// A line the tab asked to select in the panel's tree, applied on the
    /// next view (the door is the core's, reached from the view).
    reveal: Option<usize>,
    /// The tab asked for the picker, applied the same way.
    pick: bool,
    /// The line the page itself was last clicked on.
    clicked: Option<usize>,
}

impl App for Page {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        if let Some(i) = self.reveal.take()
            && let Some(k) = ui.key_of(&format!("line:{i}"))
        {
            ui.core().set_devtools_selected(Some(k));
        }
        if std::mem::take(&mut self.pick) {
            ui.core().set_devtools_pick(true);
        }
        ui.with(
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .pad(16.0)
                .gap(6.0),
            |ui| {
                ui.text(
                    "the source — each line is a node the Inspector tab can select in the panel's tree",
                    TextStyle::new(12.0).color(t.muted),
                );
                for (i, line) in LINES.iter().enumerate() {
                    let on = self.clicked == Some(i);
                    ui.with_keyed(
                        &format!("line:{i}"),
                        NodeSpec::row()
                            .width(Sizing::Grow(1.0))
                            .pad_xy(8.0, 3.0)
                            .radius(4.0)
                            .bg(if on { t.accent_soft } else { t.surface })
                            .hover_bg(t.hover)
                            .on_click(Value::map([
                                ("kind", Value::str("line")),
                                ("i", Value::Int(i as i64)),
                            ]))
                            .label(format!("line {i}").as_str()),
                        |ui| {
                            ui.text(
                                line,
                                TextStyle::new(13.0).mono().color(t.fg),
                            );
                        },
                    );
                }
                ui.text(
                    &format!("Inspector built {} time(s) — only while its tab is on show", self.built),
                    TextStyle::new(12.0).color(t.muted),
                );
            },
        );
        // The tab: declared every frame, drawn only while it is on show.
        let built = &mut self.built;
        ui.devtools_tab_with("inspector", "Inspector", |ui| {
            *built += 1;
            let t = ui.theme();
            let name = |ui: &mut Ui<'_>, k: Option<Key>| match k {
                Some(k) => ui
                    .core()
                    .label_of(k)
                    .map_or_else(|| format!("{:08x}", k.0 as u32), str::to_string),
                None => "—".into(),
            };
            let (s, h, p) = (
                ui.devtools_selected(),
                ui.devtools_hovered(),
                ui.devtools_picked(),
            );
            let selected = name(ui, s);
            let hovered = name(ui, h);
            let picked = name(ui, p);
            let picking = ui.devtools_picking();
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0))
                    .gap(6.0)
                    .scroll_y(),
                |ui| {
                    ui.text(
                        "the panel's facts, read through the doors",
                        TextStyle::new(11.0).color(t.muted),
                    );
                    for (k, v) in [
                        ("selected", &selected),
                        ("hovered", &hovered),
                        ("picked", &picked),
                    ] {
                        ui.with(NodeSpec::row().gap(8.0), |ui| {
                            ui.with(NodeSpec::row().width(Sizing::Fixed(64.0)), |ui| {
                                ui.text(k, TextStyle::new(12.0).color(t.muted));
                            });
                            ui.text(v, TextStyle::new(12.0).mono().color(t.fg));
                        });
                    }
                    // The picker, raised from here: the pick lands in
                    // `selected` above and this tab stays up.
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
                        "select a line in the panel's tree from here",
                        TextStyle::new(11.0).color(t.muted),
                    );
                    for (i, line) in LINES.iter().enumerate() {
                        widgets::button(
                            ui,
                            &format!("select line {i}"),
                            Value::map([
                                ("kind", Value::str("reveal")),
                                ("i", Value::Int(i as i64)),
                            ]),
                        );
                        ui.text(line.trim(), TextStyle::new(11.0).mono().color(t.muted));
                    }
                },
            );
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        let i = ev
            .payload
            .get("i")
            .and_then(Value::as_int)
            .map(|i| i as usize);
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("line") => self.clicked = i,
            Some("reveal") => self.reveal = i,
            Some("pick") => self.pick = true,
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
            "pick a node; the Inspector reads it as `picked`",
        ),
    ];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(620.0, 360.0)
    }

    /// The tab is declared and not built while another is up; on show it
    /// is built once a frame, over the panel's body, and its button
    /// selects the page's line in the panel's tree.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        // The bare core the drive gets has no panel: on, docked right, as
        // the harness's window would have it.
        core.set_devtools(true);
        core.set_devtools_dock(kui::DevtoolsDock::Right);
        core.set_inspect(true);
        let mut d = Drive::new(core, 620.0, 360.0);
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
                kui::KeyMods {
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
        let button = d.key_of("select line 2").ok_or("no button in the tab")?;
        let r = d.rect_of(button).ok_or("no button rect")?;
        d.check(
            r.x >= body.x && r.x + r.w <= body.x + body.w && r.y >= body.y,
            "the tab's content is laid out over the panel's body",
        )?;
        d.click_key(self, button);
        d.frame(self);
        d.frame(self);
        let line = d.key_of("line:2").ok_or("no line")?;
        d.check(
            d.core.devtools_selected() == Some(line),
            "the tab's button selected the page's line in the panel's tree",
        )?;
        // The picker, raised from the tab: over the app, and the tab stays.
        let pick = d.key_of("pick a node").ok_or("no pick button")?;
        d.click_key(self, pick);
        d.frame(self);
        d.check(d.core.devtools_picking(), "the tab raised the picker")?;
        d.frame(self);
        let up = d.key_of("kui-devtools/tab/inspector").is_some();
        d.check(up, "and stayed up while picking")?;
        // The picker reads the node under the pointer from a frame, then
        // the press lands it: move, frame, press.
        let line0 = d.key_of("line:0").ok_or("no line")?;
        d.hover(self, line0);
        d.frame(self);
        d.check(
            d.core.devtools_picked() == Some(line0),
            "the line is under the picker",
        )?;
        let r = d.rect_of(line0).ok_or("no rect")?;
        d.click(self, r.x + 4.0, r.y + r.h / 2.0);
        d.frame(self);
        d.check(
            !d.core.devtools_picking() && d.core.devtools_selected() == Some(line0),
            "the press picked the line into `selected`",
        )?;
        let up = d.key_of("kui-devtools/tab/inspector").is_some();
        d.check(up, "and the tab is still the one on show")?;
        chord(&mut d, self); // facts
        let runs = self.built;
        d.frame(self);
        d.check(self.built == runs, "another tab up: the closure rests")?;
        Ok(())
    }
}

kui_devtools::main!(Page::default());
