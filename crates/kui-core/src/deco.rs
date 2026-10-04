//! Decoration lines that are not a rect: the wavy and dotted
//! underlines a text, a span or a cell asks for. A solid line is one
//! `Solid` quad, as it always was; these are runs of
//! [`QuadKind::Segment`] — the capsule the backend already draws for a
//! `line` node — so no backend, header or protocol learns a
//! kind. A wave is a zigzag of short pieces whose round caps soften the
//! corners; a dotted line is a row of zero-length pieces, which the
//! capsule SDF draws as dots. Both are sized from the stroke the face
//! recommends, so they scale with the text and the display.
//!
//! The cost is quads: a wave is two pieces per period of six strokes, a
//! dotted line one per four. A screenful of diagnostics under 14 px text
//! at scale 2 — say thirty runs of twenty characters — is about 1,700
//! quads, a fifth of a 10k-rect frame's; a `QuadKind` of its own is the
//! next step if a profile ever shows that.

use crate::color::Color;
use crate::display::{ClipId, Quad, QuadKind};
use crate::geom::{Rect, Vec2};
use crate::spec::UnderlineStyle;

/// Pushes the quads for a line of `style` where a solid underline's rect
/// would be `(x, y, w, stroke)` in physical px: a solid line is that rect,
/// the others are built around its centre.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_line(
    out: &mut Vec<Quad>,
    style: UnderlineStyle,
    x: f32,
    y: f32,
    w: f32,
    stroke: f32,
    color: Color,
    clip: ClipId,
) {
    if w <= 0.0 {
        return;
    }
    let stroke = stroke.max(1.0);
    match style {
        UnderlineStyle::Solid => out.push(Quad {
            rect: Rect::new(x, y, w, stroke),
            color,
            border_color: Color::TRANSPARENT,
            radius: crate::display::SQUARE,
            border_w: 0.0,
            blur: 0.0,
            kind: QuadKind::Solid,
            clip,
            uv: [0; 4],
        }),
        UnderlineStyle::Wavy => {
            // Peaks a stroke above and below the line's centre, half a
            // period apart: the wave's height is three strokes, its
            // period six. The last piece ends at the run's edge, on the
            // slope it was on.
            let cy = y + stroke * 0.5;
            let amp = stroke;
            let half = stroke * 3.0;
            let mut px = x;
            let mut py = cy + amp;
            let mut up = true;
            let end = x + w;
            while px < end {
                let nx = (px + half).min(end);
                let ny = if up { cy - amp } else { cy + amp };
                let ny = if nx < px + half {
                    py + (ny - py) * ((nx - px) / half)
                } else {
                    ny
                };
                push_piece(
                    out,
                    Vec2::new(px, py),
                    Vec2::new(nx, ny),
                    stroke,
                    color,
                    clip,
                );
                px = nx;
                py = ny;
                up = !up;
            }
        }
        UnderlineStyle::Dotted => {
            // Dots two strokes across, four apart, centred on the line.
            let cy = y + stroke * 0.5;
            let step = stroke * 4.0;
            let mut px = x + stroke;
            while px + stroke <= x + w + 0.5 {
                let p = Vec2::new(px, cy);
                push_piece(out, p, p, stroke * 2.0, color, clip);
                px += step;
            }
        }
    }
}

/// One capsule piece from `a` to `b`, `width` across, physical px — the
/// shape `runtime::emit::push_segments` makes for a `line` node, boxed the
/// same way so the backend's edge ramp is never cut by the quad's edge.
fn push_piece(out: &mut Vec<Quad>, a: Vec2, b: Vec2, width: f32, color: Color, clip: ClipId) {
    let pad = width * 0.5 + 2.0;
    let (x0, x1) = (a.x.min(b.x) - pad, a.x.max(b.x) + pad);
    let (y0, y1) = (a.y.min(b.y) - pad, a.y.max(b.y) + pad);
    out.push(Quad {
        rect: Rect::new(x0, y0, x1 - x0, y1 - y0),
        color,
        border_color: Color::TRANSPARENT,
        radius: crate::display::SQUARE,
        border_w: width,
        blur: 0.0,
        kind: QuadKind::Segment,
        clip,
        uv: Quad::segment_uv([a.x, a.y, b.x, b.y]),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quads(style: UnderlineStyle, w: f32) -> Vec<Quad> {
        let mut out = Vec::new();
        push_line(&mut out, style, 10.0, 100.0, w, 1.0, Color::WHITE, 0);
        out
    }

    #[test]
    fn a_solid_line_is_the_rect_it_always_was() {
        let q = quads(UnderlineStyle::Solid, 50.0);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].kind, QuadKind::Solid);
        assert_eq!(q[0].rect, Rect::new(10.0, 100.0, 50.0, 1.0));
    }

    #[test]
    fn a_wave_is_pieces_that_meet_and_end_at_the_edge() {
        let q = quads(UnderlineStyle::Wavy, 20.0);
        // Six-stroke period, half a period a piece: 20 px is six pieces
        // and a partial seventh.
        assert_eq!(q.len(), 7);
        assert!(q.iter().all(|q| q.kind == QuadKind::Segment));
        let ends: Vec<[f32; 4]> = q.iter().map(Quad::segment_ends).collect();
        for pair in ends.windows(2) {
            assert_eq!(
                (pair[0][2], pair[0][3]),
                (pair[1][0], pair[1][1]),
                "pieces meet"
            );
        }
        assert_eq!(ends[0][0], 10.0);
        assert_eq!(ends[6][2], 30.0, "the last piece ends at the run's edge");
        // Peaks a stroke either side of the centre, y = 100.5: the first
        // piece rises from the lower peak to the upper one.
        assert_eq!(ends[0][1], 101.5);
        assert_eq!(ends[0][3], 99.5);
        assert_eq!(ends[1][3], 101.5);
    }

    #[test]
    fn dots_are_zero_length_pieces_four_strokes_apart() {
        let q = quads(UnderlineStyle::Dotted, 20.0);
        assert_eq!(q.len(), 5);
        for (i, q) in q.iter().enumerate() {
            let [x0, y0, x1, y1] = q.segment_ends();
            assert_eq!((x0, y0), (x1, y1), "a dot");
            assert_eq!(x0, 11.0 + 4.0 * i as f32);
            assert_eq!(q.border_w, 2.0);
        }
    }

    #[test]
    fn nothing_for_an_empty_run() {
        assert!(quads(UnderlineStyle::Wavy, 0.0).is_empty());
        assert!(quads(UnderlineStyle::Dotted, 0.0).is_empty());
    }
}
