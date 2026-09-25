//! wgpu backend: one über-pipeline drawing instanced quads (rounded rects,
//! borders, and atlas glyphs unified), so a whole UI is a single draw call.
//! Consumes `kui_core::DisplayList` and mirrors the core's glyph atlas.
//!
//! Where the device offers dual-source blending (Metal, DX12, most Vulkan)
//! the pipeline blends per channel, which is what LCD subpixel text needs:
//! the fragment shader emits premultiplied color plus a per-channel
//! coverage, and the blend is `src + dst * (1 - coverage)` channel-wise.
//! Otherwise it falls back to ordinary alpha blending and subpixel glyphs
//! draw from their union coverage (grayscale).

pub use wgpu;

use kui_core::atlas::GlyphAtlas;
use kui_core::{Clip, DisplayList, Quad, QuadKind};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    pos: [f32; 2],
    size: [f32; 2],
    color: [f32; 4],
    border_color: [f32; 4],
    params: [f32; 4],
    uv: [f32; 4],
    clip: [f32; 4],
    /// Corner radii, clockwise from the top-left.
    radii: [f32; 4],
    /// Radii of the clip itself; all zero = a plain rect clip.
    clip_radii: [f32; 4],
}

/// The frame's own numbers, at group 0 binding 0 for both pipelines.
/// `kui_core::fragment::PRELUDE` declares the same bytes as `KuiGlobals`
/// so an app's fragment can read `time` and `scale`; the padding is what
/// makes the struct a multiple of sixteen, which a uniform must be.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    viewport: [f32; 2],
    atlas_size: [f32; 2],
    time: f32,
    scale: f32,
    _pad: [f32; 2],
}

/// One fragment's parameters as the shader takes them — the sixteen
/// floats and the texel rect of its `image`, laid out as the epilogue's
/// `KuiFragmentParams` — padded out to the device's dynamic-offset
/// alignment so a frame's draws can share one buffer and pick their slot
/// by offset.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FragmentParams {
    params: [f32; 16],
    /// `FragmentIn::image`: `[x, y, w, h]` in the texture bound at group 0
    /// for this draw — the atlas, or the image's own; zero with none.
    image: [f32; 4],
}

/// The clip is resolved out of the frame's table here rather than read off
/// the quad: it rides as an index (`kui_core::ClipId`) so the display list
/// carries it once per distinct clip instead of once per quad.
fn instance_of(q: &Quad, clips: &[Clip], textures: &[kui_core::display::TextureDraw]) -> Instance {
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
        // atlas's place (ADR 0025, decision 3).
        QuadKind::Texture => 3.0,
    };
    // `uv` is atlas texels on every kind but two: a segment carries its
    // endpoints there as f32 bits, and a texture quad an index into the
    // side list whose entry holds the texel rect. The shader wants floats.
    let uv = if q.kind == QuadKind::Segment {
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
    Instance {
        pos: [q.rect.x, q.rect.y],
        size: [q.rect.w, q.rect.h],
        color: [q.color.r, q.color.g, q.color.b, q.color.a],
        border_color: [
            q.border_color.r,
            q.border_color.g,
            q.border_color.b,
            q.border_color.a,
        ],
        params: [q.blur, q.border_w, kind, 0.0],
        uv,
        clip: [clip.rect.x, clip.rect.y, clip.rect.w, clip.rect.h],
        radii: q.radius,
        clip_radii: clip.radius,
    }
}

/// The GPU objects a session's windows share: one instance, one adapter,
/// one device, one queue. Windows must share a device before a second one
/// is worth opening — two devices cannot see each other's buffers or
/// textures, and each costs a driver context — so a `Renderer` holds a
/// handle to one rather than making its own. Cloning a `Gpu` clones the
/// handle; `Renderer::new` makes a private one for its window, which is
/// what a single-window app gets and never has to name.
#[derive(Clone)]
pub struct Gpu(std::sync::Arc<GpuInner>);

struct GpuInner {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    dual_source: bool,
    /// One pipeline per registered fragment per surface format, built the
    /// first time a frame draws it and shared by every window on this
    /// device — the cost the ADR measured at about 0.2 ms, paid once —
    /// and dropped when a frame's list says the handle is gone
    /// (`dropped_fragments`). A `Mutex` because `Gpu` is a shared handle
    /// and building is rare; nothing here is touched on a frame that
    /// draws no new fragment.
    fragment_pipelines: std::sync::Mutex<
        std::collections::HashMap<(u64, wgpu::TextureFormat), wgpu::RenderPipeline>,
    >,
    /// One texture per texture-backed image, uploaded the first time a
    /// frame on this device draws it and again when its revision moves,
    /// shared by every window like the pipelines above, dropped when the
    /// core says the handle is gone (ADR 0025, decisions 2 and 3).
    textures: std::sync::Mutex<std::collections::HashMap<u64, std::sync::Arc<ImageTexture>>>,
    /// Set by the device's lost callback: a driver update, a GPU reset, a
    /// hang the OS answered by removing the device. Nothing on it works
    /// again; a shell opens a new one ([`Gpu::lost`]).
    lost: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// A texture-backed image on the device: the texture, and what was
/// uploaded into it. A new `Arc` is made when the size changes, which is
/// what tells a renderer its bind group is stale.
struct ImageTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    /// The revision the pixels in the texture came from, behind a lock
    /// because the texture is shared and the upload is per device.
    rev: std::sync::Mutex<u32>,
}

