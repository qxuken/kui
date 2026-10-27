//! The recording written for the tools that already exist (ADR 0058,
//! decision 12): Chrome's trace-event JSON (`chrome://tracing`,
//! Perfetto's viewer), Perfetto's own protobuf, speedscope's and the
//! Firefox Profiler's. An adapter maps what its format has and drops what
//! it has not.
//!
//! Every adapter reads one `Model` made from the timeline, so they draw
//! the same rows with the same names:
//!
//! - **lanes**: `frames` first, then `main`, `loop` (when the driver
//!   recorded it) and each declared track. A track a span or mark names
//!   but no record declared is `track N`.
//! - **slices** (intervals): a frame is `frame N` on `frames`, from its
//!   begin to its end (to the recording's end for one cut off, marked
//!   `unfinished`), with its causes, holders, unchanged, nodes and its
//!   present's render and wait as arguments; its present is `render` then
//!   `vsync wait` on `frames` after it, ending where the driver reported
//!   it and never past the next frame's begin. A span is its name on its
//!   track, its category its origin (`kui`, `host`, an extension's
//!   namespace), with its node, CPU and self time. The loop asleep is
//!   `asleep` on `loop`, from its sleep to its waking. Slices on one lane
//!   are made to nest: a child is cut to its parent's end, since no format
//!   here draws two slices that cross on one row.
//! - **instants**: inputs, events, wakes, requests and each component
//!   drawn fresh (`<component> <answer>`, its reason and node as
//!   arguments; a replayed one is not written) on `main`; the
//!   loop's wakings and OS events on `loop`; marks on their track; notes
//!   on `frames` (global in Chrome's).
//! - **series**: each counter by name, a step over each frame it moved
//!   in (its value from the frame's begin, 0 from its end); each gauge by
//!   name, a point a reading.
//!
//! A verdict, a burn or a phase is not written: they are readings of the
//! timeline, and every tool here reads its own.

mod chrome;
mod firefox;
mod json;
mod perfetto;
mod speedscope;
#[cfg(test)]
mod tests;

use std::cmp::Reverse;

use crate::model::Trace;
use crate::record::{LoopEvent, Origin, Record, Track};

/// A format a recording is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// kui's own (`.kuitrace`).
    Kuitrace,
    /// Chrome's trace-event JSON (`.json`).
    Chrome,
    /// Perfetto's protobuf (`.pftrace`, `.perfetto-trace`).
    Perfetto,
    /// speedscope's (`.speedscope.json`).
    Speedscope,
    /// The Firefox Profiler's, as a Gecko profile (`.firefox.json`).
    Firefox,
}

impl Format {
    /// The format a path's name asks for; `.kuitrace` for one it does not
    /// name.
    pub fn from_path(path: &str) -> Format {
        let p = path.to_ascii_lowercase();
        if p.ends_with(".speedscope.json") {
            Format::Speedscope
        } else if p.ends_with(".firefox.json") {
            Format::Firefox
        } else if p.ends_with(".json") {
            Format::Chrome
        } else if p.ends_with(".pftrace") || p.ends_with(".perfetto-trace") {
            Format::Perfetto
        } else {
            Format::Kuitrace
        }
    }

    /// By the name `kui-trace convert --to` takes.
    pub fn parse(s: &str) -> Option<Format> {
        Some(match s {
            "kuitrace" | "kui" => Format::Kuitrace,
            "chrome" | "json" => Format::Chrome,
            "perfetto" | "pftrace" => Format::Perfetto,
            "speedscope" => Format::Speedscope,
            "firefox" => Format::Firefox,
            _ => return None,
        })
    }

    /// The name [`Format::parse`] takes for it.
    pub fn name(self) -> &'static str {
        match self {
            Format::Kuitrace => "kuitrace",
            Format::Chrome => "chrome",
            Format::Perfetto => "perfetto",
            Format::Speedscope => "speedscope",
            Format::Firefox => "firefox",
        }
    }
}

/// `trace` in `format`.
pub fn write(trace: &Trace, format: Format) -> Vec<u8> {
    match format {
        Format::Kuitrace => trace.to_bytes(),
        Format::Chrome => chrome(trace).into_bytes(),
        Format::Perfetto => perfetto(trace),
        Format::Speedscope => speedscope(trace).into_bytes(),
        Format::Firefox => firefox(trace).into_bytes(),
    }
}

