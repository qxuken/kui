//! wgpu renderer for kui: draws a [`kui_core::DisplayList`] with one instanced pipeline, a single draw call per frame.
//!
//! kui splits a UI into a model that lays out and paints into a display
//! list (`kui-core`), a renderer that puts that list on screen (this
//! crate) and a runner that owns the window and the event loop
//! (`kui-native`, the one most apps use). Reach for `kui-wgpu` directly
//! when you are writing your own runner: you already have a window, or an
//! event loop, that `kui-native` does not fit.
//!
//! A [`Renderer`] owns the swapchain of one window — or none, drawing
//! into textures a host hands it — and a GPU copy of the core's glyph
//! atlas, kept in step with the atlas it is handed each frame. Rounded rectangles, borders, shadows, glyphs and images are all
//! instances of the same quad, so an ordinary frame is one draw call;
//! only a texture-backed image or a custom fragment shader splits it.
//! Several windows share one device through [`Gpu`].
//!
//! # Example
//!
//! A runner's whole life with the renderer: open it on a window, tell the
//! core whether subpixel text will render, then build, draw and present
//! one frame at a time. `window` is anything wgpu can make a surface
//! from, such as a `winit` window.
//!
//! ```rust,no_run
//! use kui_core::{Core, Size, TextStyle};
//! use kui_wgpu::{RenderError, Renderer};
//!
//! fn run(
//!     window: impl Into<kui_wgpu::wgpu::SurfaceTarget<'static>>,
//! ) -> Result<(), Box<dyn std::error::Error>> {
//!     let (width, height) = (800u32, 600u32);
//!     let mut renderer = pollster::block_on(Renderer::new(window, width, height))?;
//!     let mut core = Core::new();
//!     core.set_subpixel_text(renderer.subpixel_text());
//!
//!     loop {
//!         // When the windowing library reports a new size:
//!         // renderer.resize(new_width, new_height);
//!
//!         // Build the frame through the core, in logical pixels.
//!         let scale = 1.0;
//!         let viewport = Size::new(width as f32 / scale, height as f32 / scale);
//!         let mut ui = core.frame(viewport, scale);
//!         ui.text("Hello from a custom runner", TextStyle::new(24.0));
//!         ui.finish();
//!
//!         // Draw it. The atlas is `&mut` so the renderer can clear its dirty flag.
//!         let (list, atlas) = core.output();
//!         match renderer.render(list, atlas) {
//!             Ok(report) => {
//!                 let _blocked_on_vsync_ms = report.vsync_wait_ms;
//!             }
//!             Err(RenderError::Reconfigure | RenderError::Validation) => {
//!                 renderer.resize(width, height);
//!             }
//!             Err(RenderError::Skip) => {}
//!             Err(RenderError::DeviceLost) => {
//!                 // Open a new `Renderer` (and a new device) and carry on.
//!                 break;
//!             }
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # Where to look
//!
//! - [`Renderer`]: one window's swapchain, pipelines and atlas texture.
//! - [`Renderer::render`]: a display list in, a presented frame (or a
//!   [`RenderError`]) out.
//! - [`Renderer::resize`]: reconfigure after the window changed size.
//! - [`Renderer::subpixel_text`]: what to pass to `Core::set_subpixel_text`.
//! - [`Gpu`]: the device, queue and adapter that windows share;
//!   [`Renderer::new_in`] opens a second window on it.
//! - [`Renderer::windowless`] on [`Gpu::windowless`] or a host's own
//!   [`Gpu`]: no window; [`Renderer::render_to`] draws a frame into a
//!   texture the host owns, for a host compositing kui with content of
//!   its own (backlog C55).
//! - [`Renderer::capture`]: a frame read back as straight RGBA
//!   ([`Capture`]), what a test or a screenshot wants; in two halves,
//!   [`Renderer::capture_begin`] and [`PendingCapture::wait`], for a
//!   recording that waits on another thread.
//! - [`RenderError`]: what each failed frame asks the runner to do next.
//! - [`DEFAULT_FRAME_LATENCY`] and [`Renderer::set_frame_latency`]: how
//!   many frames may queue ahead of the one on screen.
//! - [`wgpu`] is re-exported, so a runner builds against the same version
//!   this crate was.
//!
//! # Subpixel text
//!
//! Where the device offers dual-source blending (Metal, DX12, most Vulkan)
//! the pipeline blends per channel, which is what LCD subpixel glyphs need.
//! Elsewhere it falls back to ordinary alpha blending and the core should
//! rasterize grayscale masks instead, which is what
//! [`Renderer::subpixel_text`] tells it.
//!
//! The book: <https://kui-book.qxuken.dev>. Repository:
//! <https://github.com/qxuken/kui>.

pub use wgpu;

mod backdrop;
mod groups;

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
    /// The turn the quad is drawn through (ADR 0043): angle in radians,
    /// scale, tx, ty — the clip entry's `transform`. The identity on
    /// every quad of a frame with no `rotate` or `scale`.
    xform: [f32; 4],
    /// The clip in the quad's own space, before the turn: x, y, w, h.
    inner: [f32; 4],
    /// Its radii, as `clip_radii` are the outer clip's.
    inner_radii: [f32; 4],
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
    }
}

/// The GPU objects an app's windows share: one instance, adapter, device and queue.
///
/// Two devices cannot see each other's buffers or textures, so every
/// window of an app draws through the same `Gpu`. A single-window app
/// never names it: [`Renderer::new`] opens a private one. A second window
/// takes the first renderer's [`Renderer::gpu`] and opens through
/// [`Renderer::new_in`]. Cloning a `Gpu` clones a handle to the same
/// device.
#[derive(Clone)]
pub struct Gpu(std::sync::Arc<GpuInner>);

struct GpuInner {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// What it was opened for ([`Gpu::new_with`]).
    options: GpuOptions,
    /// Whether a surface on this device can be presented with alpha at
    /// all: everywhere but Windows, and there only on D3D12 presenting
    /// through a composition visual ([`see_through_by_visual`]).
    alpha: bool,
    dual_source: bool,
    /// Whether a frame's instances and globals are written into mapped
    /// buffers rather than copied in by `write_buffer` ([`FrameSlot`]).
    mapped_frames: bool,
    /// One pipeline per registered fragment per surface format, built the
    /// first time a frame draws it (about 0.2 ms, paid once) and shared by
    /// every window on this device, dropped when a frame's list says the
    /// handle is gone (`dropped_fragments`). A `Mutex` because `Gpu` is a
    /// shared handle and building is rare; nothing here is touched on a
    /// frame that draws no new fragment.
    fragment_pipelines: std::sync::Mutex<
        std::collections::HashMap<(u64, wgpu::TextureFormat), wgpu::RenderPipeline>,
    >,
    /// One texture per texture-backed image, uploaded the first time a
    /// frame on this device draws it and again when its revision moves,
    /// shared by every window like the pipelines above, dropped when the
    /// core says the handle is gone.
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

/// How a [`Gpu`] is opened: what its surfaces must be able to do, decided
/// before the first one exists because some of it is the instance's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct GpuOptions {
    /// Its surfaces may be presented with alpha ([`Renderer::new_in_with`],
    /// [`Renderer::transparent`]), so a window shows what is behind it
    /// where a frame paints nothing (backlog F126). On Windows this
    /// presents D3D12 through DirectComposition
    /// (`Dx12SwapchainKind::DxgiFromVisual`), the one D3D12 swapchain that
    /// takes alpha, instead of a swapchain on the window's handle — where
    /// [`see_through_by_visual`] says the window was made for it, and
    /// nowhere else, so there the surfaces stay opaque otherwise; nothing
    /// changes on other platforms. Off by default, so an opaque app keeps
    /// the swapchain it always had.
    pub transparent: bool,
}

impl GpuOptions {
    /// Options whose surfaces may be transparent ([`Self::transparent`]).
    pub fn transparent(transparent: bool) -> Self {
        Self { transparent }
    }
}

/// Whether a see-through window on Windows is presented by D3D12 through
/// a DirectComposition visual — the one way its translucent pixels show
/// what is behind it (backlog F126, RG150). Read from the process's
/// environment once, so the two things it decides cannot disagree: the
/// runner creates the window with no GDI surface of its own
/// (`WS_EX_NOREDIRECTIONBITMAP`) only when it is true, and
/// [`Gpu::new_with`] under [`GpuOptions::transparent`] presents through a
/// visual only when it is true. A window with a GDI surface under a
/// visual composites its translucent pixels over that surface's black;
/// a window with no surface and a swapchain on its handle draws nothing.
/// When it is false a transparent window is opaque, and
/// [`Renderer::transparent`] says so. Only Windows reads it.
///
/// See [`see_through_by_visual_with`] for what the two variables say.
pub fn see_through_by_visual() -> bool {
    static DECIDED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DECIDED.get_or_init(|| {
        // `var`, not `var_os`: wgpu reads both this way, so a value that
        // is not Unicode is unset here as it is there.
        see_through_by_visual_with(
            std::env::var("WGPU_BACKEND").ok().as_deref(),
            std::env::var("WGPU_DX12_PRESENTATION_SYSTEM")
                .ok()
                .as_deref(),
        )
    })
}

/// [`see_through_by_visual`] from the values of `WGPU_BACKEND` and
/// `WGPU_DX12_PRESENTATION_SYSTEM` (`None` for unset), each parsed the
/// way wgpu parses it:
///
/// - `WGPU_BACKEND` unset is D3D12 alone, which is what kui opens on
///   Windows; set, it must list D3D12 (`dx12` or `d3d12`, in wgpu's
///   comma-separated, case-insensitive list). A list naming others too
///   is narrowed to D3D12 for a transparent device ([`Gpu::new_with`]),
///   so the window this decided on is the one the device presents to.
/// - `WGPU_DX12_PRESENTATION_SYSTEM` naming the handle (`hwnd`,
///   `DxgiFromHwnd`) is a swapchain on the window's handle, which takes
///   no alpha; unset, naming the visual (`visual`, `DxgiFromVisual`), or
///   a value wgpu does not recognise leaves kui's choice, the visual.
pub fn see_through_by_visual_with(backend: Option<&str>, presentation: Option<&str>) -> bool {
    let d3d12 =
        backend.is_none_or(|b| wgpu::Backends::from_comma_list(b).contains(wgpu::Backends::DX12));
    // Untrimmed, as wgpu matches it (`Dx12SwapchainKind::from_env`).
    let hwnd =
        presentation.is_some_and(|p| matches!(p.to_lowercase().as_str(), "dxgifromhwnd" | "hwnd"));
    d3d12 && !hwnd
}

