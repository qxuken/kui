//! Portable render-only packets. Resource handles are reminted on import;
//! historical resources cannot alias the inspector's live renderer caches.
use super::artifact;
use kui_core::display::{
    Clip, DisplayList, FragmentDraw, FragmentImage, Quad, QuadKind, TextureDraw, TexturePixels,
};
use kui_core::geom::Transform;
use kui_core::{Color, Core, Rect, Size};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
#[derive(Serialize, Deserialize)]
struct Q {
    rect: [f32; 4],
    color: [f32; 4],
    border: [f32; 4],
    radius: [f32; 4],
    border_w: f32,
    blur: f32,
    kind: usize,
    clip: u32,
    uv: [u32; 4],
}
#[derive(Serialize, Deserialize)]
struct C {
    rect: [f32; 4],
    radius: [f32; 4],
    transform: [f32; 7],
    inner: [f32; 4],
    inner_radius: [f32; 4],
}
#[derive(Serialize, Deserialize)]
struct F {
    id: u64,
    params: [f32; 16],
    image_kind: u8,
    image_index: u32,
    image_uv: [u32; 4],
    source: String,
}
#[derive(Serialize, Deserialize)]
struct T {
    id: u64,
    uv: [u32; 4],
    width: u32,
    height: u32,
    pixels: String,
}
#[derive(Serialize, Deserialize)]
struct Packet {
    version: u32,
    quads: Vec<Q>,
    clips: Vec<C>,
    fragments: Vec<F>,
    textures: Vec<T>,
    viewport: [f32; 2],
    scale: f32,
    time: f32,
    background: [f32; 4],
    atlas_size: u32,
    atlas: String,
}
thread_local! {static ATLASES:std::cell::RefCell<std::collections::HashMap<(std::path::PathBuf,u64,u64,u64),String>>=std::cell::RefCell::new(std::collections::HashMap::new());}
fn atlas_blob(root: &Path, atlas: &kui_core::atlas::GlyphAtlas) -> Result<String, String> {
    let key = (root.to_path_buf(), atlas.id(), atlas.epoch, atlas.rev);
    if let Some(name) = ATLASES
        .with(|c| c.borrow().get(&key).cloned())
        .filter(|n| root.join(n).is_file())
    {
        return Ok(name);
    }
    let name = artifact::blob(root, &atlas.pixels)?;
    ATLASES.with(|c| {
        let mut cache = c.borrow_mut();
        if cache.len() > 32 {
            cache.clear();
        }
        cache.insert(key, name.clone());
    });
    Ok(name)
}
fn rect(r: Rect) -> [f32; 4] {
    [r.x, r.y, r.w, r.h]
}
fn color(c: Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}
fn r(v: [f32; 4]) -> Rect {
    Rect::new(v[0], v[1], v[2], v[3])
}
fn c(v: [f32; 4]) -> Color {
    Color {
        r: v[0],
        g: v[1],
        b: v[2],
        a: v[3],
    }
}
pub fn save(root: &Path, core: &Core) -> Result<String, String> {
    let (dl, atlas) = core.drawn();
    let quads = dl
        .quads
        .iter()
        .map(|q| Q {
            rect: rect(q.rect),
            color: color(q.color),
            border: color(q.border_color),
            radius: q.radius,
            border_w: q.border_w,
            blur: q.blur,
            kind: q.kind as usize,
            clip: q.clip,
            uv: q.uv,
        })
        .collect();
    let clips = dl
        .clips
        .iter()
        .map(|v| C {
            rect: rect(v.rect),
            radius: v.radius,
            transform: [
                v.transform.angle,
                v.transform.scale,
                v.transform.tx,
                v.transform.ty,
                v.transform.sx,
                v.transform.sy,
                v.transform.shear,
            ],
            inner: rect(v.inner),
            inner_radius: v.inner_radius,
        })
        .collect();
    let fragments = dl
        .fragments
        .iter()
        .zip(&dl.fragment_sources)
        .map(|(f, source)| {
            let (image_kind, image_index) = match f.image {
                FragmentImage::None => (0, 0),
                FragmentImage::Atlas(_) => (1, 0),
                FragmentImage::Texture { index, .. } => (2, index),
            };
            F {
                id: f.id.to_ffi(),
                params: f.params,
                image_kind,
                image_index,
                image_uv: f.image.uv(),
                source: source.to_string(),
            }
        })
        .collect();
    let textures = dl
        .textures
        .iter()
        .zip(&dl.texture_pixels)
        .map(|(t, p)| {
            Ok(T {
                id: t.id.to_ffi(),
                uv: t.uv,
                width: p.width,
                height: p.height,
                pixels: artifact::blob(root, &p.rgba)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let packet = Packet {
        version: 1,
        quads,
        clips,
        fragments,
        textures,
        viewport: [dl.viewport.w, dl.viewport.h],
        scale: dl.scale,
        time: dl.time,
        background: color(core.theme().bg),
        atlas_size: atlas.size,
        atlas: atlas_blob(root, atlas)?,
    };
    let bytes = serde_json::to_vec(&packet).map_err(|e| e.to_string())?;
    artifact::blob(root, &bytes)
}
pub fn render(root: &Path, name: &str) -> Result<kui_native::Capture, String> {
    let bytes = artifact::read_blob(root, name, 64 * 1024 * 1024)?;
    let p: Packet = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if p.version != 1 {
        return Err("unsupported drawing packet version".into());
    }
    if p.atlas_size == 0
        || p.atlas_size > 16384
        || p.viewport
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0 || *v > 16384.0)
        || !p.scale.is_finite()
        || p.scale <= 0.0
    {
        return Err("invalid drawing dimensions".into());
    }
    let mut dl = DisplayList {
        viewport: Size::new(p.viewport[0], p.viewport[1]),
        scale: p.scale,
        time: p.time,
        ..Default::default()
    };
    for q in p.quads {
        let kind = *QuadKind::ALL.get(q.kind).ok_or("invalid quad kind")?;
        if q.clip as usize >= p.clips.len() {
            return Err("invalid clip reference".into());
        }
        dl.quads.push(Quad {
            rect: r(q.rect),
            color: c(q.color),
            border_color: c(q.border),
            radius: q.radius,
            border_w: q.border_w,
            blur: q.blur,
            kind,
            clip: q.clip,
            uv: q.uv,
        });
    }
    dl.clips = p
        .clips
        .into_iter()
        .map(|v| {
            let t = v.transform;
            Clip {
                rect: r(v.rect),
                radius: v.radius,
                transform: Transform {
                    angle: t[0],
                    scale: t[1],
                    tx: t[2],
                    ty: t[3],
                    sx: t[4],
                    sy: t[5],
                    shear: t[6],
                },
                inner: r(v.inner),
                inner_radius: v.inner_radius,
            }
        })
        .collect();
    static NEXT: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0x8000_0000_0000_0001);
    let mut ids = std::collections::HashMap::new();
    for t in p.textures {
        if t.width == 0 || t.height == 0 || t.width > 16384 || t.height > 16384 {
            return Err("invalid texture size".into());
        }
        let id = *ids.entry(t.id).or_insert_with(|| {
            kui_core::ImageId::from_ffi(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        });
        let size = t.width as usize * t.height as usize * 4;
        let rgba = artifact::read_blob(root, &t.pixels, size)?;
        if rgba.len() != size {
            return Err("invalid texture data length".into());
        }
        dl.textures.push(TextureDraw { id, uv: t.uv });
        dl.texture_pixels.push(TexturePixels {
            width: t.width,
            height: t.height,
            rev: 1,
            rgba: Arc::new(rgba),
        });
    }
    let mut fragments = std::collections::HashMap::new();
    for f in p.fragments {
        let id = *fragments.entry(f.id).or_insert_with(|| {
            kui_core::FragmentId::from_ffi(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        });
        let image = match f.image_kind {
            0 => FragmentImage::None,
            1 => FragmentImage::Atlas(f.image_uv),
            2 if (f.image_index as usize) < dl.textures.len() => FragmentImage::Texture {
                index: f.image_index,
                uv: f.image_uv,
            },
            _ => return Err("invalid fragment image".into()),
        };
        dl.fragments.push(FragmentDraw {
            id,
            params: f.params,
            image,
        });
        dl.fragment_sources.push(Arc::from(f.source));
    }
    for q in &dl.quads {
        if q.kind == QuadKind::Fragment && q.uv[0] as usize >= dl.fragments.len()
            || q.kind == QuadKind::Texture && q.uv[0] as usize >= dl.textures.len()
        {
            return Err("invalid quad resource reference".into());
        }
    }
    let size = p.atlas_size as usize * p.atlas_size as usize * 4;
    let bytes = artifact::read_blob(root, &p.atlas, size)?;
    if bytes.len() != size {
        return Err("invalid atlas data length".into());
    }
    let mut atlas = kui_core::atlas::GlyphAtlas::with_size(p.atlas_size);
    atlas.pixels = bytes;
    atlas.dirty = true;
    atlas.rev = 1;
    kui_native::capture_drawing(&dl, &atlas, c(p.background))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drawing_packet_reconstructs_pixels_without_a_live_core() {
        let dir = tempfile::tempdir().unwrap();
        let mut core = Core::new_in(&kui_core::Session::with_fonts(
            "en-US",
            [include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").to_vec()],
        ));
        let id = core.resources.add_image(2, 2, vec![255; 16]);
        core.update_image(
            id,
            2,
            2,
            vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
            ],
        );
        let fragment=core.add_fragment("fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {return kui_sample(in.local / max(in.size,vec2<f32>(1.0))) * vec4<f32>(params[0].xyz,1.0);}").unwrap();
        core.set_time(0.375);
        let mut ui = core.frame(Size::new(140.0, 100.0), 1.0);
        ui.with(
            kui_core::NodeSpec::column()
                .size(140.0, 100.0)
                .clip()
                .radius(6.0)
                .bg(Color::hex(0x345678ff)),
            |ui| {
                ui.text("recorded", kui_core::TextStyle::new(14.0));
                ui.image(
                    id,
                    kui_core::NodeSpec::row().size(40.0, 40.0).label("texture"),
                );
                ui.fragment(
                    fragment.with_image(id),
                    &[0.5, 0.75, 1.0, 0.0],
                    kui_core::NodeSpec::row()
                        .size(30.0, 20.0)
                        .label("shader texture"),
                );
            },
        );
        ui.finish();
        let _ = core.output();
        let original = kui_native::capture(&core).unwrap();
        let packet = save(dir.path(), &core).unwrap();
        drop(core);
        let replay = render(dir.path(), &packet).unwrap();
        assert_eq!(original, replay);
        assert!(artifact::safe_join(dir.path(), "../escape").is_err());
    }
}
