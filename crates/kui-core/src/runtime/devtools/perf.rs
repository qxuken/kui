//! The Perf tab: the window's frames as a browser's performance panel
//! shows them. Live, the last [`crate::stats::STATS_CAPACITY`] frames'
//! cost by phase against the frame budget. Recorded, every frame since
//! the record
//! button — why it ran, who held it, what each phase cost and whether it
//! drew anything new — as a timeline of bars that open into the frame,
//! and a summary of the whole. And the window captured: a screenshot, or
//! a screen recording until it is stopped.
//!
//! The recording itself is the main window's core's
//! (`Core::perf_begin_frame`); this module only draws it. While it runs
//! the tab draws nothing that changes from frame to frame, so the panel
//! — docked, part of the window's picture — never makes a frame look
//! changed that otherwise drew what the one before it drew.

use rustc_hash::FxHashMap;

use super::icons::Icon;
use super::panel::{icon_button, section, small_button};
use super::{PerfFrame, State, action};
use crate::color::Color;
use crate::key::Key;
use crate::spec::{Align, FloatConfig, Min, NodeSpec, TextStyle};
use crate::stats::FrameSample;
use crate::theme::Theme;
use crate::ui::Ui;

/// The phases' colours, the latency graph's.
const INPUT: Color = Color {
    r: 0.45,
    g: 0.85,
    b: 0.55,
    a: 1.0,
};
const VIEW: Color = Color {
    r: 0.28,
    g: 0.42,
    b: 0.88,
    a: 1.0,
};
const LAYOUT: Color = Color {
    r: 0.60,
    g: 0.42,
    b: 0.88,
    a: 1.0,
};
const RENDER: Color = Color {
    r: 0.94,
    g: 0.72,
    b: 0.35,
    a: 1.0,
};
const WAIT: Color = Color {
    r: 0.42,
    g: 0.45,
    b: 0.52,
    a: 0.45,
};
/// A frame that drew what the one before it drew: apart from every
/// phase's colour and from the over-budget cap.
const UNCHANGED: Color = Color {
    r: 0.93,
    g: 0.42,
    b: 0.78,
    a: 1.0,
};

/// The chart's bars' area, logical px: twice the frame budget tall, the
/// budget at half height.
const CHART_H: f32 = 64.0;
const BAR_W: f32 = 4.0;
/// The live strip's bars: thin enough that its frames fit a side dock.
const LIVE_BAR_W: f32 = 2.0;

/// One bar: the frame it stands for, its timing when the driver gave
/// one, and whether it drew nothing new.
struct Bar {
    frame: u64,
    sample: Option<FrameSample>,
    unchanged: bool,
}

pub(super) fn perf_tab(ui: &mut Ui<'_>, st: &mut State, t: &Theme) {
    toolbar(ui, st, t);
    let mut hovered: Option<Key> = None;
    if st.perf.recording {
        // Nothing here moves while frames are kept (see the module docs).
        ui.with(
            NodeSpec::column()
                .fill()
                .center()
                .gap(6.0)
                .bg(t.sunken)
                .radius(6.0)
                .pad(12.0),
            |ui| {
                ui.text(
                    "recording frames",
                    TextStyle::new(12.0).color(t.danger).bold(),
                );
                ui.text_in(
                    NodeSpec::row().max_width(280.0),
                    "use the app, then stop to see each frame: why it ran, who held it, \
                     what it cost. The panel holds still meanwhile, so it is not in what it \
                     measures.",
                    TextStyle::new(11.0).color(t.muted),
                );
            },
        );
    } else if st.perf.frames.is_empty() {
        live(ui, t);
    } else {
        ui.with(NodeSpec::column().fill().gap(10.0).scroll_y(), |ui| {
            summary(ui, st, t, &mut hovered);
            timeline(ui, st, t);
            if let Some(f) = st
                .perf
                .selected
                .and_then(|n| st.perf.frames.iter().find(|f| f.frame == n))
            {
                detail(ui, t, f, ui_budget(ui), &mut hovered);
            } else {
                ui.text(
                    "click a bar to see that frame",
                    TextStyle::new(11.0).color(t.faint),
                );
            }
        });
    }
    if st.hovered_row != hovered {
        st.hovered_row = hovered;
        st.redraw_main = true;
    }
}

