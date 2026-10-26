// Kit's skeleton shimmer, and `kit.shader({ preset = "shimmer" })`: a soft
// band of light crossing a bar from left to right, once a period, over the
// bar's own colour. Kit registers it itself (env.add_fragment; build.lua
// embeds this file) and draws each skeleton bar as a `fragment` with:
//
//   params[0]    the bar's colour, straight RGBA 0 to 1 (the skin's well)
//   params[1]    the band's colour at its middle
//   params[2].x  the period in seconds, the same for every bar, so bars of
//                any width cross in step
//   params[2].y  1 to sweep, 0 to hold still (reduced motion)
//
// The band is 40% of the bar's width, between 40 and 200 logical px. The
// rounding is the node's: Kit gives the fragment the skin's radius.
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let w = in.size.x / in.scale;
    let band = clamp(w * 0.4, 40.0, 200.0);
    let period = max(params[2].x, 0.1);
    let x = in.local.x / in.scale;
    let at = -band * 0.5 + fract(in.time / period) * (w + band);
    let k = (1.0 - smoothstep(0.0, band * 0.5, abs(x - at))) * params[2].y;
    return mix(params[0], params[1], k);
}
