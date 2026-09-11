// Lines prefixed `//DUAL:` are enabled when the device has dual-source
// blending (per-channel coverage for LCD subpixel text); `//SINGLE:` lines
// take their place otherwise. The Rust side strips one prefix or the other
// before compiling — WGSL has no preprocessor and `enable` is all-or-nothing.
//DUAL:enable dual_source_blending;

// Must stay byte for byte what `kui_core::fragment::PRELUDE` declares as
// `KuiGlobals`, because a fragment pipeline binds this same buffer at
// group 0; `globals_layout_matches` in lib.rs is the test.
struct Globals {
    viewport: vec2<f32>,
    atlas_size: vec2<f32>,
    time: f32,
    scale: f32,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
// The atlas — or, for the run of a texture-backed image, that image's own
// texture bound in its place with `atlas_size` set to its size, so the
// same divide in `vs_main` and the same branch below draw both (ADR 0025).
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_smp: sampler;
// Nearest-texel sampling for an image whose `sampling` row asked for it.
@group(0) @binding(3) var nearest_smp: sampler;

struct Instance {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    // blur (shadows), border_w (also the stroke width of a segment, and the
    // sampling flag of an image: 1 = nearest), kind (0 solid / 1 mask
    // glyph / 2 color glyph / 3 image / 4 subpixel glyph / 5 shadow /
    // 6 segment; a texture quad arrives as 3 with its own texture bound),
    // unused
    @location(4) params: vec4<f32>,
    // atlas texels: x, y, w, h — or, for a segment, its two endpoints in
    // physical px: x0, y0, x1, y1 (the Rust side decodes the bits)
    @location(5) uv: vec4<f32>,
    // clip rect in physical px: x, y, w, h
    @location(6) clip: vec4<f32>,
    // corner radii, clockwise from the top-left: tl, tr, br, bl
    @location(7) radii: vec4<f32>,
    // radii of the clip itself, same order; all zero = a plain rect clip
    @location(8) clip_radii: vec4<f32>,
};

struct VsOut {
    @builtin(position) frag_pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) params: vec4<f32>,
    @location(5) uv: vec2<f32>,
    @location(6) clip: vec4<f32>,
    @location(7) radii: vec4<f32>,
    @location(8) clip_radii: vec4<f32>,
    // The instance's uv untouched, for a segment's endpoints: `uv` above
    // has been divided by the atlas size, which is right for a glyph and
    // meaningless here.
    @location(9) seg: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Instance) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let corner = corners[vi];
    let px = inst.pos + corner * inst.size;
    let ndc = vec2<f32>(
        px.x / globals.viewport.x * 2.0 - 1.0,
        1.0 - px.y / globals.viewport.y * 2.0,
    );

    var out: VsOut;
    out.frag_pos = vec4<f32>(ndc, 0.0, 1.0);
    out.local = corner * inst.size;
    out.size = inst.size;
    out.color = inst.color;
    out.border_color = inst.border_color;
    out.params = inst.params;
    out.uv = (inst.uv.xy + corner * inst.uv.zw) / globals.atlas_size;
    out.clip = inst.clip;
    out.radii = inst.radii;
    out.clip_radii = inst.clip_radii;
    out.seg = inst.uv;
    return out;
}

// Half-width of every SDF edge ramp, in physical px.
const AA: f32 = 0.75;

// Signed distance to a box with one radius per corner. `p` is centered
// (y down), `radii` is tl, tr, br, bl; each is clamped to the half extents
// so oversized radii degrade to a pill, never a fold.
fn sd_rounded_box(p: vec2<f32>, half: vec2<f32>, radii: vec4<f32>) -> f32 {
    let right = p.x > 0.0;
    let bottom = p.y > 0.0;
    let top_r = select(radii.x, radii.y, right);
    let bottom_r = select(radii.w, radii.z, right);
    let r = select(top_r, bottom_r, bottom);
    let rr = min(r, min(half.x, half.y));
    let q = abs(p) - half + vec2<f32>(rr, rr);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - rr;
}

// Signed distance from `p` to the segment `a`–`b` with round caps of
// radius `r`: the capsule. A zero-length segment is a dot of radius `r`.
fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

// A fragment's straight (non-premultiplied) color and its per-channel
// coverage. For everything but subpixel glyphs the coverage is the same in
// every channel, i.e. plain alpha.
struct Shaded {
    color: vec3<f32>,
    coverage: vec3<f32>,
};

