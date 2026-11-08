//! The counter model, view and event handler shared by both hosts.

use kui_core::widgets;
use kui_core::{Align, Event, FloatConfig, Message, NodeSpec, TextStyle, Ui, UiEvent};

#[derive(Message, Clone, Debug, PartialEq)]
#[message(crate = "kui_core")]
enum Msg {
    Inc,
    Dec,
    Reset,
    Add10,
    /// The message on the root's `on_context_menu` and on the menu's
    /// `modal`: the press that asks for the menu, and the `dismiss` that
    /// ends it, both arrive as it, told apart by the event beside it.
    Menu,
}

#[derive(Default)]
pub(crate) struct Counter {
    pub(crate) count: i64,
    /// Where the menu is, in viewport px, while it is open.
    pub(crate) menu: Option<(f32, f32)>,
}

impl Counter {
    pub(crate) fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        // The whole example answers a secondary press. Any node can: the
        // topmost one that declared `on_context_menu` is the one asked, so
        // a row inside could offer its own menu and win over this one.
        ui.with(
            NodeSpec::column()
                .fill()
                .center()
                .gap(24.0)
                .on_context_menu(Msg::Menu),
            |ui| {
                ui.with(
                    NodeSpec::column()
                        .pad(32.0)
                        .gap(20.0)
                        .bg(t.surface)
                        .radius(12.0)
                        .border(1.0, t.border)
                        .cross_align(Align::Center)
                        .width(320.0),
                    |ui| {
                        ui.text("kui counter", TextStyle::new(14.0).color(t.muted));
                        ui.text(&self.count.to_string(), TextStyle::new(56.0));
                        ui.with(NodeSpec::row().gap(12.0), |ui| {
                            widgets::button(ui, "-1", Msg::Dec);
                            widgets::button(ui, "+1", Msg::Inc);
                            widgets::button(ui, "reset", Msg::Reset);
                        });
                    },
                );
                ui.text(
                    "right-click for a menu · clicks are data: view() never sees a callback",
                    TextStyle::new(12.0).color(t.faint),
                );
            },
        );
        // Declared last: floats stack in declaration order. One `modal`
        // row is the whole difference between this and a plain float —
        // Tab is scoped to it, the pointer cannot reach what is behind
        // it, and Escape or a press outside comes back as `dismiss`.
        if let Some((x, y)) = self.menu {
            ui.with_keyed(
                "menu",
                NodeSpec::column()
                    .float(
                        FloatConfig::viewport()
                            .inside(Align::Start, Align::Start)
                            .offset(x, y)
                            // Opened near an edge, the menu would hang off
                            // the window; `fit` mirrors it back instead.
                            .fit(),
                    )
                    .modal(Msg::Menu)
                    .label("Actions")
                    .width(120.0)
                    .pad(4.0)
                    .gap(4.0)
                    .bg(t.raised)
                    .border(1.0, t.border_strong)
                    .radius(6.0),
                |ui| {
                    widgets::button(ui, "+10", Msg::Add10);
                    widgets::button(ui, "reset", Msg::Reset);
                },
            );
        }
    }

    pub(crate) fn on_event(&mut self, ev: UiEvent) {
        match ev.split::<Msg>() {
            (Some(Msg::Inc), _) => self.count += 1,
            (Some(Msg::Dec), _) => self.count -= 1,
            (Some(Msg::Reset), _) => {
                self.count = 0;
                self.menu = None;
            }
            (Some(Msg::Add10), _) => {
                self.count += 10;
                self.menu = None;
            }
            // One message, two events: a right-click — the core reports
            // where it landed and opens nothing; the next frame's view is
            // what puts a menu there — and a `dismiss`, Escape or a press
            // outside the menu, where the core asks and the app decides.
            // This one just closes.
            (Some(Msg::Menu), Event::ContextMenu(at)) => self.menu = Some((at.x, at.y)),
            (Some(Msg::Menu), _) => self.menu = None,
            (None, _) => {}
        }
    }
}
