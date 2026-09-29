//! The renderer boundary: a flat list of quads in physical pixels. A backend
//! needs exactly two abilities — draw these quads, and mirror the glyph atlas
//! to a texture. Everything else (layout, shaping, styling) happened already.

use crate::color::Color;
use crate::geom::{Rect, Size};

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuadKind {
    /// Rounded rect with optional border; ignores uv.
    Solid,
    /// Alpha-mask glyph: atlas alpha times `color`.
    GlyphMask,
    /// Color bitmap glyph (emoji): atlas rgb, alpha times `color.a`.
    GlyphColor,
    /// Registered image blitted into the atlas: atlas rgba tinted by
    /// `color` (white = as-is), rounded by `radius` like a solid.
    /// `border_w` is the `sampling` flag — 0 linear, 1 nearest (ADR 0025,
    /// decision 4) — a slot this kind had no other use for; `blur` is 0.
    Image,
    /// LCD subpixel glyph: atlas rgb is per-channel coverage (times
    /// `color.a`), `color.rgb` the text color. Needs per-channel (dual
    /// source) blending; a backend without it treats the atlas alpha as a
    /// plain mask.
    GlyphSubpixel,
    /// Drop shadow: `color` filling a rounded rect inset from `rect` by
    /// `blur` on every side, its edge ramped over `blur` px. The core
    /// emits it just before the node's own quads, already offset and
    /// spread, so a backend only has to soften the SDF it already
    /// computes. Ignores `uv`, `border_color` and `border_w`.
    Shadow,
    /// A round-capped stroke between two endpoints
    /// (`docs/adr/0010-a-segment-primitive.md`). `uv` holds the endpoints
    /// as `[x0, y0, x1, y1]` in physical px, each an `f32` stored through
    /// `to_bits` — [`Quad::segment_ends`] reads them back — `border_w` is
    /// the stroke width and `color` the stroke. `rect` is the bounding
    /// box, the endpoints inflated by half the width plus two logical px
    /// so the edge ramp is never cut by the quad's own edge. A backend
    /// evaluates an SDF capsule against the fragment's position. Ignores
    /// `radius`, `border_color` and `blur`.
    Segment,
    /// A box a host-registered WGSL function paints
    /// (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`).
    /// `uv[0]` indexes [`DisplayList::fragments`], which carries the
    /// handle and the sixteen parameters; the other three words of `uv`
    /// are zero. `rect`, `radius`, `clip` and `clip_radius` are the
    /// node's and mean what they mean everywhere else — the backend
    /// rounds and clips a fragment exactly as it rounds and clips a
    /// solid. `color` carries the group opacity in its alpha and nothing
    /// else (`rgb` is zero), because a fragment returns its own colour
    /// and a faded subtree still has to fade it. `border_color`,
    /// `border_w` and `blur` are zero and ignored. A backend that cannot
    /// draw one — anything predating this kind — draws nothing, which is
    /// what a missing handle does too.
    Fragment,
    /// A registered image drawn from a texture of its own rather than the
    /// atlas (`docs/adr/0025-the-image-is-the-canvas.md`): one that did
    /// not fit a page, or whose pixels the app has replaced. `uv[0]`
    /// indexes [`DisplayList::textures`], which carries the handle and the
    /// texel rect *in that texture*; the other three words of `uv` are
    /// zero. Everything else is what an `Image` quad's is — `rect`,
    /// `radius`, `clip`, `color` as a tint (white = as-is) with the group
    /// opacity in its alpha, and `border_w` the sampling flag. A backend
    /// binds the texture in place of the atlas for the run and draws it
    /// as an `Image`; one that predates the kind draws nothing.
    Texture,
}

