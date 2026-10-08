// A node's backdrop blur (backlog F129): what the frame drew so far under
// the node, read back, blurred at reduced resolution and written back
// inside the node's rounded rect. Four steps, one entry point each, all
// drawn with one full-target triangle that the viewport and the scissor
// cut down to the pixels the step writes:
//
//   fs_down       the region (copied to `sharp` at its origin) into
//                 `ping` at 1/d, each texel the mean of its d x d block
//   fs_blur_h     ping -> pong, a Gaussian along x
//   fs_blur_v     pong -> ping, the same along y
//   fs_composite  into the frame, inside the shape: the sharp pixel mixed
//                 towards the blurred one by coverage times opacity, with
//                 blending off, so a translucent pixel stays premultiplied;
//                 the first draw of the pass that resumes after the node
//   fs_blit       the offscreen frame onto the surface, once a frame
//
// `sharp`, `ping` and `pong` are scratch sized to a frame's largest blur,
// not to this one, and loaded rather than cleared: past this blur's region
// and image they hold whatever an earlier blur left. No step reads there —
// fs_down clamps to the region, gauss to the image, and fs_composite's
// bilinear tap to the image's outer texel centres, where the texel beyond
// weighs nothing.
//
// Every texture holds premultiplied colour, as the frame does, so the blur
// of a transparent window's pixels is the blur of what the compositor will
// show through them.

struct Params {
    // The region read back, in target px: x, y, w, h (whole pixels). `sharp`
    // holds it at its own origin.
    region: vec4<f32>,
    // The node's rect in target px.
    rect: vec4<f32>,
    // The node's corner radii, tl tr br bl, physical px.
    radii: vec4<f32>,
    // The node's clip, target px, and its radii (all zero: a plain rect).
    clip: vec4<f32>,
    clip_radii: vec4<f32>,
    // x: the downsample factor d; y: the Gaussian's sigma in downsampled
    // texels; z: the group opacity; w: the kernel's half width in taps.
    blur: vec4<f32>,
    // xy: the blurred image's size in texels; zw: `ping`'s and `pong`'s
    // size, which may be larger.
    sizes: vec4<f32>,
    // xy: `sharp`'s size, which may be larger than the region; zw unused.
    sharp_size: vec4<f32>,
};

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var sharp: texture_2d<f32>;
@group(0) @binding(3) var smp: sampler;

// One triangle over the whole target; the viewport and scissor do the rest.
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    return vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

const AA: f32 = 0.75;

// The same rounded box `shader.wgsl` measures every other edge with.
fn sd_rounded_box(q: vec2<f32>, half: vec2<f32>, radii: vec4<f32>) -> f32 {
    let right = q.x > 0.0;
    let bottom = q.y > 0.0;
    let top_r = select(radii.x, radii.y, right);
    let bottom_r = select(radii.w, radii.z, right);
    let r = select(top_r, bottom_r, bottom);
    let rr = min(r, min(half.x, half.y));
    let d = abs(q) - half + vec2<f32>(rr, rr);
    return length(max(d, vec2<f32>(0.0, 0.0))) + min(max(d.x, d.y), 0.0) - rr;
}

fn coverage(at: vec2<f32>, r: vec4<f32>, radii: vec4<f32>) -> f32 {
    let half = r.zw * 0.5;
    return 1.0 - smoothstep(-AA, AA, sd_rounded_box(at - (r.xy + half), half, radii));
}

@fragment
fn fs_down(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let d = u32(p.blur.x + 0.5);
    let q = floor(pos.xy);
    if d < 2u {
        return textureLoad(src, vec2<i32>(q), 0);
    }
    // A bilinear tap at the corner four texels share is their mean, so
    // (d/2)^2 taps average the d x d block; one past the region's edge is
    // pulled back inside it.
    let n = d / 2u;
    let lo = vec2<f32>(0.5, 0.5);
    let hi = p.region.zw - vec2<f32>(0.5, 0.5);
    var sum = vec4<f32>(0.0);
    for (var j = 0u; j < n; j++) {
        for (var i = 0u; i < n; i++) {
            let at = q * f32(d) + vec2<f32>(f32(2u * i + 1u), f32(2u * j + 1u));
            sum += textureSampleLevel(src, smp, clamp(at, lo, hi) / p.sharp_size.xy, 0.0);
        }
    }
    return sum / f32(n * n);
}

fn gauss(dir: vec2<i32>, pos: vec2<f32>) -> vec4<f32> {
    let q = vec2<i32>(floor(pos));
    let last = vec2<i32>(p.sizes.xy) - vec2<i32>(1, 1);
    let taps = i32(p.blur.w);
    let s2 = 2.0 * p.blur.y * p.blur.y;
    var sum = vec4<f32>(0.0);
    var total = 0.0;
    for (var k = -taps; k <= taps; k++) {
        let w = exp(-f32(k * k) / s2);
        sum += w * textureLoad(src, clamp(q + dir * k, vec2<i32>(0, 0), last), 0);
        total += w;
    }
    return sum / total;
}

@fragment
fn fs_blur_h(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return gauss(vec2<i32>(1, 0), pos.xy);
}

@fragment
fn fs_blur_v(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return gauss(vec2<i32>(0, 1), pos.xy);
}

@fragment
fn fs_composite(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let local = pos.xy - p.region.xy;
    let was = textureLoad(sharp, vec2<i32>(floor(local)), 0);
    // A blurred texel j covers region px [j d, (j + 1) d), its centre at
    // (j + 0.5) d, so region px x is texel x / d; held inside the image so
    // the edge does not sample what the scratch holds past it.
    let t = clamp(local / p.blur.x, vec2<f32>(0.5, 0.5), p.sizes.xy - vec2<f32>(0.5, 0.5));
    let blurred = textureSampleLevel(src, smp, t / p.sizes.zw, 0.0);
    var inside = coverage(pos.xy, p.rect, p.radii);
    if any(p.clip_radii > vec4<f32>(0.0)) {
        inside *= coverage(pos.xy, p.clip, p.clip_radii);
    } else {
        inside *= f32(
            pos.x >= p.clip.x && pos.y >= p.clip.y
            && pos.x <= p.clip.x + p.clip.z && pos.y <= p.clip.y + p.clip.w
        );
    }
    return mix(was, blurred, clamp(inside * p.blur.z, 0.0, 1.0));
}

@fragment
fn fs_blit(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(src, vec2<i32>(floor(pos.xy)), 0);
}
