// Kit's noise preset (`kit.shader({ preset = "noise" })`): gradient noise in
// three octaves drifting slowly, between two colours. Kit registers it
// itself (env.add_fragment) and fills its params from the skin:
//
//   params[0]    the first colour, straight RGBA 0 to 1 (the skin's well)
//   params[1]    the second (the skin's accent)
//   params[2].x  the drift, in cells a second
//   params[2].y  1 to drift, 0 to hold still (reduced motion)
//   params[2].z  a cell's size in logical px
fn kit_hash(p: vec2<f32>) -> vec2<f32> {
    let q = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(q) * 43758.5453) * 2.0 - 1.0;
}

fn kit_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = dot(kit_hash(i), f);
    let b = dot(kit_hash(i + vec2<f32>(1.0, 0.0)), f - vec2<f32>(1.0, 0.0));
    let c = dot(kit_hash(i + vec2<f32>(0.0, 1.0)), f - vec2<f32>(0.0, 1.0));
    let d = dot(kit_hash(i + vec2<f32>(1.0, 1.0)), f - vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let cell = max(params[2].z, 1.0);
    let p = in.local / in.scale / cell;
    let t = in.time * params[2].x * params[2].y;
    let n = kit_noise(p + vec2<f32>(t, t * 0.6)) * 0.6
        + kit_noise(p * 2.0 - vec2<f32>(t * 0.8, 0.0)) * 0.3
        + kit_noise(p * 4.0 + vec2<f32>(0.0, t)) * 0.1;
    let k = smoothstep(0.2, 0.8, clamp(n * 0.5 + 0.5, 0.0, 1.0));
    return mix(params[0], params[1], k);
}