impl QuadKind {
    /// Every kind, in discriminant order — what the conformance report's
    /// `kinds` line counts and the C header's `KUI_QUAD_*` mirror.
    pub const ALL: [QuadKind; 9] = [
        QuadKind::Solid,
        QuadKind::GlyphMask,
        QuadKind::GlyphColor,
        QuadKind::Image,
        QuadKind::GlyphSubpixel,
        QuadKind::Shadow,
        QuadKind::Segment,
        QuadKind::Fragment,
        QuadKind::Texture,
    ];
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    /// Physical pixels.
    pub rect: Rect,
    pub color: Color,
    pub border_color: Color,
    /// Corner radii in physical pixels, clockwise from the top-left:
    /// `[tl, tr, br, bl]`.
    pub radius: [f32; 4],
    pub border_w: f32,
    /// `QuadKind::Shadow` only: the blur radius in physical pixels, which
    /// is also how far `rect` is inflated past the shape being blurred.
    /// Zero elsewhere.
    pub blur: f32,
    pub kind: QuadKind,
    /// Which entry of [`DisplayList::clips`] clips this quad: pixels
    /// outside that rect — and outside its rounded corners, when it has
    /// any — are discarded.
    ///
    /// An index rather than the clip itself because a clip is thirty-two
    /// bytes and a frame has a handful of them: every quad under one card
    /// names the same entry, and a frame that clips nothing names one
    /// entry from every quad it has. Carrying the rect and its four radii
    /// on the quad cost 32 of the 124 bytes each, on a struct written once
    /// per quad and then walked again by the fade pass, the backend's
    /// upload and the previous frame `depart` keeps. The same reasoning
    /// put a fragment's parameters in [`DisplayList::fragments`].
    pub clip: ClipId,
    /// Atlas texels: x, y, w, h. For [`QuadKind::Segment`] the two
    /// endpoints instead, as `f32` bits (see [`Quad::segment_ends`]).
    pub uv: [u32; 4],
}

/// An index into [`DisplayList::clips`]. Every quad has one; there is no
/// "no clip" value, because a frame that clips nothing still names an
/// entry — [`Clip::NONE`] scaled — and a backend that reads it needs no
/// special case.
pub type ClipId = u32;

/// The entry every frame's clip table starts with: [`Clip::NONE`] in
/// physical pixels. Emission seeds it before any quad is made, so a quad
/// that is clipped by nothing — most quads of most frames — names this
/// without interning anything.
pub const NO_CLIP_ID: ClipId = 0;

impl Quad {
    /// A [`QuadKind::Segment`]'s endpoints, `[x0, y0, x1, y1]` in physical
    /// px, decoded from the bits `uv` carries. Meaningless for any other
    /// kind.
    pub fn segment_ends(&self) -> [f32; 4] {
        self.uv.map(f32::from_bits)
    }

    /// The `uv` a [`QuadKind::Segment`] carries for these endpoints.
    pub fn segment_uv(ends: [f32; 4]) -> [u32; 4] {
        ends.map(f32::to_bits)
    }
}

/// A clip that clips nothing.
pub const NO_CLIP: Rect = Rect {
    x: -1e9,
    y: -1e9,
    w: 2e9,
    h: 2e9,
};

/// Radii that round nothing.
pub const SQUARE: [f32; 4] = [0.0; 4];

/// The clip a node inherits: a rect, and the radii to round its corners by.
///
/// A node that clips (`clip`, `scroll_x`, `scroll_y`) and has a `radius`
/// rounds what it clips — the way CSS rounds `overflow: hidden` under a
/// `border-radius` — so the children of a rounded card stay inside its
/// corners instead of poking out of them. Nothing declares this: the radii
/// are the clipping node's own.
///
/// One rounded rect cannot name the intersection of two, so nesting is
/// approximated by [`Clip::intersect`], which says what it gives up.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clip {
    pub rect: Rect,
    /// Clockwise from the top-left: `[tl, tr, br, bl]`. All zero = a plain
    /// rect clip.
    pub radius: [f32; 4],
}

impl Clip {
    /// A clip that clips nothing.
    pub const NONE: Clip = Clip {
        rect: NO_CLIP,
        radius: SQUARE,
    };

    /// A plain rect clip.
    pub fn rect(rect: Rect) -> Self {
        Self {
            rect,
            radius: SQUARE,
        }
    }

    /// Logical to physical pixels.
    pub fn scaled(&self, s: f32) -> Clip {
        Clip {
            rect: self.rect.scaled(s),
            radius: self.radius.map(|r| r * s),
        }
    }

