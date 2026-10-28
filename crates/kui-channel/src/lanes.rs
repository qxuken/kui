//! Intervals of a channel laid on lanes, read a window at a time at a
//! level of detail (ADR 0075, decision 11): what a waterfall or a timeline
//! draws a million spans as without walking them. Built once a revision,
//! in Rust: each lane's intervals sorted by start; and, made on first ask,
//! levels at a threshold an extent over 2⁴ … 2²² — an interval at least
//! that long kept, a run of shorter ones whose gaps are under it merged
//! into one strip — so a window wide or narrow answers its runs in view,
//! about one a pixel, whatever the rows behind them.
//!
//! ```
//! use kui_channel::{Channel, Column};
//!
//! let ch = Channel::table(vec![
//!     ("start".into(), Column::F64(vec![0.0, 1.0, 1.5, 10.0])),
//!     ("len".into(), Column::F64(vec![0.5, 0.2, 0.2, 5.0])),
//!     ("depth".into(), Column::I64(vec![0, 0, 0, 1])),
//! ]).unwrap();
//! let lanes = ch.lanes(0, Some(1), Some(2)).unwrap();
//! assert_eq!((lanes.count(), lanes.extent()), (2, (0.0, 15.0)));
//! let mut runs = Vec::new();
//! lanes.window(0, 0.0, 15.0, 0.001, &mut runs); // every bar
//! assert_eq!(runs.len(), 3);
//! lanes.window(0, 0.0, 15.0, 1.0, &mut runs);   // a pixel of a second
//! assert_eq!(runs.len(), 1, "merged into one strip");
//! assert_eq!(runs[0].count, 3);
//! ```

use std::sync::{Arc, OnceLock};

use crate::{Channel, ChannelError, Column};

/// The coarsest and the finest level: thresholds an extent over 2^k.
const COARSEST: u32 = 4;
const FINEST: u32 = 22;
const LEVELS: usize = (FINEST - COARSEST + 1) as usize;
/// The deepest lane `Channel::lanes` lays out: past it a lane number is an
/// id, and as many lanes would be gigabytes of nothing.
pub const MAX_LANE: usize = 1 << 16;

/// One run in a window: a single interval (its `row` in the channel), or a
/// strip of `count` of them merged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub start: f64,
    pub end: f64,
    /// The interval's row when the run is one; `None` for a strip.
    pub row: Option<usize>,
    pub count: u32,
}

/// Runs sorted by start, with the greatest end so far — what a search for
/// the first run reaching a time reads, overlapping runs or not.
#[derive(Default)]
struct Runs {
    s: Vec<f64>,
    e: Vec<f64>,
    max_e: Vec<f64>,
    row: Vec<u32>,
    count: Vec<u32>,
}

impl Runs {
    fn push(&mut self, s: f64, e: f64, row: Option<usize>, count: u32) {
        let m = self.max_e.last().map_or(e, |m| m.max(e));
        self.s.push(s);
        self.e.push(e);
        self.max_e.push(m);
        self.row.push(row.map_or(u32::MAX, |r| r as u32));
        self.count.push(count);
    }

    fn window(&self, v0: f64, v1: f64, out: &mut Vec<Run>) {
        let from = self.max_e.partition_point(|&m| m < v0);
        for i in from..self.s.len() {
            if self.s[i] > v1 {
                break;
            }
            if self.e[i] < v0 {
                continue;
            }
            out.push(Run {
                start: self.s[i],
                end: self.e[i],
                row: (self.row[i] != u32::MAX).then_some(self.row[i] as usize),
                count: self.count[i],
            });
        }
    }
}

struct Lane {
    raw: Runs,
    levels: [OnceLock<Runs>; LEVELS],
}

impl Lane {
    /// The level at threshold `thr`: what [`Lanes::window`] reads when an
    /// interval narrower than it cannot be told apart.
    fn level(&self, thr: f64) -> Runs {
        let r = &self.raw;
        let mut out = Runs::default();
        let mut i = 0;
        while i < r.s.len() {
            let (s, e) = (r.s[i], r.e[i]);
            if e - s >= thr {
                out.push(s, e, Some(r.row[i] as usize), 1);
                i += 1;
                continue;
            }
            let mut end = e;
            let mut j = i + 1;
            while j < r.s.len() && r.e[j] - r.s[j] < thr && r.s[j] - end < thr {
                end = end.max(r.e[j]);
                j += 1;
            }
            let one = (j == i + 1).then_some(r.row[i] as usize);
            out.push(s, end, one, (j - i) as u32);
            i = j;
        }
        out
    }
}

