//! Rounded span backgrounds joined into one shape (backlog F101).
//!
//! A span's background with a radius is emitted square, as every
//! background is, and noted ([`crate::text::JoinBg`]). Once the frame's
//! quads are all emitted — every text, so a row knows the rows around it
//! whichever node drew them — [`shape`] finds each piece's neighbours and
//! turns its quad into a piece of the [`JOIN`](crate::fragment::JOIN)
//! fragment: the line above and the line below that meet it edge to edge,
//! in the same colour and radius, overlapping it sideways. Pieces that
//! meet end to end on one line — a line's text and the cell an editor
//! draws for its newline, two texts in a row — are one extent first, so
//! no corner is rounded where they meet. Nothing names the shape; meeting
//! is what makes one, which is what a selection over lines is — and two
//! that touch are one.

use crate::display::{DisplayList, FragmentDraw, FragmentImage, QuadKind};
use crate::geom::Rect;
use crate::resources::FragmentId;
use crate::text::JoinBg;
use std::sync::Arc;

/// How close two edges are to be one: the pieces are on whole pixels, so
/// a join is exact, and anything more is a gap.
const MEET: f32 = 0.01;

#[derive(Clone, Copy)]
struct Piece {
    quad: usize,
    outer: crate::display::ClipId,
    /// This quad's own part of the line.
    a: f32,
    b: f32,
    /// The extent it is one with on its line: its own, and every piece
    /// meeting it end to end there.
    span: (f32, f32),
    top: f32,
    bottom: f32,
    color: [u32; 4],
    radius: f32,
}

impl Piece {
    fn joins(&self, other: &Piece) -> bool {
        self.color == other.color && self.radius == other.radius
    }

    fn overlap(&self, other: &Piece) -> f32 {
        self.span.1.min(other.span.1) - self.span.0.max(other.span.0)
    }

    fn same_line(&self, other: &Piece) -> bool {
        (self.top - other.top).abs() < MEET && (self.bottom - other.bottom).abs() < MEET
    }
}

/// Each piece's `span`: the pieces of one line that meet end to end, in
/// one colour and radius, as one extent.
fn spans_on_lines(pieces: &mut [Piece]) {
    let mut order: Vec<usize> = (0..pieces.len()).collect();
    order.sort_by(|&i, &j| {
        let (p, q) = (&pieces[i], &pieces[j]);
        p.top.total_cmp(&q.top).then(p.a.total_cmp(&q.a))
    });
    // Runs of touching pieces along each line, in order of `a`; a piece of
    // another colour between two ends the run, as it would a gap.
    let mut run: Vec<usize> = Vec::new();
    let flush = |run: &mut Vec<usize>, pieces: &mut [Piece]| {
        let a = run
            .iter()
            .map(|&k| pieces[k].a)
            .fold(f32::INFINITY, f32::min);
        let b = run
            .iter()
            .map(|&k| pieces[k].b)
            .fold(f32::NEG_INFINITY, f32::max);
        for &k in run.iter() {
            pieces[k].span = (a, b);
        }
        run.clear();
    };
    for &i in &order {
        let joins_run = run.last().is_some_and(|&k| {
            let last = &pieces[k];
            let p = &pieces[i];
            last.same_line(p) && last.joins(p) && (p.a - last.b).abs() < MEET
        });
        if !joins_run && !run.is_empty() {
            flush(&mut run, pieces);
        }
        run.push(i);
    }
    if !run.is_empty() {
        flush(&mut run, pieces);
    }
}

/// The piece meeting `p` on the line above (`above`) or below that
/// overlaps it most, if any overlaps it at all.
fn neighbour(pieces: &[Piece], i: usize, above: bool) -> Option<Piece> {
    let p = pieces[i];
    pieces
        .iter()
        .enumerate()
        .filter(|&(j, q)| {
            let edge = if above {
                (q.bottom - p.top).abs()
            } else {
                (q.top - p.bottom).abs()
            };
            j != i && q.joins(&p) && edge < MEET && q.overlap(&p) > 0.0
        })
        .max_by(|(_, x), (_, y)| x.overlap(&p).total_cmp(&y.overlap(&p)))
        .map(|(_, q)| *q)
}

/// Turns every noted background's quad into its piece of the shape it
/// makes with the ones it meets. `id` and `source` are the stock
/// [`JOIN`](crate::fragment::JOIN) fragment's; `scale` puts the logical
/// radius in physical px.
pub(crate) fn shape(
    display: &mut DisplayList,
    joins: &[JoinBg],
    id: FragmentId,
    source: &Arc<str>,
    scale: f32,
) {
    let pieces: Vec<Piece> = joins
        .iter()
        .filter_map(|j| {
            let q = display.quads.get(j.quad as usize)?;
            // What the text's own box shows of it, sideways: a no-wrap
            // line clipped there is selected no further than it is seen.
            // An ancestor's clip — a pane scrolled — cuts the shape
            // square where it cuts, as it cuts everything.
            let (lo, hi) = j.own.unwrap_or((f32::NEG_INFINITY, f32::INFINITY));
            let a = q.rect.x.max(lo);
            let b = (q.rect.x + q.rect.w).min(hi);
            (q.kind == QuadKind::Solid && b > a).then(|| Piece {
                quad: j.quad as usize,
                outer: j.outer,
                a,
                b,
                span: (a, b),
                top: q.rect.y,
                bottom: q.rect.y + q.rect.h,
                color: [
                    q.color.r.to_bits(),
                    q.color.g.to_bits(),
                    q.color.b.to_bits(),
                    q.color.a.to_bits(),
                ],
                radius: j.radius * scale,
            })
        })
        .collect();
    let mut pieces = pieces;
    spans_on_lines(&mut pieces);
    for (i, p) in pieces.iter().enumerate() {
        let prev = neighbour(&pieces, i, true);
        let next = neighbour(&pieces, i, false);
        let r = p.radius;
        let (sa, sb) = p.span;
        // The quad is this piece's part of its line, and at the line's
        // ends reaches as far as a fillet can: past the end by the radius
        // at most, and only where a neighbour reaches past it.
        let reach_a = [prev, next]
            .iter()
            .flatten()
            .map(|q| q.span.0)
            .fold(sa, f32::min);
        let reach_b = [prev, next]
            .iter()
            .flatten()
            .map(|q| q.span.1)
            .fold(sb, f32::max);
        let lo = if p.a <= sa {
            reach_a.max(sa - r).floor()
        } else {
            p.a
        };
        let hi = if p.b >= sb {
            reach_b.min(sb + r).ceil()
        } else {
            p.b
        };
        let local = |x: f32| x - lo;
        let (pa, pb) = prev.map_or((0.0, 0.0), |q| (local(q.span.0), local(q.span.1)));
        let (na, nb) = next.map_or((0.0, 0.0), |q| (local(q.span.0), local(q.span.1)));
        let flags = u8::from(prev.is_some()) | (u8::from(next.is_some()) << 1);
        let mut params = [0.0f32; 16];
        params[..8].copy_from_slice(&[local(sa), local(sb), pa, pb, na, nb, r, f32::from(flags)]);
        let index = display.fragments.len() as u32;
        display.fragments.push(FragmentDraw {
            id,
            params,
            image: FragmentImage::None,
        });
        display.fragment_sources.push(source.clone());
        let quad = &mut display.quads[p.quad];
        quad.rect = Rect::new(lo, p.top, hi - lo, p.bottom - p.top);
        quad.kind = QuadKind::Fragment;
        quad.uv = [index, 0, 0, 0];
        quad.clip = p.outer;
    }
}