    /// This clip narrowed by a clipping node's box and that node's radii.
    ///
    /// The rect is the plain intersection, as it has always been. The radii
    /// are decided per corner: a corner takes whichever of the two shapes
    /// rounds it *more* (the intersection of two rounded corners is the
    /// tighter one), and only while that corner of the result is still the
    /// same point as that corner of the shape it came from — a corner an
    /// ancestor's straight edge has already cut away is square, which is
    /// what that ancestor made it.
    ///
    /// The one case it approximates: an ancestor edge that cuts *partway*
    /// into a rounded corner moves that corner, so its radius drops to zero
    /// and a sliver at the very corner goes unclipped. A second clipper
    /// offset from the first, both rounded, is the shape that does it.
    pub fn intersect(&self, box_rect: Rect, box_radius: [f32; 4]) -> Clip {
        let rect = self.rect.intersect(&box_rect);
        if self.radius == SQUARE && box_radius == SQUARE {
            return Clip::rect(rect);
        }
        let mut radius = SQUARE;
        for (i, r) in radius.iter_mut().enumerate() {
            let mine = surviving(rect, self.rect, self.radius[i], i);
            let theirs = surviving(rect, box_rect, box_radius[i], i);
            *r = mine.max(theirs);
        }
        Clip { rect, radius }
    }
}

/// `radius`, if corner `i` of `rect` is still corner `i` of `src`; else 0.
/// Corners run clockwise from the top-left, like the radii.
fn surviving(rect: Rect, src: Rect, radius: f32, i: usize) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let (dx, dy) = match i {
        0 => (rect.x - src.x, rect.y - src.y),
        1 => (rect.x + rect.w - src.x - src.w, rect.y - src.y),
        2 => (
            rect.x + rect.w - src.x - src.w,
            rect.y + rect.h - src.y - src.h,
        ),
        _ => (rect.x - src.x, rect.y + rect.h - src.y - src.h),
    };
    // The common case is exact — the intersection kept the whole box; the
    // epsilon is for the sub-pixel drift a laid-out rect can carry.
    if dx.abs() < 0.01 && dy.abs() < 0.01 {
        radius
    } else {
        0.0
    }
}

/// What a [`QuadKind::Fragment`] quad points at: which registered WGSL
/// paints it, and the sixteen numbers that frame passes it.
///
/// It rides beside the quads rather than on them because `Quad` is copied
/// twice per node on a 10,000-node frame and 68 more bytes on it would be
/// paid by every quad of every frame, for a kind almost none of them are
/// (C15, and ADR 0010's reasoning for putting a segment's endpoints in
/// `uv`). A frame that draws no fragment leaves the vector empty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FragmentDraw {
    pub id: crate::resources::FragmentId,
    /// Positional, app-defined; the view's `params` zero-padded to
    /// sixteen. The shader reads them as four `vec4<f32>`.
    pub params: [f32; 16],
    /// The image the function samples through `kui_sample`, resolved to
    /// where its texels are this frame (backlog V1, ADR 0025 decision 7).
    pub image: FragmentImage,
}

/// Where a fragment's `image` row lands for one frame: nowhere, in the
/// glyph atlas the fragment pipeline already has bound, or in a texture
/// of the image's own that the backend binds in the atlas's place for
/// that one quad — the same swap a [`QuadKind::Texture`] quad asks for.
/// The core decides between the last two on the image's backing, so a
/// backend meets the same two cases it already draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FragmentImage {
    /// No `image` row: `kui_sample` returns transparent black.
    #[default]
    None,
    /// The image sits in the atlas at this texel rect, `[x, y, w, h]`.
    Atlas([u32; 4]),
    /// The image has a texture of its own: `index` names the entry of
    /// [`DisplayList::textures`] (and `texture_pixels`) that carries it,
    /// `uv` is the texel rect in that texture — the whole image.
    Texture { index: u32, uv: [u32; 4] },
}

impl FragmentImage {
    /// The texel rect the shader reads as `FragmentIn::image`; zero with
    /// no image.
    pub fn uv(self) -> [u32; 4] {
        match self {
            FragmentImage::None => [0; 4],
            FragmentImage::Atlas(uv) | FragmentImage::Texture { uv, .. } => uv,
        }
    }
}

/// What a [`QuadKind::Texture`] quad points at: which registered image,
/// and the texel rect of it to show (the whole image, or the crop a
/// `fit="cover"` made). Beside the quads for the reason [`FragmentDraw`]
/// is: a handle and a rect on every quad would be paid by the 20,000 that
/// are not one. A frame that draws no texture-backed image leaves the
/// vector empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureDraw {
    pub id: crate::resources::ImageId,
    /// `[x, y, w, h]` in the texture's own texels.
    pub uv: [u32; 4],
}

/// A texture-backed image's pixels as a frame hands them to a backend;
/// see [`DisplayList::texture_pixels`].
#[derive(Clone, Debug)]
pub struct TexturePixels {
    pub width: u32,
    pub height: u32,
    /// Moves with every `update_image`; a backend that uploaded this
    /// revision has nothing to do.
    pub rev: u32,
    pub rgba: std::sync::Arc<Vec<u8>>,
}

