//! A fragment: WGSL an app registers, validated here so a frame never sees
//! a source that cannot compile
//! (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`).
//!
//! The app writes one function:
//!
//! ```wgsl
//! fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
//!     let t = in.local.y / max(in.size.y, 1.0);
//!     return mix(params[0], params[1], t);
//! }
//! ```
//!
//! and this module puts [`PRELUDE`] in front of it and [`EPILOGUE`] behind
//! it. The prelude declares what the function reads; the epilogue is the
//! entry point that calls it and then does what every other quad gets for
//! free — the node's rounded box, the inherited clip (rounded when an
//! ancestor rounds it), the group opacity, and the premultiply the blend
//! expects. An app never writes a pipeline, a bind group, a clip or a
//! blend, and cannot get any of them wrong.
//!
//! [`module_source`] is the one place that assembles the three, so the
//! text the core validates at registration is character for character the
//! text a backend compiles. `kui-wgpu` calls it rather than building its
//! own.
//!
//! Everything this module declares is spelled `kui_` or `KUI_` so an app's
//! own names cannot collide with it. The two names that are the contract —
//! `FragmentIn` and `fragment` — are not prefixed, because they are what
//! the app writes.

/// Where a node's [`Draw`] lives in the frame's [`FragmentList`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FragmentDrawId(pub u32);

/// What a `fragment` node names: the function, and the image it reads
/// through `kui_sample` if it declared one (backlog V1, ADR 0025 decision
/// 7). Every fragment door takes `impl Into<FragmentRef>`, so a bare
/// [`FragmentId`](crate::resources::FragmentId) is the no-image form and
/// `id.with_image(img)` the other; nothing else about the node changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FragmentRef {
    pub id: crate::resources::FragmentId,
    pub image: Option<crate::resources::ImageId>,
}

impl From<crate::resources::FragmentId> for FragmentRef {
    fn from(id: crate::resources::FragmentId) -> Self {
        FragmentRef { id, image: None }
    }
}

impl crate::resources::FragmentId {
    /// This function reading `image`: what `kui_sample(uv)` returns texels
    /// of, and `FragmentIn::image` the texel rect of.
    pub fn with_image(self, image: crate::resources::ImageId) -> FragmentRef {
        FragmentRef {
            id: self,
            image: Some(image),
        }
    }
}

/// One `fragment` node's draw as the builder records it: what the node
/// declared, before emission resolves the image to an atlas slot or a
/// texture entry (which is per window, so it cannot happen here) and
/// writes the wire form, [`crate::display::FragmentDraw`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draw {
    pub id: crate::resources::FragmentId,
    pub image: Option<crate::resources::ImageId>,
    /// The view's `params`, zero-padded to sixteen.
    pub params: [f32; 16],
}

/// The frame's fragment draws, one per `fragment` node, indexed by the
/// [`FragmentDrawId`] the node's `NodeContent` carries.
///
/// It is a side list for the same reason `DisplayList::fragments` is: a
/// handle and sixteen floats is 72 bytes, and `NodeContent` is one entry
/// per node on a tree that pushes 10,000 of them (C15). Four bytes on the
/// node, the rest here.
///
/// The previous frame's draws are kept while an `exit` needs them, by the
/// same gated buffer swap `LineStore` and the text list use: a ghost is
/// built out of the *previous* tree, whose nodes index the list that frame
/// filled. A frame with no departure pays one `clear`.
#[derive(Default)]
pub struct FragmentList {
    draws: crate::retain::Kept<Draw>,
}

impl FragmentList {
    /// Starts a frame. `keep_prev` retains the list just finished so a
    /// departing fragment can copy its draw out of it.
    pub(crate) fn begin_frame(&mut self, keep_prev: bool) {
        self.draws.begin(keep_prev);
    }

    /// Records a draw and returns where it went.
    pub(crate) fn push(&mut self, draw: Draw) -> FragmentDrawId {
        let id = FragmentDrawId(self.draws.len() as u32);
        self.draws.push(draw);
        id
    }

    pub(crate) fn get(&self, id: FragmentDrawId) -> Draw {
        self.draws[id.0 as usize]
    }

    /// A draw from the frame before this one — what a ghost copies. Out of
    /// range when the previous list was not kept, which is a departure the
    /// swap did not expect; it draws nothing rather than something else's
    /// picture.
    pub(crate) fn prev_get(&self, id: FragmentDrawId) -> Option<Draw> {
        self.draws.prev().get(id.0 as usize).copied()
    }
}

