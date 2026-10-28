//! Research: what drawing somewhere other than the surface costs on this
//! machine's GPU — a whole frame drawn offscreen and blitted, subtrees
//! drawn into layers and composited (group opacity done right), a cached
//! layer against redrawing what it holds, reading a frame back to the CPU
//! (and on to a PNG), making the textures, and opening a device with no
//! window at all.
//!
//! Offscreen throughout, as `split` is: the "surface" is a texture of the
//! surface's size. The quad pipeline is this crate's `shader.wgsl` with
//! the private `Instance`/`Globals` mirrored from `split.rs`; the
//! composite is a small textured-quad shader of its own here, premultiplied
//! over, which is what a layer composite in kui would be.
//!
//! The numbers are backlog C55's evidence. Not a divan bench: it prints
//! tables and exits.
//!
//!   cargo bench -p kui-wgpu --bench offscreen
//!   ONLY=readback cargo bench -p kui-wgpu --bench offscreen

use std::time::Instant;

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
    radii: [f32; 4],
    clip_radii: [f32; 4],
    xform: [f32; 4],
    inner: [f32; 4],
    inner_radii: [f32; 4],
    /// The stretch before the turn: sx, sy, shear, unused.
    stretch: [f32; 4],
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            xform: [0.0, 1.0, 0.0, 0.0],
            inner: [-1e9, -1e9, 2e9, 2e9],
            stretch: [1.0, 1.0, 0.0, 0.0],
            ..bytemuck::Zeroable::zeroed()
        }
    }
}

const _: () = assert!(std::mem::size_of::<Instance>() == 192);

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    viewport: [f32; 2],
    atlas_size: [f32; 2],
    time: f32,
    scale: f32,
    _pad: [f32; 2],
}

const _: () = assert!(std::mem::size_of::<Globals>() == 32);

/// One composite's numbers: where it lands (target px), the target's size,
/// and the opacity it is drawn at.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Comp {
    dst: [f32; 4],
    viewport: [f32; 2],
    opacity: f32,
    _pad: f32,
}

const COMP_WGSL: &str = r#"
struct C { dst: vec4<f32>, viewport: vec2<f32>, opacity: f32, pad: f32 };
@group(0) @binding(0) var<uniform> c: C;
@group(0) @binding(1) var t: texture_2d<f32>;
@group(0) @binding(2) var s: sampler;
struct V { @builtin(position) p: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
    var k = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0));
    let q = k[i];
    let px = c.dst.xy + q * c.dst.zw;
    var o: V;
    o.p = vec4<f32>(px.x / c.viewport.x * 2.0 - 1.0, 1.0 - px.y / c.viewport.y * 2.0, 0.0, 1.0);
    o.uv = q;
    return o;
}
@fragment fn fs(v: V) -> @location(0) vec4<f32> {
    return textureSample(t, s, v.uv) * c.opacity;
}
"#;

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

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
const COMP_ALIGN: u64 = 256;

/// A grid of `n` rounded, bordered rects over `w` x `h`, at `alpha`.
fn grid(
    n: usize,
    (x0, y0): (f32, f32),
    w: f32,
    h: f32,
    alpha: f32,
    view: (f32, f32),
) -> Vec<Instance> {
    let cols = (n as f32).sqrt().ceil() as usize;
    let rows = n.div_ceil(cols);
    let cw = w / cols as f32;
    let ch = h / rows as f32;
    (0..n)
        .map(|i| {
            let (cx, cy) = ((i % cols) as f32, (i / cols) as f32);
            Instance {
                pos: [x0 + cx * cw, y0 + cy * ch],
                size: [(cw - 2.0).max(1.0), (ch - 2.0).max(1.0)],
                color: [0.2 + 0.6 * (cx / cols as f32), 0.3, 0.7, alpha],
                border_color: [0.9, 0.9, 0.9, alpha],
                params: [0.0, 1.0, 0.0, 0.0],
                clip: [0.0, 0.0, view.0, view.1],
                radii: [4.0; 4],
                ..Default::default()
            }
        })
        .collect()
}