#[derive(Default)]
pub struct DisplayList {
    pub quads: Vec<Quad>,
    /// The clips the quads name, in physical pixels. One entry per
    /// *distinct* clip a frame reaches — a handful, even on a frame of
    /// ten thousand quads, because a clip is inherited and only a clipping
    /// node makes a new one. Empty only on a frame that drew nothing.
    pub clips: Vec<Clip>,
    /// One entry per [`QuadKind::Fragment`] quad, indexed by its `uv[0]`.
    /// Empty on a frame that draws none.
    pub fragments: Vec<FragmentDraw>,
    /// The WGSL behind each entry of [`Self::fragments`], at the same
    /// index: what a backend compiles the first time it meets a handle.
    /// It rides here rather than on `FragmentDraw` so that struct stays
    /// `Copy` and digestible; an `Arc` clone per fragment quad is a
    /// refcount bump, and a frame with no fragment has neither vector.
    pub fragment_sources: Vec<std::sync::Arc<str>>,
    /// One entry per [`QuadKind::Texture`] quad, indexed by its `uv[0]`.
    /// Empty on a frame that draws none.
    pub textures: Vec<TextureDraw>,
    /// The pixels behind each entry of [`Self::textures`], at the same
    /// index: what a backend uploads the first time it meets a handle, and
    /// again whenever `rev` has moved. Shared with the resource entry, so
    /// this is a refcount per texture quad and no copy — the reason
    /// `fragment_sources` rides here the same way.
    pub texture_pixels: Vec<TexturePixels>,
    /// Image handles removed since the last frame whose backing was a
    /// texture: what a backend drops from its cache. Cleared with the
    /// quads, so a host that renders one list a frame sees each once —
    /// and a removal is carried by one window's list, whichever drew
    /// next after it, since the device the cache lives on is shared by
    /// every window of the session (AR8).
    pub dropped_textures: Vec<crate::resources::ImageId>,
    /// Fragment handles removed since the last frame: what a backend
    /// drops the pipelines it built for. Carried the same way.
    pub dropped_fragments: Vec<crate::resources::FragmentId>,
    /// Physical pixels.
    pub viewport: Size,
    pub scale: f32,
    /// The frame clock in seconds — the same one transitions read, as the
    /// driver last set it. A backend hands it to a fragment as
    /// `FragmentIn::time`; nothing else reads it. Zero when the driver
    /// never set a clock, which is what a headless frame looks like.
    pub time: f32,
}

impl DisplayList {
    pub fn clear(&mut self) {
        self.quads.clear();
        self.clips.clear();
        self.fragments.clear();
        self.fragment_sources.clear();
        self.textures.clear();
        self.texture_pixels.clear();
        self.dropped_textures.clear();
        self.dropped_fragments.clear();
    }

    /// The clip a quad names. Out of range — which a well-formed frame
    /// never is — reads as clipping nothing, so a malformed list draws
    /// rather than panics.
    pub fn clip_of(&self, q: &Quad) -> Clip {
        self.clips
            .get(q.clip as usize)
            .copied()
            .unwrap_or(Clip::NONE)
    }

    /// Interns a clip and returns its index. See [`intern_clip`].
    pub fn intern_clip(&mut self, clip: Clip) -> ClipId {
        intern_clip(&mut self.clips, clip)
    }
}

/// Interns a clip into a frame's table and returns the index a quad names.
///
/// Only the last entry is compared, so this is a constant-time append with
/// a run-length check and not a real intern: a clip that comes back after
/// another one gets a second entry. That is deliberate. Emission runs in
/// paint order, so equal clips arrive in runs, and the alternative — a scan
/// of the whole table — is quadratic on the one frame shape that makes many
/// clips (a screen of width-clamped labels, which narrows the clip once per
/// label). A duplicate costs thirty-two bytes on a list that is orders of
/// magnitude shorter than the quads; a quadratic scan costs the frame.
///
/// Callers avoid most of the calls entirely: a node whose clip is its
/// parent's reuses the index the parent interned without comparing anything
/// (`Core::emit_frame`).
pub fn intern_clip(clips: &mut Vec<Clip>, clip: Clip) -> ClipId {
    if let Some(last) = clips.last()
        && *last == clip
    {
        return clips.len() as ClipId - 1;
    }
    clips.push(clip);
    clips.len() as ClipId - 1
}
