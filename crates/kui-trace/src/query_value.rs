//! Presentation values shared by the CLI and Lua. All time quantities are ms,
//! as stated in each result envelope. No formatted strings hide measurements.

use crate::analysis::Verdict;
use crate::query::*;
use crate::{Session, Trace};
use kui_value::Value as V;

pub fn number(v: u64) -> V {
    // Preserve identifiers even outside JSON's signed integer representation.
    i64::try_from(v).map_or_else(|_| V::str(v.to_string()), V::Int)
}
pub fn ms(v: u64) -> V {
    V::Float(v as f64 / 1000.0)
}
fn opt<T>(v: Option<T>, f: impl FnOnce(T) -> V) -> V {
    v.map_or(V::Null, f)
}
fn list<T>(vs: impl IntoIterator<Item = T>, f: impl Fn(T) -> V) -> V {
    V::List(vs.into_iter().map(f).collect())
}
fn strings(v: &[String]) -> V {
    list(v, V::str)
}

pub fn selection(s: &Selection) -> V {
    match s {
        Selection::Whole => V::map([("kind", V::str("whole"))]),
        Selection::Frames { first, last } => V::map([
            ("kind", V::str("frames")),
            ("first", number(*first)),
            ("last", number(*last)),
        ]),
        Selection::Time { from_us, to_us } => V::map([
            ("kind", V::str("time")),
            ("from", ms(*from_us)),
            ("to", ms(*to_us)),
        ]),
    }
}

pub fn scope(timeline: &crate::Timeline, s: &Scope) -> V {
    V::map([
        ("requested", selection(&s.requested)),
        ("frame_count", s.indices.len().into()),
        (
            "first_frame",
            opt(s.indices.first(), |&i| number(timeline.frames[i].frame)),
        ),
        (
            "last_frame",
            opt(s.indices.last(), |&i| number(timeline.frames[i].frame)),
        ),
        ("duration", ms(s.duration_us())),
        ("from", opt(s.intervals.first(), |&(a, _)| ms(a))),
        ("to", opt(s.intervals.last(), |&(_, b)| ms(b))),
        ("interval_count", s.intervals.len().into()),
        (
            "elapsed",
            opt(
                s.intervals.first().zip(s.intervals.last()),
                |(&(a, _), &(_, b))| ms(b.saturating_sub(a)),
            ),
        ),
    ])
}

pub fn filter(f: &Filter) -> V {
    V::map([
        ("name", opt(f.name.as_ref(), V::str)),
        ("origins", opt(f.origins.as_ref(), |s| strings(s))),
        ("hide_names", strings(&f.hide_names)),
        ("min_duration", ms(f.min_us)),
        ("verdicts", opt(f.verdicts.as_ref(), |s| strings(s))),
        ("hide_unchanged", f.hide_unchanged.into()),
        (
            "span_ids",
            opt(f.span_ids.as_ref(), |s| list(s, |&id| id.into())),
        ),
    ])
}

pub fn session(s: Option<&Session>) -> V {
    opt(s, |s| {
        V::map([
            ("kui", V::str(&s.kui)),
            ("app", V::str(&s.app)),
            ("platform", V::str(&s.platform)),
            ("refresh_hz", s.refresh_hz.into()),
            (
                "budget",
                if s.budget_ms > 0.0 && s.budget_ms.is_finite() {
                    s.budget_ms.into()
                } else {
                    V::Null
                },
            ),
            ("scale", s.scale.into()),
            ("wall_ms", number(s.wall_ms)),
        ])
    })
}

pub fn distribution(d: &Distribution) -> V {
    V::map([
        ("samples", d.samples.into()),
        ("avg", opt(d.avg_us, |v| (v / 1000.0).into())),
        ("p50", opt(d.p50_us, ms)),
        ("p95", opt(d.p95_us, ms)),
        ("p99", opt(d.p99_us, ms)),
        ("max", opt(d.max_us, ms)),
    ])
}

