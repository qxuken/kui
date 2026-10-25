//! The Perf tab: the window's frames as a browser's performance panel
//! shows them, read from the core's recording (ADR 0058).
//!
//! Live, while the tab is on show docked in the window it measures, the
//! last frames by phase against the budget and the spans that cost the
//! most. Recorded — from the record button to the stop — the whole
//! timeline: a summary, frames counted by cause and by holder, a bar a
//! frame with the runs of frames with no input marked under them, the
//! spans grouped by name and by origin, what the gaps between frames held,
//! counters and gauges; and a frame opened says why it lagged — its
//! verdict, the path from its input to the screen, its gap, its spans as a
//! flame chart. The recording saves as a `.kuitrace` for `kui-perf` and
//! the adapters; the window is captured as a screenshot or a GIF.
//!
//! While it records the tab draws nothing that changes from frame to
//! frame, so the panel — docked, part of the window's picture — never
//! makes a frame look changed that otherwise drew what the one before it
//! drew.

use std::sync::Arc;

use kui_trace::analysis::{self, Phases, Verdict};
use kui_trace::{LoopEvent, Origin, Track};

use super::icons::Icon;
use super::panel::{icon_button, section, small_button};
use super::{PerfView, Place, State, action};
use crate::color::Color;
use crate::key::Key;
use crate::spec::{Align, FloatConfig, Min, NodeSpec, Sizing, TextStyle};
use crate::theme::Theme;
use crate::ui::Ui;

/// The phases' colours: the latency graph's, and two of their own.
const HANDLER: Color = Color {
    r: 0.45,
    g: 0.85,
    b: 0.55,
    a: 1.0,
};
const BUILD: Color = Color {
    r: 0.28,
    g: 0.42,
    b: 0.88,
    a: 1.0,
};
const DEVTOOLS: Color = Color {
    r: 0.55,
    g: 0.58,
    b: 0.66,
    a: 1.0,
};
const LAYOUT: Color = Color {
    r: 0.60,
    g: 0.42,
    b: 0.88,
    a: 1.0,
};
const EMIT: Color = Color {
    r: 0.30,
    g: 0.70,
    b: 0.78,
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
/// A frame that drew what the one before it drew.
const UNCHANGED: Color = Color {
    r: 0.93,
    g: 0.42,
    b: 0.78,
    a: 1.0,
};
/// The app's and the extensions' spans in the flame chart.
const HOST: Color = Color {
    r: 0.36,
    g: 0.62,
    b: 0.42,
    a: 1.0,
};
const EXTENSION: Color = Color {
    r: 0.74,
    g: 0.52,
    b: 0.30,
    a: 1.0,
};

/// The chart's bars' area: twice the frame budget tall.
const CHART_H: f32 = 64.0;
const BAR_W: f32 = 4.0;
const LIVE_BAR_W: f32 = 1.0;

fn ms(us: u64) -> String {
    format!("{:.2}", us as f64 / 1000.0)
}

fn phase_colors(p: &Phases) -> [(Color, u64); 7] {
    [
        (WAIT, p.wait),
        (RENDER, p.render),
        (EMIT, p.emit),
        (LAYOUT, p.layout),
        (DEVTOOLS, p.devtools),
        (BUILD, p.build),
        (HANDLER, p.handler),
    ]
}

/// One bar of a chart.
struct Bar {
    phases: Phases,
    unchanged: bool,
    burn: bool,
    /// Inside the range the summary reads.
    in_range: bool,
}

pub(super) fn perf_tab(ui: &mut Ui<'_>, st: &mut State, t: &Theme, place: Place) {
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
                    "use the app, then stop to see each frame and what happened between \
                     them: why it ran, who held it, what it cost. The panel holds still \
                     meanwhile, so it is not in what it measures.",
                    TextStyle::new(11.0).color(t.muted),
                );
            },
        );
    } else if let Some(view) = st.perf.view.clone() {
        // A drag across the bars chooses a range; let go, it stays.
        if let Some(r) = drag_range(ui, st, view.tl.frames.len()) {
            st.perf.range = Some(r);
        }
        if std::mem::take(&mut st.perf.drag_ended) {
            st.perf.drag = None;
        }
        // What the summary and the tables read: the range, read whole once
        // for it, or the recording.
        let range = st
            .perf
            .range
            .filter(|(a, b)| a <= b && *b < view.tl.frames.len());
        let stats = match range {
            Some(r) => match &st.perf.range_view {
                Some((cached, v)) if *cached == r => v.clone(),
                _ => {
                    let v = Arc::new(view.slice(r.0, r.1));
                    st.perf.range_view = Some((r, v.clone()));
                    v
                }
            },
            None => view.clone(),
        };
        ui.with(NodeSpec::column().fill().gap(10.0).scroll_y(), |ui| {
            summary(ui, t, &stats, range, &mut hovered);
            let end = std::mem::take(&mut st.perf.scroll_end);
            timeline(ui, t, &view, st.perf.selected, range, end);
            match st.perf.selected.filter(|i| *i < view.tl.frames.len()) {
                Some(i) => detail(ui, t, &view, i, &mut hovered),
                None => {
                    ui.text(
                        "click a bar to see that frame",
                        TextStyle::new(11.0).color(t.faint),
                    );
                }
            }
            tables(ui, t, &stats, range.is_some());
        });
    } else {
        live(ui, t, place);
    }
    if st.hovered_row != hovered {
        st.hovered_row = hovered;
        st.redraw_main = true;
    }
}

