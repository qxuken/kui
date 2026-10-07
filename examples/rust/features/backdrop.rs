//! A window with the desktop behind it (backlog F126): a sidebar painted
//! translucent over the material the OS draws behind the window — the
//! blurred desktop of a macOS sidebar, Windows 11's Mica Alt — and a page
//! beside it painted opaque, the way a notes app's library and note sit.
//!
//! The app asks with `Launcher::backdrop` (`Window::backdrop` here) and
//! paints from what it got, `env.window.backdrop`: where the platform has
//! no material the sidebar is opaque too, and the page says which it got.
//! `--backdrop window|sidebar|transient|transparent|opaque` picks what to
//! ask for; `sidebar` by default.
//!
//! Run: cargo run -p kui-native --example backdrop -- --dock off

use kui_devtools::{Drive, Example};
use kui_native::widgets;
use kui_native::{App, Backdrop, Color, Core, Env, NodeSpec, Sizing, TextStyle, Ui};

struct Library {
    asked: Backdrop,
    /// The sidebar's alpha in the last frame, for the headless drive.
    sidebar_alpha: f32,
}

/// The sidebar's paint for what is behind the window: see-through over a
/// material, nearly opaque over the bare desktop (which is not blurred,
/// and text over it is a lottery), and the opaque `base` over nothing.
fn sidebar_bg(env: &Env, base: Color) -> Color {
    match env.window.backdrop {
        Backdrop::Opaque => base,
        Backdrop::Transparent => base.with_alpha(0.92),
        Backdrop::Window | Backdrop::Sidebar | Backdrop::Transient => base.with_alpha(0.45),
    }
}

impl App for Library {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let env = ui.env();
        let side = sidebar_bg(&env, t.raised);
        self.sidebar_alpha = side.a;
        ui.with(NodeSpec::row().fill(), |ui| {
            ui.with(
                NodeSpec::column()
                    .width(220.0)
                    .height(Sizing::GROW)
                    .pad(12.0)
                    .gap(6.0)
                    .bg(side),
                |ui| {
                    // The strip a custom titlebar leaves for the traffic
                    // lights, and a drag handle over the sidebar's top.
                    ui.leaf(NodeSpec::row().grow_width().height(28.0).window_drag());
                    for name in ["Inbox", "Projects", "Journal", "Archive"] {
                        ui.text(name, TextStyle::new(14.0).color(t.fg));
                    }
                },
            );
            ui.with(
                NodeSpec::column().fill().bg(t.bg).pad(16.0).gap(8.0),
                |ui| {
                    widgets::titlebar_with(ui, |ui| {
                        ui.text("Note", TextStyle::new(13.0).color(t.muted));
                    });
                    ui.text(
                        &format!(
                            "asked for {}, got {}",
                            self.asked.name(),
                            env.window.backdrop.name()
                        ),
                        TextStyle::new(16.0).color(t.fg),
                    );
                    ui.text(
                        "The library on the left is painted with alpha over what the \
                         window has behind it; this page is opaque.",
                        TextStyle::new(13.0).color(t.muted),
                    );
                },
            );
        });
    }
}

impl Example for Library {
    const FLAGS: &'static [(&'static str, &'static str)] = &[(
        "--backdrop",
        "window, sidebar (the default), transient, transparent or opaque",
    )];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default()
            .size(720.0, 420.0)
            .custom_titlebar()
            .backdrop(self.asked)
    }

    /// The paint follows what the window got, not what was asked: the
    /// sidebar is translucent over a material, nearly opaque over the bare
    /// desktop, and opaque where there is nothing behind the window.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 720.0, 420.0);
        d.frame(self);
        d.check(self.sidebar_alpha == 1.0, "a headless core has no backdrop")?;
        d.core.env.window.backdrop = Backdrop::Sidebar;
        d.frame(self);
        d.check(self.sidebar_alpha < 0.5, "translucent over a material")?;
        d.core.env.window.backdrop = Backdrop::Transparent;
        d.frame(self);
        d.check(
            self.sidebar_alpha > 0.9 && self.sidebar_alpha < 1.0,
            "nearly opaque over the bare desktop",
        )
    }
}

/// `--backdrop NAME`, or `sidebar`.
fn asked() -> Backdrop {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == "--backdrop")
        .and_then(|i| args.get(i + 1))
        .and_then(|name| Backdrop::from_name(name))
        .unwrap_or(Backdrop::Sidebar)
}

kui_devtools::main!(Library {
    asked: asked(),
    sidebar_alpha: 1.0,
});