fn ui_budget(ui: &Ui<'_>) -> f32 {
    ui.env().frame_budget_ms()
}

/// Record and clear on the left, the window's captures on the right.
fn toolbar(ui: &mut Ui<'_>, st: &State, t: &Theme) {
    let p = &st.perf;
    ui.with(
        NodeSpec::row()
            .grow_width()
            .min_height(Min::FIT)
            .cross_align(Align::Center)
            .gap(2.0),
        |ui| {
            icon_button(
                ui,
                t,
                "perf-record",
                if p.recording { Icon::Stop } else { Icon::Record },
                Some(p.recording),
                if p.recording {
                    "stop recording · see the frames kept"
                } else {
                    "record frames · why each ran, who held it, what it cost"
                },
            );
            if !p.recording && !p.frames.is_empty() {
                small_button(
                    ui,
                    t,
                    "perf-clear",
                    "clear",
                    "drop the recording · back to the live view",
                    false,
                );
            }
            ui.leaf(NodeSpec::row().grow_width());
            icon_button(
                ui,
                t,
                "screenshot",
                Icon::Camera,
                None,
                "screenshot the window · saved to the desktop (KUI_CAPTURE_DIR), the path in Events",
            );
            icon_button(
                ui,
                t,
                "screen-record",
                Icon::Video,
                Some(p.screen_recording),
                if p.screen_recording {
                    "stop the screen recording · the GIF is written, its path in Events"
                } else {
                    "record the window as a GIF · until pressed again, the docked panel with it"
                },
            );
        },
    );
}

/// The last frames the driver timed, as bars against the budget, and
/// their average, 95th percentile and worst.
fn live(ui: &mut Ui<'_>, t: &Theme) {
    let budget = ui_budget(ui);
    let samples: Vec<FrameSample> = ui.core().stats.iter().collect();
    let bars: Vec<Bar> = samples
        .iter()
        .enumerate()
        .map(|(i, s)| Bar {
            frame: i as u64,
            sample: Some(*s),
            unchanged: false,
        })
        .collect();
    ui.with(NodeSpec::column().fill().gap(10.0).scroll_y(), |ui| {
        section(ui, t, "live");
        let works: Vec<f32> = samples.iter().map(FrameSample::work).collect();
        ui.text(
            &if works.is_empty() {
                "no frame timings: this driver pushes none".to_string()
            } else {
                format!(
                    "work {} · budget {budget:.1} ms · last {} frames",
                    spread(&works),
                    works.len()
                )
            },
            TextStyle::new(11.0).color(t.muted).nowrap().ellipsis(),
        );
        chart(ui, t, &bars, budget, None, false, false);
        legend(ui, t);
        ui.text_in(
            NodeSpec::row().grow_width().min_height(Min::FIT),
            &format!(
                "record to keep every frame (the newest {}) with why it ran and who held it",
                super::PERF_CAP
            ),
            TextStyle::new(11.0).color(t.faint),
        );
    });
}

/// `avg · p95 · max ms` of `v`.
fn spread(v: &[f32]) -> String {
    if v.is_empty() {
        return "—".into();
    }
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    let avg = s.iter().sum::<f32>() / s.len() as f32;
    let p95 = s[((s.len() - 1) as f32 * 0.95).round() as usize];
    format!("avg {avg:.2} · p95 {p95:.2} · max {:.2} ms", s[s.len() - 1])
}

