//! Custom chrome: `widgets::titlebar`, `titlebar_with` and
//! `window_buttons`, in the window they are for. The window opens with
//! `Chrome::Custom`, so the OS draws no titlebar of its own and the app's
//! first row is one — a full-width drag strip with the title, and the
//! window controls where the platform does not draw them itself.
//!
//! The strip reads `env.window` and adapts by itself: under macOS custom
//! chrome it insets past the native traffic lights
//! (`env.window.native_controls` says where they are) and draws no
//! buttons; elsewhere it appends minimize, maximize and close, and the
//! maximize glyph follows `env.window.maximized`. `titlebar_with` puts the
//! app's own content — tabs here — between the inset and the buttons;
//! interactive children inside it win hit-testing, so the tabs click and
//! the rest of the strip drags. `window_buttons` alone is the cluster
//! without the strip. The status line reads back the window facts the
//! strip read.
//!
//! The pin is the other thing a window with its own chrome wants (backlog
//! C30): `ui.always_on_top(pinned)` is declared every frame the app wants
//! the window above every other app's, and the frame that stops declaring
//! it is what lowers the window again — so the button toggles the app's
//! own flag and undoes nothing. Its label is drawn from
//! `env.window.always_on_top`, what the platform actually did, not from
//! the flag: a window manager can refuse or drop the level, and on Wayland
//! there is none to ask for.
//!
//! Run: cargo run -p kui-native --example titlebar

use kui_devtools::Example;
use kui_native::widgets;
use kui_native::{Align, App, NodeSpec, TextStyle, Ui, UiEvent, Value};

const TABS: [&str; 3] = ["main.rs", "layout.rs", "README"];

#[derive(Default)]
struct Chrome {
    tab: usize,
    /// The app's ask; what the window has is `env.window.always_on_top`.
    pinned: bool,
}

impl App for Chrome {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let win = ui.env().window;
        ui.always_on_top(self.pinned);
        ui.with(NodeSpec::column().fill().bg(t.bg), |ui| {
            // The strip, with tabs in it: the tabs are clickable, the rest
            // drags the window.
            widgets::titlebar_with(ui, |ui| {
                let t = ui.theme();
                ui.with(
                    NodeSpec::row()
                        .fill()
                        .gap(4.0)
                        .cross_align(Align::End),
                    |ui| {
                        for (i, name) in TABS.iter().enumerate() {
                            let on = i == self.tab;
                            ui.text_in_keyed(name, NodeSpec::row()
                                    .pad_xy(12.0, 6.0)
                                    .radius_top(6.0)
                                    .bg(if on { t.bg } else { t.surface })
                                    .hover_bg(if on { t.bg } else { t.hover })
                                    .on_click(Value::map([
                                        ("kind", Value::str("tab")),
                                        ("name", Value::str(*name)),
                                    ])), name,
                                        TextStyle::new(12.0).color(if on { t.fg } else { t.muted }));
                        }
                    },
                );
            });
            ui.with(
                NodeSpec::column()
                    .fill()
                    .pad(24.0)
                    .gap(12.0),
                |ui| {
                    ui.text(TABS[self.tab], TextStyle::new(20.0));
                    ui.text(
                        "the strip above is the app's: drag it, double-click it, click a tab",
                        TextStyle::new(13.0).color(t.muted),
                    );
                    let facts = format!(
                        "custom_chrome {} · maximized {} · fullscreen {} · always_on_top {} · native controls {}",
                        win.custom_chrome,
                        win.maximized,
                        win.fullscreen,
                        win.always_on_top,
                        match win.native_controls {
                            Some(r) => format!("{}×{} at the origin (the strip insets past them)", r.w.round(), r.h.round()),
                            None => "none (the strip draws its own buttons)".into(),
                        }
                    );
                    ui.text(&facts, TextStyle::new(12.0).color(t.faint));
                    // The pin: the label is the window's answer, not the
                    // flag's, so a platform that refused shows "pin" still.
                    ui.with(NodeSpec::row().gap(8.0).cross_align(Align::Center), |ui| {
                        widgets::button(
                            ui,
                            if win.always_on_top { "unpin" } else { "pin" },
                            Value::map([("kind", Value::str("pin"))]),
                        );
                        ui.text(
                            if self.pinned == win.always_on_top {
                                "always_on_top follows the pin: the window stays above every other app's"
                            } else {
                                "asked, and the platform has not agreed (Wayland has no window level)"
                            },
                            TextStyle::new(12.0).color(t.muted),
                        );
                    });
                    ui.text(
                        "and the cluster alone, `widgets::window_buttons` — nothing where the OS draws them:",
                        TextStyle::new(12.0).color(t.muted),
                    );
                    ui.with(
                        NodeSpec::row()
                            .pad(6.0)
                            .bg(t.surface)
                            .radius(6.0)
                            .border(1.0, t.border),
                        |ui| {
                            widgets::window_buttons(ui);
                            if win.native_controls.is_some() || !win.custom_chrome {
                                ui.text("(the OS's are the buttons here)", TextStyle::new(11.0).color(t.faint));
                            }
                        },
                    );
                },
            );
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.kind() == Some("pin") {
            self.pinned = !self.pinned;
        }
        if let Some(name) = ev.payload.get_str("name")
            && let Some(i) = TABS.iter().position(|t| *t == name)
        {
            self.tab = i;
        }
    }
}

impl Example for Chrome {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        ("drag the strip", "move the window"),
        ("double-click it", "maximize (where the platform does)"),
        (
            "pin",
            "keep the window above every other app's; again to let go",
        ),
    ];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default()
            .size(640.0, 360.0)
            .custom_titlebar()
    }

    /// The dock below the strip, so the strip stays the window's top edge.
    fn dock(&self) -> kui_devtools::Dock {
        kui_devtools::Dock::Bottom
    }
}

kui_devtools::main!(Chrome::default());
