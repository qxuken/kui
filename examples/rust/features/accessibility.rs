//! Every accessibility prop in one window, and the fixture the platform
//! audit drives (`scripts/ax-audit.swift` on macOS, or a screen reader by
//! hand). Each control is built so that assistive technology can both
//! read it and change it, and see the change: the button's name counts
//! its presses, the sliders' values follow increment and decrement, and
//! both editors report the text they hold. The two sliders differ in one
//! row: `Volume` declares only its number, so a reader turns the position
//! into a percentage, while `Focus length` also declares a `value_text`
//! and is read as "25 minutes" (backlog F8).
//!
//! Run: cargo run -p kui-native --example accessibility
//!
//! With VoiceOver (⌘F5): VO-right walks the controls, VO-space presses
//! the button, and inside either editor the arrow keys read by character
//! and VO-arrows by word and line.
//!
//! With the keyboard alone (`docs/adr/0002-keyboard-focus-as-data.md`):
//! Tab walks every control in order — the button, the icon button, the
//! switch, the two sliders, the built-in editor, the app-owned editor — with a
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

use kui_devtools::Example;
use kui_native::widgets;
use kui_native::{
    Align, App, EditOptions, FloatConfig, Key, Live, NodeSpec, Role, TextStyle, Ui, UiEvent, Value,
};

const DOC: &str = "hello world\nsecond line";

struct A11y {
    presses: u32,
    volume: f32,
    /// Minutes, in [5..60] — the pomodoro report's own range. Its slider
    /// says what the number *reads as*; the volume slider above says only
    /// the number, so the window carries both readings side by side.
    focus_min: f32,
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
            focus_min: 25.0,
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
        let t = ui.theme();
        let text = TextStyle::new(14.0).color(t.fg);
        // The label colour for the hand-built buttons below: readable on
        // whatever `button_spec` actually carries, which is the rule
        // `widgets::button_with` applies. Not `t.on_accent` — none of
        // them declares `accent`, so their background stays the stock
        // blue however the OS's accent is set, and a light accent's
        // black label would land on that blue.
        let on_button =
            widgets::readable_on(widgets::button_spec(&ui.theme(), &ui.metrics()).style.bg);
        // No pad or gap on the root: the scrolling column below owns
        // both, so the scrollbar rides the window edge rather than
        // floating inside a margin.
        ui.open(NodeSpec::column().fill().bg(t.bg));
        // The drawn titlebar is part of the fixture: the audit checks that
        // a titlebar named like the window is read once, not twice.
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
                ui.text_in(
                    NodeSpec::row().role(Role::Heading),
                    "Controls",
                    TextStyle::new(20.0).color(t.fg),
                );

                // A tab list: `selected` is which one the view shows, and every
                // tab reports the state so a reader can say which is on. Nothing
                // here says "1 of 3" — the core counts what the list holds.
                ui.with_keyed("tabs", NodeSpec::row().role(Role::TabList).gap(4.0), |ui| {
                    for (i, name) in ["General", "Network", "About"].iter().enumerate() {
                        let on = i == self.tab;
                        ui.text_in_keyed(
                            name,
                            NodeSpec::row()
                                .role(Role::Tab)
                                .selected(on)
                                .on_click(Value::Int(i as i64))
                                .pad_xy(10.0, 6.0)
                                .bg(if on { t.accent } else { t.surface })
                                .radius(6.0),
                            name,
                            TextStyle::new(13.0).color(if on { t.on_accent } else { t.muted }),
                        );
                    }
                });

                // A disclosure: `expanded` names its state, so a reader says
                // "collapsed" rather than nothing at all when it is shut.
                ui.text_in_keyed(
                    "advanced",
                    widgets::button_spec(&ui.theme(), &ui.metrics())
                        .expanded(self.advanced)
                        .on_click("advanced")
                        .label("Advanced"),
                    if self.advanced {
                        "▾ Advanced"
                    } else {
                        "▸ Advanced"
                    },
                    TextStyle::new(widgets::BUTTON_TEXT).color(on_button),
                );
                if self.advanced {
                    ui.text_in(
                        NodeSpec::row().pad_xy(10.0, 6.0).bg(t.sunken).radius(6.0),
                        "Nothing here yet.",
                        text,
                    );
                }

