//! Draw data shared by the packed app's WASI module and its WebGPU module.
//! The app sends one display list per frame and uploads only changed pixels.
use kui_core::display::{Clip, DisplayList, FragmentImage, Quad, QuadKind, TextureDraw};
use kui_core::geom::Transform;
use kui_core::{Color, Rect};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
struct Frame {
    viewport: [f32; 2],
    scale: f32,
    time: f32,
    background: [f32; 4],
    quads: Vec<QuadData>,
    clips: Vec<ClipData>,
    textures: Vec<TextureData>,
    fragments: Vec<FragmentData>,
    dropped_textures: Vec<u64>,
    dropped_fragments: Vec<u64>,
}

#[cfg(all(target_os = "wasi", not(feature = "rust-wgpu")))]
impl Frame {
    fn record(&mut self, list: &DisplayList, background: [f32; 4]) {
        self.viewport = [list.viewport.w, list.viewport.h];
        self.scale = list.scale;
        self.time = list.time;
        self.background = background;
        self.quads.clear();
        self.quads.extend(list.quads.iter().map(QuadData::from));
        self.clips.clear();
        self.clips.extend(list.clips.iter().map(ClipData::from));
        self.textures.clear();
        self.textures
            .extend(list.textures.iter().map(TextureData::from));
        self.fragments.clear();
        self.fragments
            .extend(list.fragments.iter().map(|f| FragmentData {
                id: f.id.to_ffi(),
                params: f.params,
                image: f.image.into(),
            }));
        self.dropped_textures.clear();
        self.dropped_textures
            .extend(list.dropped_textures.iter().map(|id| id.to_ffi()));
        self.dropped_fragments.clear();
        self.dropped_fragments
            .extend(list.dropped_fragments.iter().map(|id| id.to_ffi()));
    }
}

#[derive(Serialize, Deserialize)]
struct QuadData {
    rect: [f32; 4],
    color: [f32; 4],
    border_color: [f32; 4],
    radius: [f32; 4],
    border_w: f32,
    blur: f32,
    kind: u8,
    clip: u32,
    uv: [u32; 4],
}

impl From<&Quad> for QuadData {
    fn from(q: &Quad) -> Self {
        Self {
            rect: rect(q.rect),
            color: color(q.color),
            border_color: color(q.border_color),
            radius: q.radius,
            border_w: q.border_w,
            blur: q.blur,
            kind: q.kind as u8,
            clip: q.clip,
            uv: q.uv,
        }
    }
}

impl TryFrom<QuadData> for Quad {
    type Error = &'static str;