/// An app's `params` as the shader takes them: the first sixteen numbers,
/// zero-padded. The second value is how many were dropped, which the
/// builder turns into a `fragment-params-truncated` warning.
pub fn params_of(params: &[f32]) -> ([f32; 16], usize) {
    let mut out = [0.0f32; 16];
    let n = params.len().min(16);
    out[..n].copy_from_slice(&params[..n]);
    (out, params.len().saturating_sub(16))
}

/// What kui declares before the app's source: the inputs its `fragment`
/// function reads, the globals behind them, and the same rounded-box
/// distance the renderer draws every node with, so a fragment that wants
/// to draw a shape has the tool the core uses.
///
/// The `KuiGlobals` it declares must describe the same bytes as
/// `kui_wgpu`'s `Globals` struct; that crate's `globals_layout_matches` is
/// the test that says so.
pub const PRELUDE: &str = r#"
// The frame's own numbers, shared with the quad pipeline (group 0).
struct KuiGlobals {
    viewport: vec2<f32>,
    atlas_size: vec2<f32>,
    time: f32,
    scale: f32,
    _pad: vec2<f32>,
};
@group(0) @binding(0) var<uniform> kui_globals: KuiGlobals;
// The atlas — or, for a fragment whose `image` has a texture of its own,
// that texture bound in the atlas's place with `atlas_size` set to its
// size, exactly as a texture-backed `image` node is drawn (ADR 0025).
@group(0) @binding(1) var kui_atlas: texture_2d<f32>;
@group(0) @binding(2) var kui_sampler: sampler;
@group(0) @binding(3) var kui_sampler_nearest: sampler;

// The texel rect of the node's `image` in `kui_atlas`, `[x, y, w, h]`;
// zero with no image. Module-private so `kui_sample` needs no argument
// for it; the epilogue sets it before calling `fragment`.
var<private> kui_image_rect: vec4<f32>;

// What an app's `fragment` function is given.
struct FragmentIn {
    // The pixel being painted, in the node's own space: physical px from
    // the node's top-left corner, y down.
    local: vec2<f32>,
    // The node's size in physical px.
    size: vec2<f32>,
    // The frame clock in seconds, the same one transitions read. Only
    // moves between frames, so a fragment that uses it wants `animate`.
    time: f32,
    // Physical px per logical px.
    scale: f32,
    // The quad's colour, straight alpha: white on a `fragment` node, the
    // `bg` fill on a `polygon` (ADR 0025, decision 6). A fragment that
    // wants a colour the view chose reads it here rather than spending
    // four params on one.
    color: vec4<f32>,
    // The node's `image` as a texel rect, `[x, y, w, h]`, in whatever
    // `kui_sample` reads from — the atlas or the image's own texture; the
    // app never needs to know which. `zw` is the image's size in texels,
    // which is what a data texture's row and column count are. Zero with
    // no image.
    image: vec4<f32>,
};

// The node's `image` at `uv`, `(0,0)` its top-left and `(1,1)` its
// bottom-right, bilinear between texels and clamped half a texel in from
// the rect's edge so the neighbour past it — a glyph, in the atlas —
// never bleeds in. Transparent black with no image. Straight alpha, as
// the image was registered.
fn kui_sample(uv: vec2<f32>) -> vec4<f32> {
    let r = kui_image_rect;
    let lo = r.xy + vec2<f32>(0.5, 0.5);
    let hi = r.xy + r.zw - vec2<f32>(0.5, 0.5);
    let t = clamp(r.xy + clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * r.zw, lo, max(lo, hi));
    let c = textureSample(kui_atlas, kui_sampler, t / kui_globals.atlas_size);
    return select(vec4<f32>(0.0), c, r.z > 0.0 && r.w > 0.0);
}

// `kui_sample` reading the nearest texel instead of blending four — a
// heatmap cell, a pixel-art sprite, anything whose texels are values.
fn kui_sample_nearest(uv: vec2<f32>) -> vec4<f32> {
    let r = kui_image_rect;
    let lo = r.xy + vec2<f32>(0.5, 0.5);
    let hi = r.xy + r.zw - vec2<f32>(0.5, 0.5);
    let t = clamp(r.xy + clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * r.zw, lo, max(lo, hi));
    let c = textureSample(kui_atlas, kui_sampler_nearest, t / kui_globals.atlas_size);
    return select(vec4<f32>(0.0), c, r.z > 0.0 && r.w > 0.0);
}

