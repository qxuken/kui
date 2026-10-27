//! What a timeline says, read the same way by every reader (ADR 0058,
//! decisions 9 and 10): a frame's phases, the path from an input to the
//! screen, a verdict on why it lagged, the runs of frames with no input,
//! and the recording grouped by span, by origin and by what its gaps held.

use std::collections::HashMap;

use crate::model::{Frame, Input, Timeline};
use crate::record::{LoopEvent, NameId, Origin, Span, Track};

/// The core's own spans (origin [`Origin::Kui`]) and the drivers'.
pub const BUILD: &str = "build";
pub const LAYOUT: &str = "layout";
pub const EMIT: &str = "emit";
pub const DEVTOOLS: &str = "devtools";
/// A driver's call into the app's event handler (origin [`Origin::Host`]).
pub const HANDLER: &str = "handler";
/// An extension's fill is `fill <namespace>`.
pub const FILL: &str = "fill ";
/// A placement's answer when its last fill was pushed again.
pub const REPLAYED: &str = "replayed";

/// The frame-cause kinds that are input: a run of frames with none of
/// them is a burn.
pub const INPUT_CAUSES: [&str; 12] = [
    "pointerMove",
    "pointerLeave",
    "button",
    "wheel",
    "key",
    "modifiers",
    "text",
    "preedit",
    "access",
    "fileDrag",
    "files",
    "menu",
];

/// A burn: at least this many frames…
pub const BURN_FRAMES: usize = 30;
/// …each within this of the one before (µs), with no input among them.
pub const BURN_GAP_US: u64 = 100_000;

/// Something that turns a name id into its string: a [`crate::Trace`]
/// or a writer's [`crate::Names`].
pub trait NameOf {
    fn name_of(&self, id: NameId) -> &str;
}

impl NameOf for crate::model::Trace {
    fn name_of(&self, id: NameId) -> &str {
        self.name(id)
    }
}

impl NameOf for crate::model::Names {
    fn name_of(&self, id: NameId) -> &str {
        self.text(id)
    }
}

/// A frame's time by phase, µs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Phases {
    /// The app's handlers in the frame's gap.
    pub handler: u64,
    pub build: u64,
    pub layout: u64,
    pub emit: u64,
    pub devtools: u64,
    /// The driver's.
    pub render: u64,
    pub wait: u64,
}

impl Phases {
    /// `(name, µs)`, in the order a frame runs them.
    pub fn named(&self) -> [(&'static str, u64); 7] {
        [
            (HANDLER, self.handler),
            (BUILD, self.build),
            (DEVTOOLS, self.devtools),
            (LAYOUT, self.layout),
            (EMIT, self.emit),
            ("render", self.render),
            ("vsync wait", self.wait),
        ]
    }

    /// Everything but vsync's wait.
    pub fn work(&self) -> u64 {
        self.handler + self.build + self.devtools + self.layout + self.emit + self.render
    }
}

/// Frame `i`'s phases: the core's spans inside it, the handlers in its
/// gap, the driver's present.
pub fn phases(tl: &Timeline, i: usize, names: &impl NameOf) -> Phases {
    let f = &tl.frames[i];
    let mut p = Phases::default();
    for s in tl.spans_in(f.gap_from, f.end_or_begin().max(f.begin + 1)) {
        if s.track != Track::MAIN {
            continue;
        }
        let n = names.name_of(s.name);
        match (s.origin, n) {
            (Origin::Kui, BUILD) => p.build += s.dur,
            (Origin::Kui, LAYOUT) => p.layout += s.dur,
            (Origin::Kui, EMIT) => p.emit += s.dur,
            (Origin::Kui, DEVTOOLS) => p.devtools += s.dur,
            (_, HANDLER) if s.start < f.begin => p.handler += s.dur,
            _ => {}
        }
    }
    if let Some(pr) = f.present {
        p.render = pr.render;
        p.wait = pr.wait;
    }
    p
}

/// The path from the first input frame `i` answers to the screen, µs on
/// the timeline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputPath {
    pub input: Input,
    /// The first handler span after the input, before the frame.
    pub handler: Option<(u64, u64)>,
    pub begin: u64,
    pub end: Option<u64>,
    pub present: Option<u64>,
}

impl InputPath {
    /// Each leg: `(what, µs)` — waiting for a handler, in it, waiting for
    /// the frame, the frame, waiting to be shown.
    pub fn legs(&self) -> Vec<(&'static str, u64)> {
        let mut v = Vec::new();
        let mut at = self.input.t;
        if let Some((a, b)) = self.handler {
            v.push(("to handler", a.saturating_sub(at)));
            v.push(("handler", b.saturating_sub(a)));
            at = b;
        }
        v.push(("to frame", self.begin.saturating_sub(at)));
        if let Some(e) = self.end {
            v.push(("frame", e.saturating_sub(self.begin)));
            if let Some(p) = self.present {
                v.push(("to screen", p.saturating_sub(e)));
            }
        }
        v
    }

