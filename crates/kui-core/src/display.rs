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
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    /// Physical pixels.
    pub rect: Rect,
    pub color: Color,
    pub border_color: Color,
    pub radius: f32,
    pub border_w: f32,
    pub kind: QuadKind,
    /// Atlas texels: x, y, w, h.
    pub uv: [u32; 4],
    /// Clip rect in physical pixels; pixels outside are discarded.
    pub clip: Rect,
}

/// A clip that clips nothing.
pub const NO_CLIP: Rect = Rect { x: -1e9, y: -1e9, w: 2e9, h: 2e9 };

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