/// A card's content at (x, y): a background and `n - 1` rects that
/// overlap it and each other — what shows seams under a per-quad fade.
fn card(
    n: usize,
    (x, y): (f32, f32),
    (w, h): (f32, f32),
    alpha: f32,
    view: (f32, f32),
) -> Vec<Instance> {
    let mut v = vec![Instance {
        pos: [x, y],
        size: [w, h],
        color: [0.95, 0.95, 0.97, alpha],
        border_color: [0.7, 0.7, 0.7, alpha],
        params: [0.0, 1.0, 0.0, 0.0],
        clip: [0.0, 0.0, view.0, view.1],
        radii: [12.0; 4],
        ..Default::default()
    }];
    v.extend(
        grid(n - 1, (x + 8.0, y + 8.0), w - 16.0, h - 16.0, alpha, view)
            .into_iter()
            .map(|mut q| {
                q.size[0] *= 1.6; // overlap the neighbour
                q
            }),
    );
    v
}

struct Target {
    tex: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
    /// Quad globals for this target's viewport.
    quads: wgpu::BindGroup,
}

struct Rig {
    device: wgpu::Device,
    queue: wgpu::Queue,
    quad_pipe: wgpu::RenderPipeline,
    comp_over: wgpu::RenderPipeline,
    comp_copy: wgpu::RenderPipeline,
    layout0: wgpu::BindGroupLayout,
    comp_layout: wgpu::BindGroupLayout,
    atlas_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    nearest: wgpu::Sampler,
    comp_buf: wgpu::Buffer,
}