struct Inner {
    lanes: Vec<Lane>,
    lo: f64,
    hi: f64,
    len: usize,
}

/// A channel's intervals on lanes ([`Channel::lanes`]): cheap to clone,
/// built once.
#[derive(Clone)]
pub struct Lanes(Arc<Inner>);

impl Lanes {
    /// How many lanes: one past the greatest lane a row names.
    pub fn count(&self) -> usize {
        self.0.lanes.len()
    }

    /// How many intervals, every lane's.
    pub fn len(&self) -> usize {
        self.0.len
    }

    pub fn is_empty(&self) -> bool {
        self.0.len == 0
    }

    /// The earliest start and the latest end; `(0, 1)` for none.
    pub fn extent(&self) -> (f64, f64) {
        (self.0.lo, self.0.hi)
    }

    /// Lane `lane`'s runs reaching into `v0..=v1`, into `out` (cleared
    /// first), at the detail where an interval at least `min` long is its
    /// own run — a pixel's width in the intervals' units, say — and
    /// shorter ones whose gaps are under it merge; finer than the finest
    /// level, every interval.
    pub fn window(&self, lane: usize, v0: f64, v1: f64, min: f64, out: &mut Vec<Run>) {
        out.clear();
        let Some(l) = self.0.lanes.get(lane) else {
            return;
        };
        let extent = self.0.hi - self.0.lo;
        let k = if min > 0.0 && extent > 0.0 {
            (extent / min).log2().floor()
        } else {
            f64::INFINITY
        };
        if k > FINEST as f64 {
            l.raw.window(v0, v1, out);
            return;
        }
        let k = (k.max(COARSEST as f64) as u32).min(FINEST);
        let thr = extent / 2f64.powi(k as i32);
        l.levels[(k - COARSEST) as usize]
            .get_or_init(|| l.level(thr))
            .window(v0, v1, out);
    }
}

fn numbers(ch: &Channel, col: usize, what: &str) -> Result<Vec<f64>, ChannelError> {
    match ch.column(col) {
        Some(Column::F64(v)) => Ok(v.clone()),
        Some(Column::I64(v)) => Ok(v.iter().map(|&x| x as f64).collect()),
        Some(other) => Err(ChannelError(format!(
            "the {what} column holds {}s; lanes read numbers",
            other.kind()
        ))),
        None => Err(ChannelError(format!("the channel has no column {col}"))),
    }
}