/// Chrome's trace-event JSON: `X` slices, `i` instants and `C` counters,
/// a thread a lane, times in µs. Loads in `chrome://tracing` and
/// Perfetto's viewer.
pub fn chrome(trace: &Trace) -> String {
    chrome::write(&Model::of(trace))
}

/// Perfetto's protobuf trace: a track a lane and a counter track a
/// series, slices as begin and end track events, times in ns.
pub fn perfetto(trace: &Trace) -> Vec<u8> {
    perfetto::write(&Model::of(trace))
}

/// speedscope's file format: an evented profile a lane with slices.
pub fn speedscope(trace: &Trace) -> String {
    speedscope::write(&Model::of(trace))
}

/// The Firefox Profiler's: a Gecko profile of markers, a thread a lane.
pub fn firefox(trace: &Trace) -> String {
    firefox::write(&Model::of(trace))
}

/// An argument's value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Val {
    Str(String),
    U(u64),
    I(i64),
    F(f64),
    B(bool),
}

/// A row: the frames, or a track.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Lane {
    pub track: Option<Track>,
    pub name: String,
    /// The thread id Chrome's and Perfetto's give it: a track's id plus
    /// one (`main` 1, `loop` 2, declared tracks 3 on), the frames one past
    /// the last.
    pub tid: u64,
}

/// An interval on a lane.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Slice {
    pub lane: usize,
    pub start: u64,
    pub end: u64,
    /// What a timeline draws: `frame 12`.
    pub label: String,
    /// What a tool that groups by name groups it under: `frame`.
    pub name: String,
    pub cat: String,
    pub args: Vec<(&'static str, Val)>,
}

/// A point on a lane.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Instant {
    pub lane: usize,
    pub t: u64,
    pub name: String,
    pub cat: &'static str,
    pub args: Vec<(&'static str, Val)>,
    /// Across every lane, where the format has that.
    pub global: bool,
}

/// A counter or a gauge over time: [`Val::I`] or [`Val::F`] points.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Series {
    pub name: String,
    pub gauge: bool,
    pub points: Vec<(u64, Val)>,
}

/// The timeline as rows of slices, instants and series, which every
/// adapter draws.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Model {
    pub app: String,
    pub meta: Vec<(&'static str, Val)>,
    /// The wall clock at 0, ms since the Unix epoch, where recorded.
    pub wall_ms: u64,
    pub lanes: Vec<Lane>,
    /// By lane, then start, a parent before its children; nested.
    pub slices: Vec<Slice>,
    pub instants: Vec<Instant>,
    pub series: Vec<Series>,
    pub start: u64,
    pub end: u64,
}

/// The frames' lane's index.
pub(crate) const FRAMES: usize = 0;
/// `main`'s.
pub(crate) const MAIN: usize = 1;

/// An origin as text: `kui`, `host`, or an extension's namespace.
pub fn origin_label(trace: &Trace, o: Origin) -> String {
    match o {
        Origin::Kui => "kui".into(),
        Origin::Host => "host".into(),
        Origin::Extension(ns) => match trace.name(ns) {
            "" => "extension".into(),
            n => n.into(),
        },
    }
}

fn named(prefix: &str, name: &str) -> String {
    if name.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix} {name}")
    }
}

/// A node key as text.
pub(crate) fn hex(node: u64) -> String {
    format!("{node:016x}")
}

/// One step of a lane's slices in order: `(at, opens, slice index)`.
pub(crate) type Edge = (u64, bool, usize);