impl Rig {
    fn new() -> (Self, f32) {
        let t0 = Instant::now();
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("no adapter");
        let dual = adapter
            .features()
            .contains(wgpu::Features::DUAL_SOURCE_BLENDING);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: if dual {
                wgpu::Features::DUAL_SOURCE_BLENDING
            } else {
                wgpu::Features::empty()
            },
            ..Default::default()
        }))
        .expect("no device");
        let bring_up = t0.elapsed().as_secs_f32() * 1e3;
        eprintln!(
            "adapter: {} ({:?}), dual-source {dual}",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        let quad_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kui"),
            source: wgpu::ShaderSource::Wgsl(
                preprocess_shader(kui_core::gpu::SHADER_TEMPLATE, dual).into(),
            ),
        });
        let comp_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("comp"),
            source: wgpu::ShaderSource::Wgsl(COMP_WGSL.into()),
        });
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let samp_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let layout0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                tex_entry(1),
                samp_entry(2),
                samp_entry(3),
            ],
        });
        let comp_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("comp"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: std::num::NonZeroU64::new(32),
                    },
                    count: None,
                },
                tex_entry(1),
                samp_entry(2),
            ],
        });
        let blend = if dual {
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
        let attrs = wgpu::vertex_attr_array![
            0 => Float32x2, 1 => Float32x2, 2 => Float32x4,
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
            6 => Float32x4, 7 => Float32x4, 8 => Float32x4,
            9 => Float32x4, 10 => Float32x4, 11 => Float32x4,
            12 => Float32x4,
        ];
        let vbuf = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &attrs,
        };
        let quad_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout0)],
            immediate_size: 0,
        });
        let quad_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kui.quads"),
            layout: Some(&quad_layout),
            vertex: wgpu::VertexState {
                module: &quad_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(vbuf)],
            },
            fragment: Some(wgpu::FragmentState {
                module: &quad_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
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
        let comp_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&comp_layout)],
            immediate_size: 0,
        });
        let comp = |blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("comp"),
                layout: Some(&comp_pl),
                vertex: wgpu::VertexState {
                    module: &comp_module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &comp_module,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let comp_over = comp(Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING));
        let comp_copy = comp(None);
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kui.atlas"),
            size: extent((1024, 1024)),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let nearest = device.create_sampler(&Default::default());
        let comp_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("comp"),
            size: COMP_ALIGN * 256,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        (
            Self {
                device,
                queue,
                quad_pipe,
                comp_over,
                comp_copy,
                layout0,
                comp_layout,
                atlas_view,
                sampler,
                nearest,
                comp_buf,
            },
            bring_up,
        )
    }

    fn target(&self, size: (u32, u32), extra: wgpu::TextureUsages) -> Target {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("target"),
            size: extent(size),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | extra,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let globals = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(
            &globals,
            0,
            bytemuck::bytes_of(&Globals {
                viewport: [size.0 as f32, size.1 as f32],
                atlas_size: [1024.0, 1024.0],
                time: 0.0,
                scale: 2.0,
                _pad: [0.0; 2],
            }),
        );
        let quads = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout0,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.nearest),
                },
            ],
        });
        Target {
            tex,
            view,
            size,
            quads,
        }
    }

    /// A composite bind group reading `t`.
    fn comp_bind(&self, t: &Target) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.comp_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.comp_buf,
                        offset: 0,
                        size: std::num::NonZeroU64::new(32),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&t.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    fn set_comps(&self, comps: &[Comp]) {
        for (i, c) in comps.iter().enumerate() {
            self.queue
                .write_buffer(&self.comp_buf, i as u64 * COMP_ALIGN, bytemuck::bytes_of(c));
        }
    }

    fn instances(&self, v: &[Instance]) -> wgpu::Buffer {
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (v.len().max(1) * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&buf, 0, bytemuck::cast_slice(v));
        buf
    }

    fn wait(&self) {
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
    }

    /// (cpu ms to encode + submit one frame, ms a frame with the queue
    /// saturated: best of five runs of sixty frames back to back).
    fn time(&self, f: &dyn Fn(&mut wgpu::CommandEncoder)) -> (f32, f32) {
        for _ in 0..30 {
            let mut enc = self.device.create_command_encoder(&Default::default());
            f(&mut enc);
            self.queue.submit([enc.finish()]);
        }
        self.wait();
        let mut cpu = Vec::new();
        for _ in 0..100 {
            let t0 = Instant::now();
            let mut enc = self.device.create_command_encoder(&Default::default());
            f(&mut enc);
            self.queue.submit([enc.finish()]);
            cpu.push(t0.elapsed().as_secs_f32() * 1e3);
            self.wait();
        }
        cpu.sort_by(|a, b| a.total_cmp(b));
        let mut sat = Vec::new();
        for _ in 0..5 {
            let t0 = Instant::now();
            for _ in 0..60 {
                let mut enc = self.device.create_command_encoder(&Default::default());
                f(&mut enc);
                self.queue.submit([enc.finish()]);
            }
            self.wait();
            sat.push(t0.elapsed().as_secs_f32() * 1e3 / 60.0);
        }
        sat.sort_by(|a, b| a.total_cmp(b));
        (cpu[cpu.len() / 2], sat[0])
    }

    fn read(&self, t: &Target) -> Vec<u8> {
        let (w, h) = t.size;
        let bpr = (w * 4).next_multiple_of(256);
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (bpr * h) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        copy_out(&mut enc, t, &buf, bpr);
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.wait();
        let data = slice.get_mapped_range().unwrap();
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let o = (y * bpr) as usize;
            out.extend_from_slice(&data[o..o + (w * 4) as usize]);
        }
        out
    }
}

fn extent((width, height): (u32, u32)) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

fn copy_out(enc: &mut wgpu::CommandEncoder, t: &Target, buf: &wgpu::Buffer, bpr: u32) {
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &t.tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bpr),
                rows_per_image: Some(t.size.1),
            },
        },
        extent(t.size),
    );
}

fn pass<'e>(
    enc: &'e mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    clear: bool,
) -> wgpu::RenderPass<'e> {
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: if clear {
                    wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.065,
                        b: 0.08,
                        a: 1.0,
                    })
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
    })
}

fn quads(
    r: &Rig,
    p: &mut wgpu::RenderPass<'_>,
    t: &Target,
    buf: &wgpu::Buffer,
    range: std::ops::Range<u32>,
) {
    p.set_pipeline(&r.quad_pipe);
    p.set_bind_group(0, &t.quads, &[]);
    p.set_vertex_buffer(0, buf.slice(..));
    p.draw(0..6, range);
}

fn composite(r: &Rig, p: &mut wgpu::RenderPass<'_>, bind: &wgpu::BindGroup, slot: u32, over: bool) {
    p.set_pipeline(if over { &r.comp_over } else { &r.comp_copy });
    p.set_bind_group(0, bind, &[slot * COMP_ALIGN as u32]);
    p.draw(0..6, 0..1);
}