impl Gpu {
    /// Opens the shared device, choosing an adapter that can present to
    /// `target`'s surface — the first window's, whose surface comes back
    /// with it because it has to exist before the adapter can be picked.
    /// Every later window's surface comes from [`Gpu::create_surface`].
    pub async fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
    ) -> Result<(Self, wgpu::Surface<'static>), Box<dyn std::error::Error>> {
        report_faults();
        // Every backend the build has, as wgpu defaults — but on Windows
        // D3D12 alone unless `WGPU_BACKEND` names another. An instance
        // keeps every backend it enumerated alive for as long as it lives,
        // so with all of them the process holds an OpenGL context and a
        // Vulkan instance it never draws with, both in the driver's
        // `nvoglv64.dll`; and wgpu, left to choose, took Vulkan over D3D12
        // here. Under a driver update that DLL faulted in present rather
        // than answer `DEVICE_LOST`, which ended the process; D3D12's
        // `nvwgf2umx.dll` reports the removal, and the shell reopens the
        // device (`Gpu::lost`).
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        if cfg!(windows) && std::env::var_os("WGPU_BACKEND").is_none() {
            desc.backends = wgpu::Backends::DX12;
        }
        let instance = wgpu::Instance::new(desc);
        let surface = instance.create_surface(target)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;
        // Per-channel blending for LCD subpixel text, when the device has it.
        let dual_source = adapter
            .features()
            .contains(wgpu::Features::DUAL_SOURCE_BLENDING);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: if dual_source {
                    wgpu::Features::DUAL_SOURCE_BLENDING
                } else {
                    wgpu::Features::empty()
                },
                ..Default::default()
            })
            .await?;
        // What goes wrong on the device is said, not swallowed: an error
        // outside a scope, and the loss of the device itself — remembered
        // too, so a frame can tell a dead device from a stale swapchain.
        device.on_uncaptured_error(std::sync::Arc::new(|e| eprintln!("kui: wgpu: {e}")));
        let lost = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        device.set_device_lost_callback({
            let lost = lost.clone();
            move |reason, message| {
                if reason == wgpu::DeviceLostReason::Unknown {
                    eprintln!("kui: device lost: {message}");
                    lost.store(true, std::sync::atomic::Ordering::Release);
                }
            }
        });
        let gpu = Self(std::sync::Arc::new(GpuInner {
            instance,
            adapter,
            device,
            queue,
            dual_source,
            fragment_pipelines: Default::default(),
            textures: Default::default(),
            lost,
        }));
        Ok((gpu, surface))
    }

    /// Whether the device is gone — a driver update or a GPU reset took
    /// it — so every renderer on it is to be opened again on a new one
    /// ([`Renderer::render`] says so with [`RenderError::DeviceLost`]).
    pub fn lost(&self) -> bool {
        self.0.lost.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Loses the device on purpose, as a driver update or a GPU reset
    /// would — for a shell to see its reopening happen without one. On
    /// D3D12 the device is really removed (`ID3D12Device5::RemoveDevice`),
    /// so every resource on it dies as it does then and the lost callback
    /// runs as it does then; elsewhere the device is only treated as lost.
    pub fn mark_lost(&self) {
        #[cfg(windows)]
        {
            use windows::Win32::Graphics::Direct3D12::ID3D12Device5;
            use windows::core::Interface;
            // SAFETY: the hal device is only read for its raw handle, and
            // `RemoveDevice` is what D3D12 offers for exactly this.
            let removed = unsafe {
                self.0
                    .device
                    .as_hal::<wgpu::hal::api::Dx12>()
                    .and_then(|d| d.raw_device().cast::<ID3D12Device5>().ok())
                    .map(|d| d.RemoveDevice())
            };
            if removed.is_some() {
                // The loss lands on the device's next use, through the
                // lost callback, as a real one does.
                return;
            }
        }
        self.0
            .lost
            .store(true, std::sync::atomic::Ordering::Release);
    }

    /// A surface for another window on the same instance — what
    /// [`Renderer::new_in`] draws into.
    pub fn create_surface(
        &self,
        target: impl Into<wgpu::SurfaceTarget<'static>>,
    ) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError> {
        self.0.instance.create_surface(target)
    }

    pub fn instance(&self) -> &wgpu::Instance {
        &self.0.instance
    }

    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.0.adapter
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.0.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.0.queue
    }

    /// Whether this device blends per channel, i.e. LCD subpixel glyphs
    /// draw with per-channel coverage rather than their union.
    pub fn dual_source(&self) -> bool {
        self.0.dual_source
    }

    /// The texture for one texture-backed image, uploaded on first sight
    /// and whenever `rev` has moved past what the texture holds; a size
    /// change makes a new texture. `None` for a degenerate size, which
    /// draws nothing.
    fn image_texture(
        &self,
        id: u64,
        px: &kui_core::display::TexturePixels,
    ) -> Option<std::sync::Arc<ImageTexture>> {
        // Degenerate, or past what this device can hold in one texture
        // (8192 on many adapters, 16384 on Metal): draws nothing, which is
        // what the core says a texture-backed image that cannot be backed
        // does, rather than a validation error the device turns into a
        // panic.
        let max = self.0.device.limits().max_texture_dimension_2d;
        if px.width == 0 || px.height == 0 || px.width > max || px.height > max {
            return None;
        }
        let mut cache = self.0.textures.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = match cache.get(&id) {
            Some(t) if t.width == px.width && t.height == px.height => {
                let mut rev = t.rev.lock().unwrap_or_else(|e| e.into_inner());
                if *rev != px.rev {
                    upload_image(&self.0.queue, &t.texture, px);
                    *rev = px.rev;
                }
                return Some(t.clone());
            }
            _ => {
                let texture = self.0.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("kui.image"),
                    size: wgpu::Extent3d {
                        width: px.width,
                        height: px.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                upload_image(&self.0.queue, &texture, px);
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                std::sync::Arc::new(ImageTexture {
                    texture,
                    view,
                    width: px.width,
                    height: px.height,
                    rev: std::sync::Mutex::new(px.rev),
                })
            }
        };
        cache.insert(id, fresh.clone());
        Some(fresh)
    }

    /// Forgets a removed fragment's pipelines, one per surface format it
    /// was ever drawn in; the GPU frees them once no frame in flight
    /// holds one.
    fn drop_fragment_pipelines(&self, id: u64) {
        self.0
            .fragment_pipelines
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|(fid, _), _| *fid != id);
    }

    /// Forgets a removed image's texture; the GPU frees it once no bind
    /// group holds it.
    fn drop_image_texture(&self, id: u64) {
        self.0
            .textures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
    }

    /// Whether the cache still holds exactly this texture for `id` — what
    /// a renderer asks before keeping a bind group over it, since a
    /// removal reaches the cache through whichever window's frame carried
    /// it and the other windows' bind groups would otherwise hold the
    /// texture for as long as they live.
    fn holds_image_texture(&self, id: u64, texture: &std::sync::Arc<ImageTexture>) -> bool {
        self.0
            .textures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
            .is_some_and(|t| std::sync::Arc::ptr_eq(t, texture))
    }

    /// The pipeline for one registered fragment, built on first sight and
    /// then shared by every window on this device. `source` is the app's
    /// WGSL, which the core already validated; it is wrapped in the same
    /// prelude and epilogue here, from `kui_core::fragment::module_source`,
    /// so what compiles is what was validated.
    ///
    /// The source is not validated again here: `Core::add_fragment` parsed
    /// and validated this exact module text with the same naga this wgpu
    /// carries, and refused a handle for anything that failed. A module
    /// that still does not compile is a kui bug, and reaches wgpu's own
    /// error handler like any other.
    fn fragment_pipeline(
        &self,
        id: u64,
        source: &str,
        format: wgpu::TextureFormat,
        layouts: &FragmentLayouts,
    ) -> wgpu::RenderPipeline {
        let mut cache = self
            .0
            .fragment_pipelines
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(p) = cache.get(&(id, format)) {
            return p.clone();
        }
        let device = &self.0.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kui.fragment"),
            source: wgpu::ShaderSource::Wgsl(kui_core::fragment::module_source(source).into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kui.fragment"),
            layout: Some(&layouts.pipeline),
            vertex: wgpu::VertexState {
                module: &layouts.vertex,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(instance_buffer_layout(&INSTANCE_ATTRS))],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(kui_core::fragment::ENTRY_POINT),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // A fragment returns premultiplied colour, always over.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        cache.insert((id, format), pipeline.clone());
        pipeline
    }
}

