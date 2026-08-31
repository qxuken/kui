struct Globals {
    viewport: vec2<f32>,
    atlas_size: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_smp: sampler;

struct Instance {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    // radius, border_w, kind (0 solid / 1 mask glyph / 2 color glyph), unused
    @location(4) params: vec4<f32>,
    // atlas texels: x, y, w, h
    @location(5) uv: vec4<f32>,
    // clip rect in physical px: x, y, w, h
    @location(6) clip: vec4<f32>,
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
    return out;
}

fn sd_rounded_box(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let rr = min(r, min(half.x, half.y));
    let q = abs(p) - half + vec2<f32>(rr, rr);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - rr;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // frag_pos is framebuffer coords (physical px, y down) — same space as
    // clip. Clip via alpha (not discard) to keep texture sampling in uniform
    // control flow.
    let p = in.frag_pos.xy;
    let inside = f32(
        p.x >= in.clip.x && p.y >= in.clip.y
        && p.x <= in.clip.x + in.clip.z && p.y <= in.clip.y + in.clip.w
    );

    let kind = u32(in.params.z + 0.5);

    if kind == 1u {
        // Alpha-mask glyph tinted by color.
        let a = textureSample(atlas_tex, atlas_smp, in.uv).a;
        return vec4<f32>(in.color.rgb, in.color.a * a * inside);
    }
    if kind == 2u {
        // Color bitmap glyph (emoji).
        let t = textureSample(atlas_tex, atlas_smp, in.uv);
        return vec4<f32>(t.rgb, t.a * in.color.a * inside);
    }

    // Solid rounded rect with optional border, SDF antialiased.
    let half = in.size * 0.5;
    let d = sd_rounded_box(in.local - half, half, in.params.x);
    let aa = 0.75;
    let coverage = 1.0 - smoothstep(-aa, aa, d);

    var rgba = in.color;
    let bw = in.params.y;
    if bw > 0.0 {
        let border_mix = 1.0 - smoothstep(-bw - aa, -bw + aa, d);
        rgba = mix(in.border_color, in.color, border_mix);
        // Border-only nodes (transparent fill) still show their outline.
        rgba.a = mix(in.border_color.a, in.color.a, border_mix);
    }
    return vec4<f32>(rgba.rgb, rgba.a * coverage * inside);
}
