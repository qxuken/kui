//! Two recordings compared (ADR 0058, decision 12): each span's and
//! phase's spread, the burns, the gaps and the counters, before and after
//! — `kui-trace diff A B`, with `--fail-over` a CI's threshold.
//!
//! Spans are matched by their name's text and origin, never by id: ids
//! and name ids differ between two recordings of one session. What a span
//! or a gap costs is a frame's share (its total over the frames), so two
//! recordings of one script compare even when one ran a few frames
//! longer.
//!
//! [`Report::worst_growth`] reads only the rows that say what a frame
//! costs and that were big enough before to say it ([`Unit::counts`]):
//! a time of at least [`NOISE_US`] (below it, a timer's jitter is a large
//! share of nothing), and a rate of at least one a frame. A maximum, a
//! count, a share and vsync's wait are shown and never counted: one
//! outlier frame, a recording's length and a faster render (which waits
//! longer) are not regressions.

use std::collections::HashMap;
use std::fmt::Write;

use crate::adapters::origin_label;
use crate::analysis::{self, Phases};
use crate::model::Trace;

/// Below this many µs before, a time row's growth is noise.
pub const NOISE_US: f64 = 50.0;
/// Below this many a frame before, a rate row's growth is noise.
pub const NOISE_RATE: f64 = 1.0;
/// The growth [`Report::text`] marks.
pub const MARK: f64 = 0.10;

/// What a row counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// µs a frame costs: an average, a p95, a share of a total.
    Us,
    /// µs shown and not counted: a maximum, vsync's wait, time asleep.
    UsShown,
    /// How many a frame: calls, a counter's average.
    Rate,
    /// How many in the recording: frames, burns.
    Count,
    /// A fraction of the frames.
    Share,
}

impl Unit {
    /// Whether a row of this unit, `before` this, counts towards
    /// [`Report::worst_growth`].
    pub fn counts(self, before: f64) -> bool {
        match self {
            Unit::Us => before >= NOISE_US,
            Unit::Rate => before >= NOISE_RATE,
            Unit::UsShown | Unit::Count | Unit::Share => false,
        }
    }

    fn show(self, v: f64) -> String {
        match self {
            Unit::Us | Unit::UsShown => format!("{:.3} ms", v / 1000.0),
            Unit::Rate => format!("{v:.2}"),
            Unit::Count => format!("{v:.0}"),
            Unit::Share => format!("{:.1}%", v * 100.0),
        }
    }
}

/// What changed from `a` to `b`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// `(what, before, after)`, µs or a count.
    pub rows: Vec<(String, f64, f64)>,
    /// Each row's unit, by index; a row past its end is [`Unit::Us`].
    pub units: Vec<Unit>,
}

impl Report {
    /// Adds a row.
    pub fn push(&mut self, what: impl Into<String>, unit: Unit, before: f64, after: f64) {
        self.rows.push((what.into(), before, after));
        self.units.push(unit);
    }

    /// Row `i`'s unit.
    pub fn unit(&self, i: usize) -> Unit {
        self.units.get(i).copied().unwrap_or(Unit::Us)
    }

    /// Row `i`'s growth as a fraction of its before, when it counts.
    pub fn growth(&self, i: usize) -> Option<f64> {
        let (_, a, b) = self.rows[i];
        self.unit(i).counts(a).then(|| (b - a) / a)
    }

    /// The report as text, a row a line, marking a counted row that grew
    /// or shrank past [`MARK`].
    pub fn text(&self) -> String {
        self.text_with(MARK)
    }

    /// The report as text, marking a counted row whose change passes
    /// `threshold` (a fraction): `▲` grew, `▼` shrank.
    pub fn text_with(&self, threshold: f64) -> String {
        let cells: Vec<[String; 4]> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, (what, a, b))| {
                let u = self.unit(i);
                let change = if *a == 0.0 && *b == 0.0 {
                    String::new()
                } else if *a == 0.0 {
                    "new".into()
                } else {
                    let g = (b - a) / a;
                    let mark = match self.growth(i) {
                        Some(g) if g > threshold => " ▲",
                        Some(g) if g < -threshold => " ▼",
                        _ => "",
                    };
                    format!("{:+.1}%{mark}", g * 100.0)
                };
                [what.clone(), u.show(*a), u.show(*b), change]
            })
            .collect();
        let head = ["what", "before", "after", "change"].map(String::from);
        let width = |k: usize| {
            cells
                .iter()
                .chain(std::iter::once(&head))
                .map(|c| c[k].chars().count())
                .max()
                .unwrap_or(0)
        };
        let w = [width(0), width(1), width(2)];
        let mut out = String::new();
        for c in std::iter::once(&head).chain(cells.iter()) {
            let line = format!(
                "{:<w0$}  {:>w1$}  {:>w2$}  {}",
                c[0],
                c[1],
                c[2],
                c[3],
                w0 = w[0],
                w1 = w[1],
                w2 = w[2]
            );
            let _ = writeln!(out, "{}", line.trim_end());
        }
        out
    }

    /// The largest growth of any row, as a fraction of its before: over
    /// the rows that count ([`Unit::counts`]), `0` when none does.
    pub fn worst_growth(&self) -> f64 {
        (0..self.rows.len())
            .filter_map(|i| self.growth(i))
            .fold(None, |m: Option<f64>, g| Some(m.map_or(g, |m| m.max(g))))
            .unwrap_or(0.0)
    }
}

