//! Shared, windowless perf exploration (ADR 0078). All times are µs.
//! Selection chooses a frame population; span filters only affect attribution.
//! The module has no dependency on Lua, channels, a window, or serialization.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::analysis::{self, Burn, InputPath, Phases, Verdict};
use crate::{Frame, NameId, Origin, Placement, Span, Timeline, Trace};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Selection {
    #[default]
    Whole,
    /// Recorded frame numbers, inclusive at both ends.
    Frames { first: u64, last: u64 },
    /// Half-open recording-clock range, expanded to complete intersecting frames.
    Time { from_us: u64, to_us: u64 },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub name: Option<String>,
    pub origins: Option<Vec<String>>,
    pub hide_names: Vec<String>,
    pub min_us: u64,
    pub verdicts: Option<Vec<String>>,
    pub hide_unchanged: bool,
    /// Optional scripting predicate results: original IDs, never filtered rows.
    pub span_ids: Option<Vec<u32>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Summary,
    Frames,
    Frame(u64),
    Spans,
    Span(u32),
    Counters,
    Marks,
}

impl Operation {
    pub fn name(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Frames => "frames",
            Self::Frame(_) => "frame",
            Self::Spans => "spans",
            Self::Span(_) => "span",
            Self::Counters => "counters",
            Self::Marks => "marks",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub operation: Operation,
    pub selection: Selection,
    pub filter: Filter,
    pub sort: String,
    pub descending: bool,
    /// None requests all matches. Details are always complete.
    pub limit: Option<usize>,
}

impl Query {
    pub fn new(operation: Operation) -> Self {
        Self {
            operation,
            selection: Selection::Whole,
            filter: Filter::default(),
            sort: match operation {
                Operation::Frames => "work",
                Operation::Spans => "self",
                Operation::Marks => "start",
                _ => "name",
            }
            .into(),
            descending: matches!(operation, Operation::Frames | Operation::Spans),
            limit: matches!(
                operation,
                Operation::Frames | Operation::Spans | Operation::Counters | Operation::Marks
            )
            .then_some(20),
        }
    }

