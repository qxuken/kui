//! Where the free space goes and what lines up (backlog C13), and a box
//! that keeps its shape (C14). Three panels:
//!
//!   - **mainAlign**: a track of four chips of different widths, laid out
//!     by whichever alignment the buttons above it pick — the three that
//!     put the chips together, and the three spreads that deal the free
//!     space out between them (`space-between`, `-around`, `-evenly`).
//!   - **crossAlign: baseline**: a reading and its unit in three sizes,
//!     once aligned at the top and once on their baselines, where the
//!     small label and unit sit on the line the large number stands on.
//!   - **aspectRatio**: a `grow`-wide card at 16:9 that keeps its shape as
//!     the window is resized, and a row of squares sized from a fixed
//!     height alone.
//!
//! Run: cargo run -p kui --example align [-- --headless]

use kui::widgets;
use kui::{Align, App, Color, Core, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};
use kui_devtools::{Drive, Example};

/// The alignments the buttons pick, in `schema::ALIGNS` order; `baseline`
/// is a cross-axis value and has no button here.
const MAIN: [Align; 6] = [
    Align::Start,
    Align::Center,
    Align::End,
    Align::SpaceBetween,
    Align::SpaceAround,
    Align::SpaceEvenly,
];

const CHIPS: [f32; 4] = [48.0, 72.0, 36.0, 60.0];

struct Page {
    main: Align,
}

impl Page {
    fn main_panel(&self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let m = ui.metrics();
        ui.text("mainAlign", TextStyle::new(15.0).color(t.fg));
        ui.with(NodeSpec::row().gap(6.0).wrap().cross_gap(6.0), |ui| {
            for a in MAIN {
                let spec = widgets::button_spec(&t, &m).on_click(Value::str(a.name()));
                let spec = if a == self.main {
                    spec.accent()
                } else {
                    spec.bg(t.surface).hover_bg(t.hover).pressed_bg(t.pressed)
                };
                widgets::button_with(ui, a.name(), a.name(), spec, None);
            }
        });
        ui.with_keyed(
            "track",
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .pad(8.0)
                .gap(4.0)
                .radius(m.radius)
                .bg(t.sunken)
                .main_align(self.main),
            |ui| {
                for (i, w) in CHIPS.into_iter().enumerate() {
                    ui.with_keyed(
                        &format!("chip{i}"),
                        NodeSpec::column()
                            .width(Sizing::Fixed(w))
                            .height(Sizing::Fixed(24.0))
                            .radius(m.radius_inner)
                            .bg(t.accent),
                        |_| {},
                    );
                }
            },
        );
    }

    fn baseline_panel(&self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.text(
            "crossAlign: start, then baseline",
            TextStyle::new(15.0).color(t.fg),
        );
        for (label, cross) in [("top", Align::Start), ("base", Align::Baseline)] {
            ui.with_keyed(
                label,
                NodeSpec::row()
                    .gap(6.0)
                    .pad(8.0)
                    .bg(t.surface)
                    .cross_align(cross),
                |ui| {
                    for (part, text, size) in [
                        ("label", "Frame time", 13.0),
                        ("value", "8.3", 36.0),
                        ("unit", "ms", 13.0),
                    ] {
                        ui.with_keyed(part, NodeSpec::column(), |ui| {
                            let colour = if part == "value" { t.fg } else { t.muted };
                            ui.text(text, TextStyle::new(size).color(colour));
                        });
                    }
                },
            );
        }
    }