/// The phases' key.
fn legend(ui: &mut Ui<'_>, t: &Theme) {
    ui.with(
        NodeSpec::row()
            .grow_width()
            .wrap()
            .gap(10.0)
            .cross_gap(4.0)
            .role(crate::access::Role::None),
        |ui| {
            for (c, name) in [
                (INPUT, "input"),
                (VIEW, "view"),
                (LAYOUT, "layout"),
                (RENDER, "render"),
                (WAIT, "vsync wait"),
                (UNCHANGED, "drew nothing new"),
            ] {
                ui.with(NodeSpec::row().gap(4.0).cross_align(Align::Center), |ui| {
                    ui.leaf(NodeSpec::row().size(8.0, 8.0).radius(2.0).bg(c));
                    ui.text(name, TextStyle::new(10.0).color(t.muted).nowrap());
                });
            }
        },
    );
}

/// The bars, oldest on the left, each its phases stacked from the bottom
/// — vsync's wait faint on top — at [`CHART_H`] for twice `budget`, a
/// bar past that capped in the danger colour; a frame that drew nothing
/// new is marked under its bar. `pick` makes each a button that opens
/// its frame, `selected` lit. The budget is a line across.
fn chart(
    ui: &mut Ui<'_>,
    t: &Theme,
    bars: &[Bar],
    budget: f32,
    selected: Option<u64>,
    pick: bool,
    to_end: bool,
) {
    let per_ms = CHART_H / (budget * 2.0).max(1.0);
    let (w, gap) = if pick {
        (BAR_W, 1.0)
    } else {
        (LIVE_BAR_W, 0.0)
    };
    ui.with(
        NodeSpec::column()
            .grow_width()
            .height(CHART_H + 12.0)
            .bg(t.sunken)
            .radius(6.0)
            .clip()
            .role(crate::access::Role::None),
        |ui| {
            // The budget, at half height.
            ui.leaf(
                NodeSpec::row()
                    .float(
                        FloatConfig::parent()
                            .inside(Align::Start, Align::Start)
                            .offset(0.0, 4.0 + CHART_H - budget * per_ms),
                    )
                    .grow_width()
                    .height(1.0)
                    .bg(t.danger.faded(0.5)),
            );
            let bars_key = ui.with_keyed(
                "kui-devtools/perf-bars",
                NodeSpec::row()
                    .fill()
                    .pad_xy(4.0, 4.0)
                    .gap(gap)
                    .main_align(Align::End)
                    .cross_align(Align::End)
                    .scroll_x()
                    .scrollbar(crate::spec::ScrollbarMode::Auto),
                |ui| {
                    for b in bars {
                        bar(ui, t, b, w, per_ms, selected == Some(b.frame), pick);
                    }
                },
            );
            // A recording opens on its newest frames, once: a pin every
            // frame would ask for frames for ever.
            if to_end {
                ui.set_scroll(bars_key, crate::geom::Vec2::new(f32::MAX, 0.0));
            }
        },
    );
}

fn bar(ui: &mut Ui<'_>, t: &Theme, b: &Bar, w: f32, per_ms: f32, on: bool, pick: bool) {
    let mut spec = NodeSpec::column()
        .width(w)
        .grow_height()
        .main_align(Align::End)
        .radius(1.0)
        .bg(if on {
            t.accent_soft
        } else {
            Color::TRANSPARENT
        });
    if pick {
        spec = spec
            .hover_bg(if on { t.accent_soft } else { t.hover })
            .on_click(action(format!("perf-frame:{}", b.frame)))
            .label(format!("frame {}", b.frame).as_str());
    }
    let body = |ui: &mut Ui<'_>| {
        match b.sample {
            Some(s) => {
                // From the top down: what is drawn last sits lowest.
                let over = s.work() * per_ms > CHART_H;
                let mut left = CHART_H;
                if over {
                    ui.leaf(NodeSpec::row().width(w).height(3.0).bg(t.danger));
                    left -= 3.0;
                }
                let mut seg = |ui: &mut Ui<'_>, ms: f32, c: Color| {
                    let h = (ms * per_ms).min(left);
                    if h >= 0.5 {
                        ui.leaf(NodeSpec::row().width(w).height(h).bg(c));
                        left -= h;
                    }
                };
                seg(ui, s.wait_ms, WAIT);
                seg(ui, s.render_ms, RENDER);
                seg(ui, s.layout_ms, LAYOUT);
                seg(ui, s.view_ms, VIEW);
                seg(ui, s.input_ms, INPUT);
            }
            // No timing: a stub, so the frame is still there to open.
            None => {
                ui.leaf(NodeSpec::row().width(w).height(4.0).bg(t.faint));
            }
        }
        ui.leaf(NodeSpec::row().width(w).height(2.0));
        ui.leaf(NodeSpec::row().width(w).height(2.0).bg(if b.unchanged {
            UNCHANGED
        } else {
            Color::TRANSPARENT
        }));
    };
    if pick {
        ui.with_keyed(&format!("perf-bar:{}", b.frame), spec, body);
    } else {
        ui.with(spec.role(crate::access::Role::None), body);
    }
}