pub fn summary(s: &Summary) -> V {
    V::map([
        ("frames", s.frames.into()),
        ("complete_frames", s.complete_frames.into()),
        ("seconds", (s.duration_us as f64 / 1_000_000.0).into()),
        ("duration", ms(s.duration_us)),
        ("budget", opt(s.budget_us, ms)),
        ("work", distribution(&s.work)),
        ("work_avg", opt(s.work.avg_us, |v| (v / 1000.0).into())),
        ("work_p95", opt(s.work.p95_us, ms)),
        ("work_max", opt(s.work.max_us, ms)),
        ("pacing", distribution(&s.pacing)),
        (
            "phases",
            V::Map(
                s.phases
                    .iter()
                    .map(|(n, d)| (n.to_string(), distribution(d)))
                    .collect(),
            ),
        ),
        ("over_budget", opt(s.over_budget, V::from)),
        ("over_budget_percent", opt(s.over_budget_percent(), V::from)),
        ("unchanged", s.unchanged.into()),
    ])
}

pub fn verdict(trace: &Trace, v: Option<&Verdict>) -> V {
    opt(v, |v| {
        let mut fields = vec![("kind".into(), V::str(verdict_kind(v)))];
        let extra = match v {
            Verdict::Late { wait, slept } => vec![("wait", ms(*wait)), ("slept", (*slept).into())],
            Verdict::Blocked { busy } => vec![("busy", ms(*busy))],
            Verdict::Slow {
                work,
                phase,
                phase_us,
                span,
            } => vec![
                ("work", ms(*work)),
                ("phase", V::str(*phase)),
                ("phase_ms", ms(*phase_us)),
                ("span", opt(*span, |(n, _)| V::str(trace.name(n)))),
                ("span_ms", opt(*span, |(_, t)| ms(t))),
            ],
            Verdict::Waited { wall, cpu } => vec![("wall", ms(*wall)), ("cpu", ms(*cpu))],
            Verdict::Burn { k, of } => vec![("k", (*k).into()), ("of", (*of).into())],
            Verdict::Fine { share } => vec![("share", (*share).into())],
            Verdict::Wasted => vec![],
        };
        fields.extend(extra.into_iter().map(|(k, v)| (k.into(), v)));
        V::Map(fields)
    })
}

pub fn frame(trace: &Trace, d: &FrameDetail) -> V {
    let f = &d.frame;
    V::map([
        ("frame", number(f.frame)),
        ("index", d.index.into()),
        ("begin", ms(f.begin)),
        ("finish", opt(f.end, ms)),
        ("gap", ms(f.begin.saturating_sub(f.gap_from))),
        ("work", opt(frame_work_us(f), ms)),
        ("nodes", f.nodes.into()),
        ("unchanged", opt(f.unchanged, V::from)),
        (
            "phases",
            V::Map(
                d.phases
                    .named()
                    .into_iter()
                    .map(|(n, t)| {
                        (
                            n.to_string(),
                            if f.end.is_none()
                                || ((n == "render" || n == "vsync wait") && f.present.is_none())
                            {
                                V::Null
                            } else {
                                ms(t)
                            },
                        )
                    })
                    .collect(),
            ),
        ),
        ("verdict", verdict(trace, d.verdict.as_ref())),
        ("causes", list(&f.causes, |&n| V::str(trace.name(n)))),
        (
            "holders",
            list(&f.holders, |h| {
                V::map([
                    ("what", V::str(trace.name(h.what))),
                    ("name", V::str(trace.name(h.name))),
                    ("node", V::str(format!("{:x}", h.node))),
                ])
            }),
        ),
        (
            "path",
            opt(d.path, |p| {
                V::map([
                    ("input", V::str(trace.name(p.input.kind))),
                    ("endpoint", V::str(p.endpoint())),
                    ("total", ms(p.total())),
                    (
                        "legs",
                        list(p.legs(), |(what, t)| {
                            V::map([("what", V::str(what)), ("ms", ms(t))])
                        }),
                    ),
                ])
            }),
        ),
        (
            "fresh",
            list(&d.fresh, |p| {
                V::map([
                    ("component", V::str(trace.name(p.component))),
                    ("answer", V::str(trace.name(p.answer))),
                    ("why", V::str(trace.name(p.why))),
                    ("node", V::str(format!("{:x}", p.node))),
                ])
            }),
        ),
        ("notes", strings(&d.notes)),
        (
            "counters",
            V::Map(
                f.counters
                    .iter()
                    .map(|&(n, v)| (trace.name(n).into(), v.into()))
                    .collect(),
            ),
        ),
    ])
}