/// What building a fragment pipeline needs besides its own source: kui's
/// vertex stage, and the layout that puts the globals at group 0 and the
/// parameters at group 1.
struct FragmentLayouts {
    vertex: wgpu::ShaderModule,
    pipeline: wgpu::PipelineLayout,
}

/// The instance attributes both pipelines read; one array so the vertex
/// layout cannot differ between them.
const INSTANCE_ATTRS: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
    0 => Float32x2, 1 => Float32x2, 2 => Float32x4,
    3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
    6 => Float32x4, 7 => Float32x4, 8 => Float32x4,
];

fn instance_buffer_layout(attrs: &[wgpu::VertexAttribute]) -> wgpu::VertexBufferLayout<'_> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Instance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: attrs,
    }
}

impl std::fmt::Debug for Gpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gpu")
            .field("adapter", &self.0.adapter.get_info().name)
            .field("dual_source", &self.0.dual_source)
            .finish()
    }
}

pub struct Renderer {
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    globals_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bind_layout: wgpu::BindGroupLayout,
    /// The two samplers every group-0 bind group carries: linear at
    /// binding 2, nearest at 3 (ADR 0025, decision 4).
    samplers: Samplers,
    /// Per texture-backed image this window has drawn: the device's
    /// texture, and a bind group of this window's own — group 0 with that
    /// texture in the atlas's place and a globals copy whose `atlas_size`
    /// is the texture's, rewritten each frame the image is drawn.
    texture_binds: std::collections::HashMap<u64, TextureBind>,
    atlas_tex: wgpu::Texture,
    atlas_size: u32,
    atlas_epoch: u64,
    instance_buf: wgpu::Buffer,
    instance_cap: usize,
    instances: Vec<Instance>,
    /// What a fragment pipeline is built against: kui's vertex stage and
    /// the two-group layout. Built once per renderer, handed to the
    /// device's shared cache.
    fragment_layouts: FragmentLayouts,
    /// One slot per fragment this frame, each padded to the device's
    /// dynamic-offset alignment.
    fragment_params_buf: wgpu::Buffer,
    fragment_params_cap: usize,
    fragment_bind: wgpu::BindGroup,
    fragment_bind_layout: wgpu::BindGroupLayout,
    /// The alignment slots are padded to; `dynamic_offset` steps by it.
    uniform_align: u32,
    /// Scratch for one frame's padded parameter slots.
    fragment_bytes: Vec<u8>,
    pub clear_color: wgpu::Color,
}

struct Samplers {
    linear: wgpu::Sampler,
    nearest: wgpu::Sampler,
}

struct TextureBind {
    texture: std::sync::Arc<ImageTexture>,
    globals: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

fn upload_image(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    px: &kui_core::display::TexturePixels,
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &px.rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(px.width * 4),
            rows_per_image: Some(px.height),
        },
        wgpu::Extent3d {
            width: px.width,
            height: px.height,
            depth_or_array_layers: 1,
        },
    );
}

