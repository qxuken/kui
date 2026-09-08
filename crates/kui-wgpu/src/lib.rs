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
use kui_core::{DisplayList, Quad, QuadKind};

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

/// One fragment's parameters as the shader takes them, padded out to the
/// device's dynamic-offset alignment so a frame's draws can share one
/// buffer and pick their slot by offset.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FragmentParams {
    params: [f32; 16],
}

fn instance_of(q: &Quad) -> Instance {
    let kind = match q.kind {
        QuadKind::Solid => 0.0,
        QuadKind::GlyphMask => 1.0,
        QuadKind::GlyphColor => 2.0,
        QuadKind::Image => 3.0,
        QuadKind::GlyphSubpixel => 4.0,
        QuadKind::Shadow => 5.0,
        QuadKind::Segment => 6.0,
        QuadKind::Fragment => 7.0,
    };
    // `uv` is atlas texels on every kind but one; a segment carries its
    // endpoints there as f32 bits, and the shader wants them as floats.
    let uv = if q.kind == QuadKind::Segment {
        q.segment_ends()
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
        clip: [q.clip.x, q.clip.y, q.clip.w, q.clip.h],
        radii: q.radius,
        clip_radii: q.clip_radius,
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
    /// device — the cost the ADR measured at about 0.2 ms, paid once. A
    /// `Mutex` because `Gpu` is a shared handle and building is rare;
    /// nothing here is touched on a frame that draws no new fragment.
    fragment_pipelines: std::sync::Mutex<
        std::collections::HashMap<(u64, wgpu::TextureFormat), wgpu::RenderPipeline>,
    >,
}

impl Gpu {
    /// Opens the shared device, choosing an adapter that can present to
    /// `target`'s surface — the first window's, whose surface comes back
    /// with it because it has to exist before the adapter can be picked.
    /// Every later window's surface comes from [`Gpu::create_surface`].
    pub async fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
    ) -> Result<(Self, wgpu::Surface<'static>), Box<dyn std::error::Error>> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
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
        let gpu = Self(std::sync::Arc::new(GpuInner {
            instance,
            adapter,
            device,
            queue,
            dual_source,
            fragment_pipelines: Default::default(),
        }));
        Ok((gpu, surface))
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
            // One queued frame: measurably lower input-to-photon latency at
            // the cost of less slack for slow frames.
            desired_maximum_frame_latency: 1,
        };
        surface.configure(device, &config);

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
        let bind_group = create_bind_group(device, &bind_layout, &globals_buf, &atlas_tex);

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
            self.bind_group = create_bind_group(
                self.gpu.device(),
                &self.bind_layout,
                &self.globals_buf,
                &self.atlas_tex,
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
        self.sync_atlas(atlas);

        self.instances.clear();
        self.instances.extend(dl.quads.iter().map(instance_of));
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
                let slot = FragmentParams {
                    params: draw.params,
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
            wgpu::CurrentSurfaceTexture::Validation => return Err(RenderError::Validation),
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
                if fragment_pipelines.is_empty() {
                    // The whole frame in one instanced draw, as it has
                    // always been. Nothing below runs.
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &self.bind_group, &[]);
                    pass.draw(0..6, 0..self.instances.len() as u32);
                } else {
                    // A fragment interrupts the run: draw what came
                    // before with the über-pipeline, then that one quad
                    // with its own, then carry on
                    // (`docs/adr/0015-…`, decision 5). Consecutive
                    // fragment quads of the same handle still take one
                    // pipeline set each, which is the 0.6 us the ADR
                    // measured; runs of ordinary quads are unbroken.
                    let mut run_start = 0u32;
                    let mut on_quads = false;
                    for (i, q) in dl.quads.iter().enumerate() {
                        if q.kind != QuadKind::Fragment {
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
                        // is also this fragment's parameter slot.
                        let slot = q.uv[0] as usize;
                        if let Some(pipeline) = fragment_pipelines.get(slot) {
                            pass.set_pipeline(pipeline);
                            pass.set_bind_group(0, &self.bind_group, &[]);
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
    /// Validation error acquiring the surface texture.
    Validation,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reconfigure => write!(f, "surface outdated or lost; reconfigure"),
            Self::Skip => write!(f, "no frame available; skip"),
            Self::Validation => write!(f, "surface texture validation error"),
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

fn create_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    atlas: &wgpu::Texture,
) -> wgpu::BindGroup {
    let view = atlas.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("kui.atlas"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
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
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
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
}
