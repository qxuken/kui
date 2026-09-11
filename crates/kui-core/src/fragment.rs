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

/// Where a node's [`FragmentDraw`] lives in the frame's [`FragmentList`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FragmentDrawId(pub u32);

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
    draws: crate::retain::Kept<crate::display::FragmentDraw>,
}

impl FragmentList {
    /// Starts a frame. `keep_prev` retains the list just finished so a
    /// departing fragment can copy its draw out of it.
    pub(crate) fn begin_frame(&mut self, keep_prev: bool) {
        self.draws.begin(keep_prev);
    }

    /// Records a draw and returns where it went.
    pub(crate) fn push(&mut self, draw: crate::display::FragmentDraw) -> FragmentDrawId {
        let id = FragmentDrawId(self.draws.len() as u32);
        self.draws.push(draw);
        id
    }

    pub(crate) fn get(&self, id: FragmentDrawId) -> crate::display::FragmentDraw {
        self.draws[id.0 as usize]
    }

    /// A draw from the frame before this one — what a ghost copies. Out of
    /// range when the previous list was not kept, which is a departure the
    /// swap did not expect; it draws nothing rather than something else's
    /// picture.
    pub(crate) fn prev_get(&self, id: FragmentDrawId) -> Option<crate::display::FragmentDraw> {
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
};

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
/// coverage math is `shade`'s, for the solid case: one rounded-box SDF for
/// the node and one for a rounded clip, each ramped over `KUI_AA`.
pub const EPILOGUE: &str = r#"
struct KuiFragmentParams { p: array<vec4<f32>, 4> };
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
    let kui_c = fragment(kui_in, kui_fragment_params.p);

    // The node's own rounded box, exactly as a solid gets it.
    let kui_half = size * 0.5;
    let kui_d = kui_sd_rounded_box(local - kui_half, kui_half, radii);
    let kui_cov = 1.0 - smoothstep(-KUI_AA, KUI_AA, kui_d);

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

    /// `in.color` is what the prelude added for it (ADR 0025).
    #[test]
    fn the_quad_colour_is_reachable_from_the_app() {
        let src = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    return vec4<f32>(in.color.rgb * params[0].x, 1.0);
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