// Half-width of every SDF edge ramp, in physical px. The renderer's `AA`.
const KUI_AA: f32 = 0.75;

// Signed distance to a box with one radius per corner. `p` is centered
// (y down), `radii` is tl, tr, br, bl; each is clamped to the half extents
// so oversized radii degrade to a pill, never a fold.
fn kui_sd_rounded_box(p: vec2<f32>, half: vec2<f32>, radii: vec4<f32>) -> f32 {
    let right = p.x > 0.0;
    let bottom = p.y > 0.0;
    let top_r = select(radii.x, radii.y, right);
    let bottom_r = select(radii.w, radii.z, right);
    let r = select(top_r, bottom_r, bottom);
    let rr = min(r, min(half.x, half.y));
    let q = abs(p) - half + vec2<f32>(rr, rr);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - rr;
}
"#;

/// What kui declares after it: the parameters uniform and the entry point.
///
/// The locations are `VsOut`'s in `kui-wgpu/src/shader.wgsl`, because the
/// vertex stage of a fragment pipeline is that file's `vs_main`. The
/// coverage math is `shade`'s, for the solid case: a square node covers a
/// pixel by the area of it inside the quad, a rounded one by its SDF
/// ramped over `KUI_AA`, and a rounded clip the same way.
pub const EPILOGUE: &str = r#"
// The sixteen params, then the image's texel rect — one slot per draw,
// laid out as `kui_wgpu`'s `FragmentParams`.
struct KuiFragmentParams { p: array<vec4<f32>, 4>, image: vec4<f32> };
@group(1) @binding(0) var<uniform> kui_fragment_params: KuiFragmentParams;

