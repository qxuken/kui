//! Strokes: what a `line` node draws (`docs/adr/0010-a-segment-primitive.md`).
//!
//! A line is a run of points and a [`Stroke`]; the core flattens a curve
//! into a polyline here, boxes the run, and emits one
//! [`crate::QuadKind::Segment`] per straight piece. The points live in a
//! per-frame list beside the tree — rebuilt every frame, capacities kept —
//! and a node refers to its run by [`LineId`] the way a text node refers to
//! the frame's text list. The previous frame's list is kept by the same
//! gated buffer swap the text list uses, so a departing line can copy its
//! points out for its ghost.

use crate::color::Color;
use crate::geom::{Rect, Vec2};

/// Index into the frame's line list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineId(pub u32);

/// How a `line` is drawn: a width, a colour, and whether the points are
/// the corners of a polyline or the knots of a curve through them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    /// Stroke width in logical px. Round caps at both ends of every
    /// segment, which is also the join between two of them.
    pub width: f32,
    /// The stroke colour. Inside the core it rides in the node's `bg`
    /// slot, so a `transition` eases it and `enter` / `exit` can start
    /// or end it there.
    pub color: Color,
    /// Draw a smooth curve *through* the points (a centripetal Catmull-Rom
    /// spline, flattened by [`flatten_curve`]) instead of the polyline. Two
    /// points are a straight segment either way.
    pub curve: bool,
}

impl Stroke {
    pub fn new(width: f32, color: Color) -> Self {
        Self {
            width,
            color,
            curve: false,
        }
    }

    pub fn curve(mut self) -> Self {
        self.curve = true;
        self
    }
}

/// One stroke's run in the frame's point list. The points are stored
/// relative to the node's box, so a node that eases or slides carries
/// them along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Run {
    pub first: u32,
    pub len: u32,
    /// Logical px.
    pub width: f32,
}

/// The frame's strokes, and the previous frame's while an `exit` needs it.
#[derive(Default)]
pub struct LineStore {
    runs: Vec<Run>,
    points: Vec<Vec2>,
    prev_runs: Vec<Run>,
    prev_points: Vec<Vec2>,
}

/// How far a stroke's box extends past its points: half the width, plus
/// two logical px so a backend's edge ramp (`AA` = 0.75 physical px each
/// side, sampled at pixel centres) is never cut by the quad, at scale 1
/// included.
pub(crate) fn pad(width: f32) -> f32 {
    width.max(0.0) * 0.5 + 2.0
}

/// A curve span is flattened into one piece per this many logical px of
/// chord, at least one and at most [`CURVE_MAX_PIECES`]. Fixed rather than
/// tolerance-driven so the piece count is a function of the declared
/// geometry alone — every binding gets the same segments, and the
/// conformance corpus can pin them.
pub const CURVE_STEP: f32 = 6.0;
pub const CURVE_MAX_PIECES: usize = 32;

impl LineStore {
    /// Starts a frame. `keep_prev` retains the list just finished so a
    /// departing line's ghost can copy its points out of it.
    pub(crate) fn begin_frame(&mut self, keep_prev: bool) {
        if keep_prev {
            std::mem::swap(&mut self.runs, &mut self.prev_runs);
            std::mem::swap(&mut self.points, &mut self.prev_points);
        } else {
            self.prev_runs.clear();
            self.prev_points.clear();
        }
        self.runs.clear();
        self.points.clear();
    }