impl Gpu {
    /// Opens a device that can present to `target`, and returns the
    /// surface it was chosen for.
    ///
    /// The first window's surface has to exist before an adapter can be
    /// picked, so it comes back with the device; later windows get theirs
    /// from [`Gpu::create_surface`]. Most runners call [`Renderer::new`]
    /// instead, which does both and builds the renderer. On Windows only
    /// the D3D12 backend is enabled unless `WGPU_BACKEND` names another;
    /// for [`GpuOptions::transparent`], a list naming D3D12 among others
    /// is narrowed to it ([`see_through_by_visual_with`]).
    pub async fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
    ) -> Result<(Self, wgpu::Surface<'static>), Box<dyn std::error::Error>> {
        Self::new_with(target, GpuOptions::default()).await
    }

    /// [`Gpu::new`] with `options`: what every surface opened on this
    /// device must be able to do.
    pub async fn new_with(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        options: GpuOptions,
    ) -> Result<(Self, wgpu::Surface<'static>), Box<dyn std::error::Error>> {
        report_faults();
        let instance = Self::new_instance(options);
        let surface = instance.create_surface(target)?;
        let gpu = Self::open(instance, Some(&surface), options).await?;
        Ok((gpu, surface))
    }

    /// Opens a device with no window to present to, for
    /// [`Renderer::windowless`]: a host that draws kui into textures of its
    /// own, a test, or [`Renderer::capture`] in a process with no window
    /// (backlog C55).
    ///
    /// The adapter is wgpu's default with no surface to be compatible
    /// with, on the backends [`Gpu::new`] would use. Fails where there is
    /// no adapter at all — a CI container with no GPU and no software
    /// Vulkan, say.
    pub async fn windowless() -> Result<Self, Box<dyn std::error::Error>> {
        report_faults();
        let options = GpuOptions::default();
        Self::open(Self::new_instance(options), None, options).await
    }

    /// A `Gpu` over a device the host opened itself — a game engine's —
    /// so a [`Renderer::windowless`] on it draws into the host's textures
    /// ([`Renderer::render_to`]) and kui's frames share its queue.
    ///
    /// What kui would have asked of the device it reads from the device:
    /// per-channel blending where `DUAL_SOURCE_BLENDING` is enabled (pass
    /// [`Renderer::subpixel_text`] on to the core), mapped frame buffers
    /// on Metal where `MAPPABLE_PRIMARY_BUFFERS` is. kui sets no error
    /// handler and no lost callback on it, which are the host's, so
    /// [`Gpu::lost`] stays false: a host that hears its device go makes a
    /// new `Gpu` with the new one.
    pub fn from_parts(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Self {
        let features = device.features();
        let mapped_frames = adapter.get_info().backend == wgpu::Backend::Metal
            && features.contains(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS)
            && std::env::var("KUI_MAPPED_FRAMES").as_deref() != Ok("0");
        Self(std::sync::Arc::new(GpuInner {
            instance,
            adapter,
            device,
            queue,
            options: GpuOptions::default(),
            alpha: !cfg!(windows),
            dual_source: features.contains(wgpu::Features::DUAL_SOURCE_BLENDING),
            mapped_frames,
            fragment_pipelines: Default::default(),
            textures: Default::default(),
            lost: Default::default(),
        }))
    }

    /// Whether a device opened with `options` presents through a
    /// DirectComposition visual ([`see_through_by_visual`]).
    fn by_visual(options: GpuOptions) -> bool {
        cfg!(windows) && options.transparent && see_through_by_visual()
    }

    /// The instance a device opened with `options` lives on.
    fn new_instance(options: GpuOptions) -> wgpu::Instance {
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
        // A swapchain made on the window's handle is opaque whatever it is
        // configured with; one presented through a composition visual
        // takes premultiplied alpha. Only when asked, and only where the
        // runner made the window for it: `see_through_by_visual` decides
        // both, from `WGPU_BACKEND` and `WGPU_DX12_PRESENTATION_SYSTEM`.
        let by_visual = Self::by_visual(options);
        if cfg!(windows) {
            if std::env::var("WGPU_BACKEND").is_err() {
                desc.backends = wgpu::Backends::DX12;
            } else if by_visual {
                // A list that names D3D12 among others, narrowed to it:
                // the window was made with no surface of its own, and
                // only D3D12 presents to such a window. Left to choose,
                // wgpu takes Vulkan first.
                desc.backends &= wgpu::Backends::DX12;
            }
        }
        if by_visual {
            desc.backend_options.dx12.presentation_system = wgpu::Dx12SwapchainKind::DxgiFromVisual;
        }
        wgpu::Instance::new(desc)
    }

    /// The adapter and device on `instance`, able to present to `surface`
    /// when there is one.
    async fn open(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'static>>,
        options: GpuOptions,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let by_visual = Self::by_visual(options);
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: surface,
                ..Default::default()
            })
            .await?;
        // On Windows a surface takes alpha only through the visual, and
        // only D3D12 presents through one (the backends were narrowed to
        // it above; this is the check that they held).
        let alpha =
            !cfg!(windows) || (by_visual && adapter.get_info().backend == wgpu::Backend::Dx12);
        // Per-channel blending for LCD subpixel text, when the device has it.
        let dual_source = adapter
            .features()
            .contains(wgpu::Features::DUAL_SOURCE_BLENDING);
        // A frame's instances and globals written straight into memory the
        // GPU reads (backlog C53), on Metal: every Apple GPU shares memory
        // with the CPU, and `write_buffer`'s copy wakes the blit engine,
        // which holds ~70 MB for as long as frames keep coming.
        let mapped_frames = adapter.get_info().backend == wgpu::Backend::Metal
            && adapter
                .features()
                .contains(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS)
            && std::env::var("KUI_MAPPED_FRAMES").as_deref() != Ok("0");
        let mut required_features = wgpu::Features::empty();
        required_features.set(wgpu::Features::DUAL_SOURCE_BLENDING, dual_source);
        required_features.set(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS, mapped_frames);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features,
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
        Ok(Self(std::sync::Arc::new(GpuInner {
            instance,
            adapter,
            device,
            queue,
            options,
            alpha,
            dual_source,
            mapped_frames,
            fragment_pipelines: Default::default(),
            textures: Default::default(),
            lost,
        })))
    }

    /// Whether the device is gone (a driver update or a GPU reset took it).
    ///
    /// Nothing on a lost device works again: open a new `Gpu` and a new
    /// renderer on it for every window. [`Renderer::render`] reports the
    /// same condition as [`RenderError::DeviceLost`].
    pub fn lost(&self) -> bool {
        self.0.lost.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Loses the device on purpose, as a driver update or a GPU reset
    /// would, so a runner can test its reopening path without one.
    ///
    /// On D3D12 the device is really removed (`ID3D12Device5::RemoveDevice`)
    /// and the loss lands on its next use through the lost callback, as a
    /// real one does; elsewhere the device is only marked lost.
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

    /// A surface for another window on the same instance, which is what
    /// [`Renderer::new_in`] draws into.
    pub fn create_surface(
        &self,
        target: impl Into<wgpu::SurfaceTarget<'static>>,
    ) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError> {
        self.0.instance.create_surface(target)
    }

    /// What the device was opened for ([`Gpu::new_with`]).
    pub fn options(&self) -> GpuOptions {
        self.0.options
    }

    /// The wgpu instance the device was opened on.
    pub fn instance(&self) -> &wgpu::Instance {
        &self.0.instance
    }

    /// The adapter the device was requested from.
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.0.adapter
    }

    /// The device, for a runner that creates resources of its own on it.
    pub fn device(&self) -> &wgpu::Device {
        &self.0.device
    }

    /// The queue the renderer submits to.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.0.queue
    }

    /// Whether this device blends per channel (dual-source blending), so
    /// LCD subpixel glyphs draw with per-channel coverage rather than
    /// their union.
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
const INSTANCE_ATTRS: [wgpu::VertexAttribute; 12] = wgpu::vertex_attr_array![
    0 => Float32x2, 1 => Float32x2, 2 => Float32x4,
    3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
    6 => Float32x4, 7 => Float32x4, 8 => Float32x4,
    9 => Float32x4, 10 => Float32x4, 11 => Float32x4,
];

/// The quad pipeline over `module` — the shader with per-channel blending
/// or without — into `format`, blended by `blend`.
fn quad_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    blend: wgpu::BlendState,
) -> wgpu::RenderPipeline {
    let instance_attrs = INSTANCE_ATTRS;
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("kui.quads"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(instance_buffer_layout(&instance_attrs))],
        },
        fragment: Some(wgpu::FragmentState {
            module,
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
    })
}

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