    fn try_from(q: QuadData) -> Result<Self, Self::Error> {
        Ok(Self {
            rect: unpack_rect(q.rect),
            color: unpack_color(q.color),
            border_color: unpack_color(q.border_color),
            radius: q.radius,
            border_w: q.border_w,
            blur: q.blur,
            kind: *QuadKind::ALL
                .get(q.kind as usize)
                .ok_or("unknown quad kind")?,
            clip: q.clip,
            uv: q.uv,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct ClipData {
    rect: [f32; 4],
    radius: [f32; 4],
    transform: [f32; 7],
    inner: [f32; 4],
    inner_radius: [f32; 4],
}

impl From<&Clip> for ClipData {
    fn from(c: &Clip) -> Self {
        let Transform {
            angle,
            scale,
            tx,
            ty,
            sx,
            sy,
            shear,
        } = c.transform;
        Self {
            rect: rect(c.rect),
            radius: c.radius,
            transform: [angle, scale, tx, ty, sx, sy, shear],
            inner: rect(c.inner),
            inner_radius: c.inner_radius,
        }
    }
}

impl From<ClipData> for Clip {
    fn from(c: ClipData) -> Self {
        let [angle, scale, tx, ty, sx, sy, shear] = c.transform;
        Self {
            rect: unpack_rect(c.rect),
            radius: c.radius,
            transform: Transform {
                angle,
                scale,
                tx,
                ty,
                sx,
                sy,
                shear,
            },
            inner: unpack_rect(c.inner),
            inner_radius: c.inner_radius,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct TextureData {
    id: u64,
    uv: [u32; 4],
}

impl From<&TextureDraw> for TextureData {
    fn from(t: &TextureDraw) -> Self {
        Self {
            id: t.id.to_ffi(),
            uv: t.uv,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct FragmentData {
    id: u64,
    params: [f32; 16],
    image: FragmentImageData,
}

#[derive(Serialize, Deserialize)]
enum FragmentImageData {
    None,
    Atlas([u32; 4]),
    Texture { index: u32, uv: [u32; 4] },
}

impl From<FragmentImage> for FragmentImageData {
    fn from(image: FragmentImage) -> Self {
        match image {
            FragmentImage::None => Self::None,
            FragmentImage::Atlas(uv) => Self::Atlas(uv),
            FragmentImage::Texture { index, uv } => Self::Texture { index, uv },
        }
    }
}

impl From<FragmentImageData> for FragmentImage {
    fn from(image: FragmentImageData) -> Self {
        match image {
            FragmentImageData::None => Self::None,
            FragmentImageData::Atlas(uv) => Self::Atlas(uv),
            FragmentImageData::Texture { index, uv } => Self::Texture { index, uv },
        }
    }
}

fn rect(r: Rect) -> [f32; 4] {
    [r.x, r.y, r.w, r.h]
}

fn unpack_rect([x, y, w, h]: [f32; 4]) -> Rect {
    Rect::new(x, y, w, h)
}

fn color(c: Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}

fn unpack_color([r, g, b, a]: [f32; 4]) -> Color {
    Color { r, g, b, a }
}

#[cfg(all(target_os = "wasi", not(feature = "rust-wgpu")))]
mod sender {
    use super::*;
    use kui_core::{Core, FragmentId, ImageId};
    use std::collections::{HashMap, HashSet};

    #[link(wasm_import_module = "kui_gpu")]
    unsafe extern "C" {
        fn upload_atlas(size: u32, x: u32, y: u32, w: u32, h: u32, pixels: *const u8, len: usize);
        fn upload_texture(id: u64, rev: u32, w: u32, h: u32, pixels: *const u8, len: usize);
        fn upload_fragment(id: u64, source: *const u8, len: usize);
        fn render(frame: *const u8, len: usize) -> u32;
    }

    #[derive(Default)]
    pub(crate) struct Sender {
        frame: Frame,
        bytes: Vec<u8>,
        atlas_rows: Vec<u8>,
        textures: HashMap<ImageId, u32>,
        fragments: HashSet<FragmentId>,
    }

    impl Sender {
        pub fn present(&mut self, core: &mut Core) -> u32 {
            let background = color(core.theme().bg);
            let (list, atlas) = core.output();
            self.frame.record(list, background);
            self.bytes.clear();
            let Ok(bytes) = postcard::to_extend(&self.frame, std::mem::take(&mut self.bytes))
            else {
                return 2;
            };
            self.bytes = bytes;

            // The browser copies each borrowed slice synchronously and catches
            // renderer errors before returning to the app's Wasm stack.
            unsafe {
                if let Some([x, y, w, h]) = atlas.dirty_rect() {
                    let pixels = if x == 0 && w == atlas.size {
                        let start = (y * atlas.size * 4) as usize;
                        &atlas.pixels[start..start + (w * h * 4) as usize]
                    } else {
                        self.atlas_rows.clear();
                        for row in y..y + h {
                            let start = ((row * atlas.size + x) * 4) as usize;
                            self.atlas_rows
                                .extend_from_slice(&atlas.pixels[start..start + (w * 4) as usize]);
                        }
                        &self.atlas_rows
                    };
                    upload_atlas(atlas.size, x, y, w, h, pixels.as_ptr(), pixels.len());
                }
                for id in &list.dropped_textures {
                    self.textures.remove(id);
                }
                for id in &list.dropped_fragments {
                    self.fragments.remove(id);
                }
                for (draw, pixels) in list.textures.iter().zip(&list.texture_pixels) {
                    if self.textures.insert(draw.id, pixels.rev) != Some(pixels.rev) {
                        upload_texture(
                            draw.id.to_ffi(),
                            pixels.rev,
                            pixels.width,
                            pixels.height,
                            pixels.rgba.as_ptr(),
                            pixels.rgba.len(),
                        );
                    }
                }
                for (draw, source) in list.fragments.iter().zip(&list.fragment_sources) {
                    if self.fragments.insert(draw.id) {
                        upload_fragment(draw.id.to_ffi(), source.as_ptr(), source.len());
                    }
                }
                let status = render(self.bytes.as_ptr(), self.bytes.len());
                if status != 2 {
                    atlas.mark_uploaded();
                }
                status
            }
        }
    }
}

#[cfg(all(target_os = "wasi", not(feature = "rust-wgpu")))]
pub(crate) use sender::Sender;

#[cfg(feature = "rust-wgpu")]
mod receiver {
    use super::*;
    use crate::WgpuPainter;
    use kui_core::display::{FragmentDraw, TexturePixels};
    use kui_core::{FragmentId, ImageId, Size};
    use std::{collections::HashMap, sync::Arc};
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub struct PackedRenderer {
        painter: WgpuPainter,
        frame: DisplayList,
        textures: HashMap<ImageId, TexturePixels>,
        fragments: HashMap<FragmentId, Arc<str>>,
    }

    #[wasm_bindgen]
    impl PackedRenderer {
        pub async fn attach(
            canvas: web_sys::HtmlCanvasElement,
            device_lost: js_sys::Function,
        ) -> Result<PackedRenderer, JsValue> {
            let painter = WgpuPainter::new(canvas)
                .await
                .map_err(|err| JsValue::from_str(&err.to_string()))?;
            painter.on_device_lost(move |message| {
                let _ = device_lost.call1(&JsValue::UNDEFINED, &JsValue::from_str(&message));
            });
            Ok(Self {
                painter,
                frame: DisplayList::default(),
                textures: HashMap::new(),
                fragments: HashMap::new(),
            })
        }

        pub fn upload_atlas(
            &mut self,
            size: u32,
            x: u32,
            y: u32,
            w: u32,
            h: u32,
            rgba: &[u8],
        ) -> Result<(), JsValue> {
            self.painter
                .upload_atlas(size, [x, y, w, h], rgba)
                .map_err(|err| JsValue::from_str(&err.to_string()))
        }

        pub fn upload_texture(
            &mut self,
            id: u64,
            rev: u32,
            width: u32,
            height: u32,
            rgba: Vec<u8>,
        ) {
            self.textures.insert(
                ImageId::from_ffi(id),
                TexturePixels {
                    width,
                    height,
                    rev,
                    rgba: Arc::new(rgba),
                },
            );
        }

        pub fn upload_fragment(&mut self, id: u64, source: &str) {
            self.fragments
                .insert(FragmentId::from_ffi(id), Arc::from(source));
        }

        pub fn render(&mut self, bytes: &[u8], width: u32, height: u32) -> Result<u32, JsValue> {
            let packet: Frame =
                postcard::from_bytes(bytes).map_err(|err| JsValue::from_str(&err.to_string()))?;
            let frame = &mut self.frame;
            frame.clear();
            frame.viewport = Size::new(packet.viewport[0], packet.viewport[1]);
            frame.scale = packet.scale;
            frame.time = packet.time;
            for quad in packet.quads {
                frame
                    .quads
                    .push(quad.try_into().map_err(JsValue::from_str)?);
            }
            frame.clips.extend(packet.clips.into_iter().map(Clip::from));
            for id in packet.dropped_textures {
                let id = ImageId::from_ffi(id);
                self.textures.remove(&id);
                frame.dropped_textures.push(id);
            }
            for id in packet.dropped_fragments {
                let id = FragmentId::from_ffi(id);
                self.fragments.remove(&id);
                frame.dropped_fragments.push(id);
            }
            for draw in packet.textures {
                let id = ImageId::from_ffi(draw.id);
                let pixels = self
                    .textures
                    .get(&id)
                    .ok_or_else(|| JsValue::from_str("missing texture upload"))?;
                frame.textures.push(TextureDraw { id, uv: draw.uv });
                frame.texture_pixels.push(pixels.clone());
            }
            for draw in packet.fragments {
                let id = FragmentId::from_ffi(draw.id);
                let source = self
                    .fragments
                    .get(&id)
                    .ok_or_else(|| JsValue::from_str("missing fragment upload"))?;
                frame.fragments.push(FragmentDraw {
                    id,
                    params: draw.params,
                    image: draw.image.into(),
                });
                frame.fragment_sources.push(source.clone());
            }
            self.painter
                .paint_uploaded(frame, unpack_color(packet.background), [width, height])
                .map(u32::from)
                .map_err(|err| JsValue::from_str(&err.to_string()))
        }
    }
}

#[cfg(feature = "rust-wgpu")]
pub use receiver::PackedRenderer;