@fragment
fn kui_fs_fragment(
    @builtin(position) frag_pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(6) clip: vec4<f32>,
    @location(7) radii: vec4<f32>,
    @location(8) clip_radii: vec4<f32>,
) -> @location(0) vec4<f32> {
    var kui_in: FragmentIn;
    kui_in.local = local;
    kui_in.size = size;
    kui_in.time = kui_globals.time;
    kui_in.scale = kui_globals.scale;
    kui_in.color = vec4<f32>(color.rgb, 1.0);
    kui_in.image = kui_fragment_params.image;
    kui_image_rect = kui_fragment_params.image;
    let kui_c = fragment(kui_in, kui_fragment_params.p);

    // The node's own box, exactly as a solid gets it: a square one by the
    // area of the pixel inside it, so one on whole pixels is solid to its
    // edge and a stack of them meets without a seam; a rounded one by its
    // SDF.
    let kui_half = size * 0.5;
    let kui_d = kui_sd_rounded_box(local - kui_half, kui_half, radii);
    let kui_lo = max(local - vec2<f32>(0.5, 0.5), vec2<f32>(0.0, 0.0));
    let kui_hi = min(local + vec2<f32>(0.5, 0.5), size);
    let kui_area = clamp(kui_hi - kui_lo, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
    let kui_cov = select(
        1.0 - smoothstep(-KUI_AA, KUI_AA, kui_d),
        kui_area.x * kui_area.y,
        all(radii <= vec4<f32>(0.0)),
    );

    // The inherited clip, in framebuffer space, rounded when rounded.
    let kui_p = frag_pos.xy;
    var kui_inside: f32;
    if all(clip_radii <= vec4<f32>(0.0)) {
        kui_inside = f32(
            kui_p.x >= clip.x && kui_p.y >= clip.y
            && kui_p.x <= clip.x + clip.z && kui_p.y <= clip.y + clip.w
        );
    } else {
        let kui_ch = clip.zw * 0.5;
        let kui_cd = kui_sd_rounded_box(kui_p - (clip.xy + kui_ch), kui_ch, clip_radii);
        kui_inside = 1.0 - smoothstep(-KUI_AA, KUI_AA, kui_cd);
    }

    // `color.a` is the group opacity the subtree inherited, times the fill
    // alpha on a polygon; `rgb` reached the function as `in.color`, and a
    // fragment that ignores it returns its own colour as it always did.
    let kui_a = clamp(kui_c.a, 0.0, 1.0) * kui_cov * kui_inside * color.a;
    return vec4<f32>(clamp(kui_c.rgb, vec3<f32>(0.0), vec3<f32>(1.0)) * kui_a, kui_a);
}
"#;

/// The stock fragment a rounded span background is painted with once the
/// frame has joined it with the ones it meets (backlog F101,
/// `crate::join`): one piece of the shape per line, its quad as tall as
/// the line and as wide as the piece and whatever of its neighbours'
/// reach its corners can fill. The params are physical px from the
/// quad's left: `params[0]` this piece `[a, b]` and the line above's,
/// `params[1]` the line below's, the radius and which neighbours there
/// are (1 above, 2 below); the fill is `in.color`, its alpha the quad's.
///
/// A corner is convex where this piece reaches past its neighbour on that
/// side, a fillet past its end where the neighbour reaches past it
/// (concave), square where the two end together, and round where there is
/// no neighbour or it does not overlap. At a join the radius is half the
/// step at most, and both pieces work it out from the same two ends, so
/// the convex half above and the fillet below meet. The ends and the arcs
/// are sampled sixteen times a pixel, and only near them; the pieces meet
/// on whole pixels because a span's background is on them already.
/// Registered by the core, as [`POLYGON`] is.
pub const JOIN: &str = r#"
fn join_radii(cx: f32, e: f32, has: bool, sx: f32, r: f32, lone: f32) -> vec2<f32> {
    // A corner's (convex, concave) radii: `e` the neighbour's end on this
    // side, `sx` outward.
    if !has {
        return vec2<f32>(lone, 0.0);
    }
    let d = (e - cx) * sx;
    return vec2<f32>(min(r, max(-d, 0.0) * 0.5), min(r, max(d, 0.0) * 0.5));
}

fn join_cut(p: vec2<f32>, cx: f32, cy: f32, sx: f32, sy: f32, rc: f32) -> bool {
    // Inside the piece's box, but outside a convex corner's arc.
    let near = (cx - p.x) * sx < rc && (cy - p.y) * sy < rc;
    let c = vec2<f32>(cx - sx * rc, cy - sy * rc);
    return rc > 0.0 && near && distance(p, c) > rc;
}

fn join_fillet(p: vec2<f32>, cx: f32, cy: f32, sx: f32, sy: f32, rf: f32) -> bool {
    // Past the piece's end, inside a concave corner's fillet.
    let dx = (p.x - cx) * sx;
    let dy = (cy - p.y) * sy;
    let c = vec2<f32>(cx + sx * rf, cy - sy * rf);
    return rf > 0.0 && dx >= 0.0 && dx < rf && dy >= 0.0 && dy < rf && distance(p, c) >= rf;
}

fn join_inside(p: vec2<f32>, h: f32, a: f32, b: f32, pv: vec2<f32>, hp: bool, nx: vec2<f32>, hn: bool, r: f32) -> bool {
    let lone = min(r, (b - a) * 0.5);
    let tl = join_radii(a, pv.x, hp, -1.0, r, lone);
    let tr = join_radii(b, pv.y, hp, 1.0, r, lone);
    let bl = join_radii(a, nx.x, hn, -1.0, r, lone);
    let br = join_radii(b, nx.y, hn, 1.0, r, lone);
    let in_box = p.x >= a && p.x < b && p.y >= 0.0 && p.y < h;
    let cut = join_cut(p, a, 0.0, -1.0, -1.0, tl.x) || join_cut(p, b, 0.0, 1.0, -1.0, tr.x)
        || join_cut(p, a, h, -1.0, 1.0, bl.x) || join_cut(p, b, h, 1.0, 1.0, br.x);
    let fill = join_fillet(p, a, 0.0, -1.0, -1.0, tl.y) || join_fillet(p, b, 0.0, 1.0, -1.0, tr.y)
        || join_fillet(p, a, h, -1.0, 1.0, bl.y) || join_fillet(p, b, h, 1.0, 1.0, br.y);
    return (in_box && !cut) || fill;
}

fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let a = params[0].x;
    let b = params[0].y;
    let pv = params[0].zw;
    let nx = params[1].xy;
    let h = in.size.y;
    let r = min(params[1].z, h * 0.5);
    let flags = u32(params[1].w + 0.5);
    // A neighbour shapes the corners only where it overlaps this piece.
    let hp = (flags & 1u) != 0u && pv.x < b && pv.y > a;
    let hn = (flags & 2u) != 0u && nx.x < b && nx.y > a;
    let x = in.local.x;
    // Past the fillets nothing; away from both ends every pixel.
    if x < a - r - 1.0 || x > b + r + 1.0 {
        return vec4<f32>(0.0);
    }
    if x > a + r + 1.0 && x < b - r - 1.0 {
        return vec4<f32>(in.color.rgb, 1.0);
    }
    var n = 0.0;
    for (var i = 0; i < 4; i++) {
        for (var j = 0; j < 4; j++) {
            let o = vec2<f32>((f32(i) + 0.5) * 0.25 - 0.5, (f32(j) + 0.5) * 0.25 - 0.5);
            if join_inside(in.local + o, h, a, b, pv, hp, nx, hn, r) {
                n += 1.0;
            }
        }
    }
    return vec4<f32>(in.color.rgb, n / 16.0);
}
"#;