    /// Costs descend; names, frame numbers and timestamps ascend. Adapters
    /// use this before applying an explicitly requested direction.
    pub fn set_sort(&mut self, sort: impl Into<String>) {
        self.sort = sort.into();
        self.descending = matches!(
            self.sort.as_str(),
            "work" | "self" | "total" | "worst" | "calls"
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub requested: Selection,
    /// Original timeline indices, in recording order, after frame filters.
    pub indices: Vec<usize>,
    /// Effective accounting intervals, half-open, including preceding gaps.
    /// More than one interval can remain when a frame filter removes frames.
    pub intervals: Vec<(u64, u64)>,
}

impl Scope {
    pub fn contains(&self, t: u64) -> bool {
        let i = self.intervals.partition_point(|(a, _)| *a <= t);
        i > 0 && t < self.intervals[i - 1].1
    }
    pub fn duration_us(&self) -> u64 {
        self.intervals
            .iter()
            .fold(0u64, |s, (a, b)| s.saturating_add(b.saturating_sub(*a)))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Distribution {
    pub samples: usize,
    pub avg_us: Option<f64>,
    pub p50_us: Option<u64>,
    pub p95_us: Option<u64>,
    pub p99_us: Option<u64>,
    pub max_us: Option<u64>,
}

impl Distribution {
    fn of(values: &[u64]) -> Self {
        if values.is_empty() {
            return Self::default();
        }
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        // Retain the estimator used by the existing p95 reading.
        let percentile = |q: f64| sorted[((sorted.len() - 1) as f64 * q).round() as usize];
        let max = *sorted.last().unwrap();
        Self {
            samples: values.len(),
            avg_us: Some(values.iter().map(|&v| v as f64).sum::<f64>() / values.len() as f64),
            p50_us: Some(percentile(0.50)),
            p95_us: Some(percentile(0.95)),
            p99_us: Some(percentile(0.99)),
            max_us: Some(max),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub frames: usize,
    pub complete_frames: usize,
    pub duration_us: u64,
    pub budget_us: Option<u64>,
    pub work: Distribution,
    pub pacing: Distribution,
    pub phases: Vec<(&'static str, Distribution)>,
    pub over_budget: Option<usize>,
    pub unchanged: usize,
}

impl Summary {
    /// Only frames with complete work measurements enter the denominator.
    pub fn over_budget_percent(&self) -> Option<f64> {
        self.over_budget
            .filter(|_| self.complete_frames > 0)
            .map(|n| 100.0 * n as f64 / self.complete_frames as f64)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameDetail {
    pub index: usize,
    pub frame: Frame,
    pub phases: Phases,
    pub verdict: Option<Verdict>,
    pub path: Option<InputPath>,
    pub fresh: Vec<Placement>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpanDetail {
    pub span: Span,
    pub name: NameId,
    pub self_us: u64,
    pub frame: Option<u64>,
    pub depth: u32,
    pub placement: Option<Placement>,
    pub children: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpanGroup {
    pub name: NameId,
    pub origin: Origin,
    pub calls: u64,
    pub total_us: u64,
    pub self_us: u64,
    pub worst_us: u64,
    pub cpu_us: Option<u64>,
    pub example_id: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Counter {
    pub name: NameId,
    pub avg: f64,
    pub max: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gauge {
    pub name: NameId,
    pub last: f64,
    pub least: f64,
    pub most: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    Summary(Summary),
    Frames(Vec<FrameDetail>),
    Frame(Box<FrameDetail>),
    Spans(Vec<SpanGroup>),
    Span(SpanDetail),
    Counters(Vec<Counter>, Vec<Gauge>),
    Marks(Vec<crate::Mark>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultSet {
    pub query: Query,
    pub scope: Scope,
    pub matched: usize,
    pub returned: usize,
    pub data: Data,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Readings {
    pub summary: Summary,
    pub spans: Vec<SpanGroup>,
    pub origins: Vec<(Origin, u64, u64)>,
    pub components: Vec<analysis::PlacementStat>,
    pub counters: Vec<Counter>,
    pub gauges: Vec<Gauge>,
    /// Original timeline indices, so a range cannot renumber a reference.
    pub burns: Vec<Burn>,
    pub gaps: analysis::Gaps,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Delta {
    pub metric: String,
    pub unit: &'static str,
    pub before: Option<f64>,
    pub after: Option<f64>,
    pub delta: Option<f64>,
    pub percent: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub before: ResultSet,
    pub after: ResultSet,
    pub rows: Vec<Delta>,
    pub matched: usize,
    pub limit: Option<usize>,
}

/// Match semantic metric names, never recording-local IDs. Missing groups and
/// empty populations have no measurement, and a zero baseline has no percent.
pub fn compare(
    before: &Explorer<'_>,
    bq: &Query,
    after: &Explorer<'_>,
    aq: &Query,
    limit: Option<usize>,
) -> Result<Comparison, String> {
    if bq.operation != Operation::Summary || aq.operation != Operation::Summary {
        return Err("comparison inputs must be summary queries".into());
    }
    fn metrics(ex: &Explorer<'_>, r: &ResultSet) -> Vec<(String, &'static str, Option<f64>)> {
        let Data::Summary(s) = &r.data else {
            unreachable!()
        };
        let mut v = vec![
            ("duration".into(), "ms", Some(s.duration_us as f64 / 1000.0)),
            ("frames".into(), "count", Some(s.frames as f64)),
            (
                "complete_frames".into(),
                "count",
                Some(s.complete_frames as f64),
            ),
            ("unchanged".into(), "count", Some(s.unchanged as f64)),
            ("over_budget_percent".into(), "%", s.over_budget_percent()),
            (
                "over_budget".into(),
                "count",
                s.over_budget.map(|n| n as f64),
            ),
        ];
        for (n, d) in [("work", &s.work), ("pacing", &s.pacing)]
            .into_iter()
            .chain(s.phases.iter().map(|(n, d)| (*n, d)))
        {
            v.push((format!("{n}.avg"), "ms", d.avg_us.map(|t| t / 1000.0)));
            for (field, value) in [
                ("p50", d.p50_us),
                ("p95", d.p95_us),
                ("p99", d.p99_us),
                ("max", d.max_us),
            ] {
                v.push((
                    format!("{n}.{field}"),
                    "ms",
                    value.map(|t| t as f64 / 1000.0),
                ));
            }
        }
        let frames = s.frames as f64;
        let mut groups = ex.span_groups(&r.scope, &r.query.filter);
        groups.sort_by(|a, b| {
            ex.trace
                .name(a.name)
                .cmp(ex.trace.name(b.name))
                .then_with(|| origin_name(ex.trace, a.origin).cmp(origin_name(ex.trace, b.origin)))
        });
        for g in groups {
            // Quoting makes names containing separators unambiguous.
            let name = ex.trace.name(g.name);
            let origin = origin_name(ex.trace, g.origin);
            let n = format!("span[{name:?},{origin:?}]");
            for (field, unit, value) in [
                ("calls", "count", Some(g.calls as f64)),
                ("total", "ms", Some(g.total_us as f64 / 1000.0)),
                ("self", "ms", Some(g.self_us as f64 / 1000.0)),
                ("worst", "ms", Some(g.worst_us as f64 / 1000.0)),
                (
                    "calls_per_frame",
                    "count/frame",
                    (frames > 0.0).then_some(g.calls as f64 / frames),
                ),
                (
                    "total_per_frame",
                    "ms/frame",
                    (frames > 0.0).then_some(g.total_us as f64 / 1000.0 / frames),
                ),
                (
                    "self_per_frame",
                    "ms/frame",
                    (frames > 0.0).then_some(g.self_us as f64 / 1000.0 / frames),
                ),
            ] {
                v.push((format!("{n}.{field}"), unit, value));
            }
        }
        let (cs, gs) = ex.counters(&r.scope);
        for c in cs {
            v.push((
                format!("counter[{}].avg", ex.trace.name(c.name)),
                "count/frame",
                Some(c.avg),
            ));
        }
        for g in gs {
            v.push((
                format!("gauge[{}].last", ex.trace.name(g.name)),
                "value",
                Some(g.last),
            ));
        }
        v
    }
    let b = before.execute(bq)?;
    let a = after.execute(aq)?;
    let bm = metrics(before, &b);
    let am = metrics(after, &a);
    let mut order: Vec<_> = bm.iter().map(|(n, _, _)| n.clone()).collect();
    let seen: HashSet<_> = order.iter().cloned().collect();
    order.extend(
        am.iter()
            .filter(|(n, _, _)| !seen.contains(n))
            .map(|(n, _, _)| n.clone()),
    );
    let bm: HashMap<_, _> = bm.into_iter().map(|(n, u, v)| (n, (u, v))).collect();
    let am: HashMap<_, _> = am.into_iter().map(|(n, u, v)| (n, (u, v))).collect();
    let matched = order.len();
    order.truncate(limit.unwrap_or(usize::MAX));
    let rows = order
        .into_iter()
        .map(|metric| {
            let bv = bm.get(&metric);
            let av = am.get(&metric);
            let unit = bv.or(av).unwrap().0;
            let before = bv.and_then(|x| x.1);
            let after = av.and_then(|x| x.1);
            let delta = before.zip(after).map(|(b, a)| a - b);
            let percent = delta
                .zip(before)
                .and_then(|(d, b)| (b != 0.0).then_some(d / b.abs() * 100.0));
            Delta {
                metric,
                unit,
                before,
                after,
                delta,
                percent,
            }
        })
        .collect();
    Ok(Comparison {
        before: b,
        after: a,
        rows,
        matched,
        limit,
    })
}

pub fn budget_us(trace: &Trace) -> Option<u64> {
    trace
        .session
        .as_ref()
        .and_then(|s| {
            (s.budget_ms.is_finite() && s.budget_ms > 0.0)
                .then_some((s.budget_ms as f64 * 1000.0) as u64)
        })
        .filter(|b| *b > 0)
}

pub fn origin_name(trace: &Trace, origin: Origin) -> &str {
    match origin {
        Origin::Kui => "kui",
        Origin::Host => "host",
        Origin::Extension(n) => trace.name(n),
    }
}

pub fn verdict_kind(v: &Verdict) -> &'static str {
    match v {
        Verdict::Late { .. } => "late",
        Verdict::Blocked { .. } => "blocked",
        Verdict::Slow { .. } => "slow",
        Verdict::Waited { .. } => "waited",
        Verdict::Wasted => "wasted",
        Verdict::Burn { .. } => "burn",
        Verdict::Fine { .. } => "fine",
    }
}

pub fn placement_of<'t>(tl: &'t Timeline, trace: &Trace, s: &Span) -> Option<&'t Placement> {
    let ns = trace.name(s.name).strip_prefix("fill ")?;
    let at = tl.placements.partition_point(|p| p.t < s.end());
    tl.placements
        .get(at)
        .filter(|p| p.t <= s.end().saturating_add(50))
        .filter(|p| {
            trace
                .name(p.component)
                .strip_prefix(ns)
                .is_some_and(|s| s.starts_with('/'))
        })
}

/// Reuse a recording's already-built timeline; constructing an explorer is
/// linear in spans, with self costs computed against the unfiltered tree.
pub struct Explorer<'a> {
    pub trace: &'a Trace,
    pub timeline: &'a Timeline,
    index: std::borrow::Cow<'a, Index>,
}

/// Owned analysis for one immutable recording revision. Reuse it for detail
/// reads/redraws; invalidate it whenever the source timeline or session changes.
#[derive(Clone)]
pub struct Index {
    budget: Option<u64>,
    burns: Vec<Burn>,
    spans: HashMap<u32, usize>,
    frames: HashMap<u64, usize>,
    children: Vec<(u32, u32)>,
    self_times: Vec<u64>,
    depths: Vec<u32>,
}

impl std::ops::Deref for Explorer<'_> {
    type Target = Index;
    fn deref(&self) -> &Index {
        &self.index
    }
}

impl<'a> Explorer<'a> {
    pub fn new(trace: &'a Trace, timeline: &'a Timeline) -> Self {
        let spans: HashMap<u32, usize> = timeline
            .spans
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id, i))
            .collect();
        let mut children = vec![0u64; timeline.spans.len()];
        for s in &timeline.spans {
            if s.id != s.parent
                && let Some(&p) = spans.get(&s.parent)
            {
                let parent = &timeline.spans[p];
                if s.track == parent.track {
                    let length = s
                        .end()
                        .min(parent.end())
                        .saturating_sub(s.start.max(parent.start));
                    children[p] = children[p].saturating_add(length);
                }
            }
        }
        let self_times = timeline
            .spans
            .iter()
            .enumerate()
            .map(|(i, s)| s.dur.saturating_sub(children[i]))
            .collect();
        // Resolve depth by parent ID with memoization; corrupt cycles terminate.
        let mut depths = vec![None; timeline.spans.len()];
        let mut visited = vec![0usize; timeline.spans.len()];
        let mut path = Vec::new();
        for i in 0..timeline.spans.len() {
            if depths[i].is_some() {
                continue;
            }
            path.clear();
            let mut at = i;
            let depth = loop {
                if let Some(d) = depths[at] {
                    break d + 1;
                }
                if visited[at] == i + 1 {
                    break 0u32;
                }
                visited[at] = i + 1;
                path.push(at);
                let s = &timeline.spans[at];
                match spans.get(&s.parent) {
                    Some(&p) if p != at && timeline.spans[p].track == s.track => at = p,
                    _ => break 0,
                }
            };
            let mut d = depth;
            for &p in path.iter().rev() {
                depths[p] = Some(d);
                d = d.saturating_add(1);
            }
        }
        let budget = budget_us(trace);
        let frames = timeline
            .frames
            .iter()
            .enumerate()
            .map(|(i, f)| (f.frame, i))
            .collect();
        let mut children: Vec<_> = timeline
            .spans
            .iter()
            .filter(|s| s.parent != 0 && s.parent != s.id)
            .map(|s| (s.parent, s.id))
            .collect();
        children.sort_unstable();
        Self {
            trace,
            timeline,
            index: std::borrow::Cow::Owned(Index {
                budget,
                burns: analysis::burns(timeline, trace),
                spans,
                frames,
                children,
                self_times,
                depths: depths.into_iter().map(|d| d.unwrap_or(0)).collect(),
            }),
        }
    }

    pub fn with_index(trace: &'a Trace, timeline: &'a Timeline, index: &'a Index) -> Self {
        Self {
            trace,
            timeline,
            index: std::borrow::Cow::Borrowed(index),
        }
    }

    pub fn into_index(self) -> Index {
        self.index.into_owned()
    }

    pub fn frame_index(&self, id: u64) -> Option<usize> {
        self.index.frames.get(&id).copied()
    }

    pub fn frame_detail(&self, index: usize) -> FrameDetail {
        let tl = self.timeline;
        let f = &tl.frames[index];
        let end = f.end_or_begin().saturating_add(1);
        FrameDetail {
            index,
            frame: f.clone(),
            phases: analysis::phases(tl, index, self.trace),
            verdict: self
                .budget
                .filter(|_| frame_work_us(f).is_some())
                .map(|b| analysis::verdict(tl, index, b, &self.burns, self.trace)),
            path: analysis::input_path(tl, index, self.trace),
            fresh: tl
                .placements_in(f.begin, end)
                .iter()
                .filter(|p| self.trace.name(p.answer) != analysis::REPLAYED)
                .copied()
                .collect(),
            notes: tl
                .notes_in(f.gap_from, end)
                .iter()
                .map(|n| n.1.clone())
                .collect(),
        }
    }

    pub fn span_detail(&self, id: u32) -> Option<SpanDetail> {
        let &i = self.spans.get(&id)?;
        let s = &self.timeline.spans[i];
        let placement = placement_of(self.timeline, self.trace, s).copied();
        Some(SpanDetail {
            span: s.clone(),
            name: placement.map_or(s.name, |p| p.component),
            self_us: self.self_times[i],
            frame: self
                .frame_for_time(s.start)
                .map(|i| self.timeline.frames[i].frame),
            depth: self.depths[i],
            placement,
            children: {
                let a = self
                    .index
                    .children
                    .partition_point(|(parent, _)| *parent < id);
                let b = self
                    .index
                    .children
                    .partition_point(|(parent, _)| *parent <= id);
                self.index.children[a..b]
                    .iter()
                    .map(|(_, child)| *child)
                    .collect()
            },
        })
    }

    pub fn frame_for_time(&self, t: u64) -> Option<usize> {
        frame_for_time(self.timeline, t)
    }

    fn effective_end(&self, i: usize) -> u64 {
        let f = &self.timeline.frames[i];
        if f.end.is_none() {
            frame_end(f).max(
                self.timeline
                    .frames
                    .get(i + 1)
                    .map_or(self.timeline.end, |f| f.begin)
                    .saturating_add(1),
            )
        } else {
            frame_end(f)
        }
    }

    pub fn resolve(&self, selection: &Selection, filter: &Filter) -> Result<Scope, String> {
        match selection {
            Selection::Frames { first, last } if first > last => {
                return Err("frame range must be increasing".into());
            }
            Selection::Time { from_us, to_us } if from_us >= to_us => {
                return Err("time range must have positive length".into());
            }
            _ => {}
        }
        let mut indices = Vec::new();
        let mut intervals: Vec<(u64, u64)> = Vec::new();
        for (i, f) in self.timeline.frames.iter().enumerate() {
            let selected = match selection {
                Selection::Whole => true,
                Selection::Frames { first, last } => f.frame >= *first && f.frame <= *last,
                Selection::Time { from_us, to_us } => {
                    f.gap_from < *to_us && self.effective_end(i) > *from_us
                }
            };
            if !selected || !self.keeps_frame(i, filter) {
                continue;
            }
            indices.push(i);
            let end = self.effective_end(i);
            if let Some((_, b)) = intervals.last_mut()
                && f.gap_from <= *b
            {
                *b = (*b).max(end);
            } else {
                intervals.push((f.gap_from, end));
            }
        }
        // Whole-recording queries also include unframed records at either end.
        if matches!(selection, Selection::Whole)
            && filter.verdicts.is_none()
            && !filter.hide_unchanged
        {
            intervals =
                if self.timeline.end >= self.timeline.start && (!self.trace.records.is_empty()) {
                    vec![(self.timeline.start, self.timeline.end.saturating_add(1))]
                } else {
                    Vec::new()
                };
        }
        Ok(Scope {
            requested: selection.clone(),
            indices,
            intervals,
        })
    }

    pub fn keeps_frame(&self, i: usize, filter: &Filter) -> bool {
        let f = &self.timeline.frames[i];
        let verdict = filter
            .verdicts
            .as_ref()
            .and_then(|_| self.budget.filter(|_| frame_work_us(f).is_some()))
            .map(|b| analysis::verdict(self.timeline, i, b, &self.burns, self.trace));
        matches_frame(f.unchanged, verdict.as_ref().map(verdict_kind), filter)
    }

    pub fn keeps_span(&self, s: &Span, filter: &Filter) -> bool {
        matches_span(self.trace, self.timeline, s, filter)
    }

    pub fn selected_spans<'b>(
        &'b self,
        scope: &'b Scope,
        filter: &'b Filter,
    ) -> impl Iterator<Item = (usize, &'a Span)> + 'b {
        let ids = filter
            .span_ids
            .as_ref()
            .map(|v| v.iter().copied().collect::<HashSet<_>>());
        // Present intervals can overlap the following frame's gap. Their
        // union describes the clock extent, not ownership of span costs.
        let whole_population = scope.requested == Selection::Whole
            && filter.verdicts.is_none()
            && !filter.hide_unchanged;
        self.timeline
            .spans
            .iter()
            .enumerate()
            .filter(move |(_, s)| {
                scope.contains(s.start)
                    && (whole_population
                        || self
                            .frame_for_time(s.start)
                            .is_some_and(|i| scope.indices.binary_search(&i).is_ok()))
                    && self.keeps_span(s, filter)
                    && ids.as_ref().is_none_or(|ids| ids.contains(&s.id))
            })
    }

    pub fn summary(&self, scope: &Scope) -> Summary {
        let frames: Vec<_> = scope
            .indices
            .iter()
            .map(|&i| &self.timeline.frames[i])
            .collect();
        let works: Vec<_> = frames.iter().filter_map(|f| frame_work_us(f)).collect();
        let pacing: Vec<_> = scope
            .indices
            .windows(2)
            .filter(|w| w[1] == w[0] + 1)
            .map(|w| {
                self.timeline.frames[w[1]]
                    .begin
                    .saturating_sub(self.timeline.frames[w[0]].begin)
            })
            .collect();
        let phases: Vec<_> = scope
            .indices
            .iter()
            .filter(|&&i| self.timeline.frames[i].end.is_some())
            .map(|&i| {
                (
                    analysis::phases(self.timeline, i, self.trace),
                    self.timeline.frames[i].present.is_some(),
                )
            })
            .collect();
        let phases = Phases::default()
            .named()
            .into_iter()
            .enumerate()
            .map(|(j, (name, _))| {
                let values: Vec<_> = phases
                    .iter()
                    .filter(|(_, present)| j < 5 || *present)
                    .map(|(p, _)| p.named()[j].1)
                    .collect();
                (name, Distribution::of(&values))
            })
            .collect();
        Summary {
            frames: frames.len(),
            complete_frames: works.len(),
            duration_us: scope.duration_us(),
            budget_us: self.budget,
            work: Distribution::of(&works),
            pacing: Distribution::of(&pacing),
            phases,
            over_budget: self
                .budget
                .filter(|_| !works.is_empty())
                .map(|b| works.iter().filter(|&&w| w > b).count()),
            unchanged: frames.iter().filter(|f| f.unchanged == Some(true)).count(),
        }
    }

    pub fn span_groups(&self, scope: &Scope, filter: &Filter) -> Vec<SpanGroup> {
        let mut groups: HashMap<(NameId, Origin), SpanGroup> = HashMap::new();
        for (i, s) in self.selected_spans(scope, filter) {
            let name = placement_of(self.timeline, self.trace, s).map_or(s.name, |p| p.component);
            let g = groups.entry((name, s.origin)).or_insert(SpanGroup {
                name,
                origin: s.origin,
                calls: 0,
                total_us: 0,
                self_us: 0,
                worst_us: 0,
                cpu_us: Some(0),
                example_id: s.id,
            });
            g.calls += 1;
            g.total_us = g.total_us.saturating_add(s.dur);
            g.self_us = g.self_us.saturating_add(self.self_times[i]);
            if s.dur > g.worst_us || (s.dur == g.worst_us && s.id < g.example_id) {
                g.example_id = s.id;
            }
            g.worst_us = g.worst_us.max(s.dur);
            g.cpu_us = g.cpu_us.zip(s.cpu).map(|(a, b)| a.saturating_add(b));
        }
        groups.into_values().collect()
    }

    pub fn origins(&self, scope: &Scope, filter: &Filter) -> Vec<(Origin, u64, u64)> {
        let mut groups: HashMap<Origin, (u64, u64)> = HashMap::new();
        for (i, s) in self.selected_spans(scope, filter) {
            let g = groups.entry(s.origin).or_default();
            g.0 = g.0.saturating_add(self.self_times[i]);
            g.1 += 1;
        }
        let mut out: Vec<_> = groups.into_iter().map(|(o, (us, n))| (o, us, n)).collect();
        out.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| origin_name(self.trace, a.0).cmp(origin_name(self.trace, b.0)))
        });
        out
    }

    pub fn counters(&self, scope: &Scope) -> (Vec<Counter>, Vec<Gauge>) {
        let mut counters: BTreeMap<NameId, (f64, i64)> = BTreeMap::new();
        for &i in &scope.indices {
            for &(n, v) in &self.timeline.frames[i].counters {
                let g = counters.entry(n).or_insert((0.0, i64::MIN));
                g.0 += v as f64;
                g.1 = g.1.max(v);
            }
        }
        let counters = counters
            .into_iter()
            .map(|(name, (sum, max))| Counter {
                name,
                avg: sum / scope.indices.len().max(1) as f64,
                max,
            })
            .collect();
        let mut gauges: BTreeMap<NameId, Gauge> = BTreeMap::new();
        for g in self
            .timeline
            .gauges
            .iter()
            .filter(|g| scope.contains(g.t) && g.value.is_finite())
        {
            let v = gauges.entry(g.name).or_insert(Gauge {
                name: g.name,
                last: g.value,
                least: g.value,
                most: g.value,
            });
            v.last = g.value;
            v.least = v.least.min(g.value);
            v.most = v.most.max(g.value);
        }
        (counters, gauges.into_values().collect())
    }

    /// The viewer's complete tables, without CLI result limits. Attribution
    /// uses the original tree; placements/gaps describe the frame population.
    pub fn readings(&self, scope: &Scope, filter: &Filter) -> Readings {
        let mut spans = self.span_groups(scope, filter);
        spans.sort_by(|a, b| {
            b.self_us
                .cmp(&a.self_us)
                .then_with(|| self.trace.name(a.name).cmp(self.trace.name(b.name)))
                .then_with(|| {
                    origin_name(self.trace, a.origin).cmp(origin_name(self.trace, b.origin))
                })
        });
        let mut selected = Timeline::default();
        selected.placements = self
            .timeline
            .placements
            .iter()
            .filter(|p| scope.contains(p.t))
            .copied()
            .collect();
        let (counters, gauges) = self.counters(scope);
        let indices: HashSet<_> = scope.indices.iter().copied().collect();
        let mut burns = Vec::new();
        for b in &self.burns {
            let mut first = None;
            for i in b.first..=b.last {
                if indices.contains(&i) {
                    first.get_or_insert(i);
                } else if let Some(start) = first.take() {
                    burns.push(Burn {
                        first: start,
                        last: i - 1,
                    });
                }
            }
            if let Some(first) = first {
                burns.push(Burn {
                    first,
                    last: b.last,
                });
            }
        }
        Readings {
            summary: self.summary(scope),
            spans,
            origins: self.origins(scope, filter),
            components: analysis::placements(&selected, self.trace),
            counters,
            gauges,
            burns,
            gaps: analysis::gaps_in(self.timeline, self.trace, scope.indices.iter().copied()),
        }
    }

    pub fn execute(&self, query: &Query) -> Result<ResultSet, String> {
        let mut selection = query.selection.clone();
        let mut filter = query.filter.clone();
        match query.operation {
            Operation::Frame(id) => {
                selection = Selection::Frames {
                    first: id,
                    last: id,
                };
                filter = Filter::default();
            }
            Operation::Span(id) => {
                if let Some(s) = self.span_detail(id)
                    && let Some(id) = s.frame
                {
                    selection = Selection::Frames {
                        first: id,
                        last: id,
                    };
                }
                filter = Filter::default();
            }
            _ => {}
        }
        let scope = self.resolve(&selection, &filter)?;
        let limit = query.limit.unwrap_or(usize::MAX);
        let flip = |o: std::cmp::Ordering| if query.descending { o.reverse() } else { o };
        let (data, matched, returned) = match query.operation {
            Operation::Summary => (Data::Summary(self.summary(&scope)), 1, 1),
            Operation::Frame(id) => {
                let i = self
                    .frame_index(id)
                    .ok_or_else(|| format!("no frame {id} in this recording"))?;
                (Data::Frame(Box::new(self.frame_detail(i))), 1, 1)
            }
            Operation::Span(id) => (
                Data::Span(
                    self.span_detail(id)
                        .ok_or_else(|| format!("no span {id} in this recording"))?,
                ),
                1,
                1,
            ),
            Operation::Frames => {
                let mut indices = scope.indices.clone();
                if !["frame", "start", "work"].contains(&query.sort.as_str()) {
                    return Err("frames --sort needs frame, start or work".into());
                }
                indices.sort_by(|&a, &b| {
                    let a = &self.timeline.frames[a];
                    let b = &self.timeline.frames[b];
                    match query.sort.as_str() {
                        "frame" => flip(a.frame.cmp(&b.frame)),
                        "start" => flip(a.begin.cmp(&b.begin)),
                        _ => match (frame_work_us(a), frame_work_us(b)) {
                            (Some(a), Some(b)) => flip(a.cmp(&b)),
                            (None, Some(_)) => std::cmp::Ordering::Greater,
                            (Some(_), None) => std::cmp::Ordering::Less,
                            (None, None) => std::cmp::Ordering::Equal,
                        },
                    }
                    .then(a.frame.cmp(&b.frame))
                });
                let matched = indices.len();
                indices.truncate(limit);
                let returned = indices.len();
                (
                    Data::Frames(indices.into_iter().map(|i| self.frame_detail(i)).collect()),
                    matched,
                    returned,
                )
            }
            Operation::Spans => {
                let mut groups = self.span_groups(&scope, &filter);
                if !["self", "total", "calls", "worst", "name"].contains(&query.sort.as_str()) {
                    return Err("spans --sort needs self, total, calls, worst or name".into());
                }
                groups.sort_by(|a, b| {
                    flip(match query.sort.as_str() {
                        "total" => a.total_us.cmp(&b.total_us),
                        "calls" => a.calls.cmp(&b.calls),
                        "worst" => a.worst_us.cmp(&b.worst_us),
                        "name" => self.trace.name(a.name).cmp(self.trace.name(b.name)),
                        _ => a.self_us.cmp(&b.self_us),
                    })
                    .then_with(|| self.trace.name(a.name).cmp(self.trace.name(b.name)))
                    .then_with(|| {
                        origin_name(self.trace, a.origin).cmp(origin_name(self.trace, b.origin))
                    })
                });
                let matched = groups.len();
                groups.truncate(limit);
                let returned = groups.len();
                (Data::Spans(groups), matched, returned)
            }
            Operation::Counters => {
                if query.sort != "name" {
                    return Err("counters --sort needs name".into());
                }
                let (mut counters, mut gauges) = self.counters(&scope);
                // One shared ordered list/limit for both kinds, not a hidden cap per kind.
                let mut names: Vec<_> = counters
                    .iter()
                    .map(|c| (self.trace.name(c.name), 0, c.name))
                    .chain(gauges.iter().map(|g| (self.trace.name(g.name), 1, g.name)))
                    .collect();
                names.sort_by(|a, b| flip(a.0.cmp(b.0)).then(a.1.cmp(&b.1)));
                let matched = names.len();
                names.truncate(limit);
                let returned = names.len();
                counters.retain(|c| names.iter().any(|n| n.1 == 0 && n.2 == c.name));
                gauges.retain(|g| names.iter().any(|n| n.1 == 1 && n.2 == g.name));
                counters.sort_by(|a, b| flip(self.trace.name(a.name).cmp(self.trace.name(b.name))));
                gauges.sort_by(|a, b| flip(self.trace.name(a.name).cmp(self.trace.name(b.name))));
                (Data::Counters(counters, gauges), matched, returned)
            }
            Operation::Marks => {
                if !["name", "start"].contains(&query.sort.as_str()) {
                    return Err("marks --sort needs name or start".into());
                }
                let mut marks: Vec<_> = self
                    .timeline
                    .marks
                    .iter()
                    .filter(|m| scope.contains(m.t))
                    .copied()
                    .collect();
                marks.sort_by(|a, b| {
                    flip(if query.sort == "start" {
                        a.t.cmp(&b.t)
                    } else {
                        self.trace.name(a.name).cmp(self.trace.name(b.name))
                    })
                    .then(a.t.cmp(&b.t))
                    .then(a.track.0.cmp(&b.track.0))
                });
                let matched = marks.len();
                marks.truncate(limit);
                let returned = marks.len();
                (Data::Marks(marks), matched, returned)
            }
        };
        let mut query = query.clone();
        query.selection = selection;
        query.filter = filter;
        Ok(ResultSet {
            query,
            scope,
            matched,
            returned,
            data,
        })
    }
}

/// Total work needs both the core's end and the driver's rendering cost.
/// Core-only recordings still expose their known core/phase timings, but
/// absence of a present is not evidence of zero rendering work.
pub fn frame_work_us(frame: &Frame) -> Option<u64> {
    let end = frame.end?;
    let present = frame.present?;
    Some(
        end.saturating_sub(frame.begin)
            .saturating_add(present.render),
    )
}

/// A span's frame, including its preceding gap. Ownership is half-open at
/// the core end, independent of the driver's possibly overlapping present.
/// A zero-length frame occupies one clock tick; an unfinished last frame
/// owns the observed tail until its end is known.
pub fn frame_for_time(timeline: &Timeline, t: u64) -> Option<usize> {
    let i = timeline.frames.partition_point(|f| {
        f.end
            .map_or(f.begin, |end| end.max(f.begin.saturating_add(1)))
            <= t
    });
    if i == timeline.frames.len()
        && let Some(f) = timeline.frames.last()
        && f.end.is_none()
        && t >= f.gap_from
        && t <= timeline.end
    {
        return Some(i - 1);
    }
    timeline
        .frames
        .get(i)
        .filter(|f| f.gap_from <= t)
        .map(|_| i)
}

fn frame_end(f: &Frame) -> u64 {
    f.present
        .map_or(f.end_or_begin(), |p| p.t.max(f.end_or_begin()))
        .max(f.begin.saturating_add(1))
}

pub fn milliseconds(ms: f64) -> Result<u64, String> {
    if !ms.is_finite() || ms < 0.0 || ms * 1000.0 >= u64::MAX as f64 {
        return Err(
            "time must be finite, nonnegative milliseconds within the recording clock".into(),
        );
    }
    Ok((ms * 1000.0).round() as u64)
}

pub fn frame_range(s: &str) -> Result<Selection, String> {
    let (a, b) = s.split_once(':').ok_or("frame range needs FIRST:LAST")?;
    let first = a.parse().map_err(|_| "invalid first frame number")?;
    let last = b.parse().map_err(|_| "invalid last frame number")?;
    if first > last {
        return Err("frame range must be increasing".into());
    }
    Ok(Selection::Frames { first, last })
}

pub fn time_range(s: &str) -> Result<Selection, String> {
    fn endpoint(s: &str) -> Result<u64, String> {
        let (n, ms) = if let Some(n) = s.strip_suffix("us") {
            (n, 0.001)
        } else if let Some(n) = s.strip_suffix("ms") {
            (n, 1.0)
        } else if let Some(n) = s.strip_suffix('s') {
            (n, 1000.0)
        } else {
            return Err("time endpoints need us, ms or s units".into());
        };
        let n = n.parse::<f64>().map_err(|_| format!("invalid time {s}"))?;
        milliseconds(n * ms)
    }
    let (a, b) = s
        .split_once(':')
        .ok_or("time range needs FROM:TO with units, as 1s:1500ms")?;
    let from_us = endpoint(a)?;
    let to_us = endpoint(b)?;
    if from_us >= to_us {
        return Err("time range must have positive length".into());
    }
    Ok(Selection::Time { from_us, to_us })
}

/// Shared built-in span filtering, also used by the incremental Lua channels.
pub fn matches_span(trace: &Trace, timeline: &Timeline, s: &Span, filter: &Filter) -> bool {
    s.dur >= filter.min_us
        && filter
            .origins
            .as_ref()
            .is_none_or(|os| os.iter().any(|o| o == origin_name(trace, s.origin)))
        && !filter.hide_names.iter().any(|n| n == trace.name(s.name))
        && filter.name.as_ref().is_none_or(|n| {
            let n = n.to_lowercase();
            trace.name(s.name).to_lowercase().contains(&n)
                || placement_of(timeline, trace, s)
                    .is_some_and(|p| trace.name(p.component).to_lowercase().contains(&n))
        })
}

pub fn matches_frame(unchanged: Option<bool>, verdict: Option<&str>, filter: &Filter) -> bool {
    !(filter.hide_unchanged && unchanged == Some(true))
        && filter
            .verdicts
            .as_ref()
            .is_none_or(|vs| verdict.is_some_and(|v| vs.iter().any(|k| k == v)))
}

#[cfg(test)]
mod tests;