pub fn span(trace: &Trace, d: &SpanDetail) -> V {
    let s = &d.span;
    V::map([
        ("id", s.id.into()),
        ("parent", s.parent.into()),
        ("children", list(&d.children, |&id| id.into())),
        ("name", V::str(trace.name(d.name))),
        ("origin", V::str(origin_name(trace, s.origin))),
        ("start", ms(s.start)),
        ("len", ms(s.dur)),
        ("self", ms(d.self_us)),
        ("cpu", opt(s.cpu, ms)),
        ("node", V::str(format!("{:x}", s.node))),
        ("cut", s.cut.into()),
        ("track", V::str(trace.track_name(s.track))),
        ("depth", d.depth.into()),
        ("frame", opt(d.frame, number)),
        (
            "component",
            opt(d.placement, |p| V::str(trace.name(p.component))),
        ),
        ("answer", opt(d.placement, |p| V::str(trace.name(p.answer)))),
        ("why", opt(d.placement, |p| V::str(trace.name(p.why)))),
        ("span", V::str(trace.name(s.name))),
    ])
}

pub fn span_group(trace: &Trace, g: &SpanGroup, frames: usize) -> V {
    V::map([
        ("name", V::str(trace.name(g.name))),
        ("origin", V::str(origin_name(trace, g.origin))),
        ("calls", number(g.calls)),
        ("total", ms(g.total_us)),
        ("self", ms(g.self_us)),
        ("worst", ms(g.worst_us)),
        ("cpu", opt(g.cpu_us, ms)),
        ("example_id", g.example_id.into()),
        (
            "per_frame",
            if frames > 0 {
                (g.total_us as f64 / 1000.0 / frames as f64).into()
            } else {
                V::Null
            },
        ),
        (
            "self_per_frame",
            if frames > 0 {
                (g.self_us as f64 / 1000.0 / frames as f64).into()
            } else {
                V::Null
            },
        ),
        (
            "calls_per_frame",
            if frames > 0 {
                (g.calls as f64 / frames as f64).into()
            } else {
                V::Null
            },
        ),
    ])
}

pub fn counters(trace: &Trace, cs: &[Counter], gs: &[Gauge]) -> V {
    V::map([
        (
            "counters",
            list(cs, |c| {
                V::map([
                    ("name", V::str(trace.name(c.name))),
                    ("avg", c.avg.into()),
                    ("max", c.max.into()),
                ])
            }),
        ),
        (
            "gauges",
            list(gs, |g| {
                V::map([
                    ("name", V::str(trace.name(g.name))),
                    ("last", g.last.into()),
                    ("least", g.least.into()),
                    ("most", g.most.into()),
                ])
            }),
        ),
    ])
}

/// Recorded clocks describe core/driver work, not GPU completion or photons.
pub fn timing_scope() -> V {
    V::map([
        ("work", V::str("core_frame_and_driver_render")),
        ("pacing", V::str("frame_begin_intervals")),
        ("present", V::str("driver_present_record")),
        ("input", V::str("core_input_receipt")),
        ("gpu_completion", V::Null),
        ("physical_display", V::Null),
    ])
}

pub fn result(trace: &Trace, timeline: &crate::Timeline, r: &ResultSet, recording: &str) -> V {
    let data = match &r.data {
        Data::Summary(s) => summary(s),
        Data::Frames(fs) => list(fs, |f| frame(trace, f)),
        Data::Frame(f) => frame(trace, f),
        Data::Spans(ss) => list(ss, |g| span_group(trace, g, r.scope.indices.len())),
        Data::Span(s) => span(trace, s),
        Data::Counters(cs, gs) => counters(trace, cs, gs),
        Data::Marks(ms0) => list(ms0, |m| {
            V::map([
                ("name", V::str(trace.name(m.name))),
                ("at", ms(m.t)),
                ("track", V::str(trace.track_name(m.track))),
            ])
        }),
    };
    V::map([
        ("schema_version", 1.into()),
        ("operation", V::str(r.query.operation.name())),
        ("recording", V::str(recording)),
        ("time_unit", V::str("ms")),
        ("timing_scope", timing_scope()),
        ("session", session(trace.session.as_ref())),
        ("scope", scope(timeline, &r.scope)),
        ("filters", filter(&r.query.filter)),
        ("sort", V::str(&r.query.sort)),
        ("descending", r.query.descending.into()),
        ("limit", opt(r.query.limit, V::from)),
        ("matched", r.matched.into()),
        ("returned", r.returned.into()),
        ("omitted", r.matched.saturating_sub(r.returned).into()),
        ("truncated", (r.returned < r.matched).into()),
        (
            "limitations",
            V::List(vec![
                V::str("The recording format does not report dropped-event counts."),
                V::str("Incomplete frame timings and unavailable measurements are null."),
                V::str(
                    "Present records do not certify GPU completion or physical display; native render timings are CPU-side encoding/submission.",
                ),
            ]),
        ),
        ("data", data),
    ])
}