/// The entry point [`EPILOGUE`] declares — what a backend names when it
/// builds the pipeline.
pub const ENTRY_POINT: &str = "kui_fs_fragment";

/// How many vertices a `polygon` takes: one `vec2` per pair of the
/// sixteen params.
pub const POLYGON_MAX_POINTS: usize = 8;

/// The stock fragment a `polygon` node paints with (ADR 0025, decision
/// 6): up to eight vertices, one per `vec2` of the sixteen params, each
/// normalised to the node's box — a polygon's box is its own bounding box
/// inflated by a pixel, so the vertices span it — the last vertex
/// repeated to pad, filled in `in.color`. The distance is the polygon
/// SDF whose sign flips at every edge crossing — even-odd, the same rule
/// the hit test uses — so a concave outline fills correctly and a
/// self-intersecting one leaves its overlaps unfilled; a padding edge of
/// zero length is skipped before it can divide by itself. The edge ramps over one
/// physical pixel, like a box's. Registered by the core itself, once per
/// session, through the same idempotent `add_fragment` an app's source
/// takes, so `kui_fragment_source` hands a C host the function kui
/// validated.
pub const POLYGON: &str = r#"
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    var v: array<vec2<f32>, 8>;
    for (var i = 0; i < 4; i++) {
        v[i * 2] = params[i].xy * in.size;
        v[i * 2 + 1] = params[i].zw * in.size;
    }
    let p = in.local;
    var d = dot(p - v[0], p - v[0]);
    var s = 1.0;
    var j = 7;
    for (var i = 0; i < 8; i++) {
        let e = v[j] - v[i];
        let w = p - v[i];
        let ee = dot(e, e);
        if ee > 0.0 {
            let b = w - e * clamp(dot(w, e) / ee, 0.0, 1.0);
            d = min(d, dot(b, b));
            let c = vec3<bool>((p.y >= v[i].y), (p.y < v[j].y), (e.x * w.y > e.y * w.x));
            if all(c) || all(!c) {
                s = -s;
            }
        }
        j = i;
    }
    let dist = s * sqrt(d);
    let cov = clamp(0.5 - dist, 0.0, 1.0);
    return vec4<f32>(in.color.rgb, cov);
}
"#;

/// The whole module for an app's source: prelude, the app, epilogue. What
/// the core validates and what a backend compiles, from one function so
/// they cannot differ.
pub fn module_source(app: &str) -> String {
    format!("{PRELUDE}\n{app}\n{EPILOGUE}")
}

/// How far an error's line number has to move to land in the app's own
/// numbering: the prelude's own lines, plus the blank one
/// [`module_source`] puts between it and the app.
fn prelude_lines() -> usize {
    PRELUDE.lines().count() + 1
}

/// Parses and validates an app's fragment source between the prelude and
/// the epilogue. `Err` carries naga's message with line numbers moved into
/// the app's own numbering, so a caller can print it as-is.
///
/// This is 55-73 us for a typical source, which is why `add_fragment` is
/// idempotent: a view that registers every frame should pay a comparison,
/// not this.
pub fn validate(app: &str) -> Result<(), String> {
    use naga::valid::{Capabilities, ValidationFlags, Validator};
    let full = module_source(app);
    let module = naga::front::wgsl::parse_str(&full)
        .map_err(|e| renumber(&e.emit_to_string(&full), prelude_lines()))?;
    Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .map(|_| ())
        .map_err(|e| renumber(&e.emit_to_string(&full), prelude_lines()))
}

