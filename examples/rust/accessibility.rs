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
//! Four things here are **composites**
//! (`docs/adr/0007-composite-keyboard-patterns.md`): the tab list, the
//! theme radio group, the mailbox list and the Actions menu are **one**
//! Tab stop each, not one per item. Tab enters on the item that is
//! selected (or checked, or first), and inside it the arrow keys — both
//! pairs — move the selection, Home and End reach the ends, and typing a
//! name jumps to it. The tab list and the radio group *activate* as focus
//! moves, because that is what those two patterns mean on every platform;
//! the list and the menu leave activation to Enter or Space. Nothing
//! below declares any of that: the core derives a composite from a
//! container role whose items are focusable, so the tab list and the list
//! read exactly as they did before this and behave differently.
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
//! opened it. It opens on Cancel, because Cancel says `initialFocus`:
//! a destructive confirm should not be one habitual Enter away from
//! confirming.

use kui::widgets;
use kui::{
    Align, App, Color, EditOptions, FloatConfig, Key, Live, NodeSpec, Role, Sizing, TextStyle, Ui,
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
    /// The checked radio of the theme group. `checked`, not `selected`:
    /// a radio is on or off the way a checkbox is, and the *group* is
    /// what makes it one of a set.
    theme: usize,
    /// Whether the Actions menu is declared this frame. Like `dialog`,
    /// nothing else: a menu is a modal float that the app stops
    /// declaring.
    menu: bool,
    /// The live region's message, and how many saves are behind it. This
    /// is state a view reads, like everything else here — the region is
    /// `live`, so a reader hears it *because it changed*, without being
    /// asked and without the app saying "now".
    saves: u32,
    /// The other half: something to say once, with nothing on screen
    /// holding it. `on_event` takes no `Ui`, so the handler leaves it
    /// here and the view announces it and clears it — the guard the core
    /// reports as `announcement-repeated` when an app forgets it
    /// (`docs/adr/0008-live-regions-and-announcements.md`).
    pending: Option<String>,
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
            theme: 1,
            menu: false,
            saves: 0,
            pending: None,
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
        // No pad or gap on the root: the scrolling column below owns
        // both, so the scrollbar rides the window edge rather than
        // floating inside a margin.
        ui.configure_root(NodeSpec::column().fill().bg(Color::rgb8(0x11, 0x13, 0x1a)));
        widgets::titlebar(ui, "kui — accessibility");

        // Everything but the chrome scrolls. Every control below is in one
        // column, and a column that overflows squeezes its children —
        // squeezed rows are exactly what a fixture must not show when the
        // point of it is that they read correctly. Scrolling keeps them
        // their own size on a window too short for all of them, and gives
        // assistive technology a scroll view it can move (`ScrollIntoView`
        // arrives as an access request and the core applies it).
        //
        // The modal and the latency HUD stay outside it: both are floats,
        // and a float inside a clipping container is clipped by it.
        let mut sink = Key::ROOT;
        ui.with_keyed(
            "content",
            NodeSpec::column().fill().scroll_y().gap(10.0).pad(14.0),
            |ui| {
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

                // A radio group: the one pattern whose arrows *must* also check
                // the radio they land on, which is why the group is here at all.
                // `radioGroup` is the container; each `radio` says `checked`, and
                // the group's own `dir` is what tells the platform the set is
                // laid out horizontally. Nothing declares a Tab stop or an arrow
                // key — the group holds focusable radios, and that is a composite.
                ui.with(NodeSpec::row().role(Role::Heading), |ui| {
                    ui.text("Theme", TextStyle::new(15.0).color(Color::WHITE))
                });
                ui.with_keyed(
                    "theme",
                    NodeSpec::row()
                        .role(Role::RadioGroup)
                        .gap(4.0)
                        .label("Theme"),
                    |ui| {
                        for (i, name) in ["Light", "Dark", "Auto"].iter().enumerate() {
                            let on = i == self.theme;
                            ui.with_keyed(
                                name,
                                NodeSpec::row()
                                    .role(Role::Radio)
                                    .checked(on)
                                    .on_click(Value::str(format!("theme{i}")))
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
                    },
                );

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

                // A one-off announcement: nothing on screen says "Copied", and
                // nothing should — a reader hears it, everyone else sees the
                // button they just pressed. Announced here rather than in
                // `on_event` because `App::on_event` takes no `Ui`; the `take`
                // is the guard, since a view runs every frame.
                if let Some(msg) = self.pending.take() {
                    ui.announce(&msg, Live::Polite);
                }
                widgets::button(ui, "Copy", Value::str("copy"));

                // The live region. A plain box, so without the row it would be
                // elided and its text would be read only when asked for; with it
                // the box stays in the tree as a group, its liveness reaches the
                // text inside, and a reader announces the count each time it
                // changes. `polite` waits for a pause — `assertive` would
                // interrupt, which a save confirmation has not earned.
                ui.with_keyed(
                    "status",
                    NodeSpec::row()
                        .live(Live::Polite)
                        .pad_xy(10.0, 5.0)
                        .bg(Color::rgb8(0x0e, 0x10, 0x16))
                        .radius(4.0),
                    |ui| {
                        ui.text(
                            &if self.saves == 0 {
                                "No changes saved".to_string()
                            } else if self.saves == 1 {
                                "Saved 1 change".to_string()
                            } else {
                                format!("Saved {} changes", self.saves)
                            },
                            text,
                        )
                    },
                );

                // The button that opens the modal below.
                widgets::button(ui, "Delete…", Value::str("open-confirm"));

                // A menu, opened from a button: `modal` plus `role="menu"`, which
                // is the shape a context menu takes. It floats below its trigger,
                // so the plain box around the pair is what the float anchors to —
                // a button is read as one control, and a menu declared inside it
                // would be read as part of that control rather than as a menu.
                // Focus enters it, the arrows move between its items *without*
                // running them (a menu that ran whatever you passed over would be
                // unusable), and Enter, Space or Escape ends it.
                ui.with(NodeSpec::column(), |ui| {
                    widgets::button(ui, "Actions ▾", Value::str("open-menu"));
                    if self.menu {
                        ui.with_keyed(
                            "menu",
                            NodeSpec::column()
                                // Left-aligned under the button and clamped
                                // into the window: `below()` alone centres a
                                // float on its anchor — right for a tooltip,
                                // which is what it is for — so a 180-wide
                                // menu under a 90-wide button near the left
                                // edge starts outside the window, and without
                                // `fit` nothing pulls it back. A menu belongs
                                // under the left edge of the control that
                                // opened it, and `fit` is the in-window
                                // answer ADR 0004 decision 11 keeps: this one
                                // does fit, once it is placed properly.
                                .float(
                                    FloatConfig::below()
                                        .at(Align::Start, Align::End)
                                        .self_at(Align::Start, Align::Start)
                                        .fit(),
                                )
                                .modal(Value::str("menu"))
                                .role(Role::Menu)
                                .label("Actions")
                                .width(Sizing::Fixed(180.0))
                                .pad(4.0)
                                .gap(2.0)
                                .bg(Color::rgb8(0x1d, 0x20, 0x2b))
                                .border(1.0, Color::rgb8(0x3b, 0x5b, 0xd4))
                                .radius(6.0),
                            |ui| {
                                for name in ["Rename", "Duplicate", "Archive"] {
                                    ui.with_keyed(
                                        name,
                                        NodeSpec::row()
                                            .role(Role::MenuItem)
                                            .on_click(Value::str(format!("menu:{name}")))
                                            .width(Sizing::Grow(1.0))
                                            .pad_xy(8.0, 5.0)
                                            .radius(4.0)
                                            .focus_bg(Color::rgb8(0x3b, 0x5b, 0xd4)),
                                        |ui| {
                                            ui.text(
                                                name,
                                                TextStyle::new(13.0)
                                                    .color(Color::rgb8(0xd6, 0xd8, 0xe0)),
                                            )
                                        },
                                    );
                                }
                            },
                        );
                    }
                });

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
                sink = ui.with_keyed(
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
                                        ui.text(
                                            line,
                                            TextStyle::new(13.0).mono().color(text.color),
                                        );
                                    });
                                }
                            });
                        });
                    },
                );
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
                        // Where focus lands when the dialog opens, said
                        // rather than inherited from declaration order: a
                        // destructive confirm opens on its safe option, so
                        // Enter out of habit cancels. Without the row the
                        // ring's first node wins, which is Cancel here only
                        // because Cancel happens to be declared first — and
                        // that is exactly the thing an app should not have
                        // to keep true by hand.
                        ui.with_keyed(
                            "Cancel",
                            widgets::button_spec()
                                .on_click(Value::str("cancel"))
                                .initial_focus(),
                            |ui| {
                                ui.text(
                                    "Cancel",
                                    TextStyle::new(widgets::BUTTON_TEXT).color(Color::WHITE),
                                )
                            },
                        );
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
                // Changes the live region's text; nothing announces.
                self.saves += 1;
                println!("save -> {}", self.saves);
                return;
            }
            Some("copy") => {
                // Changes nothing on screen, so the announcement is the
                // only thing a reader gets.
                self.pending = Some("Copied to clipboard".to_string());
                println!("copy");
                return;
            }
            // Opening the dialog is a field the view reads; closing it is
            // the same field. The core moves focus into it and hands
            // focus back to this button when it goes away.
            Some("open-confirm") => {
                self.dialog = true;
                return;
            }
            Some("open-menu") => {
                self.menu = true;
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
        // A menu item runs and the menu goes away — the app closes it,
        // as it opened it. Prefixed like the rows, for the same reason.
        if let Some(item) = payload.as_str().and_then(|s| s.strip_prefix("menu:")) {
            self.menu = false;
            println!("menu -> {item}");
            return;
        }
        // The radios. Arrow keys reach here too, unchanged: moving focus
        // inside a radio group emits the radio's own click payload, which
        // is the whole of what decision 11 buys an app.
        if let Some(i) = payload
            .as_str()
            .and_then(|s| s.strip_prefix("theme"))
            .and_then(|s| s.parse::<usize>().ok())
        {
            self.theme = i;
            println!("theme -> {}", self.theme);
            return;
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
            // Two modals now, so the tag says which one asked to go.
            let which = payload.get("tag").and_then(Value::as_str).unwrap_or("");
            println!("dismiss {which} ({reason})");
            match which {
                "menu" => self.menu = false,
                _ => self.dialog = false,
            }
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
        // Tall enough for most of the controls; the rest are a scroll
        // away, because the column holding them scrolls (see `view`).
        .size(560.0, 820.0)
        .run(A11y::new())
}