/// One window's renderer: its surface, the pipelines and a GPU copy of the core's glyph atlas.
///
/// Open one per window with [`Renderer::new`] (first window, on a device
/// of its own) or [`Renderer::new_in`] (another window on a shared
/// [`Gpu`]). Each frame, hand [`Renderer::render`] the display list and
/// atlas from `Core::output`; call [`Renderer::resize`] when the window
/// changes size. The crate root has the whole sequence.
pub struct Renderer {
    gpu: Gpu,
    /// The window's surface; `None` for a [`Renderer::windowless`] one,
    /// which draws only into textures it is handed.
    surface: Option<wgpu::Surface<'static>>,
    /// The surface's configuration — or, windowless, the format the
    /// pipelines were built for and the size of the last target drawn.
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    /// What frames draw from: one slot written by `write_buffer`, or on a
    /// device with mapped frames a ring of them (backlog C53).
    slots: Vec<FrameSlot>,
    /// The slot this frame writes and draws.
    slot: usize,
    bind_layout: wgpu::BindGroupLayout,
    /// The two samplers every group-0 bind group carries: linear at
    /// binding 2, nearest at 3.
    samplers: Samplers,
    /// Per texture-backed image this window has drawn: the device's
    /// texture, and a bind group of this window's own — group 0 with that
    /// texture in the atlas's place and a globals copy whose `atlas_size`
    /// is the texture's, rewritten each frame the image is drawn.
    texture_binds: std::collections::HashMap<u64, TextureBind>,
    atlas_tex: wgpu::Texture,
    atlas_size: u32,
    atlas_epoch: u64,
    /// The atlas's `rev` the texture holds, for a renderer that does not
    /// own the atlas's `dirty` flag ([`Renderer::capture`]).
    atlas_rev: u64,
    /// Whose atlas that is ([`GlyphAtlas::id`]).
    atlas_id: u64,
    /// The texture the last capture drew into, kept for the next one of
    /// its size: a recording captures every frame, and the GPU orders the
    /// next frame's draw after the last one's copy out of it.
    capture_target: Option<wgpu::Texture>,
    instances: Vec<Instance>,
    /// What a fragment pipeline is built against: kui's vertex stage and
    /// the two-group layout. Built once per renderer, handed to the
    /// device's shared cache.
    fragment_layouts: FragmentLayouts,
    /// Group 1 of a fragment pipeline: its parameter slot in the frame
    /// slot's buffer, picked by dynamic offset.
    fragment_bind_layout: wgpu::BindGroupLayout,
    /// The alignment slots are padded to; `dynamic_offset` steps by it.
    uniform_align: u32,
    /// Scratch for one frame's padded parameter slots.
    fragment_bytes: Vec<u8>,
    /// The color a frame is cleared to before anything is drawn.
    ///
    /// A runner usually sets it to the theme's background each frame. It
    /// is written straight through: the renderer asks for a non-sRGB
    /// surface, so each component is the byte it lands as, the same way a
    /// quad's color is.
    pub clear_color: wgpu::Color,
    /// A picture drawn over the clear and under everything the frame
    /// draws ([`Renderer::set_ground`]), when there is one.
    ground: Option<Ground>,
    /// The backdrop blur's pipelines (backlog F129): made the first time a
    /// frame blurs, kept for the renderer's life.
    backdrop_pipes: Option<backdrop::Pipes>,
    /// Its offscreen frame, at the surface's size, and its scratch, at the
    /// largest blur's (backlog RG150): made when a frame blurs, dropped by
    /// the first frame that does not.
    backdrop: backdrop::Targets,
    /// The frame's blurs, in paint order.
    blurs: Vec<backdrop::Blur>,
    /// The frame's groups (ADR 0056), inner before outer, each placed in
    /// one of `group_targets`; empty on a frame that draws none.
    groups: Vec<groups::Plan>,
    /// The groups each group draws, and the frame's, by `groups` index:
    /// `group_kids[group_kids_at[p]..group_kids_at[p + 1]]` for plan `p`,
    /// the frame's last.
    group_kids: Vec<u32>,
    group_kids_at: Vec<u32>,
    group_targets: groups::Targets,
    /// The pipeline a group is drawn with on a device that blends per
    /// channel: the single-source one, since a picture over nothing has
    /// no colour behind a glyph to split coverage against. Made the first
    /// time a frame draws a group; elsewhere `pipeline` is that already.
    group_pipeline: Option<wgpu::RenderPipeline>,
}

/// The picture under a frame: its texture, the part of it the window
/// shows (`uv`), and the pipeline that draws it across the viewport.
struct Ground {
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    uv: wgpu::Buffer,
    /// Kept for the bind group, which holds a view of it.
    _texture: wgpu::Texture,
}

/// What one frame draws from, in one buffer: the globals at offset 0,
/// one parameter slot per fragment from [`FRAGMENTS_AT`], the instances
/// after those; group 0 naming the globals and the atlas, and group 1
/// naming the fragment slots.
///
/// On a device with mapped frames (backlog C53) a renderer keeps a ring of
/// these. A slot is written while it is mapped, unmapped for the submit
/// that draws it and asked to map again right after; the poll or submit
/// that finds the GPU done with it runs the callback and the slot is
/// ready again — the next frame's, on an M3 Pro drawing 10k quads at 120 Hz,
/// so a ring is mostly one slot. Nothing is copied, so a steady frame
/// wakes no blit. Elsewhere there is one slot, written by
/// `write_buffer`.
struct FrameSlot {
    buf: wgpu::Buffer,
    /// How many instances fit, and how many fragment slots.
    cap: usize,
    frag_cap: usize,
    /// Where the instances start: past the fragment slots.
    instances_at: u64,
    bind: wgpu::BindGroup,
    frag_bind: wgpu::BindGroup,
    /// Mapped and not in flight: set by the map callback, cleared at the
    /// unmap.
    ready: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Where a frame slot's fragment parameters start: past the globals, at
/// the uniform offset alignment wgpu holds every device to.
const FRAGMENTS_AT: u64 = 256;

/// How many frame slots a ring grows to before a frame waits for one: a
/// frame is in flight for at most the surface's latency (three), and a
/// slot comes back a poll after that.
const MAX_FRAME_SLOTS: usize = 6;

/// What making a frame slot takes from its renderer.
struct SlotKit<'a> {
    device: &'a wgpu::Device,
    mapped: bool,
    layout: &'a wgpu::BindGroupLayout,
    frag_layout: &'a wgpu::BindGroupLayout,
    atlas: &'a wgpu::Texture,
    samplers: &'a Samplers,
    /// The fragment slots' stride: the dynamic-offset alignment.
    align: u32,
}

impl FrameSlot {
    /// A slot for `cap` instances and `frag_cap` fragments: mapped and
    /// ready on a mapped device, copied into elsewhere.
    fn new(kit: &SlotKit<'_>, cap: usize, frag_cap: usize) -> Self {
        let frag_cap = frag_cap.max(1);
        let instances_at = FRAGMENTS_AT + (frag_cap * kit.align as usize) as u64;
        let buf = kit.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kui.frame"),
            size: instances_at + (cap * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::UNIFORM
                | if kit.mapped {
                    wgpu::BufferUsages::MAP_WRITE
                } else {
                    wgpu::BufferUsages::COPY_DST
                },
            mapped_at_creation: kit.mapped,
        });
        let atlas = kit.atlas.create_view(&Default::default());
        let bind = create_bind_group(
            kit.device,
            kit.layout,
            &globals_binding(&buf),
            &atlas,
            kit.samplers,
        );
        let frag_bind = create_fragment_bind_group(kit.device, kit.frag_layout, &buf);
        Self {
            buf,
            cap,
            frag_cap,
            instances_at,
            bind,
            frag_bind,
            ready: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(kit.mapped)),
        }
    }

    fn ready(&self) -> bool {
        self.ready.load(std::sync::atomic::Ordering::Acquire)
    }
}

/// A frame slot's globals, as group 0's binding 0 takes them.
fn globals_binding(buf: &wgpu::Buffer) -> wgpu::BufferBinding<'_> {
    wgpu::BufferBinding {
        buffer: buf,
        offset: 0,
        size: std::num::NonZeroU64::new(std::mem::size_of::<Globals>() as u64),
    }
}

/// The ground's shader: one quad over the viewport, sampling the part of
/// the picture `g.uv` names (`u0, v0, u1, v1`), opaque.
const GROUND_SHADER: &str = r"
struct G { uv: vec4<f32> }
@group(0) @binding(0) var<uniform> g: G;
@group(0) @binding(1) var t: texture_2d<f32>;
@group(0) @binding(2) var s: sampler;
struct V { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
    let x = f32(i & 1u);
    let y = f32((i >> 1u) & 1u);
    var o: V;
    o.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    o.uv = vec2<f32>(mix(g.uv.x, g.uv.z, x), mix(g.uv.y, g.uv.w, y));
    return o;
}
@fragment fn fs(v: V) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(t, s, v.uv).rgb, 1.0);
}
";

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

/// The alpha mode an opaque window presents with: `Opaque` wherever the
/// surface offers it — every surface a window gets does — and the
/// surface's first mode otherwise. The first mode was the choice before
/// a surface could be transparent, and is `Opaque` everywhere but a
/// D3D12 composition swapchain, which lists `Auto` first.
fn opaque_mode(modes: &[wgpu::CompositeAlphaMode]) -> wgpu::CompositeAlphaMode {
    if modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
        wgpu::CompositeAlphaMode::Opaque
    } else {
        modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
    }
}

