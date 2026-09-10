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
//! Run: cargo run -p kui --example titlebar

use kui::widgets;
use kui::{Align, App, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};
use kui_harness::Example;

const TABS: [&str; 3] = ["main.rs", "layout.rs", "README"];

#[derive(Default)]
struct Chrome {
    tab: usize,
}

impl App for Chrome {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let win = ui.env().window;
        ui.with(NodeSpec::column().fill().bg(t.bg), |ui| {
            // The strip, with tabs in it: the tabs are clickable, the rest
            // drags the window.
            widgets::titlebar_with(ui, |ui| {
                let t = ui.theme();
                ui.with(
                    NodeSpec::row()
                        .width(Sizing::Grow(1.0))
                        .height(Sizing::Grow(1.0))
                        .gap(4.0)
                        .cross_align(Align::End),
                    |ui| {
                        for (i, name) in TABS.iter().enumerate() {
                            let on = i == self.tab;
                            ui.with_keyed(
                                name,
                                NodeSpec::row()
                                    .pad_xy(12.0, 6.0)
                                    .radius_top(6.0)
                                    .bg(if on { t.bg } else { t.surface })
                                    .hover_bg(if on { t.bg } else { t.hover })
                                    .on_click(Value::Int(i as i64)),
                                |ui| {
                                    ui.text(
                                        name,
                                        TextStyle::new(12.0).color(if on { t.fg } else { t.muted }),
                                    );
                                },
                            );
                        }
                    },
                );
            });
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0))
                    .pad(24.0)
                    .gap(12.0),
                |ui| {
                    ui.text(TABS[self.tab], TextStyle::new(20.0));
                    ui.text(
                        "the strip above is the app's: drag it, double-click it, click a tab",
                        TextStyle::new(13.0).color(t.muted),
                    );
                    let facts = format!(
                        "custom_chrome {} · maximized {} · fullscreen {} · native controls {}",
                        win.custom_chrome,
                        win.maximized,
                        win.fullscreen,
                        match win.native_controls {
                            Some(r) => format!("{}×{} at the origin (the strip insets past them)", r.w.round(), r.h.round()),
                            None => "none (the strip draws its own buttons)".into(),
                        }
                    );
                    ui.text(&facts, TextStyle::new(12.0).color(t.faint));
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
        if let Some(i) = ev.payload.as_int() {
            self.tab = i as usize;
        }
    }
}

impl Example for Chrome {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        ("drag the strip", "move the window"),
        ("double-click it", "maximize (where the platform does)"),
    ];

    fn window(&self) -> kui_harness::Window {
        kui_harness::Window::default()
            .size(640.0, 360.0)
            .custom_titlebar()
    }

    /// The dock below the strip, so the strip stays the window's top edge.
    fn dock(&self) -> kui_harness::Dock {
        kui_harness::Dock::Bottom
    }
}

kui_harness::main!(Chrome::default());