/// Rewrites `wgsl:N:` line references by subtracting the prelude's length,
/// so an app sees its own line numbers. A reference inside the prelude or
/// the epilogue clamps to 1 rather than going negative — an app cannot fix
/// a line it did not write, and the message text still names what failed.
fn renumber(msg: &str, offset: usize) -> String {
    let mut out = String::with_capacity(msg.len());
    let mut rest = msg;
    while let Some(i) = rest.find("wgsl:") {
        out.push_str(&rest[..i + 5]);
        rest = &rest[i + 5..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            continue;
        }
        let n = digits.parse::<usize>().unwrap_or(offset + 1);
        out.push_str(&n.saturating_sub(offset).max(1).to_string());
        rest = &rest[digits.len()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRADIENT: &str = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let t = in.local.y / max(in.size.y, 1.0);
    return mix(params[0], params[1], t);
}";

    #[test]
    fn a_gradient_validates() {
        validate(GRADIENT).unwrap();
    }

    /// The stock polygon is validated like any app source — at build,
    /// here, rather than at the first `polygon` node of a session.
    #[test]
    fn the_stock_polygon_validates() {
        validate(POLYGON).unwrap();
    }

    #[test]
    fn the_stock_join_validates() {
        validate(JOIN).unwrap();
    }

    /// `in.color` is what the prelude added for it (ADR 0025).
    #[test]
    fn the_quad_colour_is_reachable_from_the_app() {
        let src = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    return vec4<f32>(in.color.rgb * params[0].x, 1.0);
}";
        validate(src).unwrap();
    }

    /// `kui_sample`, `kui_sample_nearest` and `in.image` are the image
    /// input (backlog V1); a source using them validates without an image
    /// bound, since binding is a per-frame fact.
    #[test]
    fn the_image_input_is_reachable_from_the_app() {
        let src = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let uv = in.local / max(in.size, vec2<f32>(1.0));
    let cells = in.image.zw;
    return mix(kui_sample(uv), kui_sample_nearest(uv), step(1.0, cells.x)) * params[0];
}";
        validate(src).unwrap();
    }

    #[test]
    fn the_prelude_is_reachable_from_the_app() {
        // An app may use `kui_sd_rounded_box`, `KUI_AA`, `time` and `scale`.
        let src = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let d = kui_sd_rounded_box(in.local - in.size * 0.5, in.size * 0.5, vec4<f32>(8.0));
    let a = 1.0 - smoothstep(-KUI_AA, KUI_AA, d);
    return vec4<f32>(params[0].rgb, a * (0.5 + 0.5 * sin(in.time)) * in.scale / in.scale);
}";
        validate(src).unwrap();
    }

    #[test]
    fn a_syntax_error_reports_the_apps_own_line() {
        // The missing semicolon is on the app's line 2.
        let src = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0)
}";
        let err = validate(src).unwrap_err();
        assert!(
            err.contains("wgsl:2:") || err.contains("wgsl:3:"),
            "should point into the app's source, got:\n{err}"
        );
        assert!(
            !err.contains(&format!("wgsl:{}:", prelude_lines() + 2)),
            "line number was not moved out of the prelude:\n{err}"
        );
    }

    #[test]
    fn a_missing_fragment_function_is_refused() {
        let err = validate("fn other() -> f32 { return 1.0; }").unwrap_err();
        assert!(err.contains("fragment"), "{err}");
    }

    #[test]
    fn the_wrong_signature_is_refused() {
        let err = validate("fn fragment() -> vec4<f32> { return vec4<f32>(1.0); }").unwrap_err();
        assert!(
            !err.is_empty(),
            "a no-argument `fragment` must not validate"
        );
    }

    #[test]
    fn an_empty_source_is_refused() {
        assert!(validate("").is_err());
    }

    #[test]
    fn the_module_is_the_three_parts_in_order() {
        let m = module_source(GRADIENT);
        let (p, a, e) = (
            m.find("struct FragmentIn").unwrap(),
            m.find("fn fragment(").unwrap(),
            m.find(ENTRY_POINT).unwrap(),
        );
        assert!(p < a && a < e, "prelude, app, epilogue");
        assert!(m.contains(ENTRY_POINT));
    }
}