    /// From the input to the screen, or to as far as the record goes.
    pub fn total(&self) -> u64 {
        self.present
            .or(self.end)
            .unwrap_or(self.begin)
            .saturating_sub(self.input.t)
    }
}

/// Frame `i`'s input path, when an input arrived in its gap.
pub fn input_path(tl: &Timeline, i: usize, names: &impl NameOf) -> Option<InputPath> {
    let f = &tl.frames[i];
    let input = *tl
        .inputs_in(f.gap_from, f.begin.max(f.gap_from + 1))
        .first()?;
    let handler = tl
        .spans_in(input.t, f.begin)
        .iter()
        .find(|s| names.name_of(s.name) == HANDLER)
        .map(|s| (s.start, s.end()));
    Some(InputPath {
        input,
        handler,
        begin: f.begin,
        end: f.end,
        present: f.present.map(|p| p.t),
    })
}

/// A run of frames with no input: indices into the timeline's frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Burn {
    pub first: usize,
    pub last: usize,
}

impl Burn {
    pub fn len(&self) -> usize {
        self.last - self.first + 1
    }

    pub fn is_empty(&self) -> bool {
        false
    }
}

fn has_input(tl: &Timeline, f: &Frame, names: &impl NameOf) -> bool {
    f.causes
        .iter()
        .any(|c| INPUT_CAUSES.contains(&names.name_of(*c)))
        || !tl
            .inputs_in(f.gap_from, f.begin.max(f.gap_from + 1))
            .is_empty()
}

/// Every burn: [`BURN_FRAMES`] frames or more, each beginning within
/// [`BURN_GAP_US`] of the one before, none answering input.
pub fn burns(tl: &Timeline, names: &impl NameOf) -> Vec<Burn> {
    let mut out = Vec::new();
    let mut run: Option<usize> = None;
    let close = |run: Option<usize>, last: usize, out: &mut Vec<Burn>| {
        if let Some(first) = run
            && last + 1 - first >= BURN_FRAMES
        {
            out.push(Burn { first, last });
        }
    };
    for (i, f) in tl.frames.iter().enumerate() {
        let quiet = !has_input(tl, f, names);
        let near = i > 0 && f.begin.saturating_sub(tl.frames[i - 1].begin) <= BURN_GAP_US;
        match (run, quiet) {
            (Some(_), true) if near => {}
            (_, true) => {
                if let Some(first) = run {
                    close(Some(first), i - 1, &mut out);
                }
                run = Some(i);
            }
            (_, false) => {
                if run.is_some() {
                    close(run, i - 1, &mut out);
                }
                run = None;
            }
        }
    }
    if run.is_some() && !tl.frames.is_empty() {
        close(run, tl.frames.len() - 1, &mut out);
    }
    out
}

/// Why a frame lagged, the first that holds (ADR 0058, decision 9).
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// An input waited this long for its frame; the loop slept through
    /// it, or was busy.
    Late { wait: u64, slept: bool },
    /// A gap this long with no sleep recorded: the thread was busy
    /// outside kui.
    Blocked { busy: u64 },
    /// Over the budget: the slowest phase, and the span with the most
    /// self time in the frame.
    Slow {
        work: u64,
        phase: &'static str,
        phase_us: u64,
        span: Option<(NameId, u64)>,
    },
    /// The frame's wall time far over its CPU time.
    Waited { wall: u64, cpu: u64 },
    /// It drew what the frame before drew.
    Wasted,
    /// Frame `k` (from 1) of a run of `of` with no input.
    Burn { k: usize, of: usize },
    /// Within the budget: its work as a share of it.
    Fine { share: f32 },
}

