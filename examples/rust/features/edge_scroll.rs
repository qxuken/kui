//! The edge scroll, tried by hand: how a held drag scrolls a container
//! at its edge (ADR 0029, `EdgeScrollFeel`).
//!
//! - a select picks a preset, slow to fast: `gentle`, `standard` (the
//!   default), `brisk`;
//! - seven sliders set the numbers themselves — the edge zone's depth,
//!   the speed at its edge and the curve to it, the ramp's time and top,
//!   the cap, and how far a pull back restarts the ramp — and the select
//!   reads `custom` once they match no preset.
//!
//! Drag-select the text on the right down to its bottom edge, or up to
//! its top, and hold: slow just inside the zone, faster deeper, faster
//! still the longer it is held; ease back and it slows at once. Maximise
//! the window and push against the screen's edge — that counts as all
//! the way in. The line under the sliders is the same feel as
//! `KUI_EDGE_SCROLL`, in a `selectable` scope, to copy and run any app
//! with it.
//!
//! The view holds no copy of the feel: it reads the core's, and the
//! handler writes the core's (`App::on_event_with`), so
//! `KUI_EDGE_SCROLL=brisk` in the environment opens the example on
//! `brisk`. An app's own drag scrolls by the same feel through
//! `Ui::edge_scroll`.
//!
//! Run: cargo run -p kui-native --example edge_scroll [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::widgets;
use kui_native::{
    Align, App, Core, EdgeScrollFeel, Event, InputEvent, KeyMods, NodeSpec, Role, TextStyle, Ui,
    UiEvent, Value,
};

/// The paragraphs to select through: enough for a long drag.
const PARAGRAPHS: usize = 120;

const WORDS: &[&str] = &[
    "a held drag at the edge of a page should move the page under it",
    "slowly at first so a selection can stop on the word it wants",
    "and then faster the longer the hand stays there",
    "until a long document goes by a few screens a second",
    "while a step back toward the words slows it at once",
];

#[derive(Default)]
struct EdgeScrollDemo;

/// The `{kind}` message every control here declares.
fn message(kind: &str) -> Value {
    Value::map([("kind", Value::str(kind))])
}

/// `feel` as the `KUI_EDGE_SCROLL` line that sets it: its preset's name
/// when it is one, else its seven numbers.
fn env_line(f: EdgeScrollFeel) -> String {
    match preset_of(f) {
        Some(i) => format!("KUI_EDGE_SCROLL={}", EdgeScrollFeel::PRESETS[i].0),
        None => format!(
            "KUI_EDGE_SCROLL={},{},{},{},{},{},{}",
            f.zone_px, f.speed_px_s, f.curve, f.ramp_secs, f.ramp_max, f.cap_px_s, f.pull_back_px
        ),
    }
}

/// The preset `feel` is, by its index in [`EdgeScrollFeel::PRESETS`];
/// `None` for numbers no preset has.
fn preset_of(feel: EdgeScrollFeel) -> Option<usize> {
    EdgeScrollFeel::PRESETS.iter().position(|(_, f)| *f == feel)
}

/// Paragraph `i` of the text: the sentences in turn, numbered.
fn paragraph(i: usize) -> String {
    let mut s = format!("{}. ", i + 1);
    for k in 0..3 {
        s.push_str(WORDS[(i + k) % WORDS.len()]);
        s.push_str(if k == 2 { "." } else { ", " });
    }
    s
}