fn mb(size: (u32, u32)) -> f32 {
    (size.0 * size.1 * 4) as f32 / (1024.0 * 1024.0)
}

const SIZES: [(u32, u32); 3] = [(1920, 1080), (3456, 2234), (5120, 2880)];

/// A: the whole frame drawn into a texture and blitted to the surface,
/// against drawing it to the surface directly — what F129's backdrop
/// path, or any "render the root to a layer" design, adds per frame.
fn whole_frame(r: &Rig) {
    println!("\n## A. whole frame offscreen + blit vs direct (ms/frame, GPU-saturated)\n");
    println!(
        "{:>11} {:>7} {:>8} {:>9} {:>9} {:>9} {:>9}",
        "size", "quads", "MB/tex", "direct", "via_blit", "via_copy", "blit_only"
    );
    for size in SIZES {
        let surf = r.target(size, wgpu::TextureUsages::COPY_DST);
        let off = r.target(
            size,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
        );
        let bind = r.comp_bind(&off);
        let view = (size.0 as f32, size.1 as f32);
        r.set_comps(&[Comp {
            dst: [0.0, 0.0, view.0, view.1],
            viewport: [view.0, view.1],
            opacity: 1.0,
            _pad: 0.0,
        }]);
        for n in [1_000usize, 10_000] {
            let inst = grid(n, (0.0, 0.0), view.0, view.1, 1.0, view);
            let buf = r.instances(&inst);
            let n = n as u32;
            let (_, direct) = r.time(&|enc| {
                let mut p = pass(enc, &surf.view, true);
                quads(r, &mut p, &surf, &buf, 0..n);
            });
            let (_, via) = r.time(&|enc| {
                {
                    let mut p = pass(enc, &off.view, true);
                    quads(r, &mut p, &off, &buf, 0..n);
                }
                let mut p = pass(enc, &surf.view, false);
                composite(r, &mut p, &bind, 0, false);
            });
            let (_, via_copy) = r.time(&|enc| {
                {
                    let mut p = pass(enc, &off.view, true);
                    quads(r, &mut p, &off, &buf, 0..n);
                }
                enc.copy_texture_to_texture(
                    off.tex.as_image_copy(),
                    surf.tex.as_image_copy(),
                    extent(size),
                );
            });
            let (_, blit) = r.time(&|enc| {
                let mut p = pass(enc, &surf.view, false);
                composite(r, &mut p, &bind, 0, false);
            });
            println!(
                "{:>11} {n:>7} {:>8.1} {direct:>9.3} {via:>9.3} {via_copy:>9.3} {blit:>9.3}",
                format!("{}x{}", size.0, size.1),
                mb(size)
            );
        }
    }
}