/// The recording as a whole: how many frames over how long, what their
/// work spread was, how many drew nothing new, and — counted — why they
/// ran and who held them, a holder with a node lighting it in the app
/// while the pointer is on it and selecting it in the tree on a click.
fn summary(ui: &mut Ui<'_>, st: &State, t: &Theme, hovered: &mut Option<Key>) {
    let p = &st.perf;
    let frames = &p.frames;
    let n = frames.len();
    let secs = match (frames.front(), frames.back()) {
        (Some(a), Some(b)) => (b.t - a.t).max(0.0),
        _ => 0.0,
    };
    let works: Vec<f32> = frames
        .iter()
        .filter_map(|f| f.sample.map(|s| s.work()))
        .collect();
    let budget = ui_budget(ui);
    let over = works.iter().filter(|w| **w > budget).count();
    let unchanged = frames.iter().filter(|f| f.unchanged == Some(true)).count();
    let mut causes: Vec<(&'static str, usize)> = Vec::new();
    for f in frames {
        for name in f.cause.names() {
            match causes.iter_mut().find(|(c, _)| *c == name) {
                Some((_, k)) => *k += 1,
                None => causes.push((name, 1)),
            }
        }
    }
    let silent = frames.iter().filter(|f| f.cause.is_empty()).count();
    if silent > 0 {
        causes.push(("nobody", silent));
    }
    causes.sort_by_key(|c| std::cmp::Reverse(c.1));
    let mut holders: FxHashMap<(&'static str, Option<Key>, &str), usize> = FxHashMap::default();
    for f in frames {
        for h in &f.holders {
            *holders.entry((h.what, h.key, h.name.as_str())).or_default() += 1;
        }
    }
    let mut holders: Vec<_> = holders.into_iter().collect();
    holders.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.2.cmp(b.0.2)));
    ui.with(
        NodeSpec::column()
            .grow_width()
            .min_height(Min::FIT)
            .gap(4.0),
        |ui| {
            section(ui, t, "recording");
            let rate = if secs > 0.0 {
                format!(" · {:.0}/s", (n.saturating_sub(1)) as f64 / secs)
            } else {
                String::new()
            };
            let dropped = if p.dropped > 0 {
                format!(" (the newest; {} earlier dropped)", p.dropped)
            } else {
                String::new()
            };
            let rows = [
                ("frames", format!("{n} over {secs:.2} s{rate}{dropped}")),
                (
                    "work",
                    if works.is_empty() {
                        "no timings: this driver pushes none".into()
                    } else {
                        spread(&works)
                    },
                ),
                ("over budget", format!("{over} past {budget:.1} ms")),
                (
                    "unchanged",
                    format!(
                        "{unchanged} ({:.0}%) drew what the frame before drew",
                        if n == 0 {
                            0.0
                        } else {
                            unchanged as f32 * 100.0 / n as f32
                        }
                    ),
                ),
            ];
            ui.with(
                NodeSpec::table().grow_width().gap(2.0).pad_xy(2.0, 2.0),
                |ui| {
                    for (k, v) in rows {
                        ui.with(NodeSpec::row().grow_width().gap(8.0), |ui| {
                            ui.text(k, TextStyle::new(11.0).color(t.muted).nowrap());
                            ui.text_in(
                                NodeSpec::row().grow_width(),
                                &v,
                                TextStyle::new(11.0).color(t.fg).mono().nowrap().ellipsis(),
                            );
                        });
                    }
                },
            );
            if !causes.is_empty() {
                section(ui, t, "why frames ran");
                chips(ui, t, causes.iter().map(|(c, k)| format!("{c} {k}")));
            }
            if !holders.is_empty() {
                section(ui, t, "who held frames");
                for ((what, key, name), count) in holders.iter().take(8) {
                    holder_row(ui, t, what, *key, name, Some(*count), hovered);
                }
                if holders.len() > 8 {
                    ui.text(
                        &format!("and {} more", holders.len() - 8),
                        TextStyle::new(10.0).color(t.faint),
                    );
                }
            }
        },
    );
}