pub fn readings(
    trace: &Trace,
    timeline: &crate::Timeline,
    s: &Scope,
    f: &Filter,
    r: &Readings,
) -> V {
    let V::Map(mut fields) = counters(trace, &r.counters, &r.gauges) else {
        unreachable!()
    };
    fields.extend([
        ("frame_indices".into(), list(&s.indices, |&i| i.into())),
        ("time_unit".into(), V::str("ms")),
        ("timing_scope".into(), timing_scope()),
        ("scope".into(), scope(timeline, s)),
        ("filters".into(), filter(f)),
        ("summary".into(), summary(&r.summary)),
        (
            "spans".into(),
            list(&r.spans, |g| span_group(trace, g, s.indices.len())),
        ),
        ("matched".into(), r.spans.len().into()),
        ("returned".into(), r.spans.len().into()),
        ("truncated".into(), false.into()),
        (
            "origins".into(),
            list(&r.origins, |&(o, t, n)| {
                V::map([
                    ("origin", V::str(origin_name(trace, o))),
                    ("self", ms(t)),
                    ("spans", number(n)),
                ])
            }),
        ),
        (
            "components".into(),
            list(&r.components, |p| {
                V::map([
                    ("component", V::str(trace.name(p.component))),
                    ("placed", number(p.placed)),
                    ("replayed", number(p.replayed)),
                    (
                        "fresh",
                        list(&p.fresh, |&(a, w, n)| {
                            V::map([
                                ("answer", V::str(trace.name(a))),
                                ("why", V::str(trace.name(w))),
                                ("count", number(n)),
                            ])
                        }),
                    ),
                ])
            }),
        ),
        (
            "burns".into(),
            list(&r.burns, |b| {
                V::map([
                    ("first", b.first.into()),
                    ("last", b.last.into()),
                    ("frames", b.len().into()),
                ])
            }),
        ),
        (
            "gaps".into(),
            V::map([
                ("total", ms(r.gaps.total)),
                ("handlers", ms(r.gaps.handlers)),
                ("asleep", ms(r.gaps.asleep)),
                ("busy", ms(r.gaps.busy)),
            ]),
        ),
    ]);
    V::Map(fields)
}

pub fn comparison(
    before: &Explorer<'_>,
    before_path: &str,
    after: &Explorer<'_>,
    after_path: &str,
    c: &Comparison,
) -> V {
    V::map([
        ("schema_version", 1.into()),
        ("operation", V::str("compare")),
        ("sort", V::str("overview")),
        ("time_unit", V::str("ms")),
        (
            "before",
            result(before.trace, before.timeline, &c.before, before_path),
        ),
        (
            "after",
            result(after.trace, after.timeline, &c.after, after_path),
        ),
        ("limit", opt(c.limit, V::from)),
        ("matched", c.matched.into()),
        ("returned", c.rows.len().into()),
        ("omitted", c.matched.saturating_sub(c.rows.len()).into()),
        ("truncated", (c.rows.len() < c.matched).into()),
        (
            "data",
            list(&c.rows, |r| {
                V::map([
                    ("metric", V::str(&r.metric)),
                    ("unit", V::str(r.unit)),
                    ("before", opt(r.before, V::from)),
                    ("after", opt(r.after, V::from)),
                    ("delta", opt(r.delta, V::from)),
                    ("percent", opt(r.percent, V::from)),
                ])
            }),
        ),
    ])
}
