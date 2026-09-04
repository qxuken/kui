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

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `shade`'s `inside` — how much of the fragment at (`x`, `y`) the quad's
/// clip lets through. The plain rect test while `clip_radius` is all zero;
/// the rounded SDF otherwise.
pub fn inside(q: &kui_core::Quad, x: f32, y: f32) -> f32 {
    if q.clip_radius.iter().all(|r| *r <= 0.0) {
        let ok =
            x >= q.clip.x && y >= q.clip.y && x <= q.clip.x + q.clip.w && y <= q.clip.y + q.clip.h;
        return f32::from(ok);
    }
    let half = [q.clip.w * 0.5, q.clip.h * 0.5];
    let d = sd_rounded_box(
        [x - (q.clip.x + half[0]), y - (q.clip.y + half[1])],
        half,
        q.clip_radius,
    );
    1.0 - smoothstep(-AA, AA, d)
}