/// The alpha mode a transparent window presents with, of the ones the
/// surface offers, or `None` when it offers none that composites the
/// frame kui draws — which is premultiplied. `PreMultiplied` first; on
/// Metal, `PostMultiplied`, which is only how wgpu names a layer that is
/// not opaque, and Core Animation composites a layer's pixels as
/// premultiplied whatever it is called; then `Inherit`, the window
/// system's own way, which under Wayland and an ARGB X11 visual is
/// premultiplied too. Never `PostMultiplied` on Vulkan or D3D12, where it
/// means straight alpha and every translucent pixel would darken.
fn transparent_mode(
    modes: &[wgpu::CompositeAlphaMode],
    backend: wgpu::Backend,
) -> Option<wgpu::CompositeAlphaMode> {
    use wgpu::CompositeAlphaMode as M;
    let mut wanted = vec![M::PreMultiplied];
    if backend == wgpu::Backend::Metal {
        wanted.push(M::PostMultiplied);
    }
    wanted.push(M::Inherit);
    wanted.into_iter().find(|m| modes.contains(m))
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

/// How many frames may be queued ahead of the one on screen by default: two, or one on Windows.
///
/// With two, a drawable to render into is waiting while the previous
/// frame's is still out, so a frame whose thread woke a little late still
/// makes its vsync (on Metal this is triple buffering). The price is that
/// a frame built the moment a drawable frees reaches the screen a vsync
/// later than it could; `kui-native` wins that back by starting frames at
/// the display's vsync, and a runner driven any other way pays it. On
/// Windows the flip-model swapchain delivers every vsync with a single
/// queued frame, so the second would be latency for nothing. Change it
/// per renderer with [`Renderer::set_frame_latency`].
pub const DEFAULT_FRAME_LATENCY: u32 = if cfg!(target_os = "windows") { 1 } else { 2 };

impl Renderer {
    /// A renderer for one window, on a device of its own.
    ///
    /// `width` and `height` are the window's size in physical pixels.
    /// Fails when no adapter can present to `target` or the surface cannot
    /// be configured. Use [`Renderer::new_in`] for every window after the
    /// first, so they share the device.
    pub async fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with(target, width, height, GpuOptions::default()).await
    }

    /// [`Renderer::new`] on a device opened with `options` — what every
    /// window on it must be able to do ([`GpuOptions`]) — and, under
    /// [`GpuOptions::transparent`], this window's surface presented with
    /// alpha where it can be ([`Renderer::transparent`]).
    pub async fn new_with(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        options: GpuOptions,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (gpu, surface) = Gpu::new_with(target, options).await?;
        Self::with_surface(gpu, surface, width, height, options.transparent)
    }

    /// Whether the surface is presented with alpha, so what is behind the
    /// window shows where a frame paints nothing and through a colour with
    /// alpha ([`Renderer::new_with`], [`Renderer::new_in_with`]). The frame
    /// is premultiplied, which is what every quad and glyph already blends
    /// to; clear it to a transparent [`Renderer::clear_color`] for the
    /// window to show through.
    pub fn transparent(&self) -> bool {
        !matches!(
            self.config.alpha_mode,
            wgpu::CompositeAlphaMode::Opaque | wgpu::CompositeAlphaMode::Auto
        )
    }

    /// A renderer for another window on an existing device, the one every
    /// window of the app shares. Get `gpu` from the first renderer's
    /// [`Renderer::gpu`].
    pub fn new_in(
        gpu: &Gpu,
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_in_with(gpu, target, width, height, false)
    }

    /// [`Renderer::new_in`] for a window whose surface is presented with
    /// alpha (`transparent`, backlog F126). Decided here, once, and not by
    /// a reconfigure: a D3D12 swapchain keeps the alpha mode it was made
    /// with, and resizing it later changes nothing — measured on Windows
    /// 11, where a surface reconfigured to premultiplied after it was made
    /// opaque went on compositing over black. Where the surface cannot
    /// take alpha — a D3D12 device not opened with
    /// [`GpuOptions::transparent`], a Vulkan or GL surface that offers
    /// only opaque — it is opaque, and [`Renderer::transparent`] says so.
    pub fn new_in_with(
        gpu: &Gpu,
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
        transparent: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let surface = gpu.create_surface(target)?;
        Self::with_surface(gpu.clone(), surface, width, height, transparent)
    }

    /// The device this renderer draws with, to open another window on.
    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// Sets how many frames may be queued ahead of the one on screen (at
    /// least one; see [`DEFAULT_FRAME_LATENCY`]). Reconfigures the surface
    /// when the value changes.
    pub fn set_frame_latency(&mut self, frames: u32) {
        let frames = frames.max(1);
        if self.config.desired_maximum_frame_latency != frames {
            self.config.desired_maximum_frame_latency = frames;
            if let Some(s) = &self.surface {
                s.configure(self.gpu.device(), &self.config);
            }
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
        transparent: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let device = gpu.device();
        let caps = surface.get_capabilities(gpu.adapter());
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let backend = gpu.adapter().get_info().backend;
        // Not on a device whose surfaces cannot show what is behind the
        // window (Windows, unless `see_through_by_visual`): there a mode
        // with alpha would composite over black and still say transparent.
        let alpha_mode = (transparent && gpu.0.alpha)
            .then(|| transparent_mode(&caps.alpha_modes, backend))
            .flatten()
            .unwrap_or_else(|| opaque_mode(&caps.alpha_modes));
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.clamp(1, device.limits().max_texture_dimension_2d),
            height: height.clamp(1, device.limits().max_texture_dimension_2d),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode,
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
        Ok(Self::assemble(gpu, Some(surface), config))
    }

    /// A renderer with no window: it draws into textures it is handed
    /// ([`Renderer::render_to`]) and reads frames back
    /// ([`Renderer::capture`]), never presents (backlog C55). For a host
    /// that composites kui with content of its own on its own device — a
    /// game engine, a 3D panel — and for pixels with no window at all,
    /// on a device from [`Gpu::windowless`].
    ///
    /// `format` is what every target must be: a non-sRGB one, as a
    /// window's surface is, so a colour lands as the byte it names;
    /// [`Renderer::capture`] reads `Rgba8Unorm` and `Bgra8Unorm`. Fails
    /// for a format the device cannot render to and blend.
    pub fn windowless(
        gpu: &Gpu,
        format: wgpu::TextureFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let caps = gpu.adapter().get_texture_format_features(format);
        if !caps
            .allowed_usages
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            || !caps
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
        {
            return Err(
                format!("{format:?} is not a format this device renders to and blends").into(),
            );
        }
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: 1,
            height: 1,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: DEFAULT_FRAME_LATENCY,
        };
        Ok(Self::assemble(gpu.clone(), None, config))
    }

    /// Everything but the surface: the pipelines for `config.format`, the
    /// atlas, the samplers and the first frame slot.
    fn assemble(
        gpu: Gpu,
        surface: Option<wgpu::Surface<'static>>,
        config: wgpu::SurfaceConfiguration,
    ) -> Self {
        let device = gpu.device();
        let dual_source = gpu.dual_source();
        let format = config.format;

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

        let pipeline = quad_pipeline(device, &pipeline_layout, &shader, format, blend);

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

        // Fragments: one uniform slot per draw in the frame slot, picked by
        // dynamic offset, and the layout their pipelines are built
        // against. All of it is built whether or not a frame ever draws
        // one — a bind group layout and sixteen slots, not a pipeline,
        // which is the part that costs and is built on first sight.
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
        let slot = FrameSlot::new(
            &SlotKit {
                device,
                mapped: gpu.0.mapped_frames,
                layout: &bind_layout,
                frag_layout: &fragment_bind_layout,
                atlas: &atlas_tex,
                samplers: &samplers,
                align: uniform_align,
            },
            // A ring may hold several; each grows to what its frames need.
            if gpu.0.mapped_frames { 1024 } else { 4096 },
            16,
        );

        Self {
            gpu,
            surface,
            config,
            pipeline,
            slots: vec![slot],
            slot: 0,
            bind_layout,
            samplers,
            texture_binds: Default::default(),
            atlas_tex,
            atlas_size,
            atlas_epoch: u64::MAX,
            atlas_rev: 0,
            atlas_id: 0,
            capture_target: None,
            instances: Vec::new(),
            fragment_layouts,
            fragment_bind_layout,
            uniform_align,
            fragment_bytes: Vec::new(),
            clear_color: wgpu::Color {
                r: 0.06,
                g: 0.065,
                b: 0.08,
                a: 1.0,
            },
            ground: None,
            backdrop_pipes: None,
            backdrop: backdrop::Targets::default(),
            groups: Vec::new(),
            group_kids: Vec::new(),
            group_kids_at: Vec::new(),
            group_targets: groups::Targets::default(),
            group_pipeline: None,
            blurs: Vec::new(),
        }
    }

    /// Draws `rgba` (`width` by `height` pixels, four bytes each, row by
    /// row from the top left, the bytes the surface takes as they are) over
    /// the clear and under everything a frame draws, opaque, stretched
    /// across the viewport and sampled with linear filtering — so a small
    /// picture reads as a soft one. What a runner draws as a window's
    /// ground where the OS has no material to put behind it: the
    /// wallpaper, scaled down and blurred once (backlog F126). Which part
    /// of the picture shows is [`Renderer::set_ground_uv`]; all of it
    /// until that is called. A degenerate size, or pixels that are not
    /// that size, clear it.
    pub fn set_ground(&mut self, rgba: &[u8], width: u32, height: u32) {
        let max = self.gpu.device().limits().max_texture_dimension_2d;
        if width == 0
            || height == 0
            || width > max
            || height > max
            || rgba.len() != width as usize * height as usize * 4
        {
            self.ground = None;
            return;
        }
        let device = self.gpu.device();
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kui.ground"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.gpu.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            size,
        );
        let uv = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kui.ground.uv"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.gpu
            .queue()
            .write_buffer(&uv, 0, bytemuck::cast_slice(&[0.0f32, 0.0, 1.0, 1.0]));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kui.ground"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
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
            ],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kui.ground"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uv.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.samplers.linear),
                },
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kui.ground"),
            source: wgpu::ShaderSource::Wgsl(GROUND_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kui.ground"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kui.ground"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        self.ground = Some(Ground {
            pipeline,
            bind,
            uv,
            _texture: texture,
        });
    }

    /// Which part of the ground shows across the viewport: `[u0, v0, u1,
    /// v1]` in the picture's own 0..1 coordinates, the top-left corner
    /// first. Outside 0..1 the picture's edge is stretched. Nothing without
    /// a ground.
    pub fn set_ground_uv(&mut self, uv: [f32; 4]) {
        if let Some(g) = &self.ground {
            self.gpu
                .queue()
                .write_buffer(&g.uv, 0, bytemuck::cast_slice(&uv));
        }
    }

    /// Takes the ground away ([`Renderer::set_ground`]).
    pub fn clear_ground(&mut self) {
        self.ground = None;
    }

    /// Whether a ground is drawn under the frame.
    pub fn has_ground(&self) -> bool {
        self.ground.is_some()
    }

    /// Whether this device can draw LCD subpixel glyphs
    /// (`QuadKind::GlyphSubpixel`) as intended.
    ///
    /// Pass it to `Core::set_subpixel_text` once after opening the
    /// renderer; when it is `false` the core keeps rasterizing grayscale
    /// masks, which every device blends correctly.
    pub fn subpixel_text(&self) -> bool {
        self.gpu.dual_source()
    }

    /// Reconfigures the swapchain for a new window size, in physical pixels.
    ///
    /// The size is clamped to at least 1 (a minimized window reports zero,
    /// and a zero-sized surface is a validation error) and to the device's
    /// largest texture (Windows hands out nonsense sizes mid-resize, and
    /// configuring past the limit panics inside wgpu). A clamped frame is
    /// one wrong picture; the next real size fixes it.
    pub fn resize(&mut self, width: u32, height: u32) {
        let max = self.gpu.device().limits().max_texture_dimension_2d;
        self.config.width = width.clamp(1, max);
        self.config.height = height.clamp(1, max);
        if let Some(s) = &self.surface {
            s.configure(self.gpu.device(), &self.config);
        }
    }

    /// The format every target this renderer draws into must be: the
    /// surface's, or the one [`Renderer::windowless`] was given.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Uploads the atlas page when it changed since this renderer last
    /// did: by its `dirty` flag for the renderer that `owns` it (the
    /// core's, which clears the flag after), and by its `rev` for one that
    /// does not and must leave the flag alone, a capture's. Leaves `dirty`
    /// to the caller.
    fn sync_atlas(&mut self, atlas: &GlyphAtlas, owns: bool) {
        if atlas.size != self.atlas_size {
            self.atlas_size = atlas.size;
            self.atlas_tex = create_atlas_texture(self.gpu.device(), atlas.size);
            let view = self
                .atlas_tex
                .create_view(&wgpu::TextureViewDescriptor::default());
            for slot in &mut self.slots {
                slot.bind = create_bind_group(
                    self.gpu.device(),
                    &self.bind_layout,
                    &globals_binding(&slot.buf),
                    &view,
                    &self.samplers,
                );
            }
            self.atlas_epoch = u64::MAX;
        }
        let changed = if owns {
            atlas.dirty
        } else {
            atlas.id() != self.atlas_id || atlas.rev != self.atlas_rev
        };
        if changed || self.atlas_epoch != atlas.epoch {
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
            self.atlas_epoch = atlas.epoch;
            self.atlas_rev = atlas.rev;
            self.atlas_id = atlas.id();
        }
    }

    /// Puts the frame's instances, fragment parameters (`fragment_bytes`)
    /// and `globals` where its draw reads them.
    ///
    /// On a device with mapped frames: picks a ready slot from the ring —
    /// polling once when none is, adding one while the ring is short,
    /// waiting on the GPU when it is full — grows it if the frame does not
    /// fit, and writes into its mapping; [`Renderer::render`] unmaps it
    /// just before the submit, so a frame that never submits leaves it
    /// ready. Elsewhere: `write_buffer` into the one slot.
    fn write_frame(&mut self, globals: &Globals) {
        let need = self.instances.len();
        let frags = self.fragment_bytes.len() / self.uniform_align as usize;
        let kit = SlotKit {
            device: self.gpu.device(),
            mapped: self.gpu.0.mapped_frames,
            layout: &self.bind_layout,
            frag_layout: &self.fragment_bind_layout,
            atlas: &self.atlas_tex,
            samplers: &self.samplers,
            align: self.uniform_align,
        };
        let grown = |slot: &FrameSlot| {
            FrameSlot::new(
                &kit,
                slot.cap.max(need.next_power_of_two()),
                slot.frag_cap.max(frags.next_power_of_two()),
            )
        };
        if !kit.mapped {
            let slot = &mut self.slots[0];
            if need > slot.cap || frags > slot.frag_cap {
                *slot = grown(slot);
            }
            let queue = self.gpu.queue();
            if need > 0 {
                queue.write_buffer(
                    &slot.buf,
                    slot.instances_at,
                    bytemuck::cast_slice(&self.instances),
                );
            }
            if frags > 0 {
                queue.write_buffer(&slot.buf, FRAGMENTS_AT, &self.fragment_bytes);
            }
            queue.write_buffer(&slot.buf, 0, bytemuck::bytes_of(globals));
            return;
        }
        let find = |slots: &[FrameSlot], from: usize| {
            (0..slots.len())
                .map(|i| (from + i) % slots.len())
                .find(|&i| slots[i].ready())
        };
        let from = (self.slot + 1) % self.slots.len();
        let mut found = find(&self.slots, from);
        if found.is_none() {
            let _ = kit.device.poll(wgpu::PollType::Poll);
            found = find(&self.slots, from);
        }
        if found.is_none() && self.slots.len() >= MAX_FRAME_SLOTS {
            let _ = kit.device.poll(wgpu::PollType::wait_indefinitely());
            found = find(&self.slots, from);
        }
        let i = found.unwrap_or_else(|| {
            // As large as the largest in the ring, so a new one does not
            // grow again a frame later.
            let largest = self
                .slots
                .iter()
                .max_by_key(|s| s.cap)
                .expect("a ring is never empty");
            self.slots.push(grown(largest));
            self.slots.len() - 1
        });
        self.slot = i;
        let slot = &mut self.slots[i];
        if need > slot.cap || frags > slot.frag_cap {
            // The old buffer goes mapped and unused; dropping it is all
            // it needs.
            *slot = grown(slot);
        }
        let mut view = match slot.buf.get_mapped_range_mut(..) {
            Ok(view) => view,
            // Not mapped after all (a device going away refuses maps):
            // a new one is, from creation.
            Err(_) => {
                *slot = grown(slot);
                slot.buf
                    .get_mapped_range_mut(..)
                    .expect("a buffer mapped at creation is mapped")
            }
        };
        view.slice(..std::mem::size_of::<Globals>())
            .copy_from_slice(bytemuck::bytes_of(globals));
        let at = FRAGMENTS_AT as usize;
        view.slice(at..at + self.fragment_bytes.len())
            .copy_from_slice(&self.fragment_bytes);
        let at = slot.instances_at as usize;
        let bytes: &[u8] = bytemuck::cast_slice(&self.instances);
        view.slice(at..at + bytes.len()).copy_from_slice(bytes);
    }

    /// Plans the frame's backdrop blurs (backlog F129), and makes or drops
    /// what drawing them takes: the pipelines once, the offscreen frame at
    /// the surface's size, the scratch at what the largest blur needs, a
    /// parameter slot per blur. `any` is whether the list has a backdrop
    /// quad at all, noticed while the instances were written, so a frame
    /// without one scans nothing.
    ///
    /// A frame without a blur drops the frame and the scratch there and
    /// then (backlog RG150; [`backdrop::Targets::fit`] says why).
    fn plan_backdrops(&mut self, dl: &DisplayList, any: bool) {
        self.blurs.clear();
        let (w, h) = (self.config.width, self.config.height);
        if any {
            for (i, q) in dl.quads.iter().enumerate() {
                if q.kind == QuadKind::Backdrop
                    && let Some(b) = backdrop::plan(i as u32, q, dl.clip_of(q), w, h)
                {
                    self.blurs.push(b);
                }
            }
        }
        if self.blurs.is_empty() {
            self.backdrop = backdrop::Targets::default();
            return;
        }
        let device = self.gpu.device();
        let align = self.uniform_align;
        let format = self.config.format;
        let pipes = self
            .backdrop_pipes
            .get_or_insert_with(|| backdrop::Pipes::new(device, format, align));
        if self.blurs.len() > pipes.params_cap {
            pipes.params_cap = self.blurs.len().next_power_of_two();
            pipes.params = backdrop::params_buffer(device, pipes.params_cap, align);
            // Every bind group names the old buffer.
            self.backdrop = backdrop::Targets::default();
        }
        self.backdrop
            .fit(device, pipes, format, (w, h), &self.blurs);
        let Some((_, scratch)) = self.backdrop.get() else {
            return;
        };
        let slot = align as usize;
        let mut bytes = vec![0u8; self.blurs.len() * slot];
        for (i, b) in self.blurs.iter().enumerate() {
            bytes[i * slot..i * slot + std::mem::size_of::<backdrop::Params>()]
                .copy_from_slice(bytemuck::bytes_of(&scratch.params(b)));
        }
        self.gpu.queue().write_buffer(&pipes.params, 0, &bytes);
    }

    /// Plans the frame's groups (ADR 0056): each placed in one of the two
    /// textures, its picture's place written into its end marker's
    /// instance, the textures fitted, and the group pipeline made the
    /// first time a device that blends per channel draws one. `any` is
    /// whether the list has a begin marker, noticed while the instances
    /// were written, so a frame without one scans nothing and drops the
    /// textures.
    ///
    /// Groups too many for a texture the device can make are drawn as
    /// they would be with groups off: each one's fade folded into the
    /// instances of its run.
    fn plan_groups(&mut self, dl: &DisplayList, any: bool) {
        self.groups.clear();
        if !any {
            self.group_targets.clear();
            return;
        }
        groups::plan(&dl.quads, &mut self.groups);
        let device = self.gpu.device();
        let max = device.limits().max_texture_dimension_2d;
        let width = dl.viewport.w.ceil() as u32;
        let Some(sizes) = groups::pack(&mut self.groups, width, max) else {
            fold_groups(&dl.quads, &mut self.instances);
            self.groups.clear();
            self.group_targets.clear();
            return;
        };
        self.group_targets
            .fit(device, self.config.format, sizes, max);
        for p in &self.groups {
            let at = p.offset();
            self.instances[p.end as usize].uv = [at[0], at[1], 0.0, 0.0];
        }
        // Each group's children, then the frame's, as runs of one list.
        let n = self.groups.len();
        let mut order: Vec<u32> = (0..n as u32).collect();
        let parent = |i: u32| match self.groups[i as usize].parent {
            groups::FRAME => n as u32,
            p => p,
        };
        order.sort_by_key(|&i| (parent(i), self.groups[i as usize].begin));
        self.group_kids_at.clear();
        self.group_kids_at.resize(n + 2, 0);
        for &i in &order {
            self.group_kids_at[parent(i) as usize + 1] += 1;
        }
        for k in 1..self.group_kids_at.len() {
            self.group_kids_at[k] += self.group_kids_at[k - 1];
        }
        self.group_kids = order;
        if self.gpu.dual_source() && self.group_pipeline.is_none() {
            let device = self.gpu.device();
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("kui.groups"),
                source: wgpu::ShaderSource::Wgsl(
                    preprocess_shader(include_str!("shader.wgsl"), false).into(),
                ),
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("kui.groups"),
                bind_group_layouts: &[Some(&self.bind_layout)],
                immediate_size: 0,
            });
            self.group_pipeline = Some(quad_pipeline(
                device,
                &layout,
                &shader,
                self.config.format,
                wgpu::BlendState::ALPHA_BLENDING,
            ));
        }
    }

    /// The groups drawn into plan `p`'s picture, or the frame's for
    /// [`groups::FRAME`], in paint order.
    fn group_kids(&self, p: u32) -> &[u32] {
        if self.groups.is_empty() {
            return &[];
        }
        let i = if p == groups::FRAME {
            self.groups.len()
        } else {
            p as usize
        };
        let (from, to) = (self.group_kids_at[i], self.group_kids_at[i + 1]);
        &self.group_kids[from as usize..to as usize]
    }

    /// Draws `range` into `pass` as [`Self::draw_quads`] does, except that
    /// each group of `parent`'s inside it is its picture, drawn at its end
    /// marker in place of its run.
    #[allow(clippy::too_many_arguments)]
    fn draw_span(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        dl: &DisplayList,
        range: std::ops::Range<u32>,
        parent: u32,
        pipeline: &wgpu::RenderPipeline,
        prep: &Prepared,
        pictures: &[Option<wgpu::BindGroup>; 2],
    ) {
        let mut at = range.start;
        for &k in self.group_kids(parent) {
            let g = &self.groups[k as usize];
            if g.begin < range.start || g.end >= range.end {
                continue;
            }
            self.draw_quads(
                pass,
                dl,
                at..g.begin,
                pipeline,
                &prep.fragment_pipelines,
                &prep.texture_binds,
            );
            if let Some(bind) = &pictures[g.tex()] {
                let frame = &self.slots[self.slot];
                pass.set_vertex_buffer(0, frame.buf.slice(frame.instances_at..));
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, bind, &[]);
                pass.draw(0..6, g.end..g.end + 1);
            }
            at = g.end + 1;
        }
        self.draw_quads(
            pass,
            dl,
            at..range.end,
            pipeline,
            &prep.fragment_pipelines,
            &prep.texture_binds,
        );
    }

    /// Draws every group's picture into its texture, deepest first: one
    /// pass per depth, each group through a viewport that moves its
    /// bounds onto its place, cut to that place.
    fn draw_groups(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        dl: &DisplayList,
        prep: &Prepared,
        pictures: &[Option<wgpu::BindGroup>; 2],
    ) {
        let Some(deepest) = self.groups.iter().map(|g| g.depth).max() else {
            return;
        };
        let pipeline = self.group_pipeline.as_ref().unwrap_or(&self.pipeline);
        let (vw, vh) = (dl.viewport.w.max(1.0), dl.viewport.h.max(1.0));
        let mut cleared = [false; 2];
        for depth in (0..=deepest).rev() {
            let t = (depth % 2) as usize;
            let Some(view) = self.group_targets.view(t) else {
                continue;
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kui.groups"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Once a frame: the groups drawn into it two
                        // depths apart keep each other's places.
                        load: if cleared[t] {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            cleared[t] = true;
            for (i, g) in self.groups.iter().enumerate() {
                if g.depth != depth || g.w == 0 || g.h == 0 {
                    continue;
                }
                let [ox, oy] = g.offset();
                pass.set_viewport(ox, oy, vw, vh, 0.0, 1.0);
                pass.set_scissor_rect(g.at.0, g.at.1, g.w, g.h);
                self.draw_span(
                    &mut pass,
                    dl,
                    g.begin + 1..g.end,
                    i as u32,
                    pipeline,
                    prep,
                    pictures,
                );
            }
        }
    }

    /// Draws the instances in `range` into `pass`: in one instanced draw,
    /// or — where a fragment or a texture-backed image interrupts the run
    /// — the run before it with the über-pipeline, then that one quad with
    /// its own pipeline (a fragment) or its own group 0 (a texture), then
    /// on. Consecutive quads of the same handle still take one set each
    /// (about 0.6 us); runs of ordinary quads are unbroken.
    fn draw_quads(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        dl: &DisplayList,
        range: std::ops::Range<u32>,
        pipeline: &wgpu::RenderPipeline,
        fragment_pipelines: &[wgpu::RenderPipeline],
        texture_binds: &[Option<u64>],
    ) {
        if range.is_empty() {
            return;
        }
        let frame = &self.slots[self.slot];
        pass.set_vertex_buffer(0, frame.buf.slice(frame.instances_at..));
        if fragment_pipelines.is_empty() && texture_binds.is_empty() {
            // The whole run in one instanced draw, as a frame has always
            // been. Nothing below runs.
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &frame.bind, &[]);
            pass.draw(0..6, range);
            return;
        }
        let mut run_start = range.start;
        let mut on_quads = false;
        for i in range.clone() {
            let q = &dl.quads[i as usize];
            if q.kind != QuadKind::Fragment && q.kind != QuadKind::Texture {
                continue;
            }
            if i > run_start {
                if !on_quads {
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, &frame.bind, &[]);
                    on_quads = true;
                }
                pass.draw(0..6, run_start..i);
            }
            // `uv[0]` is the index into the side list, which is also this
            // fragment's parameter slot, or this texture's bind.
            let slot = q.uv[0] as usize;
            if q.kind == QuadKind::Texture {
                if let Some(Some(id)) = texture_binds.get(slot)
                    && let Some(b) = self.texture_binds.get(id)
                {
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, &b.bind, &[]);
                    on_quads = false;
                    pass.draw(0..6, i..i + 1);
                }
            } else if let Some(pipeline) = fragment_pipelines.get(slot) {
                // A fragment reading a texture-backed image takes that
                // image's group 0 — the texture in the atlas's place,
                // `atlas_size` its size — exactly as a texture quad does;
                // one reading the atlas, or nothing, takes the frame's. A
                // texture the device could not make (a degenerate or
                // oversized image) draws the fragment against the atlas
                // with a zero rect, which `kui_sample` reads as no image.
                let group0 = match draw_image_texture(&dl.fragments[slot]) {
                    Some(index) => texture_binds
                        .get(index)
                        .copied()
                        .flatten()
                        .and_then(|id| self.texture_binds.get(&id))
                        .map_or(&frame.bind, |b| &b.bind),
                    None => &frame.bind,
                };
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, group0, &[]);
                pass.set_bind_group(1, &frame.frag_bind, &[slot as u32 * self.uniform_align]);
                on_quads = false;
                pass.draw(0..6, i..i + 1);
            }
            run_start = i + 1;
        }
        if range.end > run_start {
            if !on_quads {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &frame.bind, &[]);
            }
            pass.draw(0..6, run_start..range.end);
        }
    }

    /// Draws one frame and presents it.
    ///
    /// Uploads the atlas when it changed since the last frame (and clears
    /// its `dirty` flag), writes the list's quads to the instance buffer,
    /// draws them over [`Renderer::clear_color`] in one render pass, and
    /// presents. Acquiring the swapchain image blocks while vsync holds
    /// the frame back; that time comes back as
    /// [`RenderReport::vsync_wait_ms`] so a runner can tell pacing from
    /// work.
    ///
    /// Fails without presenting when the surface or the device cannot
    /// take the frame; each [`RenderError`] says what to do next. A
    /// [`Renderer::windowless`] one has nothing to present to and returns
    /// [`RenderError::Skip`], drawing nothing.
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
        if self.surface.is_none() {
            return Err(RenderError::Skip);
        }
        self.sync_atlas(atlas, true);
        atlas.dirty = false;
        let prep = self.prepare(dl);

        // Acquiring the swapchain image is where vsync backpressure blocks;
        // report it separately so latency graphs show pacing vs work.
        let t_wait = std::time::Instant::now();
        let surface = self.surface.as_ref().expect("checked above");
        let frame = match surface.get_current_texture() {
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
        let surface_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("kui") });
        self.draw(&mut encoder, dl, &prep, &surface_view);
        self.submit(encoder.finish());
        self.gpu.queue().present(frame);
        Ok(RenderReport { vsync_wait_ms })
    }

    /// Draws one frame into `target`, a texture the host owns, and submits
    /// it; nothing is presented (backlog C55). What a host compositing kui
    /// with its own content calls each frame in place of
    /// [`Renderer::render`], on a [`Renderer::windowless`] renderer on its
    /// own [`Gpu`]. A window's renderer can draw into one too.
    ///
    /// The target must be [`Renderer::format`] and have
    /// `RENDER_ATTACHMENT` usage; its size is the frame's, in physical
    /// pixels, and should be the list's viewport. It is cleared to
    /// [`Renderer::clear_color`] and holds premultiplied colour. The atlas
    /// is uploaded when it changed and its `dirty` flag cleared, as
    /// [`Renderer::render`] does, so this is the renderer for that core.
    ///
    /// [`RenderError::Validation`] for a target of another format or
    /// without the usage, [`RenderError::DeviceLost`] on a dead device.
    pub fn render_to(
        &mut self,
        dl: &DisplayList,
        atlas: &mut GlyphAtlas,
        target: &wgpu::Texture,
    ) -> Result<(), RenderError> {
        if self.gpu.lost() {
            return Err(RenderError::DeviceLost);
        }
        if target.format() != self.config.format
            || !target
                .usage()
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        {
            return Err(RenderError::Validation);
        }
        self.config.width = target.width();
        self.config.height = target.height();
        self.sync_atlas(atlas, true);
        atlas.dirty = false;
        let prep = self.prepare(dl);
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("kui") });
        self.draw(&mut encoder, dl, &prep, &view);
        self.submit(encoder.finish());
        Ok(())
    }

    /// Draws `dl` into a texture of its own, the viewport's size, and
    /// reads it back: the frame as the screen would show it, cleared to
    /// [`Renderer::clear_color`] (backlog C55).
    ///
    /// The pixels come back as RGBA, four bytes each, row by row from the
    /// top left, alpha not premultiplied — what `add_image` and
    /// `decode_image` use, so a capture can be shown again as an image.
    /// A `Bgra8Unorm` renderer's are swizzled.
    ///
    /// Takes the atlas without clearing its `dirty` flag — uploading it
    /// when its id, `epoch` or `rev` say this renderer's copy is not
    /// current — so a renderer that is not the core's own, a windowless
    /// one beside a window's, leaves the window's renderer the changes it
    /// has not drawn yet. Waits for the GPU:
    /// [`Renderer::capture_begin`] then [`PendingCapture::wait`], which a
    /// caller that should not wait splits across threads.
    ///
    /// [`RenderError::Validation`] for a list with no viewport, larger
    /// than the device's largest texture, a format other than
    /// `Rgba8Unorm` or `Bgra8Unorm`, or a draw the device refused;
    /// [`RenderError::DeviceLost`] on a dead device.
    pub fn capture(
        &mut self,
        dl: &DisplayList,
        atlas: &GlyphAtlas,
    ) -> Result<Capture, RenderError> {
        self.capture_begin(dl, atlas)?.wait()
    }

    /// [`Renderer::capture`]'s first half: the frame drawn and its
    /// readback submitted, and nothing waited for. The pixels are the
    /// [`PendingCapture`]'s, which any thread may wait on, so a recording
    /// draws each frame on the thread that has the core and leaves the
    /// GPU's wait and the copy out to another (backlog C56). About half a
    /// millisecond of CPU at 3456 by 2234, where the whole capture is ten.
    ///
    /// The same errors as [`Renderer::capture`], for what can be known at
    /// the submit.
    pub fn capture_begin(
        &mut self,
        dl: &DisplayList,
        atlas: &GlyphAtlas,
    ) -> Result<PendingCapture, RenderError> {
        if self.gpu.lost() {
            return Err(RenderError::DeviceLost);
        }
        let format = self.config.format;
        let bgra = match format {
            wgpu::TextureFormat::Rgba8Unorm => false,
            wgpu::TextureFormat::Bgra8Unorm => true,
            _ => return Err(RenderError::Validation),
        };
        let (w, h) = (
            dl.viewport.w.round().max(0.0) as u32,
            dl.viewport.h.round().max(0.0) as u32,
        );
        let max = self.gpu.device().limits().max_texture_dimension_2d;
        if w == 0 || h == 0 || w > max || h > max {
            return Err(RenderError::Validation);
        }
        let device = self.gpu.device().clone();
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let target = match self.capture_target.take() {
            Some(t) if (t.width(), t.height()) == (w, h) => t,
            _ => device.create_texture(&wgpu::TextureDescriptor {
                label: Some("kui.capture"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }),
        };
        let bpr = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kui.capture"),
            size: bpr as u64 * h as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.config.width = w;
        self.config.height = h;
        self.sync_atlas(atlas, false);
        let prep = self.prepare(dl);
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("kui") });
        self.draw(&mut encoder, dl, &prep, &view);
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(h),
                },
            },
            target.size(),
        );
        self.submit(encoder.finish());
        self.capture_target = Some(target);
        // What the device refused is known by the submit.
        if pollster::block_on(scope.pop()).is_some() {
            return Err(if self.gpu.lost() {
                RenderError::DeviceLost
            } else {
                RenderError::Validation
            });
        }
        let mapped = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(MAP_PENDING));
        buf.slice(..).map_async(wgpu::MapMode::Read, {
            let mapped = mapped.clone();
            move |r| {
                let state = if r.is_ok() { MAP_DONE } else { MAP_FAILED };
                mapped.store(state, std::sync::atomic::Ordering::Release);
            }
        });
        Ok(PendingCapture {
            gpu: self.gpu.clone(),
            buf,
            mapped,
            width: w,
            height: h,
            bpr,
            bgra,
        })
    }

    /// The CPU half of a frame: the instances, the blurs, the globals and
    /// fragment parameters written where the draw reads them, the
    /// texture-backed images' bind groups and the fragments' pipelines.
    /// The atlas is the caller's to sync first.
    fn prepare(&mut self, dl: &DisplayList) -> Prepared {
        self.instances.clear();
        let mut any_backdrop = false;
        let mut any_group = false;
        self.instances.extend(dl.quads.iter().map(|q| {
            any_backdrop |= q.kind == QuadKind::Backdrop;
            any_group |= q.kind == QuadKind::GroupBegin;
            instance_of(q, &dl.clips, &dl.textures)
        }));
        self.plan_backdrops(dl, any_backdrop);
        self.plan_groups(dl, any_group);
        let globals = Globals {
            viewport: [dl.viewport.w.max(1.0), dl.viewport.h.max(1.0)],
            atlas_size: [self.atlas_size as f32, self.atlas_size as f32],
            time: dl.time,
            scale: dl.scale,
            _pad: [0.0; 2],
        };
        self.fragment_bytes.clear();
        if !dl.fragments.is_empty() {
            let align = self.uniform_align as usize;
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
        }
        self.write_frame(&globals);

        // Texture-backed images: drop what the core
        // removed, upload what moved, and give each one drawn this frame
        // a group-0 bind group of its own with a globals copy whose
        // `atlas_size` is the texture's. All skipped on a frame that
        // draws none.
        for id in &dl.dropped_textures {
            self.texture_binds.remove(&id.to_ffi());
            self.gpu.drop_image_texture(id.to_ffi());
        }
        // And the pipelines of removed fragments — built per handle
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
                        &globals_buf.as_entire_buffer_binding(),
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

        // Each fragment's pipeline built if this device has not seen the
        // handle before (its parameters went into the frame slot with the
        // instances). Skipped whole on a frame that draws no fragment.
        let mut fragment_pipelines: Vec<wgpu::RenderPipeline> = Vec::new();
        if !dl.fragments.is_empty() {
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
        Prepared {
            texture_binds,
            fragment_pipelines,
        }
    }

    /// The GPU half of a frame: its passes into `surface_view` — the
    /// surface's image, or a texture a host or a capture handed in — over
    /// what [`Renderer::prepare`] wrote.
    fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        dl: &DisplayList,
        prep: &Prepared,
        surface_view: &wgpu::TextureView,
    ) {
        // The groups' pictures first, each pass reading only the
        // pictures one depth deeper (ADR 0056), bound in the atlas's place
        // with this frame's globals.
        let pictures: [Option<wgpu::BindGroup>; 2] = if self.groups.is_empty() {
            [None, None]
        } else {
            let frame = &self.slots[self.slot];
            [0, 1].map(|t| {
                self.group_targets.view(t).map(|view| {
                    create_bind_group(
                        self.gpu.device(),
                        &self.bind_layout,
                        &globals_binding(&frame.buf),
                        view,
                        &self.samplers,
                    )
                })
            })
        };
        self.draw_groups(encoder, dl, prep, &pictures);
        // A frame that blurs draws into the offscreen copy, so what it has
        // drawn can be read back, and breaks its pass at each blur
        // (backlog F129); one that does not draws to the surface in one
        // pass, as it always has.
        let blurring = !self.blurs.is_empty();
        let offscreen = match (&self.backdrop_pipes, self.backdrop.get(), blurring) {
            (Some(p), Some((f, s)), true) => Some((p, f, s)),
            _ => None,
        };
        let target = offscreen.map_or(surface_view, |(_, f, _)| &f.view);
        let end = self.instances.len() as u32;
        let mut start = 0u32;
        let mut next = 0usize;
        loop {
            let stop = self.blurs.get(next).map_or(end, |b| b.quad);
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("kui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: if next == 0 {
                                wgpu::LoadOp::Clear(self.clear_color)
                            } else {
                                wgpu::LoadOp::Load
                            },
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                // The ground first, opaque over the clear, so everything
                // the frame paints with alpha blends over it.
                if next == 0
                    && let Some(g) = &self.ground
                {
                    pass.set_pipeline(&g.pipeline);
                    pass.set_bind_group(0, &g.bind, &[]);
                    pass.draw(0..4, 0..1);
                }
                // The blur the last pass stopped for, written back before
                // anything after its quad draws over it.
                if let Some(((pipes, frame, scratch), b)) =
                    offscreen.zip(next.checked_sub(1).and_then(|i| self.blurs.get(i)))
                {
                    let slot = (next - 1) as u32 * self.uniform_align;
                    backdrop::composite(&mut pass, pipes, scratch, b, slot);
                    pass.set_scissor_rect(0, 0, frame.size.0, frame.size.1);
                }
                self.draw_span(
                    &mut pass,
                    dl,
                    start..stop,
                    groups::FRAME,
                    &self.pipeline,
                    prep,
                    &pictures,
                );
            }
            let (Some(b), Some((pipes, frame, scratch))) = (self.blurs.get(next), offscreen) else {
                break;
            };
            let slot = next as u32 * self.uniform_align;
            backdrop::record(encoder, pipes, frame, scratch, b, slot);
            start = b.quad + 1;
            next += 1;
        }
        if let Some((pipes, frame, _)) = offscreen {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kui.backdrop.blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipes.blit);
            pass.set_bind_group(0, &frame.blit, &[0]);
            pass.draw(0..3, 0..1);
        }
    }

    /// Submits a frame's commands, its frame slot unmapped around them
    /// on a device with mapped frames.
    fn submit(&mut self, commands: wgpu::CommandBuffer) {
        let mapped = self.gpu.0.mapped_frames;
        let slot = &self.slots[self.slot];
        if mapped {
            slot.ready
                .store(false, std::sync::atomic::Ordering::Relaxed);
            slot.buf.unmap();
        }
        self.gpu.queue().submit([commands]);
        if mapped {
            // Back to the CPU once the GPU is done with it, which the next
            // frame's poll or a later submit notices.
            let ready = slot.ready.clone();
            slot.buf.map_async(wgpu::MapMode::Write, .., move |r| {
                if r.is_ok() {
                    ready.store(true, std::sync::atomic::Ordering::Release);
                }
            });
        }
    }
}

