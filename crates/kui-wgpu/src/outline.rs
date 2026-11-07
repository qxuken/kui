//! Large hollow outlines need no fragment work in their transparent interior.
//! Keep the original quad/SDF and draw it through disjoint hardware scissors.
//! Only frame-space, untransformed solids qualify; groups keep their own space.
use kui_core::{Clip, Quad, QuadKind, Size};

pub(crate) struct Plan {
    pub quad: u32,
    pub pieces: [[u32; 4]; 4],
}

pub(crate) fn plan(quad: u32, q: &Quad, clip: &Clip, viewport: Size) -> Option<Plan> {
    if q.kind != QuadKind::Solid
        || q.color.a != 0.0
        || q.border_w <= 0.0
        || q.border_color.a <= 0.0
        || clip.turned()
        || viewport.w < 1.0
        || viewport.h < 1.0
        || ![
            q.rect.x,
            q.rect.y,
            q.rect.w,
            q.rect.h,
            q.border_w,
            viewport.w,
            viewport.h,
            clip.rect.x,
            clip.rect.y,
            clip.rect.w,
            clip.rect.h,
        ]
        .into_iter()
        .chain(q.radius)
        .all(f32::is_finite)
    {
        return None;
    }
    // A full radius plus the stroke and its 0.75px AA ramp is conservative
    // for every corner. Integer boundaries keep the four draws disjoint.
    let band = (q.radius.into_iter().fold(0.0_f32, f32::max) + q.border_w + 1.0).ceil();
    let x0 = q
        .rect
        .x
        .max(clip.rect.x - 1.0)
        .floor()
        .max(0.0)
        .min(viewport.w.round());
    let y0 = q
        .rect
        .y
        .max(clip.rect.y - 1.0)
        .floor()
        .max(0.0)
        .min(viewport.h.round());
    let x1 = (q.rect.x + q.rect.w)
        .min(clip.rect.x + clip.rect.w + 1.0)
        .ceil()
        .clamp(x0, viewport.w.round());
    let y1 = (q.rect.y + q.rect.h)
        .min(clip.rect.y + clip.rect.h + 1.0)
        .ceil()
        .clamp(y0, viewport.h.round());
    let ix0 = (q.rect.x + band).ceil().clamp(x0, x1);
    let ix1 = (q.rect.x + q.rect.w - band).floor().clamp(ix0, x1);
    let iy0 = (q.rect.y + band).ceil().clamp(y0, y1);
    let iy1 = (q.rect.y + q.rect.h - band).floor().clamp(iy0, y1);
    // Extra draws are worthwhile only when at least 256k empty pixels go.
    if (ix1 - ix0) * (iy1 - iy0) < 262_144.0 {
        return None;
    }
    let rect =
        |a: f32, b: f32, c: f32, d: f32| [a as u32, b as u32, (c - a) as u32, (d - b) as u32];
    Some(Plan {
        quad,
        pieces: [
            rect(x0, y0, x1, iy0),
            rect(x0, iy0, ix0, iy1),
            rect(ix1, iy0, x1, iy1),
            rect(x0, iy1, x1, y1),
        ],
    })
}