/// B: group opacity. `layers` cards of `per` overlapping quads each over
/// a 1k-quad page, faded to 50%, three ways:
/// - `per_quad`: kui today, alpha multiplied into every quad, one pass;
/// - `layers_first`: each card into its own texture first, then the page
///   and one composite quad per card in one pass;
/// - `break_main`: the page pass ends at each card, the card is drawn
///   into its texture, and the page resumes with `Load` — what a renderer
///   that meets the layer mid-list does if it does not hoist it (the
///   backdrop path's shape today).
fn layers(r: &Rig) {
    let size = SIZES[1];
    let view = (size.0 as f32, size.1 as f32);
    let (cw, ch) = (640u32, 400u32);
    println!(
        "\n## B. group opacity: {cw}x{ch} px cards over a 1k-quad page at {}x{} (ms/frame)\n",
        size.0, size.1
    );
    println!(
        "{:>6} {:>5} {:>8} | {:>9} {:>9} | {:>9} {:>9} | {:>9} {:>9}",
        "layers", "per", "layerMB", "per_quad", "cpu", "lay_first", "cpu", "break", "cpu"
    );
    let surf = r.target(size, wgpu::TextureUsages::COPY_SRC);
    let page = grid(1000, (0.0, 0.0), view.0, view.1, 1.0, view);
    for (count, per) in [
        (1usize, 100usize),
        (4, 100),
        (16, 100),
        (64, 100),
        (16, 1000),
    ] {
        let at = |i: usize| {
            (
                ((i * 397) % (size.0 - cw) as usize) as f32,
                ((i * 211) % (size.1 - ch) as usize) as f32,
            )
        };
        // per-quad: the page and every card's quads at half alpha, one buffer.
        let mut flat = page.clone();
        for i in 0..count {
            flat.extend(card(per, at(i), (cw as f32, ch as f32), 0.5, view));
        }
        let flat_buf = r.instances(&flat);
        let flat_n = flat.len() as u32;
        // layers: the page alone, and one card at its own origin, opaque.
        let page_buf = r.instances(&page);
        let card_buf = r.instances(&card(
            per,
            (0.0, 0.0),
            (cw as f32, ch as f32),
            1.0,
            (cw as f32, ch as f32),
        ));
        let per_n = per as u32;
        let texes: Vec<Target> = (0..count)
            .map(|_| r.target((cw, ch), wgpu::TextureUsages::TEXTURE_BINDING))
            .collect();
        let binds: Vec<wgpu::BindGroup> = texes.iter().map(|t| r.comp_bind(t)).collect();
        let comps: Vec<Comp> = (0..count)
            .map(|i| {
                let (x, y) = at(i);
                Comp {
                    dst: [x, y, cw as f32, ch as f32],
                    viewport: [view.0, view.1],
                    opacity: 0.5,
                    _pad: 0.0,
                }
            })
            .collect();
        r.set_comps(&comps);

        let (pq_cpu, pq) = r.time(&|enc| {
            let mut p = pass(enc, &surf.view, true);
            quads(r, &mut p, &surf, &flat_buf, 0..flat_n);
        });
        let (lf_cpu, lf) = r.time(&|enc| {
            for t in &texes {
                let mut p = pass(enc, &t.view, true);
                quads(r, &mut p, t, &card_buf, 0..per_n);
            }
            let mut p = pass(enc, &surf.view, true);
            quads(r, &mut p, &surf, &page_buf, 0..1000);
            for (i, b) in binds.iter().enumerate() {
                composite(r, &mut p, b, i as u32, true);
            }
        });
        let (br_cpu, br) = r.time(&|enc| {
            let mut first = true;
            let mut drawn_page = false;
            for (i, t) in texes.iter().enumerate() {
                {
                    let mut p = pass(enc, &surf.view, first);
                    first = false;
                    if !drawn_page {
                        quads(r, &mut p, &surf, &page_buf, 0..1000);
                        drawn_page = true;
                    }
                    if i > 0 {
                        composite(r, &mut p, &binds[i - 1], i as u32 - 1, true);
                    }
                }
                let mut p = pass(enc, &t.view, true);
                quads(r, &mut p, t, &card_buf, 0..per_n);
            }
            let mut p = pass(enc, &surf.view, false);
            composite(r, &mut p, &binds[count - 1], count as u32 - 1, true);
        });
        println!(
            "{count:>6} {per:>5} {:>8.1} | {pq:>9.3} {pq_cpu:>9.3} | {lf:>9.3} {lf_cpu:>9.3} | {br:>9.3} {br_cpu:>9.3}",
            mb((cw, ch)) * count as f32
        );
        if count == 1 && per == 100 {
            // The composite really lands: the card's middle is the card's
            // near-white at half over the dark clear, not the clear.
            let px = r.read(&surf);
            let (x, y) = at(0);
            let o = (((y as u32 + 4) * size.0 + x as u32 + 4) * 4) as usize;
            assert!(
                px[o + 1] > 90,
                "the layer did not composite: {:?}",
                &px[o..o + 4]
            );
        }
    }
}