    /// Adds a stroke through `points` (parent-box coordinates): flattens a
    /// curve, boxes the run, and stores the points relative to the box.
    /// Returns the id and the box, or None for fewer than two points,
    /// which draw nothing.
    pub(crate) fn push(&mut self, points: &[Vec2], stroke: Stroke) -> Option<(LineId, Rect)> {
        if points.len() < 2 {
            return None;
        }
        let first = self.points.len();
        if stroke.curve && points.len() > 2 {
            flatten_curve(points, &mut self.points);
        } else {
            self.points.extend_from_slice(points);
        }
        let run = &mut self.points[first..];
        let (mut min, mut max) = (run[0], run[0]);
        for p in run.iter() {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        let pad = pad(stroke.width);
        let origin = Vec2::new(min.x - pad, min.y - pad);
        for p in run.iter_mut() {
            p.x -= origin.x;
            p.y -= origin.y;
        }
        let rect = Rect::new(
            origin.x,
            origin.y,
            max.x - min.x + 2.0 * pad,
            max.y - min.y + 2.0 * pad,
        );
        let id = LineId(self.runs.len() as u32);
        self.runs.push(Run {
            first: first as u32,
            len: (self.points.len() - first) as u32,
            width: stroke.width,
        });
        Some((id, rect))
    }

    /// This frame's run: its width and its points, relative to the node.
    pub(crate) fn run(&self, id: LineId) -> (Run, &[Vec2]) {
        let run = self.runs[id.0 as usize];
        (
            run,
            &self.points[run.first as usize..(run.first + run.len) as usize],
        )
    }

    /// The same, read from the previous frame's list (a departing line's
    /// id indexes that list, not this frame's). Empty for an id the kept
    /// frame does not have, which cannot happen while `begin_frame` keeps
    /// the two in step.
    pub(crate) fn prev_run(&self, id: LineId) -> (Run, &[Vec2]) {
        match self.prev_runs.get(id.0 as usize) {
            Some(run) => (
                *run,
                &self.prev_points[run.first as usize..(run.first + run.len) as usize],
            ),
            None => (
                Run {
                    first: 0,
                    len: 0,
                    width: 0.0,
                },
                &[],
            ),
        }
    }

    /// Runs this frame.
    pub fn len(&self) -> usize {
        self.runs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
}

/// Flattens a **centripetal** Catmull-Rom spline through `knots` (at least
/// three) into `out`, starting at the first knot and ending at the last.
/// Each span is cut into `ceil(chord / CURVE_STEP)` pieces, clamped to
/// `1..=CURVE_MAX_PIECES`.
///
/// Centripetal means the spline's knot parameter advances by the square
/// root of each chord (see [`chord_and_step`]) instead of by 1. A *uniform*
/// spline — the textbook one, and what this was until it drew a mind
/// map — ignores how far apart its knots are, so knots that are close
/// together get as much parameter as knots that are far apart, and the
/// curve has to move fast through the tight ones. On an elbow (a long run,
/// then a sharp turn) that shows up as a bow out of the wrong side of the
/// corner; on knots spaced unevenly enough it becomes a cusp or a loop,
/// which a centripetal span provably never has. It bows less
/// too, though it is not overshoot-free: a spline that must pass *through*
/// a corner has to lean into it. A shape that should only be *pulled*
/// towards its middle points is a Bézier, and the caller samples one
/// (`examples/rust/widgets/line.rs`).
///
/// The end knots are not doubled — a repeated knot is a zero-length chord,
/// which centripetal has no parameter for. Each end gets a mirrored
/// phantom instead (`2·p1 − p2`), which spaces evenly and leaves the first
/// span's tangent along its own chord. A coincident pair of *real* knots
/// takes the same branch, so a duplicated point in a caller's list stays a
/// harmless kink rather than a division by zero.
///
/// The chords are rolled forward rather than recomputed, so a span costs
/// two square roots and not six; `frame_1k_curves` is the bench that
/// watches the rest of it.
pub fn flatten_curve(knots: &[Vec2], out: &mut Vec<Vec2>) {
    let n = knots.len();
    if n < 2 {
        return;
    }
    out.push(knots[0]);
    // This span's chord and its knot step (√chord), and the span before's:
    // a zero chord means there is no neighbour on that side, either
    // because the run ends there or because the two knots coincide.
    let (mut prev, mut prev_step) = (0.0, 0.0);
    let (mut chord, mut step) = chord_and_step(knots[0], knots[1]);
    for i in 0..n - 1 {
        let (p1, p2) = (knots[i], knots[i + 1]);
        let (next, next_step) = match knots.get(i + 2) {
            Some(&p) => chord_and_step(p2, p),
            None => (0.0, 0.0),
        };
        let pieces = ((chord / CURVE_STEP).ceil() as usize).clamp(1, CURVE_MAX_PIECES);
        if chord == 0.0 {
            // Nothing to parameterize: the span is a point.
            out.push(p2);
        } else {
            let mirror = |p: Vec2, q: Vec2| Vec2::new(2.0 * p.x - q.x, 2.0 * p.y - q.y);
            let (p0, s0) = match prev > 0.0 {
                true => (knots[i - 1], prev_step),
                false => (mirror(p1, p2), step),
            };
            let (p3, s2) = match next > 0.0 {
                true => (knots[i + 2], next_step),
                false => (mirror(p2, p1), step),
            };
            // Knot times: 0, then one step per span.
            let span = Span::new([p0, p1, p2, p3], [0.0, s0, s0 + step, s0 + step + s2]);
            for k in 1..pieces {
                out.push(span.at(s0 + step * (k as f32 / pieces as f32)));
            }
            // The knot itself rather than the evaluation at its time, so
            // the run passes through it bit for bit whatever the
            // arithmetic rounds to.
            out.push(p2);
        }
        (prev, prev_step) = (chord, step);
        (chord, step) = (next, next_step);
    }
}

/// A span's length and the parameter it is worth: `√chord` is Lee's
/// α = ½, the centripetal one. α = 0 would be `1.0` (uniform) and α = 1
/// the chord itself (chordal).
fn chord_and_step(a: Vec2, b: Vec2) -> (f32, f32) {
    let chord = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
    (chord, chord.sqrt())
}

/// One span of the spline, evaluated between `p[1]` and `p[2]` by Barry
/// and Goldman's pyramid: three interpolations of the knots in their own
/// times, then two of those, then one. Written this way rather than as a
/// cubic in `t` because the times are no longer evenly spaced.
///
/// A span is built once and asked for every piece, because the pyramid's
/// five distinct denominators are the same five numbers each time. They
/// are reciprocals, so `at(t[2])` is only *nearly* `p[2]` — the caller
/// pushes the knot itself instead of asking for it.
struct Span {
    p: [Vec2; 4],
    t: [f32; 4],
    /// `1/(t[1]-t[0])`, `1/(t[2]-t[1])`, `1/(t[3]-t[2])`, `1/(t[2]-t[0])`,
    /// `1/(t[3]-t[1])`, in the order the pyramid needs them.
    inv: [f32; 5],
}

impl Span {
    fn new(p: [Vec2; 4], t: [f32; 4]) -> Self {
        let inv = [
            1.0 / (t[1] - t[0]),
            1.0 / (t[2] - t[1]),
            1.0 / (t[3] - t[2]),
            1.0 / (t[2] - t[0]),
            1.0 / (t[3] - t[1]),
        ];
        Span { p, t, inv }
    }

    fn at(&self, u: f32) -> Vec2 {
        let (p, t) = (&self.p, &self.t);
        let blend = |a: Vec2, b: Vec2, ta: f32, inv: f32| {
            let w = (u - ta) * inv;
            Vec2::new(a.x + (b.x - a.x) * w, a.y + (b.y - a.y) * w)
        };
        let a1 = blend(p[0], p[1], t[0], self.inv[0]);
        let a2 = blend(p[1], p[2], t[1], self.inv[1]);
        let a3 = blend(p[2], p[3], t[2], self.inv[2]);
        let b1 = blend(a1, a2, t[0], self.inv[3]);
        let b2 = blend(a2, a3, t[1], self.inv[4]);
        blend(b1, b2, t[1], self.inv[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_curve_passes_through_its_knots_and_ends_where_they_end() {
        let knots = [
            Vec2::new(0.0, 0.0),
            Vec2::new(30.0, 40.0),
            Vec2::new(60.0, 0.0),
        ];
        let mut out = Vec::new();
        flatten_curve(&knots, &mut out);
        assert_eq!(out[0], knots[0]);
        assert_eq!(*out.last().unwrap(), knots[2]);
        // Chord 50 → 9 pieces per span, 1 + 9 + 9 points.
        assert_eq!(out.len(), 19);
        assert_eq!(out[9], knots[1]);
    }

    /// How sharply the run doubles back, worst piece, in degrees. A
    /// smoothly flattened curve turns a few degrees a piece; a cusp turns
    /// most of the way round.
    fn worst_turn(pts: &[Vec2]) -> f32 {
        pts.windows(3).fold(0.0f32, |worst, w| {
            let (a, b) = (
                Vec2::new(w[1].x - w[0].x, w[1].y - w[0].y),
                Vec2::new(w[2].x - w[1].x, w[2].y - w[1].y),
            );
            let len = |v: Vec2| (v.x * v.x + v.y * v.y).sqrt();
            let (la, lb) = (len(a), len(b));
            if la < 1e-6 || lb < 1e-6 {
                return worst;
            }
            let cos = ((a.x * b.x + a.y * b.y) / (la * lb)).clamp(-1.0, 1.0);
            worst.max(cos.acos().to_degrees())
        })
    }

    /// Whether any two non-adjacent pieces of the run cross.
    fn ties_a_loop(pts: &[Vec2]) -> bool {
        let side =
            |a: Vec2, b: Vec2, c: Vec2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        (0..pts.len() - 1).any(|i| {
            (i + 2..pts.len() - 1).any(|j| {
                let (a, b, c, d) = (pts[i], pts[i + 1], pts[j], pts[j + 1]);
                side(a, b, c) * side(a, b, d) < 0.0 && side(c, d, a) * side(c, d, b) < 0.0
            })
        })
    }

    /// The reason the parameterization is centripetal and not uniform.
    /// Chords 671, 36 and 328: a knot pair nineteen times tighter than its
    /// neighbours. A uniform parameter hands that 36px chord as much curve
    /// as the 671px one, and the middle span has to tie a loop to spend
    /// it — at `CURVE_STEP`'s own sampling, one crossing and a piece that
    /// turns 95°. Centripetal spends parameter by `√chord`, so the tight
    /// pair is just a corner.
    #[test]
    fn a_tight_knot_between_two_long_ones_neither_loops_nor_cusps() {
        let knots = [
            Vec2::new(0.0, 400.0),
            Vec2::new(600.0, 100.0),
            Vec2::new(630.0, 80.0),
            Vec2::new(560.0, 400.0),
        ];
        let mut out = Vec::new();
        flatten_curve(&knots, &mut out);
        assert!(!ties_a_loop(&out), "the run crosses itself");
        assert!(worst_turn(&out) < 45.0, "cusp: {:.1}°", worst_turn(&out));
    }

    /// A caller's list may repeat a point — a mind map with two cards at
    /// the same place, a path snapped to a grid. The mirrored phantom
    /// covers it: a zero chord has no `√chord` to divide by, and every
    /// point that comes back is a number.
    #[test]
    fn a_repeated_knot_is_a_kink_and_not_a_division_by_zero() {
        let knots = [
            Vec2::new(0.0, 0.0),
            Vec2::new(40.0, 0.0),
            Vec2::new(40.0, 0.0),
            Vec2::new(40.0, 40.0),
            Vec2::new(80.0, 40.0),
        ];
        let mut out = Vec::new();
        flatten_curve(&knots, &mut out);
        assert!(out.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        assert_eq!(out[0], knots[0]);
        assert_eq!(*out.last().unwrap(), knots[4]);
    }

    #[test]
    fn two_points_are_one_segment_even_as_a_curve() {
        let mut store = LineStore::default();
        let (id, rect) = store
            .push(
                &[Vec2::new(10.0, 10.0), Vec2::new(50.0, 40.0)],
                Stroke::new(2.0, Color::WHITE).curve(),
            )
            .unwrap();
        let (run, pts) = store.run(id);
        assert_eq!(run.len, 2);
        assert_eq!(run.width, 2.0);
        // Padded by half the width plus two: the box starts at (7, 7).
        assert_eq!((rect.x, rect.y, rect.w, rect.h), (7.0, 7.0, 46.0, 36.0));
        assert_eq!(pts[0], Vec2::new(3.0, 3.0));
        assert_eq!(pts[1], Vec2::new(43.0, 33.0));
    }

    #[test]
    fn one_point_draws_nothing() {
        let mut store = LineStore::default();
        assert!(
            store
                .push(&[Vec2::new(1.0, 1.0)], Stroke::new(1.0, Color::WHITE))
                .is_none()
        );
    }

    #[test]
    fn the_previous_frame_is_kept_only_when_asked() {
        let mut store = LineStore::default();
        let (id, _) = store
            .push(
                &[Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)],
                Stroke::new(1.0, Color::WHITE),
            )
            .unwrap();
        store.begin_frame(true);
        assert_eq!(store.prev_run(id).0.len, 2);
        assert!(store.is_empty());
        store.begin_frame(false);
        assert_eq!(store.prev_run(id).0.len, 0);
    }
}
