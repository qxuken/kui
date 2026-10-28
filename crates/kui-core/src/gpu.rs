//! What a renderer of kui's display list draws with, for any GPU API that
//! takes WGSL (backlog F234): the instance a quad becomes, the shader that
//! draws it, and the frame's uniform. kui-wgpu draws through these, and so
//! does a host with its own wgpu device (C's `kui_pack_instances` and
//! `kui_renderer_wgsl`), so a release that moves one moves every renderer
//! with it instead of leaving a copy behind.
//!
//! A frame is a triangle list, six vertices per instance (`draw(0..6,
//! 0..n)`, the shader making the quad's corners from the vertex index),
//! with the instances in a vertex buffer stepped per
//! instance at [`INSTANCE_STRIDE`] bytes and the thirteen attributes at
//! [`INSTANCE_ATTRIBUTES`]. Group 0 binds the [`Globals`] uniform (binding
//! 0), the atlas (1), a linear sampler (2) and a nearest one (3). What it
//! leaves out are passes, not instances: a backdrop blur
//! ([`QuadKind::Backdrop`], drawn as nothing here), an opacity group's
//! picture (only a core asked for groups emits one), a texture-backed
//! image (kind 3 with that image's texture bound in the atlas's place,
//! a run of its own) and a registered fragment (a pipeline of its own).

use crate::display::{Clip, Quad, QuadKind};

/// The WGSL template both variants come from: lines prefixed `//DUAL:`
/// are kept on a device with dual-source blending (LCD subpixel text),
/// `//SINGLE:` ones otherwise ([`shader`]).
pub const SHADER_TEMPLATE: &str = include_str!("gpu/shader.wgsl");

/// One quad as the shader takes it: [`INSTANCE_STRIDE`] bytes, thirteen
/// `vec2`/`vec4` of `f32` at `@location(0)` .. `@location(12)`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug, PartialEq)]
pub struct Instance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
    pub border_color: [f32; 4],
    pub params: [f32; 4],
    pub uv: [f32; 4],
    pub clip: [f32; 4],
    /// Corner radii, clockwise from the top-left.
    pub radii: [f32; 4],
    /// Radii of the clip itself; all zero = a plain rect clip.
    pub clip_radii: [f32; 4],
    /// The turn the quad is drawn through (ADR 0043): angle in radians,
    /// scale, tx, ty — the clip entry's `transform`. The identity on
    /// every quad of a frame with no `rotate` or `scale`.
    pub xform: [f32; 4],
    /// The clip in the quad's own space, before the turn: x, y, w, h.
    pub inner: [f32; 4],
    /// Its radii, as `clip_radii` are the outer clip's.
    pub inner_radii: [f32; 4],
    /// The stretch before the turn (`scaleX` / `scaleY`): sx, sy, shear,
    /// unused — the clip entry's. `[1, 1, 0, 0]` when nothing stretches.
    pub stretch: [f32; 4],
}

/// The bytes one instance takes in the vertex buffer.
pub const INSTANCE_STRIDE: usize = std::mem::size_of::<Instance>();

/// Each attribute's shader location, its float count (2 or 4) and its byte
/// offset in an [`Instance`]: what a pipeline's vertex layout lists.
pub const INSTANCE_ATTRIBUTES: [(u32, u32, usize); 13] = [
    (0, 2, 0),
    (1, 2, 8),
    (2, 4, 16),
    (3, 4, 32),
    (4, 4, 48),
    (5, 4, 64),
    (6, 4, 80),
    (7, 4, 96),
    (8, 4, 112),
    (9, 4, 128),
    (10, 4, 144),
    (11, 4, 160),
    (12, 4, 176),
];

/// The frame's own numbers, at group 0 binding 0: physical viewport, the
/// atlas's (or the bound texture's) size, the frame clock and the scale;
/// the padding makes a uniform's multiple of sixteen.
/// `crate::fragment::PRELUDE` declares the same bytes as `KuiGlobals`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Globals {
    pub viewport: [f32; 2],
    pub atlas_size: [f32; 2],
    pub time: f32,
    pub scale: f32,
    pub _pad: [f32; 2],
}