/// Picks the `//DUAL:` or `//SINGLE:` lines of the shader template.
fn preprocess_shader(src: &str, dual: bool) -> String {
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

/// How many frames may be queued ahead of the one on screen, by default:
/// two, so a drawable to render into is waiting when the previous frame's
/// is still out (backlog C47). On Metal wgpu makes this the layer's
/// `maximumDrawableCount` less one, so two is triple buffering — what gpui
/// runs with. With one, a frame whose thread woke a little late at light
/// load found no free drawable and missed its vsync: 1–6% of them on an
/// M3 Pro under macOS 27 at 100 and 2,500 boxes, none under heavy load,
/// where there is no idle gap to wake late from. Two delivered 1198–1201
/// of ~1200 vsyncs in every run. Queued behind a frame, though, a frame
/// built as soon as a drawable frees reaches the screen a vsync later
/// while frames run back to back — 27.6 ms sampling-to-photon against
/// 19.3 — so `kui`'s runner starts such frames at the display's vsync
/// instead (its `pacer`, macOS 14+), where the extra drawable is slack
/// and not a queue: 17.5–19.2 ms, every vsync delivered. A renderer
/// driven any other way pays the frame.
pub const DEFAULT_FRAME_LATENCY: u32 = 2;

impl Renderer {
    /// A renderer for one window, on a device of its own.
    pub async fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (gpu, surface) = Gpu::new(target).await?;
        Self::with_surface(gpu, surface, width, height)
    }

    /// A renderer for another window on an existing device — the one every
    /// window of a session shares. Get it from [`Renderer::gpu`].
    pub fn new_in(
        gpu: &Gpu,
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let surface = gpu.create_surface(target)?;
        Self::with_surface(gpu.clone(), surface, width, height)
    }

    /// The device this renderer draws with, to open another window on.
    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// How many frames may be queued ahead of the one on screen (at least
    /// one); see [`DEFAULT_FRAME_LATENCY`]. Reconfigures the surface when
    /// it changes.
    pub fn set_frame_latency(&mut self, frames: u32) {
        let frames = frames.max(1);
        if self.config.desired_maximum_frame_latency != frames {
            self.config.desired_maximum_frame_latency = frames;
            self.surface.configure(self.gpu.device(), &self.config);
        }
    }

    /// The frame latency the surface is configured with.
    pub fn frame_latency(&self) -> u32 {
        self.config.desired_maximum_frame_latency
    }

    fn with_surface(
        gpu: Gpu,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let device = gpu.device();
        let dual_source = gpu.dual_source();

        let caps = surface.get_capabilities(gpu.adapter());
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.clamp(1, device.limits().max_texture_dimension_2d),
            height: height.clamp(1, device.limits().max_texture_dimension_2d),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: vec![],
            // See `DEFAULT_FRAME_LATENCY`; a runner that wants another
            // says so through `set_frame_latency`.
            desired_maximum_frame_latency: DEFAULT_FRAME_LATENCY,
        };
        // A configure that fails only reports to the device's error
        // handler, and the first acquire on the unconfigured surface is a
        // panic inside wgpu; caught here, it is this constructor's error
        // — a window DXGI will not give a second swapchain, say.
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        surface.configure(device, &config);
        if let Some(err) = pollster::block_on(scope.pop()) {
            return Err(format!("configuring the surface: {err}").into());
        }

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kui"),
            source: wgpu::ShaderSource::Wgsl(
                preprocess_shader(include_str!("shader.wgsl"), dual_source).into(),
            ),
        });
        // Dual source: the shader outputs premultiplied color and a
        // per-channel coverage; out = src + dst * (1 - coverage). For
        // ordinary quads every channel's coverage equals alpha, which is
        // exactly premultiplied alpha blending.
        let blend = if dual_source {
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc1,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc1Alpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }
        } else {
            wgpu::BlendState::ALPHA_BLENDING
        };

        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kui.globals"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kui"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });

        let instance_attrs = INSTANCE_ATTRS;
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kui.quads"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(instance_buffer_layout(&instance_attrs))],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kui.globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let atlas_size = kui_core::atlas::ATLAS_SIZE;
        let atlas_tex = create_atlas_texture(device, atlas_size);
        let samplers = Samplers {
            linear: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("kui.linear"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            nearest: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("kui.nearest"),
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            }),
        };
        let atlas_view = atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group =
            create_bind_group(device, &bind_layout, &globals_buf, &atlas_view, &samplers);

        let instance_cap = 4096;
        let instance_buf = create_instance_buffer(device, instance_cap);

        // Fragments: one uniform slot per draw, picked by dynamic offset,
        // and the layout their pipelines are built against. All of it is
        // built whether or not a frame ever draws one — a bind group
        // layout and an empty buffer, not a pipeline, which is the part
        // that costs and is built on first sight.
        let fragment_bind_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("kui.fragment.params"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                            FragmentParams,
                        >()
                            as u64),
                    },
                    count: None,
                }],
            });
        let fragment_layouts = FragmentLayouts {
            vertex: shader,
            pipeline: device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("kui.fragment"),
                bind_group_layouts: &[Some(&bind_layout), Some(&fragment_bind_layout)],
                immediate_size: 0,
            }),
        };
        // The *device's* limit, not the adapter's: the device is opened with
        // `Limits::default()`, whose `min_uniform_buffer_offset_alignment` is
        // 256, and validation holds a dynamic offset to what the device asked
        // for rather than to what the hardware could have done. An adapter
        // reporting the smaller 64 — which DX12 does — then gave 64-byte slots
        // and a validation error on the frame's second fragment.
        let uniform_align = device.limits().min_uniform_buffer_offset_alignment;
        let fragment_params_cap = 16;
        let fragment_params_buf =
            create_fragment_params_buffer(device, fragment_params_cap, uniform_align);
        let fragment_bind =
            create_fragment_bind_group(device, &fragment_bind_layout, &fragment_params_buf);

        Ok(Self {
            gpu,
            surface,
            config,
            pipeline,
            globals_buf,
            bind_group,
            bind_layout,
            samplers,
            texture_binds: Default::default(),
            atlas_tex,
            atlas_size,
            atlas_epoch: u64::MAX,
            instance_buf,
            instance_cap,
            instances: Vec::new(),
            fragment_layouts,
            fragment_params_buf,
            fragment_params_cap,
            fragment_bind,
            fragment_bind_layout,
            uniform_align,
            fragment_bytes: Vec::new(),
            clear_color: wgpu::Color {
                r: 0.06,
                g: 0.065,
                b: 0.08,
                a: 1.0,
            },
        })
    }

    /// Whether this device blends per channel, i.e. LCD subpixel glyphs
    /// (`QuadKind::GlyphSubpixel`) render as intended. Drivers feed this to
    /// `Core::set_subpixel_text`; without it the core should keep
    /// rasterizing alpha masks.
    pub fn subpixel_text(&self) -> bool {
        self.gpu.dual_source()
    }

    /// Reconfigures the swapchain for a new window size.
    ///
    /// Both bounds are the platform's, not ours. `max(1)` because a
    /// minimized window reports zero and a zero-sized surface is a
    /// validation error; `min(max_texture_dimension_2d)` because Windows
    /// hands out a nonsense size mid-resize — a 2600x1500 move on Windows
    /// 11 arrived as 2578x32711 — and configuring a surface larger than
    /// the device can hold panics inside wgpu, taking the app with it. A
    /// clamped frame is one wrong picture; the next real size fixes it.
    pub fn resize(&mut self, width: u32, height: u32) {
        let max = self.gpu.device().limits().max_texture_dimension_2d;
        self.config.width = width.clamp(1, max);
        self.config.height = height.clamp(1, max);
        self.surface.configure(self.gpu.device(), &self.config);
    }

    fn sync_atlas(&mut self, atlas: &mut GlyphAtlas) {
        if atlas.size != self.atlas_size {
            self.atlas_size = atlas.size;
            self.atlas_tex = create_atlas_texture(self.gpu.device(), atlas.size);
            let view = self
                .atlas_tex
                .create_view(&wgpu::TextureViewDescriptor::default());
            self.bind_group = create_bind_group(
                self.gpu.device(),
                &self.bind_layout,
                &self.globals_buf,
                &view,
                &self.samplers,
            );
            self.atlas_epoch = u64::MAX;
        }
        if atlas.dirty || self.atlas_epoch != atlas.epoch {
            self.gpu.queue().write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &atlas.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(atlas.size * 4),
                    rows_per_image: Some(atlas.size),
                },
                wgpu::Extent3d {
                    width: atlas.size,
                    height: atlas.size,
                    depth_or_array_layers: 1,
                },
            );
            atlas.dirty = false;
            self.atlas_epoch = atlas.epoch;
        }
    }

    pub fn render(
        &mut self,
        dl: &DisplayList,
        atlas: &mut GlyphAtlas,
    ) -> Result<RenderReport, RenderError> {
        // A dead device takes no work: everything below would only add
        // errors to the one that lost it.
        if self.gpu.lost() {
            return Err(RenderError::DeviceLost);
        }
        self.sync_atlas(atlas);

        self.instances.clear();
        self.instances.extend(
            dl.quads
                .iter()
                .map(|q| instance_of(q, &dl.clips, &dl.textures)),
        );
        if self.instances.len() > self.instance_cap {
            self.instance_cap = self.instances.len().next_power_of_two();
            self.instance_buf = create_instance_buffer(self.gpu.device(), self.instance_cap);
        }
        if !self.instances.is_empty() {
            self.gpu.queue().write_buffer(
                &self.instance_buf,
                0,
                bytemuck::cast_slice(&self.instances),
            );
        }
        let globals = Globals {
            viewport: [dl.viewport.w.max(1.0), dl.viewport.h.max(1.0)],
            atlas_size: [self.atlas_size as f32, self.atlas_size as f32],
            time: dl.time,
            scale: dl.scale,
            _pad: [0.0; 2],
        };
        self.gpu
            .queue()
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        // Texture-backed images (ADR 0025, decision 3): drop what the core
        // removed, upload what moved, and give each one drawn this frame
        // a group-0 bind group of its own with a globals copy whose
        // `atlas_size` is the texture's. All skipped on a frame that
        // draws none.
        for id in &dl.dropped_textures {
            self.texture_binds.remove(&id.to_ffi());
            self.gpu.drop_image_texture(id.to_ffi());
        }
        // And the pipelines of removed fragments (AR8) — built per handle
        // and shared by every window, so one window's list carries the
        // removal and this is the only eviction they get.
        for id in &dl.dropped_fragments {
            self.gpu.drop_fragment_pipelines(id.to_ffi());
        }
        // A drop another window's frame carried: the cache no longer
        // holds the texture this bind group does. One lock per frame,
        // and only for a window that has ever drawn a texture.
        if !self.texture_binds.is_empty() {
            let gpu = &self.gpu;
            self.texture_binds
                .retain(|id, b| gpu.holds_image_texture(*id, &b.texture));
        }
        let mut texture_binds: Vec<Option<u64>> = Vec::new();
        if !dl.textures.is_empty() {
            texture_binds.reserve(dl.textures.len());
            for (draw, px) in dl.textures.iter().zip(&dl.texture_pixels) {
                let id = draw.id.to_ffi();
                let Some(texture) = self.gpu.image_texture(id, px) else {
                    texture_binds.push(None);
                    continue;
                };
                let stale = self
                    .texture_binds
                    .get(&id)
                    .is_none_or(|b| !std::sync::Arc::ptr_eq(&b.texture, &texture));
                if stale {
                    let device = self.gpu.device();
                    let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("kui.image.globals"),
                        size: std::mem::size_of::<Globals>() as u64,
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    let bind = create_bind_group(
                        device,
                        &self.bind_layout,
                        &globals_buf,
                        &texture.view,
                        &self.samplers,
                    );
                    self.texture_binds.insert(
                        id,
                        TextureBind {
                            texture: texture.clone(),
                            globals: globals_buf,
                            bind,
                        },
                    );
                }
                let b = &self.texture_binds[&id];
                let mine = Globals {
                    atlas_size: [texture.width as f32, texture.height as f32],
                    ..globals
                };
                self.gpu
                    .queue()
                    .write_buffer(&b.globals, 0, bytemuck::bytes_of(&mine));
                texture_binds.push(Some(id));
            }
        }

        // Each fragment's parameters into its own slot, and its pipeline
        // built if this device has not seen the handle before. Both are
        // skipped whole on a frame that draws no fragment.
        let mut fragment_pipelines: Vec<wgpu::RenderPipeline> = Vec::new();
        if !dl.fragments.is_empty() {
            let align = self.uniform_align as usize;
            if dl.fragments.len() > self.fragment_params_cap {
                self.fragment_params_cap = dl.fragments.len().next_power_of_two();
                self.fragment_params_buf = create_fragment_params_buffer(
                    self.gpu.device(),
                    self.fragment_params_cap,
                    self.uniform_align,
                );
                self.fragment_bind = create_fragment_bind_group(
                    self.gpu.device(),
                    &self.fragment_bind_layout,
                    &self.fragment_params_buf,
                );
            }
            self.fragment_bytes.clear();
            self.fragment_bytes.resize(dl.fragments.len() * align, 0);
            for (i, draw) in dl.fragments.iter().enumerate() {
                let uv = draw.image.uv();
                let slot = FragmentParams {
                    params: draw.params,
                    image: [uv[0] as f32, uv[1] as f32, uv[2] as f32, uv[3] as f32],
                };
                let at = i * align;
                self.fragment_bytes[at..at + std::mem::size_of::<FragmentParams>()]
                    .copy_from_slice(bytemuck::bytes_of(&slot));
            }
            self.gpu
                .queue()
                .write_buffer(&self.fragment_params_buf, 0, &self.fragment_bytes);
            fragment_pipelines.reserve(dl.fragments.len());
            for (draw, source) in dl.fragments.iter().zip(&dl.fragment_sources) {
                fragment_pipelines.push(self.gpu.fragment_pipeline(
                    draw.id.to_ffi(),
                    source,
                    self.config.format,
                    &self.fragment_layouts,
                ));
            }
        }

        // Acquiring the swapchain image is where vsync backpressure blocks;
        // report it separately so latency graphs show pacing vs work.
        let t_wait = std::time::Instant::now();
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Err(RenderError::Skip);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                return Err(RenderError::Reconfigure);
            }
            // The acquire's error went to the device's error handler; if
            // it was the device itself, the lost callback has run by now.
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(if self.gpu.lost() {
                    RenderError::DeviceLost
                } else {
                    RenderError::Validation
                });
            }
        };
        let vsync_wait_ms = t_wait.elapsed().as_secs_f32() * 1e3;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("kui") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if !self.instances.is_empty() {
                pass.set_vertex_buffer(0, self.instance_buf.slice(..));
                if fragment_pipelines.is_empty() && texture_binds.is_empty() {
                    // The whole frame in one instanced draw, as it has
                    // always been. Nothing below runs.
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &self.bind_group, &[]);
                    pass.draw(0..6, 0..self.instances.len() as u32);
                } else {
                    // A fragment or a texture-backed image interrupts the
                    // run: draw what came before with the über-pipeline,
                    // then that one quad with its own pipeline (a
                    // fragment) or its own group 0 (a texture), then
                    // carry on (`docs/adr/0015-…` decision 5, ADR 0025
                    // decision 3). Consecutive quads of the same handle
                    // still take one set each, which is the 0.6 us the
                    // first ADR measured; runs of ordinary quads are
                    // unbroken.
                    let mut run_start = 0u32;
                    let mut on_quads = false;
                    for (i, q) in dl.quads.iter().enumerate() {
                        if q.kind != QuadKind::Fragment && q.kind != QuadKind::Texture {
                            continue;
                        }
                        let i = i as u32;
                        if i > run_start {
                            if !on_quads {
                                pass.set_pipeline(&self.pipeline);
                                pass.set_bind_group(0, &self.bind_group, &[]);
                                on_quads = true;
                            }
                            pass.draw(0..6, run_start..i);
                        }
                        // `uv[0]` is the index into the side list, which
                        // is also this fragment's parameter slot, or this
                        // texture's bind.
                        let slot = q.uv[0] as usize;
                        if q.kind == QuadKind::Texture {
                            if let Some(Some(id)) = texture_binds.get(slot)
                                && let Some(b) = self.texture_binds.get(id)
                            {
                                pass.set_pipeline(&self.pipeline);
                                pass.set_bind_group(0, &b.bind, &[]);
                                on_quads = false;
                                pass.draw(0..6, i..i + 1);
                            }
                        } else if let Some(pipeline) = fragment_pipelines.get(slot) {
                            // A fragment reading a texture-backed image
                            // takes that image's group 0 — the texture in
                            // the atlas's place, `atlas_size` its size —
                            // exactly as a texture quad does; one reading
                            // the atlas, or nothing, takes the frame's.
                            // A texture the device could not make (a
                            // degenerate or oversized image) draws the
                            // fragment against the atlas with a zero rect,
                            // which `kui_sample` reads as no image.
                            let group0 = match draw_image_texture(&dl.fragments[slot]) {
                                Some(index) => texture_binds
                                    .get(index)
                                    .copied()
                                    .flatten()
                                    .and_then(|id| self.texture_binds.get(&id))
                                    .map_or(&self.bind_group, |b| &b.bind),
                                None => &self.bind_group,
                            };
                            pass.set_pipeline(pipeline);
                            pass.set_bind_group(0, group0, &[]);
                            pass.set_bind_group(
                                1,
                                &self.fragment_bind,
                                &[slot as u32 * self.uniform_align],
                            );
                            on_quads = false;
                            pass.draw(0..6, i..i + 1);
                        }
                        run_start = i + 1;
                    }
                    let end = self.instances.len() as u32;
                    if end > run_start {
                        if !on_quads {
                            pass.set_pipeline(&self.pipeline);
                            pass.set_bind_group(0, &self.bind_group, &[]);
                        }
                        pass.draw(0..6, run_start..end);
                    }
                }
            }
        }
        self.gpu.queue().submit([encoder.finish()]);
        self.gpu.queue().present(frame);
        Ok(RenderReport { vsync_wait_ms })
    }
}