/// Frame `i`'s verdict against `budget` µs; `burns` from [`burns`].
pub fn verdict(
    tl: &Timeline,
    i: usize,
    budget: u64,
    burns: &[Burn],
    names: &impl NameOf,
) -> Verdict {
    let f = &tl.frames[i];
    let budget = budget.max(1);
    if let Some(path) = input_path(tl, i, names) {
        let wait = f.begin.saturating_sub(path.input.t);
        if wait > budget {
            let slept = tl
                .loops_in(path.input.t, f.begin)
                .iter()
                .any(|(_, e)| matches!(e, LoopEvent::Sleep { .. }));
            return Verdict::Late { wait, slept };
        }
    }
    if tl.has_loop() && i > 0 {
        let gap = f.begin.saturating_sub(f.gap_from);
        let slept = tl
            .loops_in(f.gap_from, f.begin)
            .iter()
            .any(|(_, e)| matches!(e, LoopEvent::Sleep { .. }));
        if !slept && gap > budget * 2 {
            let handled: u64 = tl
                .spans_in(f.gap_from, f.begin)
                .iter()
                .filter(|s| s.parent == 0 && s.track == Track::MAIN)
                .map(|s| s.dur)
                .sum();
            return Verdict::Blocked {
                busy: gap.saturating_sub(handled),
            };
        }
    }
    let p = phases(tl, i, names);
    let work = f.work_us();
    if work > budget {
        let (phase, phase_us) = p.named()[1..6]
            .iter()
            .copied()
            .max_by_key(|(_, us)| *us)
            .unwrap_or(("", 0));
        let span = tl
            .spans_in(f.begin, f.end_or_begin().max(f.begin + 1))
            .iter()
            .filter(|s| s.track == Track::MAIN)
            .map(|s| (s.name, tl.self_us(s)))
            .max_by_key(|(_, us)| *us);
        return Verdict::Slow {
            work,
            phase,
            phase_us,
            span,
        };
    }
    let (wall, cpu) = tl
        .spans_in(f.begin, f.end_or_begin().max(f.begin + 1))
        .iter()
        .filter(|s| s.parent == 0 && s.track == Track::MAIN)
        .fold((0u64, None::<u64>), |(w, c), s| {
            (
                w + s.dur,
                match (c, s.cpu) {
                    (None, Some(x)) => Some(x),
                    (Some(a), Some(x)) => Some(a + x),
                    (c, None) => c,
                },
            )
        });
    if let Some(cpu) = cpu
        && wall > cpu + 2_000
        && wall - cpu > wall / 3
    {
        return Verdict::Waited { wall, cpu };
    }
    if f.unchanged == Some(true) {
        return Verdict::Wasted;
    }
    if let Some(b) = burns.iter().find(|b| (b.first..=b.last).contains(&i)) {
        return Verdict::Burn {
            k: i - b.first + 1,
            of: b.len(),
        };
    }
    Verdict::Fine {
        share: work as f32 / budget as f32,
    }
}

/// A span name's spread over frames: calls, total, self, worst, and CPU
/// where read.
#[derive(Clone, Debug, PartialEq)]
pub struct SpanStat {
    pub name: NameId,
    pub origin: Origin,
    pub calls: u64,
    pub total: u64,
    pub self_total: u64,
    pub worst: u64,
    pub cpu: Option<u64>,
}

/// Every span of the timeline, grouped by name and origin, the most total
/// first.
pub fn by_span(tl: &Timeline, spans: &[Span]) -> Vec<SpanStat> {
    let mut map: HashMap<(NameId, Origin), SpanStat> = HashMap::new();
    for s in spans {
        let st = map.entry((s.name, s.origin)).or_insert(SpanStat {
            name: s.name,
            origin: s.origin,
            calls: 0,
            total: 0,
            self_total: 0,
            worst: 0,
            cpu: None,
        });
        st.calls += 1;
        st.total += s.dur;
        st.self_total += tl.self_us(s);
        st.worst = st.worst.max(s.dur);
        if let Some(c) = s.cpu {
            st.cpu = Some(st.cpu.unwrap_or(0) + c);
        }
    }
    let mut v: Vec<SpanStat> = map.into_values().collect();
    v.sort_by(|a, b| b.total.cmp(&a.total).then(a.name.cmp(&b.name)));
    v
}

/// Self time by origin — the host, each extension, kui — the most first.
pub fn by_origin(tl: &Timeline, spans: &[Span]) -> Vec<(Origin, u64, u64)> {
    let mut map: HashMap<Origin, (u64, u64)> = HashMap::new();
    for s in spans {
        let e = map.entry(s.origin).or_default();
        e.0 += tl.self_us(s);
        e.1 += 1;
    }
    let mut v: Vec<(Origin, u64, u64)> = map.into_iter().map(|(o, (t, n))| (o, t, n)).collect();
    v.sort_by_key(|o| std::cmp::Reverse(o.1));
    v
}

/// What the gaps between frames held, summed over the timeline, µs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gaps {
    /// The whole of every gap.
    pub total: u64,
    /// In the app's handlers.
    pub handlers: u64,
    /// The loop asleep.
    pub asleep: u64,
    /// The rest, where a driver recorded its loop: busy outside kui.
    pub busy: u64,
}