/// C: a cached layer against redrawing what it holds. A full-viewport
/// document of `n` quads, redrawn every frame, against one composite of
/// a texture that holds it (drawn once, outside the timing) plus a small
/// live overlay — the GPU half of a retained layer. The CPU half (the
/// core not emitting the cached quads) is not measured here.
fn cache(r: &Rig) {
    let size = SIZES[1];
    let view = (size.0 as f32, size.1 as f32);
    println!(
        "\n## C. cached layer vs redraw, full viewport at {}x{} (ms/frame)\n",
        size.0, size.1
    );
    println!(
        "{:>7} {:>9} {:>9} {:>9}",
        "quads", "redraw", "cached", "fill_once"
    );
    let surf = r.target(size, wgpu::TextureUsages::empty());
    let layer = r.target(size, wgpu::TextureUsages::TEXTURE_BINDING);
    let bind = r.comp_bind(&layer);
    r.set_comps(&[Comp {
        dst: [0.0, 0.0, view.0, view.1],
        viewport: [view.0, view.1],
        opacity: 1.0,
        _pad: 0.0,
    }]);
    let overlay = r.instances(&grid(20, (100.0, 100.0), 400.0, 300.0, 1.0, view));
    for n in [1_000usize, 10_000, 50_000, 200_000] {
        let buf = r.instances(&grid(n, (0.0, 0.0), view.0, view.1, 1.0, view));
        let n = n as u32;
        let (_, redraw) = r.time(&|enc| {
            let mut p = pass(enc, &surf.view, true);
            quads(r, &mut p, &surf, &buf, 0..n);
            quads(r, &mut p, &surf, &overlay, 0..20);
        });
        let (_, fill) = r.time(&|enc| {
            let mut p = pass(enc, &layer.view, true);
            quads(r, &mut p, &layer, &buf, 0..n);
        });
        let (_, cached) = r.time(&|enc| {
            let mut p = pass(enc, &surf.view, true);
            composite(r, &mut p, &bind, 0, false);
            quads(r, &mut p, &surf, &overlay, 0..20);
        });
        println!("{n:>7} {redraw:>9.3} {cached:>9.3} {fill:>9.3}");
    }
}

/// D: reading a frame back. One frame on demand (copy, map, wait, and the
/// rows unpadded into a Vec), frames streamed through a ring of three
/// buffers (a capture every frame, as a recorder or a remote view would),
/// and a PNG of the result.
fn readback(r: &Rig) {
    println!("\n## D. readback (ms)\n");
    println!(
        "{:>11} {:>7} {:>10} {:>10} {:>10} {:>11} {:>9} {:>9}",
        "size", "MB", "draw+wait", "+readback", "unpad", "ring/frame", "png_fast", "png_KB"
    );
    for size in SIZES {
        let view = (size.0 as f32, size.1 as f32);
        let t = r.target(size, wgpu::TextureUsages::COPY_SRC);
        let buf = r.instances(&grid(1000, (0.0, 0.0), view.0, view.1, 1.0, view));
        let (w, h) = size;
        let bpr = (w * 4).next_multiple_of(256);
        let draw = |enc: &mut wgpu::CommandEncoder| {
            let mut p = pass(enc, &t.view, true);
            quads(r, &mut p, &t, &buf, 0..1000);
        };
        let rb = r.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (bpr * h) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut only = Vec::new();
        let mut with = Vec::new();
        let mut unpad = Vec::new();
        let mut img = Vec::new();
        for _ in 0..20 {
            let t0 = Instant::now();
            let mut enc = r.device.create_command_encoder(&Default::default());
            draw(&mut enc);
            r.queue.submit([enc.finish()]);
            r.wait();
            only.push(t0.elapsed().as_secs_f32() * 1e3);

            let t0 = Instant::now();
            let mut enc = r.device.create_command_encoder(&Default::default());
            draw(&mut enc);
            copy_out(&mut enc, &t, &rb, bpr);
            r.queue.submit([enc.finish()]);
            let slice = rb.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            r.wait();
            with.push(t0.elapsed().as_secs_f32() * 1e3);
            let t1 = Instant::now();
            {
                let data = slice.get_mapped_range().unwrap();
                img.clear();
                img.reserve((w * h * 4) as usize);
                for y in 0..h {
                    let o = (y * bpr) as usize;
                    img.extend_from_slice(&data[o..o + (w * 4) as usize]);
                }
            }
            rb.unmap();
            unpad.push(t1.elapsed().as_secs_f32() * 1e3);
        }
        // A ring of three: frame i copies into buffer i % 3 and maps it;
        // the CPU takes frame i - 2's pixels, which are long done.
        let ring: Vec<wgpu::Buffer> = (0..3)
            .map(|_| {
                r.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: (bpr * h) as u64,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let ready: Vec<std::sync::Arc<std::sync::atomic::AtomicBool>> =
            (0..3).map(|_| Default::default()).collect();
        let frames = 120;
        let mut sink = 0u64;
        let t0 = Instant::now();
        for i in 0..frames {
            let k = i % 3;
            if i >= 3 {
                // Wait for this slot's last map, take its pixels, unmap.
                while !ready[k].load(std::sync::atomic::Ordering::Acquire) {
                    let _ = r.device.poll(wgpu::PollType::Poll);
                }
                {
                    let data = ring[k].slice(..).get_mapped_range().unwrap();
                    for y in 0..h {
                        let o = (y * bpr) as usize;
                        img[(y * w * 4) as usize..((y + 1) * w * 4) as usize]
                            .copy_from_slice(&data[o..o + (w * 4) as usize]);
                    }
                    sink = sink.wrapping_add(img[0] as u64);
                }
                ring[k].unmap();
                ready[k].store(false, std::sync::atomic::Ordering::Relaxed);
            }
            let mut enc = r.device.create_command_encoder(&Default::default());
            draw(&mut enc);
            copy_out(&mut enc, &t, &ring[k], bpr);
            r.queue.submit([enc.finish()]);
            let flag = ready[k].clone();
            ring[k]
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |res| {
                    if res.is_ok() {
                        flag.store(true, std::sync::atomic::Ordering::Release);
                    }
                });
            let _ = r.device.poll(wgpu::PollType::Poll);
        }
        r.wait();
        let ring_ms = t0.elapsed().as_secs_f32() * 1e3 / frames as f32;
        std::hint::black_box(sink);
        // A PNG of the last frame, the fast setting.
        let t0 = Instant::now();
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, w, h);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.set_compression(png::Compression::Fast);
            let mut wr = e.write_header().unwrap();
            wr.write_image_data(&img).unwrap();
        }
        let png_ms = t0.elapsed().as_secs_f32() * 1e3;
        let med = |mut v: Vec<f32>| {
            v.sort_by(|a, b| a.total_cmp(b));
            v[v.len() / 2]
        };
        println!(
            "{:>11} {:>7.1} {:>10.3} {:>10.3} {:>10.3} {ring_ms:>11.3} {png_ms:>9.1} {:>9}",
            format!("{w}x{h}"),
            mb(size),
            med(only),
            med(with),
            med(unpad),
            out.len() / 1024
        );
    }
}