/// A recording's rows, in the order a report shows them.
fn rows(trace: &Trace) -> Vec<(String, Unit, f64)> {
    let tl = trace.timeline();
    let n = tl.frames.len();
    let per = n.max(1) as f64;
    let mut out: Vec<(String, Unit, f64)> = Vec::new();
    let mut push = |what: String, u: Unit, v: f64| out.push((what, u, v));

    push("frames".into(), Unit::Count, n as f64);
    let phases: Vec<Phases> = (0..n).map(|i| analysis::phases(&tl, i, trace)).collect();
    for (k, (name, _)) in Phases::default().named().iter().enumerate() {
        let v: Vec<u64> = phases.iter().map(|p| p.named()[k].1).collect();
        let (avg, p95, max) = analysis::spread(&v);
        let u = if *name == "vsync wait" {
            Unit::UsShown
        } else {
            Unit::Us
        };
        push(format!("phase {name} avg"), u, avg);
        push(format!("phase {name} p95"), u, p95 as f64);
        push(format!("phase {name} max"), Unit::UsShown, max as f64);
    }
    let work: Vec<u64> = tl.frames.iter().map(|f| f.work_us()).collect();
    let (avg, p95, max) = analysis::spread(&work);
    push("frame work avg".into(), Unit::Us, avg);
    push("frame work p95".into(), Unit::Us, p95 as f64);
    push("frame work max".into(), Unit::UsShown, max as f64);
    let traced = tl.frames.iter().filter(|f| f.unchanged.is_some()).count();
    let unchanged = tl
        .frames
        .iter()
        .filter(|f| f.unchanged == Some(true))
        .count();
    push(
        "unchanged share".into(),
        Unit::Share,
        unchanged as f64 / traced.max(1) as f64,
    );
    let burns = analysis::burns(&tl, trace);
    push("burns".into(), Unit::Count, burns.len() as f64);
    push(
        "burn frames".into(),
        Unit::Count,
        burns.iter().map(|b| b.len()).sum::<usize>() as f64,
    );
    let g = analysis::gaps(&tl, trace);
    push("gap µs/frame".into(), Unit::UsShown, g.total as f64 / per);
    push(
        "gap handlers µs/frame".into(),
        Unit::Us,
        g.handlers as f64 / per,
    );
    push(
        "gap asleep µs/frame".into(),
        Unit::UsShown,
        g.asleep as f64 / per,
    );
    push("gap busy µs/frame".into(), Unit::Us, g.busy as f64 / per);
    for st in analysis::by_span(&tl, &tl.spans) {
        let what = format!(
            "span {} [{}]",
            trace.name(st.name),
            origin_label(trace, st.origin)
        );
        push(
            format!("{what} calls/frame"),
            Unit::Rate,
            st.calls as f64 / per,
        );
        push(format!("{what} µs/frame"), Unit::Us, st.total as f64 / per);
        push(format!("{what} worst"), Unit::UsShown, st.worst as f64);
    }
    for (name, avg, _) in analysis::counters(&tl) {
        push(format!("counter {} avg", trace.name(name)), Unit::Rate, avg);
    }
    out
}