/// The timeline: one bar a frame, newest on the right, scrolled there.
fn timeline(ui: &mut Ui<'_>, st: &mut State, t: &Theme) {
    let bars: Vec<Bar> = st
        .perf
        .frames
        .iter()
        .map(|f| Bar {
            frame: f.frame,
            sample: f.sample,
            unchanged: f.unchanged == Some(true),
        })
        .collect();
    ui.with(
        NodeSpec::column()
            .grow_width()
            .min_height(Min::FIT)
            .gap(4.0),
        |ui| {
            section(ui, t, "frames");
            let end = std::mem::take(&mut st.perf.scroll_end);
            chart(ui, t, &bars, ui_budget(ui), st.perf.selected, true, end);
            legend(ui, t);
        },
    );
}

/// One frame opened: its time, its phases, why it ran, whether it drew
/// anything new, and who held it.
fn detail(ui: &mut Ui<'_>, t: &Theme, f: &PerfFrame, budget: f32, hovered: &mut Option<Key>) {
    ui.with(
        NodeSpec::column()
            .grow_width()
            .min_height(Min::FIT)
            .gap(6.0)
            .pad(8.0)
            .bg(t.raised)
            .radius(6.0)
            .border(1.0, t.border),
        |ui| {
            ui.with(
                NodeSpec::row()
                    .grow_width()
                    .gap(6.0)
                    .cross_align(Align::Center),
                |ui| {
                    ui.text(
                        &format!("frame {}", f.frame),
                        TextStyle::new(12.0).color(t.fg).mono().nowrap(),
                    );
                    ui.text_in(
                        NodeSpec::row().grow_width(),
                        &format!("at {:.3} s · {} nodes", f.t, f.nodes),
                        TextStyle::new(11.0)
                            .color(t.muted)
                            .mono()
                            .nowrap()
                            .ellipsis(),
                    );
                    icon_button(
                        ui,
                        t,
                        &format!("perf-frame:{}", f.frame),
                        Icon::Close,
                        None,
                        "close the frame",
                    );
                },
            );
            match f.sample {
                Some(s) => {
                    ui.with(NodeSpec::table().grow_width().gap(2.0), |ui| {
                        for (c, name, ms) in [
                            (INPUT, "input", s.input_ms),
                            (VIEW, "view", s.view_ms),
                            (LAYOUT, "layout", s.layout_ms),
                            (RENDER, "render", s.render_ms),
                            (WAIT, "vsync wait", s.wait_ms),
                        ] {
                            ui.with(
                                NodeSpec::row()
                                    .grow_width()
                                    .gap(6.0)
                                    .cross_align(Align::Center),
                                |ui| {
                                    ui.leaf(NodeSpec::row().size(8.0, 8.0).radius(2.0).bg(c));
                                    ui.text(name, TextStyle::new(11.0).color(t.muted).nowrap());
                                    ui.text(
                                        &format!("{ms:.2} ms"),
                                        TextStyle::new(11.0).color(t.fg).mono().nowrap(),
                                    );
                                },
                            );
                        }
                    });
                    let work = s.work();
                    ui.text(
                        &format!(
                            "work {work:.2} ms — {}",
                            if work > budget {
                                format!("past the {budget:.1} ms budget")
                            } else {
                                format!("{:.0}% of the budget", work * 100.0 / budget)
                            }
                        ),
                        TextStyle::new(11.0)
                            .color(if work > budget { t.danger } else { t.muted })
                            .nowrap(),
                    );
                }
                None => {
                    ui.text(
                        "no timing for this frame: not drawn, or the driver pushes none",
                        TextStyle::new(11.0).color(t.faint),
                    );
                }
            }
            if f.unchanged == Some(true) {
                ui.text_in(
                    NodeSpec::row().grow_width().min_height(Min::FIT),
                    "drew exactly what the frame before drew: nothing on screen changed",
                    TextStyle::new(11.0).color(UNCHANGED),
                );
            }
            section(ui, t, "why it ran");
            if f.cause.is_empty() {
                ui.text(
                    "nobody here asked: the platform's own redraw",
                    TextStyle::new(11.0).color(t.faint),
                );
            } else {
                chips(ui, t, f.cause.names().map(str::to_string));
            }
            if !f.holders.is_empty() {
                section(ui, t, "who held it");
                for h in &f.holders {
                    holder_row(ui, t, h.what, h.key, &h.name, None, hovered);
                }
            }
        },
    );
}

