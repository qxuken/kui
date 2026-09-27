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