/// E: making (and dropping) a render target: the texture, its view, and
/// a first clear so the memory is really there.
fn alloc(r: &Rig) {
    println!("\n## E. allocating a target (ms, median of 20)\n");
    println!(
        "{:>11} {:>7} {:>10} {:>12}",
        "size", "MB", "create", "create+clear"
    );
    for size in [(256, 256), (640, 400), SIZES[0], SIZES[1], SIZES[2]] {
        let mut c = Vec::new();
        let mut cc = Vec::new();
        for _ in 0..20 {
            let t0 = Instant::now();
            let t = r.target(size, wgpu::TextureUsages::TEXTURE_BINDING);
            c.push(t0.elapsed().as_secs_f32() * 1e3);
            let mut enc = r.device.create_command_encoder(&Default::default());
            drop(pass(&mut enc, &t.view, true));
            r.queue.submit([enc.finish()]);
            r.wait();
            cc.push(t0.elapsed().as_secs_f32() * 1e3);
        }
        c.sort_by(|a, b| a.total_cmp(b));
        cc.sort_by(|a, b| a.total_cmp(b));
        println!(
            "{:>11} {:>7.2} {:>10.3} {:>12.3}",
            format!("{}x{}", size.0, size.1),
            mb(size),
            c[10],
            cc[10]
        );
    }
}

fn main() {
    let (rig, bring_up) = Rig::new();
    println!("\nheadless device bring-up (instance + adapter + device): {bring_up:.1} ms");
    let only = std::env::var("ONLY").ok();
    let on = |name: &str| only.as_deref().is_none_or(|o| o == name);
    if on("whole") {
        whole_frame(&rig);
    }
    if on("layers") {
        layers(&rig);
    }
    if on("cache") {
        cache(&rig);
    }
    if on("readback") {
        readback(&rig);
    }
    if on("alloc") {
        alloc(&rig);
    }
}