impl App for EdgeScrollDemo {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let m = ui.metrics();
        let feel = ui.core().edge_scroll_feel();
        let heading = |ui: &mut Ui<'_>, s: &str| {
            ui.text(s, TextStyle::new(12.0).color(t.muted));
        };
        // A labelled slider row: the name, the slider, the number.
        let slider = |ui: &mut Ui<'_>,
                      name: &str,
                      kind: &str,
                      now: f32,
                      range: (f32, f32, f32),
                      shown: String| {
            ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                ui.text_in(
                    NodeSpec::row().width(80.0),
                    name,
                    TextStyle::new(13.0).color(t.fg),
                );
                widgets::slider_with(
                    ui,
                    name,
                    widgets::slider_spec(&m)
                        .width(180.0)
                        .value_now(now)
                        .value_min(range.0)
                        .value_max(range.1)
                        .value_step(range.2)
                        .value_text(shown.clone())
                        .on_change(message(kind)),
                    None,
                );
                ui.text(&shown, TextStyle::new(13.0).color(t.fg).mono());
            });
        };
        ui.with(NodeSpec::row().fill().pad(24.0).gap(24.0).bg(t.bg), |ui| {
            // The settings.
            ui.with(
                NodeSpec::column()
                    .width(400.0)
                    .grow_height()
                    .gap(10.0)
                    .cross_align(Align::Start),
                |ui| {
                    heading(ui, "a preset, slow to fast");
                    ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                        ui.text_in(
                            NodeSpec::row().width(80.0),
                            "Preset",
                            TextStyle::new(13.0).color(t.fg),
                        );
                        let names: Vec<&str> =
                            EdgeScrollFeel::PRESETS.iter().map(|(n, _)| *n).collect();
                        widgets::select(ui, "preset", &names, preset_of(feel));
                        if preset_of(feel).is_none() {
                            ui.text("custom", TextStyle::new(13.0).color(t.muted));
                        }
                    });

                    heading(ui, "or the numbers");
                    slider(
                        ui,
                        "Zone",
                        "zone",
                        feel.zone_px,
                        (8.0, 160.0, 4.0),
                        format!("{:.0} px", feel.zone_px),
                    );
                    slider(
                        ui,
                        "Speed",
                        "speed",
                        feel.speed_px_s,
                        (50.0, 2000.0, 50.0),
                        format!("{:.0} px/s", feel.speed_px_s),
                    );
                    slider(
                        ui,
                        "Curve",
                        "curve",
                        feel.curve,
                        (0.5, 4.0, 0.1),
                        format!("{:.1}", feel.curve),
                    );
                    slider(
                        ui,
                        "Ramp",
                        "ramp",
                        feel.ramp_secs,
                        (0.0, 4.0, 0.1),
                        format!("{:.1} s", feel.ramp_secs),
                    );
                    slider(
                        ui,
                        "Ramp to",
                        "ramp-max",
                        feel.ramp_max,
                        (1.0, 12.0, 0.5),
                        format!("{:.1}×", feel.ramp_max),
                    );
                    slider(
                        ui,
                        "Cap",
                        "cap",
                        feel.cap_px_s,
                        (200.0, 10000.0, 100.0),
                        format!("{:.0} px/s", feel.cap_px_s),
                    );
                    slider(
                        ui,
                        "Pull back",
                        "pull-back",
                        feel.pull_back_px,
                        (0.0, 40.0, 1.0),
                        format!("{:.0} px", feel.pull_back_px),
                    );

                    ui.leaf(NodeSpec::column().height(4.0));
                    heading(ui, "the same feel for any app — select it and copy");
                    // A selectable scope: drag or triple-click, then
                    // Ctrl+C (⌘C), as in any text the runner selects.
                    ui.with_keyed(
                        "env-line",
                        NodeSpec::row()
                            .pad_xy(8.0, 4.0)
                            .radius(4.0)
                            .bg(t.sunken)
                            .selectable(),
                        |ui| {
                            ui.text(&env_line(feel), TextStyle::new(12.0).color(t.fg).mono());
                        },
                    );
                    heading(
                        ui,
                        "the zone is the band inside an edge where a held drag \
                         scrolls: slow at its inner edge, the speed at the edge \
                         itself; held there it ramps up to the cap, and a step \
                         back by more than the pull-back starts the ramp again",
                    );
                },
            );

            // What to try it on.
            ui.with(
                NodeSpec::column().grow_width().grow_height().gap(8.0),
                |ui| {
                    heading(ui, "drag-select to an edge and hold");
                    ui.with_keyed(
                        "text",
                        NodeSpec::column()
                            .grow_width()
                            .grow_height()
                            .scroll_y()
                            .selectable()
                            .pad(16.0)
                            .gap(10.0)
                            .bg(t.sunken)
                            .radius(8.0)
                            .label("Text to select"),
                        |ui| {
                            for i in 0..PARAGRAPHS {
                                ui.text_in(
                                    NodeSpec::column().grow_width(),
                                    &paragraph(i),
                                    TextStyle::new(14.0).color(t.fg),
                                );
                            }
                        },
                    );
                },
            );
        });
    }

    fn on_event_with(&mut self, ev: UiEvent, core: &mut Core) {
        let mut feel = core.edge_scroll_feel();
        let (msg, event) = ev.split::<Value>();
        let kind = msg.as_ref().and_then(|m| m.get_str("kind"));
        match (kind, event) {
            // The select's choice: the preset's name, on the field's key.
            (_, Event::Menu(choice)) => {
                if let Some(f) = choice.item.as_str().and_then(EdgeScrollFeel::preset) {
                    feel = f;
                }
            }
            (Some(kind), Event::SliderChange(c)) => {
                let v = c.value as f32;
                match kind {
                    "zone" => feel.zone_px = v,
                    "speed" => feel.speed_px_s = v,
                    "curve" => feel.curve = v,
                    "ramp" => feel.ramp_secs = v,
                    "ramp-max" => feel.ramp_max = v,
                    "cap" => feel.cap_px_s = v,
                    "pull-back" => feel.pull_back_px = v,
                    _ => return,
                }
            }
            _ => return,
        }
        core.set_edge_scroll_feel(feel);
    }
}

