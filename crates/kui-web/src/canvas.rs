use kui_core::display::{Clip, NO_CLIP_ID, Quad, TextureDraw};
use kui_core::resources::ImageId;
use kui_core::{Color, Core, QuadKind, Rect, Size};
use std::collections::HashMap;

#[link(wasm_import_module = "canvas")]
unsafe extern "C" {
    fn background(r: f32, g: f32, b: f32, a: f32, x: f32, y: f32, w: f32, h: f32);
    fn present();
    fn upload_atlas(pixels: *const u8, size: u32, x: u32, y: u32, w: u32, h: u32);
    fn upload_texture(id: u64, pixels: *const u8, width: u32, height: u32);
    fn drop_texture(id: u64);
    fn image(
        texture: u64,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        sx: u32,
        sy: u32,
        sw: u32,
        sh: u32,
        tl: f32,
        tr: f32,
        br: f32,
        bl: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        angle: f32,
        nearest: u32,
    );
    fn reset_clip();
    fn clip(x: f32, y: f32, w: f32, h: f32, tl: f32, tr: f32, br: f32, bl: f32);
    fn solid(
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tl: f32,
        tr: f32,
        br: f32,
        bl: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        border_width: f32,
        rr: f32,
        rg: f32,
        rb: f32,
        ra: f32,
    );
    fn glyph(
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        sx: u32,
        sy: u32,
        sw: u32,
        sh: u32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        colored: u32,
    );
}

#[derive(Default)]
pub(crate) struct Painter {
    surface: Option<(Size, Color)>,
    quads: Vec<Quad>,
    clips: Vec<Clip>,
    textures: Vec<TextureDraw>,
    uploaded: HashMap<ImageId, u32>,
    atlas_uploaded: bool,
}

impl Painter {
    pub(crate) fn invalidate(&mut self) {
        self.surface = None;
        self.uploaded.clear();
    }