    fn aspect_panel(&self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.text("aspectRatio", TextStyle::new(15.0).color(t.fg));
        ui.with(NodeSpec::row().width(Sizing::Grow(1.0)).gap(12.0), |ui| {
            ui.with_keyed(
                "video",
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .max_width(360.0)
                    .aspect_ratio(16.0 / 9.0)
                    .radius(ui.metrics().radius)
                    .bg(t.raised)
                    .center(),
                |ui| {
                    ui.text("16 : 9", TextStyle::new(14.0).color(t.muted));
                },
            );
            ui.with(NodeSpec::row().gap(6.0), |ui| {
                for (i, c) in [0x3b5bd4ffu32, 0x73d98cff, 0xffcc00ff]
                    .into_iter()
                    .enumerate()
                {
                    ui.with_keyed(
                        &format!("square{i}"),
                        NodeSpec::column()
                            .height(Sizing::Fixed(40.0))
                            .aspect_ratio(1.0)
                            .bg(Color::hex(c)),
                        |_| {},
                    );
                }
            });
        });
    }
}

impl App for Page {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(24.0)
                .gap(14.0)
                .scroll_y()
                .bg(t.bg),
            |ui| {
                self.main_panel(ui);
                self.baseline_panel(ui);
                self.aspect_panel(ui);
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        if let Some(name) = ev.payload.as_str()
            && let Some(a) = MAIN.into_iter().find(|a| a.name() == name)
        {
            self.main = a;
        }
    }
}

impl Example for Page {
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        core.set_inspect(true);
        let mut d = Drive::new(core, 800.0, 600.0);
        let rect = |d: &mut Drive<'_>, label: &str| {
            let key = d.key_of(label).ok_or(format!("no {label}"))?;
            d.core
                .nodes()
                .into_iter()
                .find(|n| n.key == key)
                .map(|n| n.rect)
                .ok_or(format!("{label} is not in the snapshot"))
        };
        d.frame(self);

        // mainAlign: spaceBetween puts the first chip at the track's
        // padding and the last against its far padding.
        let between = d.key_of("spaceBetween").ok_or("no spaceBetween button")?;
        d.click_key(self, between);
        d.frame(self);
        let track = rect(&mut d, "track")?;
        let (first, last) = (rect(&mut d, "chip0")?, rect(&mut d, "chip3")?);
        d.check(
            (first.x - (track.x + 8.0)).abs() < 0.5,
            "the first chip starts the track",
        )?;
        d.check(
            (last.x + last.w - (track.x + track.w - 8.0)).abs() < 0.5,
            "and the last ends it",
        )?;
        let evenly = d.key_of("spaceEvenly").ok_or("no spaceEvenly button")?;
        d.click_key(self, evenly);
        d.frame(self);
        let (first, last) = (rect(&mut d, "chip0")?, rect(&mut d, "chip3")?);
        let track = rect(&mut d, "track")?;
        let lead = first.x - (track.x + 8.0);
        let tail = track.x + track.w - 8.0 - (last.x + last.w);
        d.check(
            lead > 1.0 && (lead - tail).abs() < 0.5,
            "spaceEvenly leaves equal ends",
        )?;

        // crossAlign: at the top the three tops meet; on the baseline the
        // small label drops below the large value's top.
        let tops: Vec<f32> = d
            .core
            .nodes()
            .into_iter()
            .filter(|n| n.label.as_deref() == Some("label") || n.label.as_deref() == Some("value"))
            .map(|n| n.rect.y)
            .collect();
        d.check(tops.len() == 4, "two rows of a label and a value")?;
        d.check(
            (tops[0] - tops[1]).abs() < 0.5,
            "aligned at the top, the tops meet",
        )?;
        d.check(
            tops[2] > tops[3] + 4.0,
            "on the baseline, the small label sits lower",
        )?;

        // aspectRatio: the card is 16:9 whatever width it grew to, and a
        // square is as wide as its fixed height.
        let video = rect(&mut d, "video")?;
        d.check(
            (video.w / video.h - 16.0 / 9.0).abs() < 0.01,
            "the card keeps 16:9",
        )?;
        let sq = rect(&mut d, "square0")?;
        d.check(
            (sq.w - 40.0).abs() < 0.01 && (sq.h - 40.0).abs() < 0.01,
            "a 40 px square",
        )?;
        let warned = d.core.take_warnings();
        d.check(warned.is_empty(), "nothing warned")?;
        Ok(())
    }
}

kui_devtools::main!(Page { main: Align::Start });
