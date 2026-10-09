//! The pieces of `src/shader.wgsl` that decide geometry, mirrored on the
//! CPU so a test can evaluate them over quads the core actually emitted.
//!
//! Shader validation proves the WGSL compiles. It cannot prove that the
//! half the core computes and the half the shader computes are the same
//! agreement — a sign flipped in either lands a shape somewhere else and
//! still draws. Everything here is a line-for-line transcription; when the
//! shader changes, this changes with it.

// Each test binary that includes this module uses part of it; nothing here
// is dead, it is just not all needed twice.
#![allow(dead_code)]

/// `AA` in the shader: half-width of every SDF edge ramp, in physical px.
pub const AA: f32 = 0.75;

/// `sd_rounded_box`; `p` is centred, y down, `radii` clockwise from the
/// top-left.
pub fn sd_rounded_box(p: [f32; 2], half: [f32; 2], radii: [f32; 4]) -> f32 {
    let right = p[0] > 0.0;
    let bottom = p[1] > 0.0;
    let top_r = if right { radii[1] } else { radii[0] };
    let bottom_r = if right { radii[2] } else { radii[3] };
    let r = if bottom { bottom_r } else { top_r };
    let rr = r.min(half[0].min(half[1]));
    let q = [p[0].abs() - half[0] + rr, p[1].abs() - half[1] + rr];
    let outside = (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2)).sqrt();
    outside + q[0].max(q[1]).min(0.0) - rr
}

/// `sd_segment`: the capsule from `a` to `b` of radius `r`, y down.
pub fn sd_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2], r: f32) -> f32 {
    let pa = [p[0] - a[0], p[1] - a[1]];
    let ba = [b[0] - a[0], b[1] - a[1]];
    let h = ((pa[0] * ba[0] + pa[1] * ba[1]) / (ba[0] * ba[0] + ba[1] * ba[1]).max(1e-6))
        .clamp(0.0, 1.0);
    let d = [pa[0] - ba[0] * h, pa[1] - ba[1] * h];
    (d[0] * d[0] + d[1] * d[1]).sqrt() - r
}