impl Example for EdgeScrollDemo {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(900.0, 600.0)
    }

    /// The drawn menu, so the select's rows are in the frame to click.
    fn native_menus(&self) -> Option<bool> {
        Some(false)
    }

    /// A drag-select held at the text's bottom edge scrolls it, faster
    /// as it is held; leaving the zone stops it; a preset chosen in the
    /// select is the core's feel; a slider's step makes the feel custom.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 900.0, 600.0);
        d.frame(self);
        d.frame(self);
        d.check(
            d.core.edge_scroll_feel() == EdgeScrollFeel::STANDARD,
            "the standard feel to start",
        )?;
        let text = d.key_of("text").ok_or("no text")?;
        let r = d.rect_of(text).ok_or("the text has no rect")?;
        let offset = |d: &Drive<'_>| d.core.scroll_geometry(text).map_or(-1.0, |g| g.offset.y);
        // Press in the middle, drag to just inside the bottom edge, hold.
        let mid = r.center();
        d.move_to(self, mid.x, mid.y);
        d.input(self, InputEvent::mouse_down(1));
        d.move_to(self, mid.x, mid.y + 20.0);
        d.move_to(self, mid.x, r.y + r.h - 1.0);
        let mut steps = Vec::new();
        for _ in 0..60 {
            let before = offset(&d);
            d.advance(1.0 / 60.0);
            d.frame(self);
            steps.push(offset(&d) - before);
        }
        d.check(
            steps[0] > 0.0,
            &format!("held at the edge it scrolls: {}", steps[0]),
        )?;
        d.check(
            steps[59] > steps[0] * 2.0,
            &format!("and faster as it is held: {} then {}", steps[0], steps[59]),
        )?;
        d.check(
            d.core.selection_text().is_some_and(|s| s.len() > 200),
            "the selection followed the scroll",
        )?;
        // Back to the middle: it stops.
        d.move_to(self, mid.x, mid.y);
        let before = offset(&d);
        d.advance(1.0 / 60.0);
        d.frame(self);
        d.check(offset(&d) == before, "out of the zone it stops")?;
        d.input(self, InputEvent::mouse_up());
        d.frame(self);

        // The select: open it, choose `brisk`.
        let field = d.key_of("preset").ok_or("no preset select")?;
        d.click_key(self, field);
        d.frame(self);
        let row = d
            .core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.role == Role::MenuItem && n.name.as_deref() == Some("brisk"))
            .map(|n| n.rect)
            .ok_or("the preset rows are not drawn")?;
        d.click(self, row.x + row.w / 2.0, row.y + row.h / 2.0);
        d.frame(self);
        d.check(
            d.core.edge_scroll_feel() == EdgeScrollFeel::BRISK,
            "the select's choice is the core's feel",
        )?;
        let env = d.key_of("env-line").ok_or("no env line")?;
        d.core.select_all_in(env);
        d.frame(self);
        let picked = d.core.selection_text().unwrap_or_default();
        d.check(
            picked == "KUI_EDGE_SCROLL=brisk",
            &format!("the env line selects whole, a preset by name: {picked:?}"),
        )?;

        // A slider's step: the zone, four px deeper; no preset any more.
        let zone = d
            .core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.role == Role::Slider && n.name.as_deref() == Some("Zone"))
            .map(|n| n.key)
            .ok_or("no Zone slider")?;
        d.focus(self, zone);
        d.key(self, "right", KeyMods::default());
        d.frame(self);
        d.check(
            d.core.edge_scroll_feel().zone_px == 52.0,
            "Right is one 4 px step on the zone",
        )?;
        d.check(
            d.texts_under("env-line") == ["KUI_EDGE_SCROLL=52,700,2,1,7,4900,8"],
            &format!(
                "the env line spells the numbers: {:?}",
                d.texts_under("env-line")
            ),
        )?;
        let warned = d.warnings();
        d.check(warned.is_empty(), &format!("nothing warned: {warned:?}"))
    }
}

kui_devtools::main!(EdgeScrollDemo);