                // A radio group: the one pattern whose arrows *must* also check
                // the radio they land on, which is why the group is here at all.
                // The stock group (`docs/adr/0034-stock-controls-over-the-roles.md`)
                // is the `radioGroup` container; each stock `radio` says
                // `checked`, and the group's `row` spec is what tells the
                // platform the set is laid out horizontally. Nothing declares a
                // Tab stop or an arrow key — the group holds focusable radios,
                // and that is a composite.
                ui.text_in(
                    NodeSpec::row().role(Role::Heading),
                    "Theme",
                    TextStyle::new(15.0).color(t.fg),
                );
                widgets::radio_group_with(ui, "Theme", NodeSpec::row().gap(16.0), |ui| {
                    for (i, name) in ["Light", "Dark", "Auto"].iter().enumerate() {
                        widgets::radio(ui, name, i == self.theme, format!("theme{i}"));
                    }
                });

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
                            ui.text_in_keyed(
                                name,
                                NodeSpec::row()
                                    .role(Role::ListItem)
                                    .selected(on)
                                    .focusable()
                                    .on_click(Value::str(format!("row{i}")))
                                    .width(200.0)
                                    .pad_xy(10.0, 5.0)
                                    .bg(if on { t.accent_pressed } else { t.surface })
                                    .radius(4.0),
                                name,
                                TextStyle::new(13.0).color(if on { t.on_accent } else { t.muted }),
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
                ui.text_in_keyed(
                    "count",
                    widgets::button_spec(&ui.theme(), &ui.metrics()).on_click("press"),
                    &format!("count {}", self.presses),
                    TextStyle::new(widgets::BUTTON_TEXT).color(on_button),
                );

                // An icon button: nothing to read inside, so it needs a label.
                ui.text_in_keyed(
                    "save",
                    widgets::button_spec(&ui.theme(), &ui.metrics())
                        .on_click("save")
                        .label("Save"),
                    "⌘",
                    TextStyle::new(15.0).color(on_button),
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
                ui.text_in_keyed(
                    "status",
                    NodeSpec::row()
                        .live(Live::Polite)
                        .pad_xy(10.0, 5.0)
                        .bg(t.sunken)
                        .radius(4.0),
                    &if self.saves == 0 {
                        "No changes saved".to_string()
                    } else if self.saves == 1 {
                        "Saved 1 change".to_string()
                    } else {
                        format!("Saved {} changes", self.saves)
                    },
                    text,
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
                                .modal("menu")
                                .role(Role::Menu)
                                .label("Actions")
                                .width(180.0)
                                .pad(4.0)
                                .gap(2.0)
                                .bg(t.surface)
                                .border(1.0, t.accent)
                                .radius(6.0),
                            |ui| {
                                for name in ["Rename", "Duplicate", "Archive"] {
                                    ui.text_in_keyed(
                                        name,
                                        NodeSpec::row()
                                            .role(Role::MenuItem)
                                            .on_click(Value::str(format!("menu:{name}")))
                                            .grow_width()
                                            .pad_xy(8.0, 5.0)
                                            .radius(4.0)
                                            .focus_bg(t.accent_soft),
                                        name,
                                        TextStyle::new(13.0).color(t.fg),
                                    );
                                }
                            },
                        );
                    }
                });

                // A switch: `checked` is the state assistive technology reads,
                // and its text is its name.
                widgets::switch(ui, "Mute", self.muted, "mute");

                // A stock slider: the value, the range and the step are data,
                // and every way of moving it — the pointer, the arrows, the
                // Page keys, Home and End, a reader's increment and decrement —
                // arrives as one `change` event proposing the new value.
                let m = ui.metrics();
                widgets::slider(ui, "Volume", self.volume, 0.0, 10.0, 1.0, "volume");

                // The same control, saying what its position *reads as*.
                // With only `value_now` and the range a reader has to
                // invent a reading and says a percentage — 25 in [5..60]
                // is "36 percent", which is what the pomodoro report hit
                // (backlog F8). `value_text` is the reading itself, and it
                // replaces the number rather than joining it. It is not
                // the `label`: the name of the control does not change
                // when its value does.
                // Five minutes a step, which is the step a reader's increment
                // and the arrows both take.
                widgets::slider_with(
                    ui,
                    "Focus length",
                    widgets::slider_spec(&m)
                        .value_now(self.focus_min)
                        .value_min(5.0)
                        .value_max(60.0)
                        .value_step(5.0)
                        .value_text(format!("{} minutes", self.focus_min as i32))
                        .on_change("focus"),
                    None,
                );

                // A built-in editor: the core owns the buffer, so its runs, caret
                // and selection come out of the edit store, and a screen reader's
                // selection and text requests are applied for you.
                ui.text_in(
                    NodeSpec::row().role(Role::Heading),
                    "Built-in editor",
                    TextStyle::new(15.0).color(t.fg),
                );
                self.edit = ui.text_edit(
                    "doc",
                    DOC,
                    &EditOptions {
                        multiline: true,
                        style: text,
                        ..Default::default()
                    },
                    NodeSpec::column()
                        .size(280.0, 56.0)
                        .pad(8.0)
                        .bg(t.sunken)
                        .radius(6.0)
                        .clip()
                        .label("Notes"),
                );

                // An editor the app owns: the sink says what it is, each drawn
                // row is a line of its text, and the caret rides along as a byte
                // offset. Text requests come back as `access` events.
                ui.text_in(
                    NodeSpec::row().role(Role::Heading),
                    "App-owned editor",
                    TextStyle::new(15.0).color(t.fg),
                );
                let (lines, caret) = (&self.lines, self.caret);
                sink = ui.with_keyed(
                    "code",
                    NodeSpec::column()
                        .width(280.0)
                        .pad(8.0)
                        .bg(t.sunken)
                        .radius(6.0)
                        .on_key("code")
                        .role(Role::MultilineTextInput)
                        .label("Source"),
                    |ui| {
                        ui.with(NodeSpec::row().gap(8.0), |ui| {
                            // Decoration: line numbers are not part of the text.
                            ui.with(NodeSpec::column().role(Role::None), |ui| {
                                for i in 0..lines.len() {
                                    ui.text(
                                        &format!("{}", i + 1),
                                        TextStyle::new(12.0).mono().color(t.faint),
                                    );
                                }
                            });
                            ui.with(NodeSpec::column(), |ui| {
                                for (i, line) in lines.iter().enumerate() {
                                    let mut row = NodeSpec::row().role(Role::Line);
                                    if caret.0 == i {
                                        row = row.caret(caret.1.min(line.len()) as u32);
                                    }
                                    ui.text_in_keyed(
                                        &format!("l{i}"),
                                        row,
                                        line,
                                        TextStyle::new(13.0).mono().color(text.color_or_default()),
                                    );
                                }
                            });
                        });
                    },
                );
            },
        );
        ui.take_key_focus(sink);

        // The modal, declared last so it floats over everything. One row
        // makes it modal:
        // focus enters it, Tab cannot leave, nothing behind it takes
        // input, and a screen reader announces a dialog and stays inside.
        if self.dialog {
            ui.with_keyed(
                "confirm",
                NodeSpec::column()
                    .float(FloatConfig::viewport().inside(Align::Center, Align::Center))
                    .modal("confirm")
                    .label("Delete note")
                    .width(260.0)
                    .gap(10.0)
                    .pad(14.0)
                    .bg(t.surface)
                    .border(1.0, t.accent)
                    .radius(8.0),
                |ui| {
                    ui.text("Delete this note?", TextStyle::new(15.0).color(t.fg));
                    ui.with(NodeSpec::row().gap(8.0), |ui| {
                        // Where focus lands when the dialog opens, said
                        // rather than inherited from declaration order: a
                        // destructive confirm opens on its safe option, so
                        // Enter out of habit cancels. Without the row the
                        // ring's first node wins, which is Cancel here only
                        // because Cancel happens to be declared first — and
                        // that is exactly the thing an app should not have
                        // to keep true by hand.
                        ui.text_in_keyed(
                            "Cancel",
                            widgets::button_spec(&ui.theme(), &ui.metrics())
                                .on_click("cancel")
                                .initial_focus(),
                            "Cancel",
                            TextStyle::new(widgets::BUTTON_TEXT).color(on_button),
                        );
                        widgets::button(ui, "Delete", Value::str("delete"));
                    });
                },
            );
        }
        ui.close();
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
        if payload.get_str("kind") == Some("dismiss") {
            let reason = payload.get_str("reason").unwrap_or("");
            // Two modals now, so the tag says which one asked to go.
            let which = payload.get_str("tag").unwrap_or("");
            println!("dismiss {which} ({reason})");
            match which {
                "menu" => self.menu = false,
                _ => self.dialog = false,
            }
            return;
        }
        // The sliders: the core proposes a value — stepped, clamped and
        // snapped already — and which slider moved is its own tag. The
        // reading the next frame declares is what a reader announces.
        if payload.get_str("kind") == Some("change") {
            let value = payload.get_float("value").unwrap_or(0.0) as f32;
            match payload.get_str("tag") {
                Some("focus") => {
                    self.focus_min = value;
                    println!("focus length -> {} minutes", self.focus_min as i32);
                }
                _ => {
                    self.volume = value;
                    println!("volume -> {}", self.volume);
                }
            }
            return;
        }
        if payload.get_str("kind") != Some("access") {
            return;
        }
        let action = payload.get_str("action").unwrap_or("");
        match action {
            // The app-owned editor: line ordinals among the rows it drew
            // (all of them here), byte offsets into their text.
            "setTextSelection" => {
                if let Some(f) = payload.get("focus") {
                    let line = f.get_int("line").unwrap_or(0) as usize;
                    let offset = f.get_int("offset").unwrap_or(0) as usize;
                    self.caret = (line, offset);
                    println!("caret -> {line}:{offset}");
                }
            }
            "replaceSelectedText" | "setValue" => {
                let text = payload.get_str("text").unwrap_or("");
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

impl Example for A11y {
    /// Tall enough for most of the controls; the rest are a scroll away,
    /// because the column holding them scrolls (see `view`). Custom
    /// chrome, so the drawn titlebar the audit checks is there.
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default()
            .size(560.0, 820.0)
            .custom_titlebar()
    }

    /// The fixture is the window the audit walks, and nothing else: no
    /// dock, no sink of the harness's, no focus it took. `--dock side`
    /// puts the readout beside it for a look by hand.
    fn dock(&self) -> kui_devtools::Dock {
        kui_devtools::Dock::Off
    }
}

kui_devtools::main!(A11y::new());
