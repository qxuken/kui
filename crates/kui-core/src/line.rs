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
    /// Draw a smooth curve *through* the points (a Catmull-Rom spline,
    /// flattened by [`flatten_curve`]) instead of the polyline. Two points
    /// are a straight segment either way.
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

/// Flattens a Catmull-Rom spline through `knots` (at least three) into
/// `out`, starting at the first knot and ending at the last. Each span is
/// cut into `ceil(chord / CURVE_STEP)` pieces, clamped to
/// `1..=CURVE_MAX_PIECES`; the end knots are doubled, which is the
/// standard open-curve convention and keeps the curve through every knot.
pub fn flatten_curve(knots: &[Vec2], out: &mut Vec<Vec2>) {
    let n = knots.len();
    if n < 2 {
        return;
    }
    out.push(knots[0]);
    for i in 0..n - 1 {
        let p0 = knots[i.saturating_sub(1)];
        let p1 = knots[i];
        let p2 = knots[i + 1];
        let p3 = knots[(i + 2).min(n - 1)];
        let chord = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt();
        let pieces = ((chord / CURVE_STEP).ceil() as usize).clamp(1, CURVE_MAX_PIECES);
        for k in 1..=pieces {
            let t = k as f32 / pieces as f32;
            out.push(catmull_rom(p0, p1, p2, p3, t));
        }
    }
}

/// The uniform Catmull-Rom point between `p1` and `p2` at `t`.
fn catmull_rom(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let axis = |a: f32, b: f32, c: f32, d: f32| {
        0.5 * (2.0 * b
            + (-a + c) * t
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
            + (-a + 3.0 * b - 3.0 * c + d) * t * t * t)
    };
    Vec2::new(axis(p0.x, p1.x, p2.x, p3.x), axis(p0.y, p1.y, p2.y, p3.y))
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