// Waited on from a recording's thread (kui-native's `Captures`).
const _: fn() = || {
    fn send<T: Send>() {}
    send::<PendingCapture>();
};

const MAP_PENDING: u8 = 0;
const MAP_DONE: u8 = 1;
const MAP_FAILED: u8 = 2;

/// A frame [`Renderer::capture_begin`] drew and asked back, on its way:
/// [`PendingCapture::wait`] has the pixels. `Send`, so the wait and the
/// copy out may be another thread's.
pub struct PendingCapture {
    gpu: Gpu,
    buf: wgpu::Buffer,
    mapped: std::sync::Arc<std::sync::atomic::AtomicU8>,
    width: u32,
    height: u32,
    bpr: u32,
    bgra: bool,
}

impl PendingCapture {
    /// Waits for the GPU and copies the frame out, as [`Renderer::capture`]
    /// returns it. [`RenderError::DeviceLost`] when the device went before
    /// it was done, [`RenderError::Validation`] when the readback failed.
    pub fn wait(self) -> Result<Capture, RenderError> {
        let device = self.gpu.device();
        let failed = |gpu: &Gpu| {
            if gpu.lost() {
                RenderError::DeviceLost
            } else {
                RenderError::Validation
            }
        };
        if self.mapped.load(std::sync::atomic::Ordering::Acquire) == MAP_PENDING {
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
        }
        if self.mapped.load(std::sync::atomic::Ordering::Acquire) != MAP_DONE {
            return Err(failed(&self.gpu));
        }
        let (w, h) = (self.width as usize, self.height as usize);
        let mut rgba = Vec::with_capacity(w * h * 4);
        {
            let data = self
                .buf
                .slice(..)
                .get_mapped_range()
                .map_err(|_| failed(&self.gpu))?;
            for row in data.chunks(self.bpr as usize).take(h) {
                rgba.extend_from_slice(&row[..w * 4]);
            }
        }
        self.buf.unmap();
        straighten(&mut rgba, self.bgra);
        Ok(Capture {
            width: self.width,
            height: self.height,
            rgba,
        })
    }
}