/// `shade`'s `coverage` for a solid or an image quad at the fragment
/// centred on (`x`, `y`): the area of that pixel inside the rect while
/// the radii are all zero, the rounded SDF's ramp otherwise.
pub fn quad_coverage(q: &kui_core::Quad, x: f32, y: f32) -> f32 {
    let local = [x - q.rect.x, y - q.rect.y];
    let size = [q.rect.w, q.rect.h];
    if q.radius.iter().all(|v| *v <= 0.0) {
        let span = |i: usize| {
            let lo = (local[i] - 0.5).max(0.0);
            let hi = (local[i] + 0.5).min(size[i]);
            (hi - lo).clamp(0.0, 1.0)
        };
        return span(0) * span(1);
    }
    let half = [size[0] * 0.5, size[1] * 0.5];
    let d = sd_rounded_box([local[0] - half[0], local[1] - half[1]], half, q.radius);
    1.0 - smoothstep(-AA, AA, d)
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `shade`'s `inside` — how much of the fragment at (`x`, `y`) the given
/// clip lets through. The plain rect test while the radii are all zero;
/// the rounded SDF otherwise. The clip comes from `DisplayList::clips`,
/// which is where a quad's `clip` index points.
pub fn inside(clip: &kui_core::Clip, x: f32, y: f32) -> f32 {
    let r = clip.rect;
    if clip.radius.iter().all(|v| *v <= 0.0) {
        let ok = x >= r.x && y >= r.y && x <= r.x + r.w && y <= r.y + r.h;
        return f32::from(ok);
    }
    let half = [r.w * 0.5, r.h * 0.5];
    let d = sd_rounded_box(
        [x - (r.x + half[0]), y - (r.y + half[1])],
        half,
        clip.radius,
    );
    1.0 - smoothstep(-AA, AA, d)
}

/// `vs_main`'s turn (ADR 0043): where a point of a quad's own rect lands
/// on screen, through its clip entry's transform — `R(angle) · scale · p
/// + t`, y down. The identity on a frame that turns nothing.
pub fn turned(clip: &kui_core::Clip, x: f32, y: f32) -> (f32, f32) {
    let t = clip.transform;
    let (s, c) = t.angle.sin_cos();
    let (sx, sy) = (x * t.scale, y * t.scale);
    (sx * c - sy * s + t.tx, sx * s + sy * c + t.ty)
}

/// `shade`'s second `inside` factor — how much of the fragment whose
/// pre-turn position is (`x`, `y`) the inner clip lets through — the
/// rect test while the radii are zero, the rounded SDF otherwise.
pub fn inside_inner(clip: &kui_core::Clip, x: f32, y: f32) -> f32 {
    let r = clip.inner;
    if clip.inner_radius.iter().all(|v| *v <= 0.0) {
        let half = [r.w * 0.5, r.h * 0.5];
        let q = [
            (x - (r.x + half[0])).abs() - half[0],
            (y - (r.y + half[1])).abs() - half[1],
        ];
        return 1.0 - smoothstep(-AA, AA, q[0].max(q[1]));
    }
    let half = [r.w * 0.5, r.h * 0.5];
    let d = sd_rounded_box(
        [x - (r.x + half[0]), y - (r.y + half[1])],
        half,
        clip.inner_radius,
    );
    1.0 - smoothstep(-AA, AA, d)
}

/// `kui_core::fragment::JOIN`'s shape (backlog F101), line for line: the
/// alpha of the piece at the pixel centred on `local` (physical px from
/// the quad's top-left) of a quad `size` tall, from its sixteen params,
/// before the quad's colour and the epilogue's coverage. The fragment
/// lives in the core rather than `shader.wgsl`, but what a test needs of
/// it is the same agreement.
pub fn join_alpha(local: (f32, f32), size: (f32, f32), p: &[f32; 16]) -> f32 {
    fn radii(cx: f32, e: f32, has: bool, sx: f32, r: f32, lone: f32) -> (f32, f32) {
        if !has {
            return (lone, 0.0);
        }
        let d = (e - cx) * sx;
        (r.min((-d).max(0.0) * 0.5), r.min(d.max(0.0) * 0.5))
    }
    fn dist(p: (f32, f32), c: (f32, f32)) -> f32 {
        ((p.0 - c.0).powi(2) + (p.1 - c.1).powi(2)).sqrt()
    }
    fn cut(p: (f32, f32), cx: f32, cy: f32, sx: f32, sy: f32, rc: f32) -> bool {
        let near = (cx - p.0) * sx < rc && (cy - p.1) * sy < rc;
        rc > 0.0 && near && dist(p, (cx - sx * rc, cy - sy * rc)) > rc
    }
    fn fillet(p: (f32, f32), cx: f32, cy: f32, sx: f32, sy: f32, rf: f32) -> bool {
        let dx = (p.0 - cx) * sx;
        let dy = (cy - p.1) * sy;
        rf > 0.0
            && (0.0..rf).contains(&dx)
            && (0.0..rf).contains(&dy)
            && dist(p, (cx + sx * rf, cy - sy * rf)) >= rf
    }
    #[allow(clippy::too_many_arguments)]
    fn inside(
        p: (f32, f32),
        h: f32,
        a: f32,
        b: f32,
        pv: (f32, f32),
        hp: bool,
        nx: (f32, f32),
        hn: bool,
        r: f32,
    ) -> bool {
        let lone = r.min((b - a) * 0.5);
        let tl = radii(a, pv.0, hp, -1.0, r, lone);
        let tr = radii(b, pv.1, hp, 1.0, r, lone);
        let bl = radii(a, nx.0, hn, -1.0, r, lone);
        let br = radii(b, nx.1, hn, 1.0, r, lone);
        let in_box = p.0 >= a && p.0 < b && p.1 >= 0.0 && p.1 < h;
        let c = cut(p, a, 0.0, -1.0, -1.0, tl.0)
            || cut(p, b, 0.0, 1.0, -1.0, tr.0)
            || cut(p, a, h, -1.0, 1.0, bl.0)
            || cut(p, b, h, 1.0, 1.0, br.0);
        let f = fillet(p, a, 0.0, -1.0, -1.0, tl.1)
            || fillet(p, b, 0.0, 1.0, -1.0, tr.1)
            || fillet(p, a, h, -1.0, 1.0, bl.1)
            || fillet(p, b, h, 1.0, 1.0, br.1);
        (in_box && !c) || f
    }
    let (a, b) = (p[0], p[1]);
    let pv = (p[2], p[3]);
    let nx = (p[4], p[5]);
    let h = size.1;
    let r = p[6].min(h * 0.5);
    let flags = (p[7] + 0.5) as u32;
    let hp = flags & 1 != 0 && pv.0 < b && pv.1 > a;
    let hn = flags & 2 != 0 && nx.0 < b && nx.1 > a;
    let x = local.0;
    if x < a - r - 1.0 || x > b + r + 1.0 {
        return 0.0;
    }
    if x > a + r + 1.0 && x < b - r - 1.0 {
        return 1.0;
    }
    let mut n = 0.0;
    for i in 0..4 {
        for j in 0..4 {
            let o = ((i as f32 + 0.5) * 0.25 - 0.5, (j as f32 + 0.5) * 0.25 - 0.5);
            if inside((local.0 + o.0, local.1 + o.1), h, a, b, pv, hp, nx, hn, r) {
                n += 1.0;
            }
        }
    }
    n / 16.0
}