impl Channel {
    /// The intervals `start` and `start + len` (columns `start` and
    /// `len`; points of no length without `len`) on the lanes column
    /// `lane` names (all on one without it), for [`Lanes::window`]; a
    /// negative lane is lane 0. Numbers or integers each, or an error.
    pub fn lanes(
        &self,
        start: usize,
        len: Option<usize>,
        lane: Option<usize>,
    ) -> Result<Lanes, ChannelError> {
        let s = numbers(self, start, "start")?;
        let l = match len {
            Some(c) => numbers(self, c, "length")?,
            None => vec![0.0; s.len()],
        };
        let d = match lane {
            Some(c) => numbers(self, c, "lane")?,
            None => vec![0.0; s.len()],
        };
        // A lane is a depth, laid out one above the next: a number past
        // the most is an id in the wrong column, not that many lanes.
        if let Some(&x) = d.iter().find(|&&x| x > MAX_LANE as f64) {
            return Err(ChannelError(format!(
                "lane {x} is past the most a channel lays out ({MAX_LANE})"
            )));
        }
        let n = d
            .iter()
            .map(|&x| x.max(0.0) as usize + 1)
            .max()
            .unwrap_or(0);
        let mut by: Vec<Vec<u32>> = vec![Vec::new(); n];
        for (row, &x) in d.iter().enumerate() {
            by[x.max(0.0) as usize].push(row as u32);
        }
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let lanes = by
            .into_iter()
            .map(|mut rows| {
                rows.sort_by(|&a, &b| s[a as usize].total_cmp(&s[b as usize]));
                let mut raw = Runs::default();
                for r in rows {
                    let (a, z) = (s[r as usize], s[r as usize] + l[r as usize].max(0.0));
                    lo = lo.min(a);
                    hi = hi.max(z);
                    raw.push(a, z, Some(r as usize), 1);
                }
                Lane {
                    raw,
                    levels: std::array::from_fn(|_| OnceLock::new()),
                }
            })
            .collect();
        if lo > hi {
            (lo, hi) = (0.0, 1.0);
        }
        Ok(Lanes(Arc::new(Inner {
            lanes,
            lo,
            hi,
            len: s.len(),
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(start: Vec<f64>, len: Vec<f64>, lane: Vec<i64>) -> Channel {
        Channel::table(vec![
            ("start".into(), Column::F64(start)),
            ("len".into(), Column::F64(len)),
            ("lane".into(), Column::I64(lane)),
        ])
        .unwrap()
    }

    /// Rows out of order and on lanes; a window answers the runs reaching
    /// into it, overlapping ones included, at every interval's detail.
    #[test]
    fn a_window_finds_the_runs_reaching_into_it() {
        let c = ch(
            vec![5.0, 0.0, 2.0, 1.0, 9.0],
            vec![1.0, 10.0, 1.0, 0.5, 1.0],
            vec![0, 0, 1, 0, 3],
        );
        let l = c.lanes(0, Some(1), Some(2)).unwrap();
        assert_eq!((l.count(), l.len(), l.extent()), (4, 5, (0.0, 10.0)));
        let mut out = Vec::new();
        l.window(0, 4.0, 4.5, 1e-9, &mut out);
        let rows: Vec<_> = out.iter().map(|r| r.row).collect();
        assert_eq!(rows, [Some(1)], "the long one reaching over");
        l.window(0, 0.0, 10.0, 1e-9, &mut out);
        assert_eq!(
            out.iter().map(|r| r.row.unwrap()).collect::<Vec<_>>(),
            [1, 3, 0]
        );
        l.window(2, 0.0, 10.0, 1e-9, &mut out);
        assert!(out.is_empty(), "an empty lane");
        l.window(7, 0.0, 10.0, 1e-9, &mut out);
        assert!(out.is_empty(), "past the last lane");
    }

    /// A lane number past the deepest is refused, not laid out as that
    /// many empty lanes: an id in the lane column, or one that saturates.
    #[test]
    fn a_lane_past_the_deepest_is_refused() {
        for far in [3e9 as i64, i64::MAX] {
            let c = ch(vec![0.0, 1.0], vec![1.0, 1.0], vec![0, far]);
            let Err(e) = c.lanes(0, Some(1), Some(2)) else {
                panic!("lane {far} laid out");
            };
            assert!(e.0.contains("past the most"), "{}", e.0);
        }
        let c = ch(vec![0.0, 1.0], vec![1.0, 1.0], vec![0, MAX_LANE as i64]);
        assert_eq!(c.lanes(0, Some(1), Some(2)).unwrap().count(), MAX_LANE + 1);
    }

    /// A million intervals a lane, zoomed fully out at a pixel's detail:
    /// a few hundred runs, each strip counting what it holds.
    #[test]
    fn a_million_intervals_answer_a_runs_a_pixel() {
        let n = 1_000_000;
        let c = ch((0..n).map(|i| i as f64).collect(), vec![0.5; n], vec![0; n]);
        let l = c.lanes(0, Some(1), Some(2)).unwrap();
        let mut out = Vec::new();
        let px = n as f64 / 1200.0;
        l.window(0, 0.0, n as f64, px, &mut out);
        assert!(out.len() <= 1300, "{} runs", out.len());
        assert_eq!(out.iter().map(|r| r.count as usize).sum::<usize>(), n);
        l.window(0, 1000.0, 1010.0, 0.001, &mut out);
        assert_eq!(out.len(), 11, "close in, the intervals themselves");
        assert!(out.iter().all(|r| r.row.is_some() && r.count == 1));
    }

    #[test]
    fn lanes_read_numbers() {
        let c = Channel::table(vec![("s".into(), Column::strs(["a"]))]).unwrap();
        assert!(c.lanes(0, None, None).is_err());
    }
}
