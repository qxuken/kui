//! A node that blurs what is drawn beneath it (backlog F129): CSS's
//! `backdrop-filter: blur()`, `NodeSpec::backdrop_blur`. A page of bright
//! cards scrolls under a frosted toolbar and a frosted badge; each is a
//! translucent `bg` over a blur of whatever the page drew there, so the
//! cards smear as they pass under the glass and stay sharp beside it.
//!
//! What blurs is what the app painted before the node — the page, its
//! cards, and in a window with a `backdrop` the desktop showing through —
//! and nothing of the node's own: its `bg`, its border and its children
//! lie on top. `--radius N` sets the toolbar's blur (16 by default),
//! `--backdrop blur` puts the window's own backdrop under the page too.
//!
//! Run: cargo run -p kui-native --example backdrop_blur -- --dock off
//! Run: cargo run -p kui-native --example backdrop_blur -- --headless

use kui_devtools::{Drive, Example};
use kui_native::{
    Align, App, Backdrop, Color, Core, FloatConfig, NodeSpec, QuadKind, Sizing, TextStyle, Ui,
};

struct Glass {
    radius: f32,
    backdrop: Backdrop,
}

/// The page's colours, bright and different enough that a blur shows.
const CARDS: [u32; 6] = [
    0xe8505bff, 0xf9a03fff, 0xf6d55cff, 0x3caea3ff, 0x20639bff, 0x8e5bd6ff,
];

impl App for Glass {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let env = ui.env();
        // Over a window backdrop the page is translucent too, so the blur
        // takes in the desktop as well as the cards.
        let page = if env.window.backdrop.is_translucent() {
            t.bg.with_alpha(0.55)
        } else {
            t.bg
        };
        ui.with_keyed("page", NodeSpec::column().fill().bg(page), |ui| {
            ui.with_keyed(
                "scroll",
                NodeSpec::column().fill().scroll_y().pad(16.0).gap(12.0),
                |ui| {
                    // The first row starts under the toolbar, as a scrolled
                    // page's rows pass under it.
                    for row in 0..8 {
                        ui.with(NodeSpec::row().grow_width().gap(12.0), |ui| {
                            for col in 0..3 {
                                let c = CARDS[(row * 3 + col) % CARDS.len()];
                                ui.with(
                                    NodeSpec::column()
                                        .grow_width()
                                        .height(96.0)
                                        .radius(12.0)
                                        .pad(12.0)
                                        .bg(Color::hex(c)),
                                    |ui| {
                                        ui.text(
                                            &format!("Card {}", row * 3 + col + 1),
                                            TextStyle::new(18.0).color(Color::WHITE),
                                        );
                                    },
                                );
                            }
                        });
                    }
                },
            );
            // The toolbar: a quarter-white wash over a blur of the page.
            ui.with_keyed(
                "toolbar",
                NodeSpec::row()
                    .float(FloatConfig::parent().inside(Align::Start, Align::Start))
                    .width(Sizing::Percent(1.0))
                    .height(56.0)
                    .pad(16.0)
                    .cross_align(Align::Center)
                    .bg(Color::WHITE.with_alpha(0.25))
                    .backdrop_blur(self.radius),
                |ui| {
                    ui.text(
                        &format!("Frosted, blur {}", self.radius),
                        TextStyle::new(18.0).color(t.fg),
                    );
                },
            );
            // A rounded badge, its own blur clipped to its corners.
            ui.with_keyed(
                "badge",
                NodeSpec::column()
                    .float(
                        FloatConfig::parent()
                            .inside(Align::End, Align::End)
                            .offset(-24.0, -24.0),
                    )
                    .size(180.0, 120.0)
                    .radius(24.0)
                    .pad(16.0)
                    .border(1.0, Color::WHITE.with_alpha(0.5))
                    .bg(Color::hex(0x10121a40))
                    .backdrop_blur(24.0),
                |ui| {
                    ui.text("Badge", TextStyle::new(16.0).color(Color::WHITE));
                    ui.text("blur 24", TextStyle::new(13.0).color(Color::WHITE));
                },
            );
        });
    }
}

impl Example for Glass {
    const FLAGS: &'static [(&'static str, &'static str)] = &[
        ("--radius", "the toolbar's blur in px (16)"),
        (
            "--backdrop",
            "the window's own backdrop under the page: opaque (the default), blur, tinted or transparent",
        ),
    ];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default()
            .size(720.0, 480.0)
            .backdrop(self.backdrop)
    }

    /// What a renderer is handed: one backdrop quad per glass node, before
    /// its own background, carrying the radius and the node's corners.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 720.0, 480.0);
        d.frame(self);
        let quads = d.core.output().0.quads.clone();
        let blurs: Vec<_> = quads
            .iter()
            .enumerate()
            .filter(|(_, q)| q.kind == QuadKind::Backdrop)
            .collect();
        d.check(blurs.len() == 2, "the toolbar and the badge each blur")?;
        let (i, toolbar) = blurs[0];
        d.check(
            toolbar.blur == self.radius && toolbar.rect.w == 720.0,
            "the toolbar's blur, the window wide",
        )?;
        d.check(
            quads[i + 1].kind == QuadKind::Solid,
            "its translucent wash over the blur",
        )?;
        let badge = blurs[1].1;
        d.check(
            badge.radius == [24.0; 4],
            "the badge's blur keeps its corners",
        )
    }
}

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

kui_devtools::main!(Glass {
    radius: arg("--radius").and_then(|r| r.parse().ok()).unwrap_or(16.0),
    backdrop: arg("--backdrop")
        .and_then(|b| Backdrop::from_name(&b))
        .unwrap_or(Backdrop::Opaque),
});