pub fn gaps(tl: &Timeline, names: &impl NameOf) -> Gaps {
    let mut g = Gaps::default();
    for f in tl.frames.iter().skip(1) {
        let (a, b) = (f.gap_from, f.begin);
        g.total += b.saturating_sub(a);
        g.handlers += tl
            .spans_in(a, b)
            .iter()
            .filter(|s| names.name_of(s.name) == HANDLER)
            .map(|s| s.dur.min(b.saturating_sub(s.start)))
            .sum::<u64>();
        let mut sleep_at: Option<u64> = None;
        for (t, e) in tl.loops_in(a, b) {
            match e {
                LoopEvent::Sleep { .. } => sleep_at = Some(*t),
                LoopEvent::Woke { .. } => {
                    if let Some(s) = sleep_at.take() {
                        g.asleep += t - s;
                    }
                }
                LoopEvent::Os { .. } => {}
            }
        }
        if let Some(s) = sleep_at {
            g.asleep += b - s;
        }
    }
    if tl.has_loop() {
        g.busy = g.total.saturating_sub(g.handlers + g.asleep);
    }
    g
}

/// `(avg, p95, max)` of `v`; zeros for none.
pub fn spread(v: &[u64]) -> (f64, u64, u64) {
    if v.is_empty() {
        return (0.0, 0, 0);
    }
    let mut s = v.to_vec();
    s.sort_unstable();
    let avg = s.iter().sum::<u64>() as f64 / s.len() as f64;
    let p95 = s[((s.len() - 1) as f64 * 0.95).round() as usize];
    (avg, p95, s[s.len() - 1])
}

/// Each counter's average a frame and its most, by name. A frame that
/// recorded no value for a counter counts as 0 — the core writes a
/// counter only where it moved — so a glyph rasterized once in a hundred
/// frames averages 0.01, not 1.
pub fn counters(tl: &Timeline) -> Vec<(NameId, f64, i64)> {
    let mut map: HashMap<NameId, (i64, i64)> = HashMap::new();
    for f in &tl.frames {
        for (n, v) in &f.counters {
            let e = map.entry(*n).or_insert((0, i64::MIN));
            e.0 += v;
            e.1 = e.1.max(*v);
        }
    }
    let frames = tl.frames.len().max(1) as f64;
    let mut out: Vec<(NameId, f64, i64)> = map
        .into_iter()
        .map(|(n, (sum, max))| (n, sum as f64 / frames, max))
        .collect();
    out.sort_by_key(|c| c.0);
    out
}

/// Each gauge's last, least and most, by name.
pub fn gauges(tl: &Timeline) -> Vec<(NameId, f64, f64, f64)> {
    let mut map: HashMap<NameId, (f64, f64, f64)> = HashMap::new();
    for g in &tl.gauges {
        let e = map.entry(g.name).or_insert((g.value, g.value, g.value));
        e.0 = g.value;
        e.1 = e.1.min(g.value);
        e.2 = e.2.max(g.value);
    }
    let mut out: Vec<(NameId, f64, f64, f64)> = map
        .into_iter()
        .map(|(n, (l, lo, hi))| (n, l, lo, hi))
        .collect();
    out.sort_by_key(|g| g.0);
    out
}

/// A component kind's placements over the timeline: how many, how many
/// replayed, and the rest by answer and reason, the most first.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacementStat {
    pub component: NameId,
    pub placed: u64,
    pub replayed: u64,
    /// `(answer, why, count)` for each way it was drawn fresh.
    pub fresh: Vec<(NameId, NameId, u64)>,
}

impl PlacementStat {
    /// Drawn fresh: placed and not replayed.
    pub fn drawn(&self) -> u64 {
        self.placed - self.replayed
    }
}

/// Every placement of the timeline, grouped by component kind, the most
/// drawn fresh first.
pub fn placements(tl: &Timeline, names: &impl NameOf) -> Vec<PlacementStat> {
    /// Placed, replayed, and drawn fresh by `(answer, why)`.
    type Tally = (u64, u64, HashMap<(NameId, NameId), u64>);
    let mut map: HashMap<NameId, Tally> = HashMap::new();
    for p in &tl.placements {
        let e = map.entry(p.component).or_default();
        e.0 += 1;
        if names.name_of(p.answer) == REPLAYED {
            e.1 += 1;
        } else {
            *e.2.entry((p.answer, p.why)).or_default() += 1;
        }
    }
    let mut v: Vec<PlacementStat> = map
        .into_iter()
        .map(|(component, (placed, replayed, fresh))| {
            let mut fresh: Vec<(NameId, NameId, u64)> =
                fresh.into_iter().map(|((a, w), n)| (a, w, n)).collect();
            fresh.sort_by(|a, b| b.2.cmp(&a.2).then((a.0, a.1).cmp(&(b.0, b.1))));
            PlacementStat {
                component,
                placed,
                replayed,
                fresh,
            }
        })
        .collect();
    v.sort_by(|a, b| {
        b.drawn()
            .cmp(&a.drawn())
            .then(a.component.cmp(&b.component))
    });
    v
}