/// What [`Renderer::prepare`] leaves the draw: per texture draw, the
/// image whose bind group it uses (`None` draws nothing), and per
/// fragment draw, its pipeline.
struct Prepared {
    texture_binds: Vec<Option<u64>>,
    fragment_pipelines: Vec<wgpu::RenderPipeline>,
}

/// A frame read back by [`Renderer::capture`]: `width` by `height` pixels
/// in physical px, RGBA, four bytes each, row by row from the top left,
/// alpha not premultiplied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Premultiplied pixels, as a target holds them, to straight RGBA:
/// swizzled from BGRA when `bgra`, each colour divided by its alpha where
/// the alpha is neither 0 nor 255. A frame over an opaque clear is all
/// 255, and only the swizzle costs anything.
fn straighten(px: &mut [u8], bgra: bool) {
    for p in px.as_chunks_mut::<4>().0 {
        if bgra {
            p.swap(0, 2);
        }
        let a = p[3] as u32;
        if a != 0 && a != 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

/// The `textures` entry a fragment draw reads its image from, if its
/// image has a texture of its own.
/// Every group's fade folded into the instances of its run, and its
/// picture drawn as nothing: the frame as it would be with groups off,
/// for groups too many for the device's textures.
fn fold_groups(quads: &[Quad], instances: &mut [Instance]) {
    let mut fades: Vec<f32> = Vec::new();
    let mut k = 1.0f32;
    for (q, inst) in quads.iter().zip(instances) {
        match q.kind {
            QuadKind::GroupBegin => {
                fades.push(q.color.a);
                k *= q.color.a;
            }
            QuadKind::GroupEnd => {
                fades.pop();
                k = fades.iter().product();
                inst.size = [0.0, 0.0];
            }
            // A shadow carries its knock-out's radius in the border's
            // alpha.
            QuadKind::Shadow => inst.color[3] *= k,
            _ => {
                inst.color[3] *= k;
                inst.border_color[3] *= k;
            }
        }
    }
}

fn draw_image_texture(draw: &kui_core::FragmentDraw) -> Option<usize> {
    match draw.image {
        kui_core::FragmentImage::Texture { index, .. } => Some(index as usize),
        _ => None,
    }
}

/// Timing details from one [`Renderer::render`] call.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderReport {
    /// Milliseconds spent blocked acquiring the swapchain image, which is
    /// where vsync backpressure shows up.
    pub vsync_wait_ms: f32,
}

/// A frame that produced no image, and what to do about it.
///
/// Mapped from wgpu's `CurrentSurfaceTexture`; the crate root's example
/// handles every variant.
#[derive(Clone, Copy, Debug)]
pub enum RenderError {
    /// The surface is outdated or lost: call [`Renderer::resize`] with the
    /// window's size and draw again.
    Reconfigure,
    /// Nothing can be presented right now (the window is occluded, or the
    /// acquire timed out): try again next frame.
    Skip,
    /// The surface is configured wrong for the window: call
    /// [`Renderer::resize`] with the window's size and draw again. A
    /// surface that stays wrong is best given up with its device.
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
    globals: &wgpu::BufferBinding<'_>,
    view: &wgpu::TextureView,
    samplers: &Samplers,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("kui"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(globals.clone()),
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
                offset: FRAGMENTS_AT,
                size: std::num::NonZeroU64::new(std::mem::size_of::<FragmentParams>() as u64),
            }),
        }],
    })
}