/// Record, clear and save on the left, the window's captures on the right.
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
                    "record frames · why each ran, who held it, what it cost, and what happened between them"
                },
            );
            if !p.recording && p.view.is_some() {
                small_button(
                    ui,
                    t,
                    "perf-save",
                    "save trace",
                    "save the recording as a .kuitrace · kui-perf opens it, kui-trace converts it",
                    false,
                );
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

/// The recent frames: the core's own recording of them while the tab is
/// on show docked in the window it measures, the driver's timings
/// otherwise.
fn live(ui: &mut Ui<'_>, t: &Theme, place: Place) {
    let budget = (ui.env().frame_budget_ms() * 1000.0) as u64;
    let recorded = matches!(place, Place::Main(_)) && ui.core().perf_measuring();
    let (bars, top) = if recorded {
        let trace = ui.core().perf_trace_read();
        let tl = trace.timeline();
        let bars: Vec<Bar> = (0..tl.frames.len())
            .map(|i| Bar {
                phases: analysis::phases(&tl, i, &trace),
                unchanged: false,
                burn: false,
                in_range: false,
            })
            .collect();
        let top: Vec<(String, u64, u64)> = analysis::by_span(&tl, &tl.spans)
            .into_iter()
            .take(6)
            .map(|s| (trace.name(s.name).to_string(), s.total, s.calls))
            .collect();
        (bars, top)
    } else {
        let bars = ui
            .core()
            .stats
            .iter()
            .map(|s| Bar {
                phases: Phases {
                    build: (s.view_ms * 1000.0) as u64,
                    layout: (s.layout_ms * 1000.0) as u64,
                    handler: (s.input_ms * 1000.0) as u64,
                    render: (s.render_ms * 1000.0) as u64,
                    wait: (s.wait_ms * 1000.0) as u64,
                    ..Default::default()
                },
                unchanged: false,
                burn: false,
                in_range: false,
            })
            .collect();
        (bars, Vec::new())
    };
    ui.with(NodeSpec::column().fill().gap(10.0).scroll_y(), |ui| {
        section(ui, t, "live");
        let works: Vec<u64> = bars.iter().map(|b| b.phases.work()).collect();
        let (avg, p95, max) = analysis::spread(&works);
        ui.text(
            &if works.is_empty() {
                "no frames yet".to_string()
            } else {
                format!(
                    "work avg {:.2} · p95 {} · max {} ms · budget {} ms · last {} frames",
                    avg / 1000.0,
                    ms(p95),
                    ms(max),
                    ms(budget),
                    works.len()
                )
            },
            TextStyle::new(11.0).color(t.muted).nowrap().ellipsis(),
        );
        chart(ui, t, &bars, budget, None, false, false);
        legend(ui, t);
        if !top.is_empty() {
            section(ui, t, "spans · last frames");
            ui.with(NodeSpec::table().grow_width().gap(2.0), |ui| {
                for (name, total, calls) in &top {
                    ui.with(NodeSpec::row().grow_width().gap(8.0), |ui| {
                        ui.text_in(
                            NodeSpec::row().grow_width(),
                            name,
                            TextStyle::new(11.0).color(t.fg).mono().nowrap().ellipsis(),
                        );
                        ui.text(
                            &format!("{} ms", ms(*total)),
                            TextStyle::new(11.0).color(t.muted).mono().nowrap(),
                        );
                        ui.text(
                            &format!("×{calls}"),
                            TextStyle::new(11.0).color(t.faint).mono().nowrap(),
                        );
                    });
                }
            });
        }
        ui.text_in(
            NodeSpec::row().grow_width().min_height(Min::FIT),
            &format!(
                "record to keep every frame (the newest {}) with why it ran, who held it and what happened between frames",
                crate::runtime::perf::RING_FRAMES
            ),
            TextStyle::new(11.0).color(t.faint),
        );
    });
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
                (HANDLER, "handler"),
                (BUILD, "build"),
                (DEVTOOLS, "devtools"),
                (LAYOUT, "layout"),
                (EMIT, "emit"),
                (RENDER, "render"),
                (WAIT, "vsync wait"),
                (UNCHANGED, "drew nothing new"),
                (t.warning, "burn"),
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
/// at [`CHART_H`] for twice `budget` µs, one past that capped in the
/// danger colour; under each, a mark for a frame that drew nothing new
/// and a band under a burn. `pick` makes each a button opening its frame,
/// `selected` lit. The budget is a line across.
fn chart(
    ui: &mut Ui<'_>,
    t: &Theme,
    bars: &[Bar],
    budget: u64,
    selected: Option<usize>,
    pick: bool,
    to_end: bool,
) {
    let per_us = CHART_H / (budget as f32 * 2.0).max(1.0);
    let (w, gap) = if pick {
        (BAR_W, 1.0)
    } else {
        (LIVE_BAR_W, 0.0)
    };
    // The live strip is a fixed number of slots, the newest on the right,
    // so a bar keeps its width while the ring fills; it never scrolls, so
    // it shares no scroll state with a recording's chart and shows no
    // bar. A recording's chart scrolls once its bars are past the width.
    let (key, empty) = if pick {
        ("kui-devtools/perf-bars", 0)
    } else {
        (
            "kui-devtools/perf-live",
            crate::runtime::perf::LIVE_FRAMES.saturating_sub(bars.len()),
        )
    };
    let bars = if pick {
        bars
    } else {
        &bars[bars.len().saturating_sub(crate::runtime::perf::LIVE_FRAMES)..]
    };
    ui.with(
        NodeSpec::column()
            .grow_width()
            .height(CHART_H + 14.0)
            .bg(t.sunken)
            .radius(6.0)
            .clip()
            .role(crate::access::Role::None),
        |ui| {
            ui.leaf(
                NodeSpec::row()
                    .float(
                        FloatConfig::parent()
                            .inside(Align::Start, Align::Start)
                            .offset(0.0, 4.0 + CHART_H - budget as f32 * per_us),
                    )
                    .grow_width()
                    .height(1.0)
                    .bg(t.danger.faded(0.5)),
            );
            let bars_key = ui.with_keyed(
                key,
                {
                    let row = NodeSpec::row()
                        .fill()
                        .pad_xy(4.0, 4.0)
                        .gap(gap)
                        .main_align(Align::End)
                        .cross_align(Align::End);
                    if pick {
                        row.scroll_x()
                            .scrollbar(crate::spec::ScrollbarMode::Auto)
                            .on_drag(action("perf-drag"))
                    } else {
                        row
                    }
                },
                |ui| {
                    for _ in 0..empty {
                        ui.leaf(NodeSpec::row().grow_width().min_width(w));
                    }
                    for (i, b) in bars.iter().enumerate() {
                        bar(ui, t, i, b, w, per_us, selected == Some(i), pick);
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

#[allow(clippy::too_many_arguments)]
fn bar(ui: &mut Ui<'_>, t: &Theme, i: usize, b: &Bar, w: f32, per_us: f32, on: bool, pick: bool) {
    // Each bar an equal share of the chart's width, `w` at the least: a
    // short recording fills the chart, a long one scrolls.
    let mut spec = NodeSpec::column()
        .grow_width()
        .min_width(w)
        .grow_height()
        .main_align(Align::End)
        .radius(1.0)
        .bg(if on {
            t.accent_soft
        } else if b.in_range {
            t.accent_soft.faded(0.45)
        } else {
            Color::TRANSPARENT
        });
    if pick {
        spec = spec
            .hover_bg(if on { t.accent_soft } else { t.hover })
            .on_click(action(format!("perf-frame:{i}")))
            // A bar takes the press, so the drag that reads a range
            // starts on it as well as between bars.
            .on_drag(action("perf-drag"))
            .label(format!("frame {i}").as_str());
    }
    let body = |ui: &mut Ui<'_>| {
        let over = b.phases.work() as f32 * per_us > CHART_H;
        let mut left = CHART_H;
        if over {
            ui.leaf(NodeSpec::row().grow_width().height(3.0).bg(t.danger));
            left -= 3.0;
        }
        let mut any = false;
        for (c, us) in phase_colors(&b.phases) {
            let h = (us as f32 * per_us).min(left);
            if h >= 0.5 {
                ui.leaf(NodeSpec::row().grow_width().height(h).bg(c));
                left -= h;
                any = true;
            }
        }
        if !any {
            ui.leaf(NodeSpec::row().grow_width().height(2.0).bg(t.faint));
        }
        ui.leaf(NodeSpec::row().grow_width().height(2.0));
        ui.leaf(NodeSpec::row().grow_width().height(2.0).bg(if b.unchanged {
            UNCHANGED
        } else {
            Color::TRANSPARENT
        }));
        ui.leaf(NodeSpec::row().grow_width().height(2.0).bg(if b.burn {
            t.warning
        } else {
            Color::TRANSPARENT
        }));
    };
    if pick {
        ui.with_keyed(&format!("perf-bar:{i}"), spec, body);
    } else {
        ui.with(spec.role(crate::access::Role::None), body);
    }
}

/// The frames a drag across the bars covers, by the bars' geometry as
/// the last frame laid them out: `None` while there is no drag, or one
/// shorter than a click's slop.
fn drag_range(ui: &mut Ui<'_>, st: &State, n: usize) -> Option<(usize, usize)> {
    let (x0, x1) = st.perf.drag?;
    if n == 0 || (x1 - x0).abs() < 4.0 {
        return None;
    }
    let key = ui.core().key_of("kui-devtools/perf-bars")?;
    let g = ui.scroll_geometry(key)?;
    let content = (g.rect.w + g.max_offset.x - 8.0).max(1.0);
    let stride = content / n as f32;
    let at = |x: f32| {
        (((x - g.rect.x - 4.0 + g.offset.x) / stride)
            .floor()
            .max(0.0) as usize)
            .min(n - 1)
    };
    Some((at(x0.min(x1)), at(x0.max(x1))))
}

/// A small table of `(name, value)` rows.
fn pairs(ui: &mut Ui<'_>, t: &Theme, rows: &[(&str, String)]) {
    ui.with(
        NodeSpec::table().grow_width().gap(2.0).pad_xy(2.0, 2.0),
        |ui| {
            for (k, v) in rows {
                ui.with(NodeSpec::row().grow_width().gap(8.0), |ui| {
                    ui.text(k, TextStyle::new(11.0).color(t.muted).nowrap());
                    ui.text_in(
                        NodeSpec::row().grow_width(),
                        v,
                        TextStyle::new(11.0).color(t.fg).mono().nowrap().ellipsis(),
                    );
                });
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

fn cause_name(view: &PerfView, id: kui_trace::NameId) -> &str {
    match view.trace.name(id) {
        "" => "nobody",
        n => n,
    }
}

/// The recording as a whole, frames counted by cause and by holder.
fn summary(
    ui: &mut Ui<'_>,
    t: &Theme,
    view: &Arc<PerfView>,
    range: Option<(usize, usize)>,
    hovered: &mut Option<Key>,
) {
    let tl = &view.tl;
    let n = tl.frames.len();
    let secs = tl.end.saturating_sub(tl.start) as f64 / 1e6;
    let works: Vec<u64> = view.phases.iter().map(Phases::work).collect();
    let (avg, p95, max) = analysis::spread(&works);
    let over = works.iter().filter(|w| **w > view.budget).count();
    let unchanged = tl
        .frames
        .iter()
        .filter(|f| f.unchanged == Some(true))
        .count();
    let burned: usize = view.burns.iter().map(|b| b.len()).sum();
    let g = view.gaps;
    let mut rows = vec![
        (
            "frames",
            format!(
                "{n} over {secs:.2} s{}",
                if secs > 0.0 {
                    format!(" · {:.0}/s", n.saturating_sub(1) as f64 / secs)
                } else {
                    String::new()
                }
            ),
        ),
        (
            "work",
            format!(
                "avg {:.2} · p95 {} · max {} ms",
                avg / 1000.0,
                ms(p95),
                ms(max)
            ),
        ),
        ("over budget", format!("{over} past {} ms", ms(view.budget))),
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
        (
            "burns",
            format!(
                "{} · {burned} frames with no input, each within 100 ms",
                view.burns.len()
            ),
        ),
    ];
    if g.total > 0 {
        rows.push((
            "between frames",
            if g.busy > 0 || g.asleep > 0 {
                format!(
                    "{} ms: handlers {} · asleep {} · busy elsewhere {}",
                    ms(g.total),
                    ms(g.handlers),
                    ms(g.asleep),
                    ms(g.busy)
                )
            } else {
                format!("{} ms: handlers {}", ms(g.total), ms(g.handlers))
            },
        ));
    }
    ui.with(
        NodeSpec::column()
            .grow_width()
            .min_height(Min::FIT)
            .gap(4.0),
        |ui| {
            match range {
                Some((a, b)) => {
                    ui.with(
                        NodeSpec::row()
                            .grow_width()
                            .gap(6.0)
                            .cross_align(Align::Center),
                        |ui| {
                            ui.text_in(
                                NodeSpec::row().grow_width(),
                                &format!("range · frames {a}–{b} of the recording"),
                                TextStyle::new(10.0).color(t.accent).bold().nowrap(),
                            );
                            small_button(
                                ui,
                                t,
                                "perf-range-clear",
                                "whole recording",
                                "read the whole recording again",
                                false,
                            );
                        },
                    );
                }
                None => section(ui, t, "recording"),
            }
            pairs(ui, t, &rows);
            if !view.causes.is_empty() {
                section(ui, t, "why frames ran");
                chips(
                    ui,
                    t,
                    view.causes
                        .iter()
                        .take(16)
                        .map(|(c, k)| format!("{} {k}", cause_name(view, *c))),
                );
            }
            if !view.holders.is_empty() {
                section(ui, t, "who held frames");
                for ((what, name, node), count) in view.holders.iter().take(8) {
                    holder_row(
                        ui,
                        t,
                        view.trace.name(*what),
                        *node,
                        view.trace.name(*name),
                        Some(*count),
                        hovered,
                    );
                }
            }
        },
    );
}

/// The timeline: a bar a frame, newest on the right, scrolled there once.
fn timeline(
    ui: &mut Ui<'_>,
    t: &Theme,
    view: &Arc<PerfView>,
    selected: Option<usize>,
    range: Option<(usize, usize)>,
    to_end: bool,
) {
    let mut burn = vec![false; view.tl.frames.len()];
    for b in &view.burns {
        for x in &mut burn[b.first..=b.last] {
            *x = true;
        }
    }
    let bars: Vec<Bar> = view
        .tl
        .frames
        .iter()
        .enumerate()
        .map(|(i, f)| Bar {
            phases: view.phases[i],
            unchanged: f.unchanged == Some(true),
            burn: burn[i],
            in_range: range.is_some_and(|(a, b)| (a..=b).contains(&i)),
        })
        .collect();
    ui.with(
        NodeSpec::column()
            .grow_width()
            .min_height(Min::FIT)
            .gap(4.0),
        |ui| {
            section(ui, t, "frames");
            chart(ui, t, &bars, view.budget, selected, true, to_end);
            legend(ui, t);
            if range.is_none() {
                ui.text(
                    "drag across the bars, or shift-click one, to read a range",
                    TextStyle::new(10.0).color(t.faint),
                );
            }
        },
    );
}

fn verdict_line(view: &PerfView, i: usize, t: &Theme) -> (String, Color) {
    let v = analysis::verdict(&view.tl, i, view.budget, &view.burns, &view.trace);
    match v {
        Verdict::Late { wait, slept } => (
            format!(
                "late — its input waited {} ms for this frame, the loop {}",
                ms(wait),
                if slept { "asleep through it" } else { "busy" }
            ),
            t.danger,
        ),
        Verdict::Blocked { busy } => (
            format!(
                "blocked — {} ms before it with no sleep: the thread was busy outside kui",
                ms(busy)
            ),
            t.danger,
        ),
        Verdict::Slow {
            work,
            phase,
            phase_us,
            span,
        } => (
            format!(
                "slow — {} ms of work, {} ms of it in {phase}{}",
                ms(work),
                ms(phase_us),
                span.map_or(String::new(), |(n, us)| format!(
                    "; most in `{}` ({} ms of its own)",
                    view.trace.name(n),
                    ms(us)
                ))
            ),
            t.danger,
        ),
        Verdict::Waited { wall, cpu } => (
            format!(
                "waited — {} ms of wall time, {} ms on a CPU: a lock, a page read back in, \
                 the core given away",
                ms(wall),
                ms(cpu)
            ),
            t.warning,
        ),
        Verdict::Wasted => (
            "wasted — drew exactly what the frame before drew".to_string(),
            UNCHANGED,
        ),
        Verdict::Burn { k, of } => (
            format!("burn — frame {k} of {of} with no input between them"),
            t.warning,
        ),
        Verdict::Fine { share } => (
            format!("fine — {:.0}% of the budget", share * 100.0),
            t.muted,
        ),
    }
}

/// One frame opened: why it lagged, its input's path, its phases, its
/// gap, its spans, its causes and holders and counters.
fn detail(ui: &mut Ui<'_>, t: &Theme, view: &Arc<PerfView>, i: usize, hovered: &mut Option<Key>) {
    let tl = &view.tl;
    let f = &tl.frames[i];
    let name = |id| view.trace.name(id);
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
                        &format!(
                            "at {:.3} s · {} nodes · gap {} ms",
                            f.begin as f64 / 1e6,
                            f.nodes,
                            ms(f.begin.saturating_sub(f.gap_from))
                        ),
                        TextStyle::new(11.0)
                            .color(t.muted)
                            .mono()
                            .nowrap()
                            .ellipsis(),
                    );
                    icon_button(
                        ui,
                        t,
                        &format!("perf-frame:{i}"),
                        Icon::Close,
                        None,
                        "close the frame",
                    );
                },
            );
            let (line, color) = verdict_line(view, i, t);
            ui.text_in(
                NodeSpec::row().grow_width().min_height(Min::FIT),
                &line,
                TextStyle::new(11.0).color(color).bold(),
            );
            if let Some(path) = analysis::input_path(tl, i, &view.trace) {
                section(ui, t, "input to screen");
                chips(
                    ui,
                    t,
                    std::iter::once(format!("{} {} ms", name(path.input.kind), ms(path.total())))
                        .chain(
                            path.legs()
                                .into_iter()
                                .map(|(what, us)| format!("{what} {}", ms(us))),
                        ),
                );
            }
            section(ui, t, "phases");
            ui.with(NodeSpec::table().grow_width().gap(2.0), |ui| {
                for ((pname, us), c) in view.phases[i]
                    .named()
                    .iter()
                    .zip([HANDLER, BUILD, DEVTOOLS, LAYOUT, EMIT, RENDER, WAIT])
                {
                    if *us == 0 {
                        continue;
                    }
                    ui.with(
                        NodeSpec::row()
                            .grow_width()
                            .gap(6.0)
                            .cross_align(Align::Center),
                        |ui| {
                            ui.leaf(NodeSpec::row().size(8.0, 8.0).radius(2.0).bg(c));
                            ui.text(pname, TextStyle::new(11.0).color(t.muted).nowrap());
                            ui.text(
                                &format!("{} ms", ms(*us)),
                                TextStyle::new(11.0).color(t.fg).mono().nowrap(),
                            );
                        },
                    );
                }
            });
            gap(ui, t, view, i);
            flame(ui, t, view, i, hovered);
            section(ui, t, "why it ran");
            if f.causes.is_empty() {
                ui.text(
                    "nobody here asked: the platform's own redraw",
                    TextStyle::new(11.0).color(t.faint),
                );
            } else {
                chips(ui, t, f.causes.iter().map(|c| name(*c).to_string()));
            }
            if !f.holders.is_empty() {
                section(ui, t, "who held it");
                for h in &f.holders {
                    holder_row(ui, t, name(h.what), h.node, name(h.name), None, hovered);
                }
            }
            if !f.counters.is_empty() {
                section(ui, t, "counters");
                chips(
                    ui,
                    t,
                    f.counters.iter().map(|(n, v)| format!("{} {v}", name(*n))),
                );
            }
        },
    );
}

/// What frame `i`'s gap held: inputs, events, wakes, requests, the loop,
/// marks.
fn gap(ui: &mut Ui<'_>, t: &Theme, view: &PerfView, i: usize) {
    let tl = &view.tl;
    let f = &tl.frames[i];
    let (a, b) = (f.gap_from, f.begin.max(f.gap_from + 1));
    let name = |id| view.trace.name(id);
    let mut items: Vec<String> = Vec::new();
    for inp in tl.inputs_in(a, b) {
        items.push(if inp.count > 1 {
            format!("{} ×{}", name(inp.kind), inp.count)
        } else {
            name(inp.kind).to_string()
        });
    }
    for e in tl.events_in(a, b) {
        items.push(format!("→ {}", name(e.kind)));
    }
    for (_, w) in tl.wakes_in(a, b) {
        items.push(format!("wake {}", name(*w)));
    }
    for r in tl.requests_in(a, b) {
        items.push(format!("request {}", name(r.why)));
    }
    let mut slept = 0u64;
    let mut at: Option<u64> = None;
    for (lt, e) in tl.loops_in(a, b) {
        match e {
            LoopEvent::Sleep { .. } => at = Some(*lt),
            LoopEvent::Woke { by } => {
                if let Some(s) = at.take() {
                    slept += lt - s;
                }
                items.push(format!("woke by {}", name(*by)));
            }
            LoopEvent::Os { kind } => items.push(format!("os {}", name(*kind))),
        }
    }
    if let Some(s) = at {
        slept += b - s;
    }
    if slept > 0 {
        items.push(format!("asleep {} ms", ms(slept)));
    }
    for m in tl.marks_in(a, b) {
        items.push(format!("◆ {}", name(m.name)));
    }
    if items.is_empty() {
        return;
    }
    section(ui, t, "between frames");
    chips(ui, t, items.into_iter().take(40));
}

/// Frame `i`'s spans on the main track as a flame chart — a row per
/// depth, each bar at its start and as long as it took over the frame
/// and the handlers in its gap — a span with a node lighting it in the
/// app under the pointer and selecting it in the tree on a click.
fn flame(ui: &mut Ui<'_>, t: &Theme, view: &PerfView, i: usize, hovered: &mut Option<Key>) {
    let tl = &view.tl;
    let f = &tl.frames[i];
    let end = f.end_or_begin().max(f.begin + 1);
    let spans: Vec<&kui_trace::Span> = tl
        .spans_in(f.gap_from, end)
        .iter()
        .filter(|s| s.track == Track::MAIN)
        .filter(|s| s.start >= f.begin || view.trace.name(s.name) == analysis::HANDLER)
        .collect();
    if spans.is_empty() {
        return;
    }
    let from = spans.iter().map(|s| s.start).min().unwrap_or(f.begin);
    let len = end.saturating_sub(from).max(1) as f32;
    // Depth by the parent chain, within what is drawn.
    let mut depth: rustc_hash::FxHashMap<u32, usize> = Default::default();
    let mut rows: Vec<Vec<&kui_trace::Span>> = Vec::new();
    for s in &spans {
        let d = depth.get(&s.parent).map_or(0, |d| d + 1);
        depth.insert(s.id, d);
        if rows.len() <= d {
            rows.resize(d + 1, Vec::new());
        }
        rows[d].push(s);
    }
    section(ui, t, &format!("spans · {} ms", ms(end - from)));
    ui.with(
        NodeSpec::column()
            .grow_width()
            .min_height(Min::FIT)
            .gap(1.0)
            .role(crate::access::Role::None),
        |ui| {
            for row in rows.iter().take(12) {
                ui.with(NodeSpec::row().grow_width().height(16.0), |ui| {
                    let mut at = from;
                    for s in row {
                        if s.start > at {
                            ui.leaf(
                                NodeSpec::row()
                                    .width(Sizing::Percent((s.start - at) as f32 / len))
                                    .grow_height(),
                            );
                        }
                        let w = (s.dur as f32 / len).clamp(0.0, 1.0);
                        let color = match s.origin {
                            Origin::Kui => match view.trace.name(s.name) {
                                analysis::LAYOUT => LAYOUT,
                                analysis::EMIT => EMIT,
                                analysis::DEVTOOLS => DEVTOOLS,
                                _ => BUILD,
                            },
                            Origin::Host => HOST,
                            Origin::Extension(_) => EXTENSION,
                        };
                        let label = format!("perf-span:{}", s.id);
                        let key = ui.child_key(&label);
                        let node = (s.node != 0).then_some(Key(s.node));
                        if ui.is_hovered(key) && node.is_some() {
                            *hovered = node;
                        }
                        let hint = format!(
                            "{} · {} ms, {} ms of its own{}",
                            view.trace.name(s.name),
                            ms(s.dur),
                            ms(tl.self_us(s)),
                            s.cpu
                                .map_or(String::new(), |c| format!(" · {} ms on a CPU", ms(c)))
                        );
                        let mut spec = NodeSpec::row()
                            .width(Sizing::Percent(w))
                            .min_width(1.0)
                            .grow_height()
                            .pad_xy(3.0, 0.0)
                            .radius(2.0)
                            .bg(color.faded(0.85))
                            .cross_align(Align::Center)
                            .clip()
                            .apply_tooltip(&hint)
                            .hoverable();
                        if let Some(k) = node {
                            spec = spec
                                .on_click(action(format!("goto:{:016x}", k.0)))
                                .label(hint.as_str());
                        } else {
                            spec = spec.role(crate::access::Role::None);
                        }
                        ui.with_keyed(&label, spec, |ui| {
                            ui.text(
                                view.trace.name(s.name),
                                TextStyle::new(10.0).color(t.fg).nowrap().ellipsis(),
                            );
                            if ui.is_hovered(key) {
                                crate::widgets::hover_hint(ui, &hint);
                            }
                        });
                        at = s.end().max(at);
                    }
                });
            }
        },
    );
}

/// The spans by name and by origin, counters and gauges, and the burns.
fn tables(ui: &mut Ui<'_>, t: &Theme, view: &Arc<PerfView>, ranged: bool) {
    let frames = view.tl.frames.len().max(1) as f64;
    let name = |id| view.trace.name(id);
    if !view.spans.is_empty() {
        section(
            ui,
            t,
            if ranged {
                "spans · ms over the range"
            } else {
                "spans · ms over the recording"
            },
        );
        ui.with(NodeSpec::table().grow_width().gap(2.0), |ui| {
            ui.with(NodeSpec::row().grow_width().gap(8.0), |ui| {
                for h in ["span", "calls", "per frame", "worst", "self", "cpu"] {
                    ui.text(h, TextStyle::new(10.0).color(t.faint).nowrap());
                }
            });
            for s in view.spans.iter().take(14) {
                let origin = match s.origin {
                    Origin::Extension(ns) => format!("{} · ", name(ns)),
                    Origin::Kui | Origin::Host => String::new(),
                };
                ui.with(NodeSpec::row().grow_width().gap(8.0), |ui| {
                    ui.text_in(
                        NodeSpec::row().grow_width(),
                        &format!("{origin}{}", name(s.name)),
                        TextStyle::new(11.0).color(t.fg).mono().nowrap().ellipsis(),
                    );
                    let cell = |ui: &mut Ui<'_>, v: String| {
                        ui.text(&v, TextStyle::new(11.0).color(t.muted).mono().nowrap());
                    };
                    cell(ui, s.calls.to_string());
                    cell(ui, format!("{:.3}", s.total as f64 / 1000.0 / frames));
                    cell(ui, ms(s.worst));
                    cell(ui, ms(s.self_total));
                    cell(
                        ui,
                        s.cpu.map_or("—".to_string(), |c| {
                            format!("{:.0}%", c as f64 * 100.0 / s.total.max(1) as f64)
                        }),
                    );
                });
            }
        });
    }
    if !view.origins.is_empty() {
        section(ui, t, "by origin · own time");
        let rows: Vec<(String, String)> = view
            .origins
            .iter()
            .map(|(o, us, n)| {
                (
                    match o {
                        Origin::Kui => "kui".to_string(),
                        Origin::Host => "the app".to_string(),
                        Origin::Extension(ns) => name(*ns).to_string(),
                    },
                    format!("{} ms · {n} spans", ms(*us)),
                )
            })
            .collect();
        let refs: Vec<(&str, String)> = rows.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        pairs(ui, t, &refs);
    }
    if !view.counters.is_empty() {
        section(ui, t, "counters · a frame");
        let rows: Vec<(&str, String)> = view
            .counters
            .iter()
            .map(|(n, avg, max)| (name(*n), format!("avg {avg:.2} · most {max}")))
            .collect();
        pairs(ui, t, &rows);
    }
    if !view.gauges.is_empty() {
        section(ui, t, "gauges");
        let rows: Vec<(&str, String)> = view
            .gauges
            .iter()
            .map(|(n, last, lo, hi)| (name(*n), format!("{last} · least {lo} · most {hi}")))
            .collect();
        pairs(ui, t, &rows);
    }
    if !view.burns.is_empty() {
        section(ui, t, "burns");
        for b in view.burns.iter().take(10) {
            let first = &view.tl.frames[b.first];
            let last = &view.tl.frames[b.last];
            let mut causes: Vec<&str> = view.tl.frames[b.first..=b.last]
                .iter()
                .flat_map(|f| f.causes.iter().map(|c| cause_name(view, *c)))
                .collect();
            causes.sort_unstable();
            causes.dedup();
            ui.text_in_keyed(
                &format!("perf-burn:{}", b.first),
                NodeSpec::row()
                    .grow_width()
                    .pad_xy(2.0, 1.0)
                    .radius(3.0)
                    .hover_bg(t.hover)
                    .on_click(action(format!("perf-frame:{}", b.first)))
                    .label(format!("burn of {} frames", b.len()).as_str()),
                &format!(
                    "{} frames over {} ms · {}",
                    b.len(),
                    ms(last.end_or_begin().saturating_sub(first.begin)),
                    causes.join(", ")
                ),
                TextStyle::new(11.0)
                    .color(t.warning)
                    .mono()
                    .nowrap()
                    .ellipsis(),
            );
        }
    }
}

/// A holder: what for, its name, how many frames when counted. One with
/// a node goes to it in the tree on a click and lights it in the app
/// while the pointer is on it.
fn holder_row(
    ui: &mut Ui<'_>,
    t: &Theme,
    what: &str,
    node: u64,
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
                .color(if node != 0 { t.accent } else { t.fg })
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
    if node != 0 {
        let k = Key(node);
        let label = format!("perf-holder:{what}:{node:016x}:{}", count.is_some());
        if ui.is_hovered(ui.child_key(&label)) {
            *hovered = Some(k);
        }
        ui.with_keyed(
            &label,
            spec.hover_bg(t.hover)
                .on_click(action(format!("goto:{node:016x}")))
                .label(format!("{what} {name}").as_str()),
            body,
        );
    } else {
        ui.with(spec, body);
    }
}
