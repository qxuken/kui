//! A fragment: WGSL an app registers, validated here so a frame never sees
//! a source that cannot compile.
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
//! The fixed [`POLYGON`] and [`JOIN`] fragments are validated by the build
//! script with these same sources, wrapper and rules. Their registration
//! does not retain the runtime WGSL compiler; application-provided source
//! still goes through [`validate`] when registered with `Core::add_fragment`.
//!
//! Everything this module declares is spelled `kui_` or `KUI_` so an app's
//! own names cannot collide with it. The two names that are the contract —
//! `FragmentIn` and `fragment` — are not prefixed, because they are what
//! the app writes.

/// Where a node's [`Draw`] lives in the frame's [`FragmentList`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FragmentDrawId(pub u32);

/// What a `fragment` node names: the function, and the image it reads
/// through `kui_sample` if it declared one. Every fragment door takes `impl
/// Into<FragmentRef>`, so a bare
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
/// per node on a tree that pushes 10,000 of them. Four bytes on the
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

mod shader;
pub(crate) use shader::Builtin;
pub use shader::{
    ENTRY_POINT, EPILOGUE, JOIN, POLYGON, POLYGON_MAX_POINTS, PRELUDE, module_source, validate,
};