/// The `textures` entry a fragment draw reads its image from, if its
/// image has a texture of its own.
fn draw_image_texture(draw: &kui_core::FragmentDraw) -> Option<usize> {
    match draw.image {
        kui_core::FragmentImage::Texture { index, .. } => Some(index as usize),
        _ => None,
    }
}

/// Timing details from one `render` call.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderReport {
    /// Time blocked acquiring the swapchain image (vsync backpressure).
    pub vsync_wait_ms: f32,
}

/// A frame that produced no image, mapped from `CurrentSurfaceTexture`.
#[derive(Clone, Copy, Debug)]
pub enum RenderError {
    /// Surface outdated/lost: `resize` (reconfigure) and redraw.
    Reconfigure,
    /// Nothing to present right now (occluded/timeout): try next frame.
    Skip,
    /// Validation error acquiring the surface texture: the surface is
    /// configured wrong for the window — `resize` to its size and redraw.
    Validation,
    /// The device is gone ([`Gpu::lost`]): open a new one, and a renderer
    /// on it for every window.
    DeviceLost,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reconfigure => write!(f, "surface outdated or lost; reconfigure"),
            Self::Skip => write!(f, "no frame available; skip"),
            Self::Validation => write!(f, "surface texture validation error"),
            Self::DeviceLost => write!(f, "device lost; reopen"),
        }
    }
}