impl Model {
    pub fn of(trace: &Trace) -> Model {
        let tl = trace.timeline();
        let mut m = Model {
            start: tl.start,
            end: tl.end,
            ..Default::default()
        };
        m.app = trace
            .session
            .as_ref()
            .map(|s| s.app.clone())
            .filter(|a| !a.is_empty())
            .unwrap_or_else(|| "kui".into());
        if let Some(s) = &trace.session {
            m.wall_ms = s.wall_ms;
            m.meta = vec![
                ("kui", Val::Str(s.kui.clone())),
                ("app", Val::Str(s.app.clone())),
                ("platform", Val::Str(s.platform.clone())),
                ("refresh_hz", Val::F(s.refresh_hz as f64)),
                ("budget_ms", Val::F(s.budget_ms as f64)),
                ("scale", Val::F(s.scale as f64)),
                ("wall_ms", Val::U(s.wall_ms)),
            ];
        }

        // The lanes: frames, main, loop when used, the declared tracks,
        // then any named by a record but never declared.
        let mut used: Vec<Track> = tl
            .spans
            .iter()
            .map(|s| s.track)
            .chain(tl.marks.iter().map(|k| k.track))
            .collect();
        if !tl.loops.is_empty() {
            used.push(Track::LOOP);
        }
        let mut declared: Vec<Track> = trace.tracks.iter().map(|(t, _)| *t).collect();
        declared.sort();
        let mut order = vec![Track::MAIN];
        if used.contains(&Track::LOOP) {
            order.push(Track::LOOP);
        }
        for t in declared {
            if !order.contains(&t) {
                order.push(t);
            }
        }
        used.sort();
        used.dedup();
        for t in used {
            if !order.contains(&t) {
                order.push(t);
            }
        }
        m.lanes.push(Lane {
            track: None,
            name: "frames".into(),
            tid: 0,
        });
        for t in &order {
            let name = match trace.track_name(*t) {
                "" => format!("track {}", t.0),
                n => n.to_string(),
            };
            m.lanes.push(Lane {
                track: Some(*t),
                name,
                tid: t.0 as u64 + 1,
            });
        }
        m.lanes[FRAMES].tid = m.lanes.iter().map(|l| l.tid).max().unwrap_or(0) + 1;
        let lane_of = |t: Track| -> usize { 1 + order.iter().position(|o| *o == t).unwrap_or(0) };

        // Frames and their presents.
        for (i, f) in tl.frames.iter().enumerate() {
            let end = f.end.unwrap_or(tl.end.max(f.begin));
            let mut args = vec![("frame", Val::U(f.frame))];
            if !f.causes.is_empty() {
                let causes: Vec<&str> = f.causes.iter().map(|c| trace.name(*c)).collect();
                args.push(("causes", Val::Str(causes.join(", "))));
            }
            if !f.holders.is_empty() {
                let holders: Vec<String> = f
                    .holders
                    .iter()
                    .map(|h| {
                        let mut s = named(trace.name(h.what), trace.name(h.name));
                        if h.node != 0 {
                            s.push_str(&format!(" node {}", hex(h.node)));
                        }
                        s
                    })
                    .collect();
                args.push(("holders", Val::Str(holders.join("; "))));
            }
            if let Some(u) = f.unchanged {
                args.push(("unchanged", Val::B(u)));
            }
            args.push(("nodes", Val::U(f.nodes as u64)));
            if let Some(p) = f.present {
                args.push(("render_us", Val::U(p.render)));
                args.push(("wait_us", Val::U(p.wait)));
            }
            if f.end.is_none() {
                args.push(("unfinished", Val::B(true)));
            }
            m.slices.push(Slice {
                lane: FRAMES,
                start: f.begin,
                end,
                label: format!("frame {}", f.frame),
                name: "frame".into(),
                cat: "frame".into(),
                args,
            });
            if let Some(p) = f.present {
                // The driver reports once render and the wait are done;
                // both after the frame, neither into the next.
                let limit = tl.frames.get(i + 1).map_or(u64::MAX, |n| n.begin.max(end));
                let rs = p.t.saturating_sub(p.render + p.wait).max(end).min(limit);
                let re = rs.saturating_add(p.render).min(limit);
                let we = re.saturating_add(p.wait).min(limit);
                for (name, a, b) in [("render", rs, re), ("vsync wait", re, we)] {
                    if b > a {
                        m.slices.push(Slice {
                            lane: FRAMES,
                            start: a,
                            end: b,
                            label: name.into(),
                            name: name.into(),
                            cat: "present".into(),
                            args: vec![("frame", Val::U(f.frame))],
                        });
                    }
                }
            }
        }

        // Spans.
        for s in &tl.spans {
            let name = match trace.name(s.name) {
                "" => "(unnamed)".to_string(),
                n => n.to_string(),
            };
            let cat = origin_label(trace, s.origin);
            let mut args = vec![("origin", Val::Str(cat.clone()))];
            if s.node != 0 {
                args.push(("node", Val::Str(hex(s.node))));
            }
            if let Some(c) = s.cpu {
                args.push(("cpu_us", Val::U(c)));
            }
            args.push(("self_us", Val::U(tl.self_us(s))));
            args.push(("id", Val::U(s.id as u64)));
            if s.parent != 0 {
                args.push(("parent", Val::U(s.parent as u64)));
            }
            if s.cut {
                args.push(("cut", Val::B(true)));
            }
            m.slices.push(Slice {
                lane: lane_of(s.track),
                start: s.start,
                end: s.end(),
                label: name.clone(),
                name,
                cat,
                args,
            });
        }

        // The loop: asleep as slices, the rest as instants.
        let lp = lane_of(Track::LOOP);
        let mut asleep: Option<(u64, Option<u64>)> = None;
        let close = |m: &mut Model, at: u64, by: Option<&str>, sleep: (u64, Option<u64>)| {
            let mut args = Vec::new();
            if let Some(u) = sleep.1 {
                args.push(("until_us", Val::U(u)));
            }
            if let Some(by) = by.filter(|b| !b.is_empty()) {
                args.push(("woke_by", Val::Str(by.to_string())));
            }
            m.slices.push(Slice {
                lane: lp,
                start: sleep.0,
                end: at.max(sleep.0),
                label: "asleep".into(),
                name: "asleep".into(),
                cat: "loop".into(),
                args,
            });
        };
        for (t, e) in &tl.loops {
            match e {
                LoopEvent::Sleep { until } => {
                    if let Some(s) = asleep.take() {
                        close(&mut m, *t, None, s);
                    }
                    asleep = Some((*t, *until));
                }
                LoopEvent::Woke { by } => {
                    if let Some(s) = asleep.take() {
                        close(&mut m, *t, Some(trace.name(*by)), s);
                    }
                    m.instants.push(Instant {
                        lane: lp,
                        t: *t,
                        name: named("woke", trace.name(*by)),
                        cat: "loop",
                        args: Vec::new(),
                        global: false,
                    });
                }
                LoopEvent::Os { kind } => m.instants.push(Instant {
                    lane: lp,
                    t: *t,
                    name: named("os", trace.name(*kind)),
                    cat: "loop",
                    args: Vec::new(),
                    global: false,
                }),
            }
        }
        if let Some(s) = asleep {
            close(&mut m, tl.end, None, s);
        }

        // Instants.
        for i in &tl.inputs {
            let mut args = vec![("kind", Val::Str(trace.name(i.kind).into()))];
            if i.count > 1 {
                args.push(("count", Val::U(i.count as u64)));
                args.push(("until_us", Val::U(i.until)));
            }
            if let Some(o) = i.os_t {
                args.push(("os_us", Val::U(o)));
            }
            m.instants.push(Instant {
                lane: MAIN,
                t: i.t,
                name: named("input", trace.name(i.kind)),
                cat: "input",
                args,
                global: false,
            });
        }
        for e in &tl.events {
            let mut args = vec![("kind", Val::Str(trace.name(e.kind).into()))];
            if e.node != 0 {
                args.push(("node", Val::Str(hex(e.node))));
            }
            m.instants.push(Instant {
                lane: MAIN,
                t: e.t,
                name: named("event", trace.name(e.kind)),
                cat: "event",
                args,
                global: false,
            });
        }
        for (t, n) in &tl.wakes {
            m.instants.push(Instant {
                lane: MAIN,
                t: *t,
                name: named("wake", trace.name(*n)),
                cat: "wake",
                args: Vec::new(),
                global: false,
            });
        }
        for r in &tl.requests {
            let mut args = Vec::new();
            if !trace.name(r.at).is_empty() {
                args.push(("at", Val::Str(trace.name(r.at).into())));
            }
            m.instants.push(Instant {
                lane: MAIN,
                t: r.t,
                name: named("request", trace.name(r.why)),
                cat: "request",
                args,
                global: false,
            });
        }
        for k in &tl.marks {
            m.instants.push(Instant {
                lane: lane_of(k.track),
                t: k.t,
                name: match trace.name(k.name) {
                    "" => "mark".into(),
                    n => n.into(),
                },
                cat: "mark",
                args: Vec::new(),
                global: false,
            });
        }
        for p in &tl.placements {
            if trace.name(p.answer) == crate::analysis::REPLAYED {
                continue;
            }
            let mut args = vec![("node", Val::Str(format!("{:016x}", p.node)))];
            if !trace.name(p.why).is_empty() {
                args.push(("why", Val::Str(trace.name(p.why).into())));
            }
            m.instants.push(Instant {
                lane: MAIN,
                t: p.t,
                name: format!("{} {}", trace.name(p.component), trace.name(p.answer)),
                cat: "placement",
                args,
                global: false,
            });
        }
        for (t, text) in tl.notes.clone() {
            m.instants.push(Instant {
                lane: FRAMES,
                t,
                name: "note".into(),
                cat: "note",
                args: vec![("text", Val::Str(text))],
                global: true,
            });
        }
        m.instants.sort_by_key(|i| i.t);

        // Series: a counter is a frame's total, written only for a frame
        // it moved in, so it is drawn as a step over its frame — its value
        // from the frame's begin, back to 0 at its end — or a viewer that
        // holds a counter's last value (all of them) shows one busy frame
        // as busy ever after.
        let span_of: std::collections::HashMap<u64, (u64, u64)> = tl
            .frames
            .iter()
            .map(|f| (f.frame, (f.begin, f.end_or_begin())))
            .collect();
        for r in &trace.records {
            if let Record::Counter {
                t,
                frame,
                name,
                value,
            } = r
            {
                let (begin, end) = span_of.get(frame).copied().unwrap_or((*t, *t));
                let name = trace.name(*name);
                m.point(name, false, begin, Val::I(*value));
                m.point(name, false, end.max(begin + 1), Val::I(0));
            }
        }
        for g in &tl.gauges {
            m.point(trace.name(g.name), true, g.t, Val::F(g.value));
        }
        for s in &mut m.series {
            s.points.sort_by_key(|p| p.0);
        }

        m.nest();
        m
    }