/// [`INSTANCE_STRIDE`], for a host that asks.
pub fn instance_stride() -> usize {
    INSTANCE_STRIDE
}

/// [`INSTANCE_ATTRIBUTES`], for a host that asks.
pub fn instance_attributes() -> &'static [(u32, u32, usize)] {
    &INSTANCE_ATTRIBUTES
}

/// The shader for a device with dual-source blending (`dual`) or without.
pub fn shader(dual: bool) -> String {
    preprocess(SHADER_TEMPLATE, dual)
}

/// Picks the `//DUAL:` or `//SINGLE:` lines of the shader template.
fn preprocess(src: &str, dual: bool) -> String {
    let (keep, drop) = if dual {
        ("//DUAL:", "//SINGLE:")
    } else {
        ("//SINGLE:", "//DUAL:")
    };
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        if let Some(rest) = line.strip_prefix(keep) {
            out.push_str(rest);
        } else if line.starts_with(drop) {
            continue;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// The clip is resolved out of the frame's table here rather than read off
/// the quad: it rides as an index (`kui_core::ClipId`) so the display list
/// carries it once per distinct clip instead of once per quad.
pub fn instance_of(q: &Quad, clips: &[Clip], textures: &[crate::display::TextureDraw]) -> Instance {
    let clip = clips.get(q.clip as usize).copied().unwrap_or(Clip::NONE);
    let kind = match q.kind {
        QuadKind::Solid => 0.0,
        QuadKind::GlyphMask => 1.0,
        QuadKind::GlyphColor => 2.0,
        QuadKind::Image => 3.0,
        QuadKind::GlyphSubpixel => 4.0,
        QuadKind::Shadow => 5.0,
        QuadKind::Segment => 6.0,
        QuadKind::Fragment => 7.0,
        // Drawn by the image branch with its own texture bound in the
        // atlas's place.
        QuadKind::Texture => 3.0,
        // Never drawn by this pipeline: the pass breaks at it and
        // `backdrop` blurs what is under it. A solid with no colour, so
        // one a frame did not plan a blur for draws nothing in a run.
        QuadKind::Backdrop => 0.0,
        // Where a group's run starts: nothing to draw (ADR 0056).
        QuadKind::GroupBegin => 0.0,
        // A group's picture, its place in the texture written into `uv`
        // once the frame's groups are placed (`plan_groups`).
        QuadKind::GroupEnd => 8.0,
    };
    // `uv` is atlas texels on every kind but two: a segment carries its
    // endpoints there as f32 bits, and a texture quad an index into the
    // side list whose entry holds the texel rect. The shader wants floats.
    let mut border_color = [
        q.border_color.r,
        q.border_color.g,
        q.border_color.b,
        q.border_color.a,
    ];
    let uv = if q.kind == QuadKind::Segment {
        q.segment_ends()
    } else if q.kind == QuadKind::Shadow {
        // The node's own box, knocked out of its shadow (`shade`, kind 5):
        // its insets are `uv`'s float bits as a segment's ends are, and its
        // radii `border_color`'s rgb and `border_w`, the bottom-left one
        // moved into the alpha the shader reads them from
        // (`Quad::shadow_knockout`).
        border_color[3] = q.border_w;
        q.segment_ends()
    } else if q.kind == QuadKind::Texture {
        let uv = textures.get(q.uv[0] as usize).map_or([0; 4], |t| t.uv);
        [uv[0] as f32, uv[1] as f32, uv[2] as f32, uv[3] as f32]
    } else {
        [
            q.uv[0] as f32,
            q.uv[1] as f32,
            q.uv[2] as f32,
            q.uv[3] as f32,
        ]
    };
    if matches!(q.kind, QuadKind::Backdrop | QuadKind::GroupBegin) {
        return Instance {
            pos: [q.rect.x, q.rect.y],
            size: [0.0, 0.0],
            color: [0.0; 4],
            border_color: [0.0; 4],
            params: [0.0; 4],
            uv: [0.0; 4],
            clip: [0.0; 4],
            radii: [0.0; 4],
            clip_radii: [0.0; 4],
            xform: [0.0, 1.0, 0.0, 0.0],
            inner: [0.0; 4],
            inner_radii: [0.0; 4],
            stretch: [1.0, 1.0, 0.0, 0.0],
        };
    }
    Instance {
        pos: [q.rect.x, q.rect.y],
        size: [q.rect.w, q.rect.h],
        color: [q.color.r, q.color.g, q.color.b, q.color.a],
        border_color,
        params: [q.blur, q.border_w, kind, 0.0],
        uv,
        clip: [clip.rect.x, clip.rect.y, clip.rect.w, clip.rect.h],
        radii: q.radius,
        clip_radii: clip.radius,
        xform: [
            clip.transform.angle,
            clip.transform.scale,
            clip.transform.tx,
            clip.transform.ty,
        ],
        inner: [clip.inner.x, clip.inner.y, clip.inner.w, clip.inner.h],
        inner_radii: clip.inner_radius,
        stretch: [
            clip.transform.sx,
            clip.transform.sy,
            clip.transform.shear,
            0.0,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The attribute table is the struct's own layout, so a field added to
    /// one is added to the other or this fails.
    #[test]
    fn the_attribute_table_is_the_instance_s_layout() {
        use std::mem::offset_of;
        let fields = [
            (offset_of!(Instance, pos), 2),
            (offset_of!(Instance, size), 2),
            (offset_of!(Instance, color), 4),
            (offset_of!(Instance, border_color), 4),
            (offset_of!(Instance, params), 4),
            (offset_of!(Instance, uv), 4),
            (offset_of!(Instance, clip), 4),
            (offset_of!(Instance, radii), 4),
            (offset_of!(Instance, clip_radii), 4),
            (offset_of!(Instance, xform), 4),
            (offset_of!(Instance, inner), 4),
            (offset_of!(Instance, inner_radii), 4),
            (offset_of!(Instance, stretch), 4),
        ];
        for (i, ((loc, n, at), (off, len))) in INSTANCE_ATTRIBUTES.iter().zip(fields).enumerate() {
            assert_eq!((*loc as usize, *n, *at), (i, len, off), "attribute {i}");
        }
        let last = INSTANCE_ATTRIBUTES[12];
        assert_eq!(INSTANCE_STRIDE, last.2 + last.1 as usize * 4);
        assert_eq!(INSTANCE_STRIDE, 192);
        assert_eq!(std::mem::size_of::<Globals>(), 32);
    }

    /// Both variants are valid WGSL with the entry points and bindings the
    /// module's doc promises.
    #[test]
    fn both_shaders_parse_and_validate() {
        for dual in [false, true] {
            let src = shader(dual);
            assert_eq!(src.contains("dual_source_blending"), dual);
            let module = naga::front::wgsl::parse_str(&src).expect("parses");
            let caps = if dual {
                naga::valid::Capabilities::DUAL_SOURCE_BLENDING
            } else {
                naga::valid::Capabilities::empty()
            };
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), caps)
                .validate(&module)
                .expect("validates");
            let entries: Vec<&str> = module
                .entry_points
                .iter()
                .map(|e| e.name.as_str())
                .collect();
            assert_eq!(entries, ["vs_main", "fs_main"]);
        }
    }

    /// A backdrop and a group's begin marker draw nothing here: a quad of
    /// no size.
    #[test]
    fn a_pass_s_marker_is_an_empty_instance() {
        let mut q = Quad {
            rect: crate::geom::Rect::new(1.0, 2.0, 30.0, 40.0),
            color: crate::color::Color::WHITE,
            border_color: crate::color::Color::TRANSPARENT,
            radius: crate::display::SQUARE,
            border_w: 0.0,
            blur: 0.0,
            kind: QuadKind::Solid,
            clip: 0,
            uv: [0; 4],
        };
        assert_eq!(instance_of(&q, &[], &[]).size, [30.0, 40.0]);
        q.kind = QuadKind::Backdrop;
        assert_eq!(instance_of(&q, &[], &[]).size, [0.0, 0.0]);
    }
}