impl std::error::Error for RenderError {}

fn create_atlas_texture(device: &wgpu::Device, size: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kui.atlas"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// Group 0: the globals, a texture — the atlas, or a texture-backed image
/// in its place — and the two samplers.
fn create_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    view: &wgpu::TextureView,
    samplers: &Samplers,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("kui"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&samplers.linear),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&samplers.nearest),
            },
        ],
    })
}

fn create_instance_buffer(device: &wgpu::Device, cap: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("kui.instances"),
        size: (cap * std::mem::size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_fragment_params_buffer(device: &wgpu::Device, cap: usize, align: u32) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("kui.fragment.params"),
        size: (cap.max(1) * align as usize) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_fragment_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buf: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("kui.fragment.params"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: buf,
                offset: 0,
                size: std::num::NonZeroU64::new(std::mem::size_of::<FragmentParams>() as u64),
            }),
        }],
    })
}

/// Says where a crash is. A fault in a driver — a GPU whose driver is
/// being replaced under the app — ends the process with no line from
/// anyone: not a panic, so nothing of ours prints, and Windows reports
/// only `0xC000041D` for an exception in a window callback. This prints
/// the code and the module the faulting address is in, then lets the
/// crash go on as it would have; a diagnostic, not a recovery.
///
/// Only a crash: the line is said from the process's unhandled-exception
/// filter, which runs once every frame handler has declined the
/// exception — not from a vectored handler, which sees each exception
/// first, before anyone has had the chance to handle it, and so sees the
/// faults that are part of normal running: a driver probing memory under
/// its own `__try`, V8's WebAssembly bounds checks under `node.exe`. Said
/// from there, those spent the one report each kind had on something
/// harmless, and the crash that followed was never named. A filter set
/// before this one is called after it, with its answer returned, so a
/// crash reporter the host installed first still gets the crash; one set
/// after replaces this one, as it would any filter.
///
/// A vectored handler is still installed, last among them, but it says
/// nothing: it remembers the last fault each thread saw. An exception
/// that escapes a window callback reaches the filter as `0xC000041D`, not
/// as itself, and the fault inside it is found on the record's own chain
/// or, failing that, in what the thread last remembered.
#[cfg(windows)]
pub fn report_faults() {
    use std::cell::Cell;
    use windows::Win32::Foundation::{
        EXCEPTION_ACCESS_VIOLATION, EXCEPTION_ILLEGAL_INSTRUCTION, EXCEPTION_IN_PAGE_ERROR,
        EXCEPTION_STACK_OVERFLOW, HMODULE, NTSTATUS, STATUS_FATAL_USER_CALLBACK_EXCEPTION,
    };
    use windows::Win32::Storage::FileSystem::WriteFile;
    use windows::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE};
    use windows::Win32::System::Diagnostics::Debug::{
        AddVectoredExceptionHandler, EXCEPTION_POINTERS, EXCEPTION_RECORD,
        LPTOP_LEVEL_EXCEPTION_FILTER, SetUnhandledExceptionFilter,
    };
    use windows::Win32::System::LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        GetModuleFileNameW, GetModuleHandleExW,
    };

    const CONTINUE_SEARCH: i32 = 0;
    /// The faults a driver ends a process with: the ones remembered for
    /// a callback's `0xC000041D` to be read by.
    const FAULTS: [NTSTATUS; 4] = [
        EXCEPTION_ACCESS_VIOLATION,
        EXCEPTION_ILLEGAL_INSTRUCTION,
        EXCEPTION_IN_PAGE_ERROR,
        EXCEPTION_STACK_OVERFLOW,
    ];
    /// The filter this one replaced, called after it; set once, with the
    /// two handlers, by the one call that installs them.
    static PREVIOUS: std::sync::OnceLock<LPTOP_LEVEL_EXCEPTION_FILTER> = std::sync::OnceLock::new();
    thread_local! {
        /// The last fault this thread saw, code and address, handled or
        /// not. A `const` cell with no destructor: a plain thread-local
        /// slot, read and written without allocating or registering
        /// anything, from inside an exception.
        static LAST: Cell<Option<(i32, usize)>> = const { Cell::new(None) };
    }

    /// Remembers a fault; says nothing and handles nothing.
    unsafe extern "system" fn remember(info: *mut EXCEPTION_POINTERS) -> i32 {
        // SAFETY: the system hands a valid record for the exception.
        if let Some(record) =
            (unsafe { info.as_ref() }).and_then(|i| unsafe { i.ExceptionRecord.as_ref() })
            && FAULTS.contains(&record.ExceptionCode)
        {
            let seen = (record.ExceptionCode.0, record.ExceptionAddress as usize);
            let _ = LAST.try_with(|l| l.set(Some(seen)));
        }
        CONTINUE_SEARCH
    }

    /// The first fault on a record's chain of nested exceptions, past the
    /// record itself; a few links, since a chain is one or two long and a
    /// broken one is not worth following further.
    fn nested(record: &EXCEPTION_RECORD) -> Option<(i32, usize)> {
        let mut at = record.ExceptionRecord;
        for _ in 0..4 {
            // SAFETY: a nested record the system chained to this one.
            let inner = unsafe { at.as_ref() }?;
            if FAULTS.contains(&inner.ExceptionCode) {
                return Some((inner.ExceptionCode.0, inner.ExceptionAddress as usize));
            }
            at = inner.ExceptionRecord;
        }
        None
    }

    /// Writes one crash's line to stderr, straight to the handle: no
    /// `eprintln!`, which takes a lock and, on a console, converts
    /// through a stack buffer eight kilobytes deep — more than a stack
    /// overflow leaves.
    fn say(code: i32, at: usize, escaped: Option<i32>) {
        let mut module = HMODULE::default();
        let mut name = [0u16; 260];
        // SAFETY: `at` is only looked up, never read; the buffers are ours.
        let found = unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                windows::core::PCWSTR(at as *const u16),
                &mut module,
            )
        }
        .is_ok();
        let path = found.then(|| {
            // SAFETY: the module was just found; the buffer is ours.
            let n = unsafe { GetModuleFileNameW(Some(module), &mut name) } as usize;
            &name[..n.min(name.len())]
        });
        let line = FaultLine::new(code as u32, at, path, escaped.map(|c| c as u32));
        // SAFETY: a handle the process was given, written from our buffer.
        if let Ok(err) = unsafe { GetStdHandle(STD_ERROR_HANDLE) } {
            let mut written = 0u32;
            let _ = unsafe { WriteFile(err, Some(line.bytes()), Some(&mut written), None) };
        }
    }

    /// The crash: said, then handed to the filter before this one.
    unsafe extern "system" fn filter(info: *const EXCEPTION_POINTERS) -> i32 {
        // SAFETY: the system hands a valid record for the exception.
        if let Some(record) =
            (unsafe { info.as_ref() }).and_then(|i| unsafe { i.ExceptionRecord.as_ref() })
        {
            let (code, at) = (record.ExceptionCode, record.ExceptionAddress as usize);
            let inner = if code == STATUS_FATAL_USER_CALLBACK_EXCEPTION {
                nested(record).or_else(|| LAST.try_with(Cell::get).ok().flatten())
            } else {
                None
            };
            match inner {
                Some((fault, fault_at)) => say(fault, fault_at, Some(code.0)),
                None => say(code.0, at, None),
            }
        }
        match PREVIOUS.get().copied().flatten() {
            // SAFETY: the filter the system held before ours, called as
            // the system would have called it.
            Some(previous) => unsafe { previous(info) },
            None => CONTINUE_SEARCH,
        }
    }

    PREVIOUS.get_or_init(|| {
        // SAFETY: both handlers read only what the system gives them and
        // write only their own thread-local slot and stderr.
        unsafe {
            AddVectoredExceptionHandler(0, Some(remember));
            SetUnhandledExceptionFilter(Some(filter))
        }
    });
}

