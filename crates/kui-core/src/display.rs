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
    /// Atlas texels: x, y, w, h.
    pub uv: [u32; 4],
    /// Clip rect in physical pixels; pixels outside are discarded.
    pub clip: Rect,
    /// Corner radii of the clip, clockwise from the top-left, in physical
    /// pixels: pixels outside the *rounded* `clip` are discarded too. All
    /// zero — every quad of a frame that has no rounded clipper — means the
    /// clip is the plain rect, which is what lets a backend skip the second
    /// SDF. See [`Clip`] for where the radii come from.
    pub clip_radius: [f32; 4],
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

#[derive(Default)]
pub struct DisplayList {
    pub quads: Vec<Quad>,
    /// Physical pixels.
    pub viewport: Size,
    pub scale: f32,
}

impl DisplayList {
    pub fn clear(&mut self) {
        self.quads.clear();
    }
}