    pub(crate) fn paint(&mut self, core: &mut Core) {
        let bg = core.theme().bg;
        let (list, atlas) = core.output();
        let surface = (list.viewport, bg);
        let atlas_dirty = if self.atlas_uploaded {
            atlas.dirty_rect()
        } else {
            Some([0, 0, atlas.size, atlas.size])
        };
        let mut textures_changed = self.textures != list.textures;
        // SAFETY: the imports consume the borrowed pixels synchronously.
        unsafe {
            for id in &list.dropped_textures {
                self.uploaded.remove(id);
                drop_texture(id.to_ffi());
            }
            for (draw, pixels) in list.textures.iter().zip(&list.texture_pixels) {
                if self.uploaded.insert(draw.id, pixels.rev) != Some(pixels.rev) {
                    upload_texture(
                        draw.id.to_ffi(),
                        pixels.rgba.as_ptr(),
                        pixels.width,
                        pixels.height,
                    );
                    textures_changed = true;
                }
            }
        }
        let damage = if self.surface != Some(surface)
            || bg.a != 1.0
            || atlas_dirty.is_some()
            || self.clips != list.clips
            || textures_changed
        {
            None
        } else {
            let Some(bounds) = changed_bounds(&self.quads, &list.quads) else {
                return;
            };
            Some(bounds)
        };
        self.surface = Some(surface);
        self.quads.clear();
        self.quads.extend_from_slice(&list.quads);
        self.clips.clone_from(&list.clips);
        self.textures.clone_from(&list.textures);

        let mut current_clip = NO_CLIP_ID;
        // A zero width requests a full repaint. Partial repaints draw
        // intersecting quads in order and present only whole pixels.
        let rect = damage.unwrap_or_default();
        // SAFETY: The canvas imports take scalars except for upload_atlas. It
        // consumes the borrowed pixels synchronously and keeps no Wasm memory view.
        unsafe {
            background(bg.r, bg.g, bg.b, bg.a, rect.x, rect.y, rect.w, rect.h);
            if let Some([x, y, w, h]) = atlas_dirty {
                upload_atlas(atlas.pixels.as_ptr(), atlas.size, x, y, w, h);
                atlas.mark_uploaded();
                self.atlas_uploaded = true;
            }
            for quad in &list.quads {
                if let Some(damage) = damage {
                    let overlap = quad.rect.intersect(&damage);
                    if overlap.w == 0.0 || overlap.h == 0.0 {
                        continue;
                    }
                }
                if quad.clip != current_clip {
                    reset_clip();
                    current_clip = quad.clip;
                    if current_clip != NO_CLIP_ID {
                        let c = &list.clips[current_clip as usize];
                        let [tl, tr, br, bl] = c.radius;
                        clip(c.rect.x, c.rect.y, c.rect.w, c.rect.h, tl, tr, br, bl);
                    }
                }
                let rect = quad.rect;
                let c = quad.color;
                match quad.kind {
                    QuadKind::Solid => {
                        let [tl, tr, br, bl] = quad.radius;
                        let b = quad.border_color;
                        solid(
                            rect.x,
                            rect.y,
                            rect.w,
                            rect.h,
                            tl,
                            tr,
                            br,
                            bl,
                            c.r,
                            c.g,
                            c.b,
                            c.a,
                            quad.border_w,
                            b.r,
                            b.g,
                            b.b,
                            b.a,
                        );
                    }
                    QuadKind::GlyphMask | QuadKind::GlyphColor | QuadKind::GlyphSubpixel => {
                        let [sx, sy, sw, sh] = quad.uv;
                        glyph(
                            rect.x,
                            rect.y,
                            rect.w,
                            rect.h,
                            sx,
                            sy,
                            sw,
                            sh,
                            c.r,
                            c.g,
                            c.b,
                            c.a,
                            u32::from(quad.kind == QuadKind::GlyphColor),
                        );
                    }
                    QuadKind::Image | QuadKind::Texture => {
                        let (texture, uv) = if quad.kind == QuadKind::Texture {
                            let draw = list.textures[quad.uv[0] as usize];
                            (draw.id.to_ffi(), draw.uv)
                        } else {
                            (0, quad.uv)
                        };
                        let [sx, sy, sw, sh] = uv;
                        let [tl, tr, br, bl] = quad.radius;
                        image(
                            texture,
                            rect.x,
                            rect.y,
                            rect.w,
                            rect.h,
                            sx,
                            sy,
                            sw,
                            sh,
                            tl,
                            tr,
                            br,
                            bl,
                            c.r,
                            c.g,
                            c.b,
                            c.a,
                            quad.blur,
                            u32::from(quad.border_w != 0.0),
                        );
                    }
                    _ => {}
                }
            }
            reset_clip();
            present();
        }
    }
}

fn same_quad(a: &Quad, b: &Quad) -> bool {
    a.kind == b.kind
        && a.rect == b.rect
        && a.clip == b.clip
        && a.color == b.color
        && a.uv == b.uv
        && a.radius == b.radius
        && a.blur == b.blur
        && a.border_w == b.border_w
        && a.border_color == b.border_color
}

fn changed_bounds(before: &[Quad], after: &[Quad]) -> Option<Rect> {
    let prefix = before
        .iter()
        .zip(after)
        .take_while(|(a, b)| same_quad(a, b))
        .count();
    let (before, after) = (&before[prefix..], &after[prefix..]);
    let suffix = before
        .iter()
        .rev()
        .zip(after.iter().rev())
        .take_while(|(a, b)| same_quad(a, b))
        .count();
    let bounds = before[..before.len() - suffix]
        .iter()
        .chain(&after[..after.len() - suffix])
        .map(|quad| quad.rect)
        .reduce(|a, b| a.union(&b))?;
    // Leave one pixel for edge coverage when a quad lands between pixels.
    let x = bounds.x.floor() - 1.0;
    let y = bounds.y.floor() - 1.0;
    Some(Rect::new(
        x,
        y,
        (bounds.x + bounds.w).ceil() + 1.0 - x,
        (bounds.y + bounds.h).ceil() + 1.0 - y,
    ))
}