#[cfg(not(windows))]
pub fn report_faults() {}

/// One crash's line for `report_faults`, written into a buffer on the
/// stack: it is said with whatever stack the crash left (a stack
/// overflow leaves the few pages the thread reserved for its handlers)
/// and in a process whose heap may be what faulted, so nothing here
/// allocates. A line too long for it is cut, at a character, and still
/// ends in a newline. Built on every platform so it is tested on every
/// platform; only Windows says one.
#[cfg_attr(not(windows), allow(dead_code))]
struct FaultLine {
    buf: [u8; 640],
    len: usize,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl FaultLine {
    /// `kui: fault <code> at <address> in <module>`, and for a fault that
    /// escaped a window callback, the code it escaped as. `module` is the
    /// UTF-16 path Windows gives; none is a fault outside any module.
    fn new(code: u32, at: usize, module: Option<&[u16]>, escaped: Option<u32>) -> Self {
        use std::fmt::Write;
        let mut line = Self {
            buf: [0; 640],
            len: 0,
        };
        let _ = write!(line, "kui: fault {code:#010x} at {at:#x} in ");
        match module {
            Some(path) => {
                for c in char::decode_utf16(path.iter().copied()) {
                    let _ = line.write_char(c.unwrap_or(char::REPLACEMENT_CHARACTER));
                }
            }
            None => {
                let _ = line.write_str("no module (jit or freed code)");
            }
        }
        if let Some(escaped) = escaped {
            let _ = write!(line, ", escaped from a window callback as {escaped:#010x}");
        }
        // The newline has its byte kept for it (`write_str`).
        line.buf[line.len] = b'\n';
        line.len += 1;
        line
    }

    fn bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl std::fmt::Write for FaultLine {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        // One byte short of the buffer, for the newline.
        let room = self.buf.len() - 1 - self.len;
        let mut n = s.len().min(room);
        while !s.is_char_boundary(n) {
            n -= 1;
        }
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The globals are one buffer read by two pipelines whose modules
    /// declare it separately: this crate's `shader.wgsl` for quads, and
    /// `kui_core::fragment::PRELUDE` for every fragment. If the two
    /// declarations drift, a fragment reads the wrong bytes and there is
    /// nothing to catch it at runtime — the buffer is the right size and
    /// the numbers are just wrong. So: same field names, same order, and
    /// the size the Rust struct actually is.
    #[test]
    fn globals_layout_matches() {
        let fields = ["viewport", "atlas_size", "time", "scale", "_pad"];
        let of = |src: &str, name: &str| {
            let start = src
                .find(name)
                .unwrap_or_else(|| panic!("{name} is not declared in\n{src}"));
            let body = &src[start..];
            let end = body.find('}').expect("a closing brace");
            body[..end].to_string()
        };
        let quads = of(include_str!("shader.wgsl"), "struct Globals {");
        let frags = of(kui_core::fragment::PRELUDE, "struct KuiGlobals {");
        let read = |body: &str| -> Vec<String> {
            body.lines()
                .filter_map(|l| l.split_once(':'))
                .map(|(name, ty)| format!("{}: {}", name.trim(), ty.trim().trim_end_matches(',')))
                .collect()
        };
        let (a, b) = (read(&quads), read(&frags));
        assert_eq!(a, b, "shader.wgsl and the fragment prelude disagree");
        assert_eq!(
            a.len(),
            fields.len(),
            "a field was added to the globals without this test being told"
        );
        for (row, want) in a.iter().zip(fields) {
            assert!(row.starts_with(want), "expected {want}, got {row}");
        }
        // vec2 + vec2 + f32 + f32 + vec2 = 32 bytes, and a uniform's size
        // must be a multiple of sixteen, which is what `_pad` is for.
        assert_eq!(std::mem::size_of::<Globals>(), 32);
    }

    /// The fragment parameter slot is the other buffer two declarations
    /// read: `FragmentParams` here and `KuiFragmentParams` in the
    /// epilogue. Sixteen floats then the image's rect, 80 bytes.
    #[test]
    fn fragment_params_layout_matches() {
        assert_eq!(std::mem::size_of::<FragmentParams>(), 80);
        assert_eq!(std::mem::offset_of!(FragmentParams, image), 64);
        let epilogue = kui_core::fragment::EPILOGUE;
        assert!(
            epilogue
                .contains("struct KuiFragmentParams { p: array<vec4<f32>, 4>, image: vec4<f32> };"),
            "the epilogue's params struct moved without this test being told"
        );
    }

    /// The bindings the prelude declares at group 0 are this crate's, by
    /// number and kind, since a fragment pipeline binds the quad
    /// pipeline's group 0 layout as it is.
    #[test]
    fn prelude_bindings_match_group_zero() {
        let prelude = kui_core::fragment::PRELUDE;
        for line in [
            "@group(0) @binding(0) var<uniform> kui_globals: KuiGlobals;",
            "@group(0) @binding(1) var kui_atlas: texture_2d<f32>;",
            "@group(0) @binding(2) var kui_sampler: sampler;",
            "@group(0) @binding(3) var kui_sampler_nearest: sampler;",
        ] {
            assert!(prelude.contains(line), "prelude lacks `{line}`");
        }
        let quads = include_str!("shader.wgsl");
        for line in [
            "@group(0) @binding(1) var atlas_tex: texture_2d<f32>;",
            "@group(0) @binding(2) var atlas_smp: sampler;",
            "@group(0) @binding(3) var nearest_smp: sampler;",
        ] {
            assert!(quads.contains(line), "shader.wgsl lacks `{line}`");
        }
    }

    /// Both preprocessed variants of the shader must parse and validate
    /// (pipeline creation would otherwise fail at runtime, in a window).
    #[test]
    fn shader_variants_validate() {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        for dual in [false, true] {
            let src = preprocess_shader(include_str!("shader.wgsl"), dual);
            let module = wgpu::naga::front::wgsl::parse_str(&src)
                .unwrap_or_else(|e| panic!("dual={dual}: {}", e.emit_to_string(&src)));
            let caps = if dual {
                Capabilities::DUAL_SOURCE_BLENDING
            } else {
                Capabilities::empty()
            };
            Validator::new(ValidationFlags::all(), caps)
                .validate(&module)
                .unwrap_or_else(|e| panic!("dual={dual}: {e:?}"));
        }
    }

    /// The line `report_faults` says, built without the heap (RG31): the
    /// code, the address and the module's UTF-16 path decoded, and for a
    /// fault that escaped a window callback the code it escaped as.
    #[test]
    fn a_fault_line_names_the_code_the_address_and_the_module() {
        let path: Vec<u16> = r"C:\Windows\System32\nvoglv64.dll".encode_utf16().collect();
        let line = FaultLine::new(0xC000_0005, 0x7ff6_1234, Some(&path), None);
        assert_eq!(
            std::str::from_utf8(line.bytes()).unwrap(),
            "kui: fault 0xc0000005 at 0x7ff61234 in C:\\Windows\\System32\\nvoglv64.dll\n"
        );
        let line = FaultLine::new(0xC000_0005, 0x10, None, Some(0xC000_041D));
        assert_eq!(
            std::str::from_utf8(line.bytes()).unwrap(),
            "kui: fault 0xc0000005 at 0x10 in no module (jit or freed code), \
             escaped from a window callback as 0xc000041d\n"
        );
        // A path that is not UTF-16 is said, not refused.
        let line = FaultLine::new(0xC000_001D, 0x20, Some(&[0x44, 0xD800, 0x45]), None);
        assert_eq!(
            std::str::from_utf8(line.bytes()).unwrap(),
            "kui: fault 0xc000001d at 0x20 in D\u{FFFD}E\n"
        );
    }

    /// A line longer than its stack buffer is cut, between characters,
    /// and still ends in its newline.
    #[test]
    fn a_fault_line_too_long_is_cut_at_a_character() {
        let path: Vec<u16> = "é".repeat(1000).encode_utf16().collect();
        let line = FaultLine::new(0xC000_00FD, 0x30, Some(&path), Some(0xC000_041D));
        let text = std::str::from_utf8(line.bytes()).expect("cut at a character");
        assert!(text.ends_with("é\n"), "{text:?}");
        assert!(text.len() <= 640 && text.len() >= 638, "{}", text.len());
        assert!(text.starts_with("kui: fault 0xc00000fd at 0x30 in é"));
    }
}