    fn point(&mut self, name: &str, gauge: bool, t: u64, v: Val) {
        let name = if name.is_empty() { "(unnamed)" } else { name };
        let i = match self
            .series
            .iter()
            .position(|s| s.name == name && s.gauge == gauge)
        {
            Some(i) => i,
            None => {
                self.series.push(Series {
                    name: name.into(),
                    gauge,
                    points: Vec::new(),
                });
                self.series.len() - 1
            }
        };
        self.series[i].points.push((t, v));
    }

    /// Sorts the slices by lane and start, a parent first, and cuts each to
    /// the slice it starts inside, so a lane's slices nest.
    fn nest(&mut self) {
        let mut idx: Vec<usize> = (0..self.slices.len()).collect();
        idx.sort_by_key(|&i| {
            let s = &self.slices[i];
            (s.lane, s.start, Reverse(s.end), i)
        });
        let mut sorted: Vec<Slice> = idx.into_iter().map(|i| self.slices[i].clone()).collect();
        let mut stack: Vec<(usize, u64)> = Vec::new();
        for s in &mut sorted {
            while let Some(&(lane, end)) = stack.last() {
                if lane != s.lane || end <= s.start {
                    stack.pop();
                } else {
                    break;
                }
            }
            if let Some(&(_, end)) = stack.last() {
                s.end = s.end.min(end);
            }
            stack.push((s.lane, s.end));
        }
        // A cut can leave equal starts out of order by end.
        sorted.sort_by_key(|s| (s.lane, s.start, Reverse(s.end)));
        self.slices = sorted;
    }

    /// The slices of `lane`, as opens and closes in order: each close
    /// before what opens at its time, a child's before its parent's.
    pub fn edges(&self, lane: usize) -> Vec<Edge> {
        let mut out = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        for (i, s) in self.slices.iter().enumerate() {
            if s.lane != lane {
                continue;
            }
            while let Some(&top) = stack.last() {
                if self.slices[top].end <= s.start {
                    out.push((self.slices[top].end, false, top));
                    stack.pop();
                } else {
                    break;
                }
            }
            out.push((s.start, true, i));
            stack.push(i);
        }
        while let Some(top) = stack.pop() {
            out.push((self.slices[top].end, false, top));
        }
        out
    }

    /// Whether `lane` has a slice.
    pub fn has_slices(&self, lane: usize) -> bool {
        self.slices.iter().any(|s| s.lane == lane)
    }
}