/// Prints the code and module of a crash to stderr before the process dies (Windows only).
///
/// A fault in a GPU driver, such as one being replaced under the app, ends
/// the process with no line from anyone: it is not a panic, and Windows
/// reports only `0xC000041D` for an exception in a window callback. This
/// installs an unhandled-exception filter that names the exception code
/// and the module the faulting address is in, then lets the crash go on;
/// it is a diagnostic, not a recovery. A crash reporter the host installed
/// first is still called, with its answer returned; one installed after
/// replaces this filter.
///
/// [`Gpu::new`] calls it, so a runner rarely needs to. Installing it more
/// than once is harmless.
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

/// Prints the code and module of a crash to stderr before the process dies (Windows only).
///
/// On Windows a fault in a GPU driver ends the process with no line from
/// anyone, and this installs the exception filter that names it. On every
/// other platform it does nothing. [`Gpu::new`] calls it, so a runner
/// rarely needs to.
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

    /// An opaque window keeps the mode it always had, `Opaque` — also on
    /// a composition swapchain, which lists `Auto` first and would have
    /// presented `DXGI_ALPHA_MODE_UNSPECIFIED` (backlog F126).
    #[test]
    fn an_opaque_window_presents_opaque_wherever_it_can() {
        use wgpu::CompositeAlphaMode as M;
        assert_eq!(opaque_mode(&[M::Opaque]), M::Opaque);
        assert_eq!(opaque_mode(&[M::Opaque, M::PostMultiplied]), M::Opaque);
        assert_eq!(
            opaque_mode(&[
                M::Auto,
                M::Inherit,
                M::Opaque,
                M::PostMultiplied,
                M::PreMultiplied
            ]),
            M::Opaque
        );
        assert_eq!(opaque_mode(&[M::Inherit]), M::Inherit);
    }

    /// One answer for the window's style and the swapchain (backlog
    /// RG150): D3D12 through a visual unless the backend list leaves
    /// D3D12 out or the presentation system names the handle — each
    /// variable read as wgpu reads it.
    #[test]
    fn the_window_and_the_swapchain_are_decided_together() {
        let by = see_through_by_visual_with;
        // Nothing set: kui opens D3D12 alone, through the visual.
        assert!(by(None, None));
        // `WGPU_BACKEND` naming D3D12, alone or in a list, in either
        // spelling and any case, spaces stripped.
        for b in ["dx12", "D3D12", "DX12", "vulkan,dx12", " gl , d3d12 "] {
            assert!(by(Some(b), None), "{b}");
        }
        // Naming only others, or nothing wgpu knows.
        for b in ["vulkan", "gl", "vk,gl", "", "directx", "dx11"] {
            assert!(!by(Some(b), None), "{b}");
        }
        // The presentation system: the handle's swapchain takes no alpha;
        // the visual's, or a value wgpu does not read, leave kui's choice.
        for p in ["hwnd", "Hwnd", "DxgiFromHwnd", "dxgifromhwnd"] {
            assert!(!by(None, Some(p)), "{p}");
            assert!(!by(Some("dx12"), Some(p)), "{p}");
        }
        for p in ["visual", "DxgiFromVisual", "", "nonsense", " hwnd"] {
            assert!(by(None, Some(p)), "{p}");
        }
        // Both must allow it.
        assert!(!by(Some("vulkan"), Some("visual")));
    }

    /// A transparent one takes a mode that composites premultiplied
    /// pixels, and none where only straight alpha or opaque is offered.
    #[test]
    fn a_transparent_window_presents_premultiplied_or_not_at_all() {
        use wgpu::{Backend, CompositeAlphaMode as M};
        // D3D12 through a composition visual.
        let visual = [
            M::Auto,
            M::Inherit,
            M::Opaque,
            M::PostMultiplied,
            M::PreMultiplied,
        ];
        assert_eq!(
            transparent_mode(&visual, Backend::Dx12),
            Some(M::PreMultiplied)
        );
        // D3D12 on the window's handle: opaque only.
        assert_eq!(transparent_mode(&[M::Opaque], Backend::Dx12), None);
        // Metal names its non-opaque layer post-multiplied.
        let metal = [M::Opaque, M::PostMultiplied];
        assert_eq!(
            transparent_mode(&metal, Backend::Metal),
            Some(M::PostMultiplied)
        );
        // Vulkan's post-multiplied is straight alpha: not that.
        assert_eq!(
            transparent_mode(&[M::Opaque, M::PostMultiplied], Backend::Vulkan),
            None
        );
        assert_eq!(
            transparent_mode(&[M::Opaque, M::Inherit], Backend::Vulkan),
            Some(M::Inherit)
        );
    }

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

    /// The ground's shader parses and validates, as the pipeline that
    /// draws a window's wallpaper would otherwise fail in a window
    /// (backlog F126).
    #[test]
    fn the_ground_shader_validates() {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let module = wgpu::naga::front::wgsl::parse_str(GROUND_SHADER)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(GROUND_SHADER)));
        Validator::new(ValidationFlags::all(), Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    /// The backdrop blur's shader (backlog F129), every entry point, and
    /// its `Params` the size the Rust struct writes.
    #[test]
    fn the_backdrop_shader_validates() {
        use wgpu::naga::valid::{Capabilities, ValidationFlags, Validator};
        let src = include_str!("backdrop.wgsl");
        let module = wgpu::naga::front::wgsl::parse_str(src)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(src)));
        Validator::new(ValidationFlags::all(), Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{e:?}"));
        let names: Vec<&str> = module
            .entry_points
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        for want in [
            "vs",
            "fs_down",
            "fs_blur_h",
            "fs_blur_v",
            "fs_composite",
            "fs_blit",
        ] {
            assert!(names.contains(&want), "{want} in {names:?}");
        }
        let params = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Params"))
            .expect("Params")
            .1;
        let wgpu::naga::TypeInner::Struct { span, .. } = params.inner else {
            panic!("Params is a struct");
        };
        assert_eq!(span as usize, std::mem::size_of::<backdrop::Params>());
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