/// Small rounded labels, wrapping.
fn chips(ui: &mut Ui<'_>, t: &Theme, items: impl Iterator<Item = String>) {
    ui.with(
        NodeSpec::row()
            .grow_width()
            .min_height(Min::FIT)
            .wrap()
            .gap(4.0)
            .cross_gap(4.0),
        |ui| {
            for item in items {
                ui.text_in(
                    NodeSpec::row().pad_xy(5.0, 1.0).radius(3.0).bg(t.hover),
                    &item,
                    TextStyle::new(10.0).color(t.fg).mono().nowrap(),
                );
            }
        },
    );
}

/// A holder: what for, its name, how many frames when counted. One with
/// a node goes to it in the tree on a click and lights it in the app
/// while the pointer is on it.
fn holder_row(
    ui: &mut Ui<'_>,
    t: &Theme,
    what: &str,
    key: Option<Key>,
    name: &str,
    count: Option<usize>,
    hovered: &mut Option<Key>,
) {
    let body = |ui: &mut Ui<'_>| {
        ui.text(what, TextStyle::new(11.0).color(t.muted).nowrap());
        ui.text_in(
            NodeSpec::row().grow_width(),
            name,
            TextStyle::new(11.0)
                .color(if key.is_some() { t.accent } else { t.fg })
                .mono()
                .nowrap()
                .ellipsis(),
        );
        if let Some(n) = count {
            ui.text(
                &format!("×{n}"),
                TextStyle::new(11.0).color(t.faint).mono().nowrap(),
            );
        }
    };
    let spec = NodeSpec::row()
        .grow_width()
        .gap(8.0)
        .pad_xy(2.0, 1.0)
        .radius(3.0)
        .cross_align(Align::Center);
    match key {
        Some(k) => {
            let label = format!("perf-holder:{what}:{:016x}:{}", k.0, count.is_some());
            if ui.is_hovered(ui.child_key(&label)) {
                *hovered = Some(k);
            }
            ui.with_keyed(
                &label,
                spec.hover_bg(t.hover)
                    .on_click(action(format!("goto:{:016x}", k.0)))
                    .label(format!("{what} {name}").as_str()),
                body,
            );
        }
        None => {
            ui.with(spec, body);
        }
    }
}