fn shade(in: VsOut) -> Shaded {
    // frag_pos is framebuffer coords (physical px, y down) — same space as
    // clip. Clip via coverage (not discard) to keep texture sampling in
    // uniform control flow.
    let p = in.frag_pos.xy;
    var inside: f32;
    if all(in.clip_radii <= vec4<f32>(0.0)) {
        inside = f32(
            p.x >= in.clip.x && p.y >= in.clip.y
            && p.x <= in.clip.x + in.clip.z && p.y <= in.clip.y + in.clip.w
        );
    } else {
        // A clipping node with a radius rounds what it clips, so the
        // children of a rounded card stay inside its corners. One more SDF,
        // and only where a rounded clipper actually contains something:
        // clip_radii is zero on every quad of a frame with no rounded
        // clipper, and it comes off the instance, so the branch is uniform
        // across all of a quad's fragments.
        let ch = in.clip.zw * 0.5;
        let cd = sd_rounded_box(p - (in.clip.xy + ch), ch, in.clip_radii);
        inside = 1.0 - smoothstep(-AA, AA, cd);
    }

    let kind = u32(in.params.z + 0.5);

    if kind == 1u {
        // Alpha-mask glyph tinted by color.
        let a = textureSample(atlas_tex, atlas_smp, in.uv).a;
        return Shaded(in.color.rgb, vec3<f32>(in.color.a * a * inside));
    }
    if kind == 2u {
        // Color bitmap glyph (emoji).
        let t = textureSample(atlas_tex, atlas_smp, in.uv);
        return Shaded(t.rgb, vec3<f32>(t.a * in.color.a * inside));
    }
    if kind == 4u {
        // LCD subpixel glyph: the atlas holds one coverage per channel.
        let t = textureSample(atlas_tex, atlas_smp, in.uv);
        return Shaded(in.color.rgb, t.rgb * in.color.a * inside);
    }

    if kind == 6u {
        // A round-capped stroke between two endpoints, in the same
        // framebuffer space as the clip: the capsule SDF against the
        // fragment, half the stroke width as its radius, ramped over the
        // same AA as every other edge. The quad is the bounding box padded
        // past the ramp, so nothing is cut by its edge.
        let d = sd_segment(p, in.seg.xy, in.seg.zw, in.params.y * 0.5);
        let cov = 1.0 - smoothstep(-AA, AA, d);
        return Shaded(in.color.rgb, vec3<f32>(in.color.a * cov * inside));
    }

    // Solid rounded rect with optional border, SDF antialiased. Images
    // share the SDF so radius rounds their corners too.
    let half = in.size * 0.5;

    if kind == 5u {
        // Drop shadow: the quad is the shadow's shape inflated by `blur`
        // on every side, so inset by the same amount to get the shape back
        // and ramp the edge over the blur. A linear-ish ramp, not a true
        // Gaussian — one instanced quad, no second pass, and at UI blur
        // radii the difference does not read.
        let blur = in.params.x;
        let sh = max(in.size * 0.5 - vec2<f32>(blur, blur), vec2<f32>(0.0, 0.0));
        let sd = sd_rounded_box(in.local - half, sh, in.radii);
        let ramp = max(blur, AA);
        let a = in.color.a * (1.0 - smoothstep(-ramp, ramp, sd));
        return Shaded(in.color.rgb, vec3<f32>(a * inside));
    }

    let d = sd_rounded_box(in.local - half, half, in.radii);
    let coverage = 1.0 - smoothstep(-AA, AA, d);

    if kind == 3u {
        // Registered image tinted by color (white = as-is). Both samplers
        // are read so sampling stays in uniform control flow; the flag
        // picks one.
        let lin = textureSample(atlas_tex, atlas_smp, in.uv);
        let near = textureSample(atlas_tex, nearest_smp, in.uv);
        let t = select(lin, near, in.params.y > 0.5);
        return Shaded(t.rgb * in.color.rgb, vec3<f32>(t.a * in.color.a * coverage * inside));
    }

    var rgba = in.color;
    let bw = in.params.y;
    if bw > 0.0 {
        let border_mix = 1.0 - smoothstep(-bw - AA, -bw + AA, d);
        rgba = mix(in.border_color, in.color, border_mix);
        // Border-only nodes (transparent fill) still show their outline.
        rgba.a = mix(in.border_color.a, in.color.a, border_mix);
    }
    return Shaded(rgba.rgb, vec3<f32>(rgba.a * coverage * inside));
}

//DUAL:struct FsOut {
//DUAL:    // Premultiplied color; blended with src = One, dst = 1 - mask.
//DUAL:    @location(0) @blend_src(0) color: vec4<f32>,
//DUAL:    // Per-channel coverage: the "alpha" each channel blends with.
//DUAL:    @location(0) @blend_src(1) mask: vec4<f32>,
//DUAL:};
//DUAL:
//DUAL:@fragment
//DUAL:fn fs_main(in: VsOut) -> FsOut {
//DUAL:    let s = shade(in);
//DUAL:    let a = max(s.coverage.r, max(s.coverage.g, s.coverage.b));
//DUAL:    return FsOut(vec4<f32>(s.color * s.coverage, a), vec4<f32>(s.coverage, a));
//DUAL:}

//SINGLE:@fragment
//SINGLE:fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
//SINGLE:    // No per-channel blending: subpixel coverage collapses to its union
//SINGLE:    // (the atlas alpha), i.e. a grayscale mask.
//SINGLE:    let s = shade(in);
//SINGLE:    let a = max(s.coverage.r, max(s.coverage.g, s.coverage.b));
//SINGLE:    return vec4<f32>(s.color, a);
//SINGLE:}