/// `a` against `b`.
pub fn diff(a: &Trace, b: &Trace) -> Report {
    let ra = rows(a);
    let rb = rows(b);
    let after: HashMap<&str, f64> = rb.iter().map(|(w, _, v)| (w.as_str(), *v)).collect();
    let before: HashMap<&str, f64> = ra.iter().map(|(w, _, v)| (w.as_str(), *v)).collect();
    let mut report = Report::default();
    for (what, u, v) in &ra {
        report.push(
            what.clone(),
            *u,
            *v,
            after.get(what.as_str()).copied().unwrap_or(0.0),
        );
    }
    for (what, u, v) in &rb {
        if !before.contains_key(what.as_str()) {
            report.push(what.clone(), *u, 0.0, *v);
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Origin, Record, Span, Track};

    /// `n` frames 16 ms apart, each a `build` of `build_us` with a `rows`
    /// child of `rows_us`, a counter, and a present.
    fn recording(n: u64, build_us: u64, rows_us: u64) -> Trace {
        let mut t = Trace::new();
        let (key, _) = t.names.get("key");
        let (build, _) = t.names.get("build");
        let (rows, _) = t.names.get("rows");
        let (laid, _) = t.names.get("laid");
        for i in 0..n {
            let at = i * 16_000;
            t.records.push(Record::FrameBegin {
                t: at,
                frame: i + 1,
                causes: vec![key],
            });
            let id = i as u32 * 2 + 1;
            t.records.push(Record::Span(Span {
                track: Track::MAIN,
                id: id + 1,
                parent: id,
                name: rows,
                origin: Origin::Host,
                start: at + 20,
                dur: rows_us,
                cpu: None,
                node: 0,
                cut: false,
            }));
            t.records.push(Record::Span(Span {
                track: Track::MAIN,
                id,
                parent: 0,
                name: build,
                origin: Origin::Kui,
                start: at + 10,
                dur: build_us,
                cpu: Some(build_us),
                node: 0,
                cut: false,
            }));
            t.records.push(Record::Counter {
                t: at + 10 + build_us,
                frame: i + 1,
                name: laid,
                value: 40,
            });
            t.records.push(Record::FrameEnd {
                t: at + 10 + build_us,
                frame: i + 1,
                unchanged: Some(false),
                nodes: 10,
            });
            t.records.push(Record::Present {
                t: at + 12_000,
                frame: i + 1,
                render: 1_000,
                wait: 500,
            });
        }
        t
    }

    fn row<'a>(r: &'a Report, what: &str) -> (usize, &'a (String, f64, f64)) {
        r.rows
            .iter()
            .enumerate()
            .find(|(_, row)| row.0 == what)
            .unwrap_or_else(|| panic!("no row {what}:\n{}", r.text()))
    }

    #[test]
    fn identical_recordings_grow_nothing() {
        let a = recording(20, 2_000, 600);
        let r = diff(&a, &a.clone());
        assert_eq!(r.worst_growth(), 0.0);
        assert!(r.rows.iter().all(|(_, a, b)| a == b));
        assert_eq!(r.rows.len(), r.units.len());
    }

    #[test]
    fn a_span_that_doubles_is_the_worst_growth() {
        let a = recording(20, 2_000, 600);
        let b = recording(20, 2_000, 1_200);
        let r = diff(&a, &b);
        let (i, (_, before, after)) = row(&r, "span rows [host] µs/frame");
        assert_eq!((*before, *after), (600.0, 1_200.0));
        assert!((r.growth(i).unwrap() - 1.0).abs() < 1e-9);
        assert!(r.worst_growth() > 0.9, "{}", r.text());
        let text = r.text();
        let line = text
            .lines()
            .find(|l| l.starts_with("span rows [host] µs/frame"))
            .unwrap();
        assert!(line.ends_with("+100.0% ▲"), "{line}");
        // Matched by text: the same names given other ids still match.
        let mut c = recording(20, 2_000, 600);
        c.names = crate::Names::new();
        for n in ["pad", "key", "build", "rows", "laid"] {
            c.names.get(n);
        }
        for r in &mut c.records {
            match r {
                Record::Span(s) => s.name += 1,
                Record::FrameBegin { causes, .. } => causes.iter_mut().for_each(|c| *c += 1),
                Record::Counter { name, .. } => *name += 1,
                _ => {}
            }
        }
        assert_eq!(diff(&a, &c).worst_growth(), 0.0, "{}", diff(&a, &c).text());
    }

    #[test]
    fn a_tiny_time_and_a_maximum_do_not_count() {
        // 10 µs → 40 µs is +300%, and below the noise floor.
        let a = recording(20, 2_000, 10);
        let b = recording(20, 2_000, 40);
        let r = diff(&a, &b);
        let (i, _) = row(&r, "span rows [host] µs/frame");
        assert_eq!(r.growth(i), None);
        let (i, _) = row(&r, "span rows [host] worst");
        assert_eq!(r.unit(i), Unit::UsShown);
        assert!(r.worst_growth() < 0.01, "{}", r.text());
    }

    #[test]
    fn a_span_only_after_is_new() {
        let a = recording(5, 2_000, 600);
        let mut b = a.clone();
        let (fill, _) = b.names.get("fill tabs");
        b.records.push(Record::Span(Span {
            track: Track::MAIN,
            id: 999,
            parent: 0,
            name: fill,
            origin: Origin::Kui,
            start: 100,
            dur: 300,
            cpu: None,
            node: 0,
            cut: false,
        }));
        let r = diff(&a, &b);
        let (_, row) = row(&r, "span fill tabs [kui] µs/frame");
        assert_eq!((row.1, row.2), (0.0, 60.0));
        assert!(r.text().contains("new"));
    }
}
