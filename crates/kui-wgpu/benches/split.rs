//! What ADR 0015's draw-call split costs: the frame this crate draws as one
//! instanced draw, drawn instead as runs interrupted by N fragment quads,
//! each with its own pipeline and a dynamic-offset uniform.
//!
//! Offscreen, so there is no surface and no vsync to hide behind. The quad
//! pipeline is this crate's: the same `shader.wgsl`, the same descriptor,
//! the same blend. `Instance`, `Globals` and `preprocess_shader` are copied
//! from `lib.rs` because all three are private; if any changes there and not
//! here the numbers stop meaning anything. The size assertions below catch a
//! field added on one side only, which is the drift that has actually
//! happened to both structs (C15: `Instance` 92 to 124 bytes, one field at a
//! time; ADR 0015: `Globals` 16 to 32).
//!
//! Not a divan bench — it drives a GPU and reports three different things,
//! so it prints a table and exits.
//!
//!   cargo bench -p kui-wgpu --bench split              # card-sized fragments
//!   FRAG=small cargo bench -p kui-wgpu --bench split   # 8x8: the split alone
//!   COMPILE=1 cargo bench -p kui-wgpu --bench split    # first-sight cost
//!   cargo bench -p kui-wgpu --bench split -- 1000      # a 1k-quad frame
//!   TEX=1 cargo bench -p kui-wgpu --bench split        # ADR 0025: texture-backed
//!                                                      # images instead of fragments —
//!                                                      # the split is a group-0 swap
//!                                                      # under the same pipeline

use std::time::Instant;

const W: u32 = 2560;
const H: u32 = 1440;

/// Mirrors the private `Instance` in `lib.rs`, field for field.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Default)]
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
}

/// 32 floats. A mismatch here means `lib.rs` moved and this bench did not.
const _: () = assert!(std::mem::size_of::<Instance>() == 128);

/// Mirrors the private `Globals` in `lib.rs`, field for field. The quad
/// pipeline here is built from that crate's `shader.wgsl`, so this is the
/// layout the shader reads and a short buffer fails the draw outright.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    viewport: [f32; 2],
    atlas_size: [f32; 2],
    time: f32,
    scale: f32,
    _pad: [f32; 2],
}

/// 32 bytes, as `globals_layout_matches` in `lib.rs` asserts of the real one.
const _: () = assert!(std::mem::size_of::<Globals>() == 32);

/// Mirrors the private `preprocess_shader` in `lib.rs`.
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

/// The fragment stage a fragment pipeline gets: the shape of what ADR
/// 0015's prelude generates around an app's `fragment` function. The
/// vertex stage is `vs_main` from `shader.wgsl`, so these locations are
/// `VsOut`'s.
const FRAG_WGSL: &str = r#"
struct Params { a: vec4<f32>, b: vec4<f32>, c: vec4<f32>, d: vec4<f32> };
@group(1) @binding(0) var<uniform> params: Params;

@fragment
fn fs_frag(
    @location(0) local: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(6) clip: vec4<f32>,
    @builtin(position) frag_pos: vec4<f32>,
) -> @location(0) vec4<f32> {
    // The app's function: a vertical gradient between two params.
    let t = clamp(local.y / max(size.y, 1.0), 0.0, 1.0);
    let col = mix(params.a, params.b, t);
    // kui's epilogue: the clip coverage every quad gets, then premultiply.
    let p = frag_pos.xy;
    let inx = step(clip.x, p.x) * step(p.x, clip.x + clip.z);
    let iny = step(clip.y, p.y) * step(p.y, clip.y + clip.w);
    let a = col.a * inx * iny;
    return vec4<f32>(col.rgb * a, a);
}
"#;

/// A grid of rounded rects covering the viewport, like `frame_10k_rects`.
fn quads(n: usize) -> Vec<Instance> {
    let cols = (n as f32).sqrt().ceil() as usize;
    let cw = W as f32 / cols as f32;
    let ch = H as f32 / cols as f32;
    (0..n)
        .map(|i| {
            let (cx, cy) = ((i % cols) as f32, (i / cols) as f32);
            Instance {
                pos: [cx * cw, cy * ch],
                size: [cw - 2.0, ch - 2.0],
                color: [0.2 + 0.6 * (cx / cols as f32), 0.3, 0.7, 1.0],
                border_color: [0.9, 0.9, 0.9, 1.0],
                params: [0.0, 1.0, 0.0, 0.0],
                uv: [0.0; 4],
                clip: [0.0, 0.0, W as f32, H as f32],
                radii: [4.0; 4],
                clip_radii: [0.0; 4],
            }
        })
        .collect()
}

/// `FRAG=small` shrinks the fragment boxes to 8x8, so their fill is
/// negligible and what the table shows is the state change alone. The
/// default is a card, where fill is most of what a fragment costs.
fn frag_size() -> (f32, f32) {
    match std::env::var("FRAG").ok().as_deref() {
        Some("small") => (8.0, 8.0),
        Some("full") => (W as f32, H as f32),
        _ => (320.0, 180.0),
    }
}

fn fragment_instance(i: usize) -> Instance {
    let (fw, fh) = frag_size();
    // Under `TEX=1` the split quad is an image (kind 3) showing the whole
    // 1080p texture, so the sampling is paid too.
    let textures = std::env::var("TEX").is_ok();
    Instance {
        pos: [((i * 137) % 2000) as f32, ((i * 219) % 1200) as f32],
        size: [fw, fh],
        color: [1.0; 4],
        params: [0.0, 0.0, if textures { 3.0 } else { 0.0 }, 0.0],
        uv: if textures {
            [0.0, 0.0, 1920.0, 1080.0]
        } else {
            [0.0; 4]
        },
        clip: [0.0, 0.0, W as f32, H as f32],
        radii: [8.0; 4],
        ..Default::default()
    }
}

struct Run {
    range: std::ops::Range<u32>,
    /// `None` = the über-pipeline; `Some(i)` = fragment `i`'s.
    fragment: Option<usize>,
}

/// Spreads `frags` fragment instances evenly through `n_quads` quads —
/// the worst case, since nothing batches — and returns the instance
/// buffer with the runs it draws as.
fn scene(n_quads: usize, frags: usize) -> (Vec<Instance>, Vec<Run>) {
    let qs = quads(n_quads);
    let mut buf = Vec::with_capacity(n_quads + frags);
    let mut runs = Vec::new();
    if frags == 0 {
        buf.extend_from_slice(&qs);
        runs.push(Run {
            range: 0..n_quads as u32,
            fragment: None,
        });
        return (buf, runs);
    }
    let per = n_quads / (frags + 1);
    let mut start = 0u32;
    for f in 0..frags {
        let chunk = &qs[f * per..(f + 1) * per];
        buf.extend_from_slice(chunk);
        runs.push(Run {
            range: start..start + chunk.len() as u32,
            fragment: None,
        });
        start += chunk.len() as u32;
        buf.push(fragment_instance(f));
        runs.push(Run {
            range: start..start + 1,
            fragment: Some(f),
        });
        start += 1;
    }
    let tail = &qs[frags * per..];
    buf.extend_from_slice(tail);
    runs.push(Run {
        range: start..start + tail.len() as u32,
        fragment: None,
    });
    (buf, runs)
}

struct Bench {
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: wgpu::Texture,
    view: wgpu::TextureView,
    quad_module: wgpu::ShaderModule,
    quad_pipeline: wgpu::RenderPipeline,
    frag_pipeline: wgpu::RenderPipeline,
    frag_layout: wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    bind0: wgpu::BindGroup,
    bind1: wgpu::BindGroup,
    /// Group 0 with a 1080p texture in the atlas's place (`TEX=1`).
    bind_tex: wgpu::BindGroup,
    /// Whether a split quad is a texture run (bind swap) rather than a
    /// fragment (pipeline swap).
    textures: bool,
    instance_buf: wgpu::Buffer,
    align: u32,
}

impl Bench {
    fn new() -> Self {
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
        let align = adapter.limits().min_uniform_buffer_offset_alignment;
        eprintln!(
            "adapter: {} ({:?}), dual-source {dual}",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        let format = wgpu::TextureFormat::Bgra8Unorm;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("target"),
            size: wgpu::Extent3d {
                width: W,
                height: H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let quad_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kui"),
            source: wgpu::ShaderSource::Wgsl(
                preprocess_shader(include_str!("../src/shader.wgsl"), dual).into(),
            ),
        });
        let frag_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kui.fragment"),
            source: wgpu::ShaderSource::Wgsl(FRAG_WGSL.into()),
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
        let layout1 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kui.fragment.params"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: std::num::NonZeroU64::new(64),
                },
                count: None,
            }],
        });
        let quad_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kui"),
            bind_group_layouts: &[Some(&layout0)],
            immediate_size: 0,
        });
        let frag_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kui.fragment"),
            bind_group_layouts: &[Some(&layout0), Some(&layout1)],
            immediate_size: 0,
        });

        let attrs = wgpu::vertex_attr_array![
            0 => Float32x2, 1 => Float32x2, 2 => Float32x4,
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
            6 => Float32x4, 7 => Float32x4, 8 => Float32x4,
        ];
        let vbuf = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &attrs,
        };
        let quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kui.quads"),
            layout: Some(&quad_layout),
            vertex: wgpu::VertexState {
                module: &quad_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(vbuf.clone())],
            },
            fragment: Some(wgpu::FragmentState {
                module: &quad_module,
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
        let frag_pipeline = make_fragment_pipeline(
            &device,
            &frag_layout,
            &quad_module,
            &frag_module,
            format,
            &vbuf,
        );

        // Under `TEX=1` the "atlas" of the split runs is the 1080p texture,
        // so the uv divide has to be by its size; the bench has one globals
        // buffer, and the atlas is never sampled in that mode.
        let textures = std::env::var("TEX").is_ok();
        let globals = Globals {
            viewport: [W as f32, H as f32],
            atlas_size: if textures {
                [1920.0, 1080.0]
            } else {
                [1024.0, 1024.0]
            },
            time: 0.0,
            scale: 1.0,
            _pad: [0.0; 2],
        };
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kui.globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&globals_buf, 0, bytemuck::bytes_of(&globals));
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kui.atlas"),
            size: wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let group0 = |view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("kui"),
                layout: &layout0,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: globals_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&nearest),
                    },
                ],
            })
        };
        let bind0 = group0(&atlas_view);
        // A second texture, the size of a 1080p stream, for the `TEX=1`
        // mode: the split there is this bind group in place of `bind0`.
        let stream = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stream"),
            size: wgpu::Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Solid orange, so `verify` can tell the texture was sampled.
        let orange: Vec<u8> = [0xd8u8, 0x86, 0x3b, 0xff].repeat(1920 * 1080);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &stream,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &orange,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1920 * 4),
                rows_per_image: Some(1080),
            },
            wgpu::Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1,
            },
        );
        let stream_view = stream.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_tex = group0(&stream_view);

        // One params slot per fragment, padded to the device's alignment.
        let slots = 128u32;
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kui.fragment.params"),
            size: (align * slots) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let p: [f32; 16] = [
            0.9, 0.3, 0.2, 1.0, 0.2, 0.4, 0.9, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        for i in 0..slots {
            queue.write_buffer(&params_buf, (i * align) as u64, bytemuck::bytes_of(&p));
        }
        let bind1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kui.fragment.params"),
            layout: &layout1,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &params_buf,
                    offset: 0,
                    size: std::num::NonZeroU64::new(64),
                }),
            }],
        });

        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kui.instances"),
            size: (100_000 * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            target,
            view,
            quad_module,
            quad_pipeline,
            frag_pipeline,
            frag_layout,
            format,
            bind0,
            bind1,
            bind_tex,
            textures,
            instance_buf,
            align,
        }
    }

    fn encode(&self, pass: &mut wgpu::RenderPass<'_>, runs: &[Run]) {
        pass.set_vertex_buffer(0, self.instance_buf.slice(..));
        let mut on_quads = false;
        for run in runs {
            match run.fragment {
                None => {
                    if !on_quads {
                        pass.set_pipeline(&self.quad_pipeline);
                        pass.set_bind_group(0, &self.bind0, &[]);
                        on_quads = true;
                    }
                    pass.draw(0..6, run.range.clone());
                }
                Some(_) if self.textures => {
                    // ADR 0025, decision 3: the quad pipeline stays, group
                    // 0 changes to the image's own texture.
                    pass.set_pipeline(&self.quad_pipeline);
                    pass.set_bind_group(0, &self.bind_tex, &[]);
                    on_quads = false;
                    pass.draw(0..6, run.range.clone());
                }
                Some(i) => {
                    pass.set_pipeline(&self.frag_pipeline);
                    pass.set_bind_group(0, &self.bind0, &[]);
                    pass.set_bind_group(1, &self.bind1, &[(i as u32 % 128) * self.align]);
                    on_quads = false;
                    pass.draw(0..6, run.range.clone());
                }
            }
        }
    }

    fn pass<'a>(&'a self, enc: &'a mut wgpu::CommandEncoder) -> wgpu::RenderPass<'a> {
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("kui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.06,
                        g: 0.065,
                        b: 0.08,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    /// One frame. Returns the CPU cost of upload + encode + submit — what
    /// the runner records as `render_ms` — and the same plus waiting for
    /// the GPU to finish that frame.
    fn frame(&self, instances: &[Instance], runs: &[Run]) -> (f32, f32) {
        let t0 = Instant::now();
        self.queue
            .write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(instances));
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("kui") });
        {
            let mut pass = self.pass(&mut enc);
            self.encode(&mut pass, runs);
        }
        self.queue.submit([enc.finish()]);
        let cpu = t0.elapsed().as_secs_f32() * 1e3;
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        (cpu, t0.elapsed().as_secs_f32() * 1e3)
    }

    /// `k` frames submitted back to back and waited on once: ms per frame
    /// with the queue saturated, which is where the GPU's own cost shows.
    fn saturated(&self, instances: &[Instance], runs: &[Run], k: usize) -> f32 {
        self.queue
            .write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(instances));
        let t0 = Instant::now();
        for _ in 0..k {
            let mut enc = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut pass = self.pass(&mut enc);
                self.encode(&mut pass, runs);
            }
            self.queue.submit([enc.finish()]);
        }
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        t0.elapsed().as_secs_f32() * 1e3 / k as f32
    }

    /// Draws one viewport-sized fragment and reads a pixel back, so the
    /// table above is known to be shading something. Without this the
    /// whole bench would happily measure a pipeline that draws nothing.
    fn verify(&self) {
        let inst = Instance {
            size: [W as f32, H as f32],
            clip: [0.0, 0.0, W as f32, H as f32],
            // Under `TEX=1`: the whole orange texture as one image quad.
            color: [1.0; 4],
            params: [0.0, 0.0, if self.textures { 3.0 } else { 0.0 }, 0.0],
            uv: if self.textures {
                [0.0, 0.0, 1920.0, 1080.0]
            } else {
                [0.0; 4]
            },
            ..Default::default()
        };
        self.frame(
            &[inst],
            &[Run {
                range: 0..1,
                fragment: Some(0),
            }],
        );
        let bpr = (W * 4).next_multiple_of(256);
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (bpr * H) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(H),
                },
            },
            wgpu::Extent3d {
                width: W,
                height: H,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let data = slice.get_mapped_range().unwrap();
        let px = |x: u32, y: u32| {
            let o = (y * bpr + x * 4) as usize;
            (data[o + 2], data[o + 1], data[o])
        };
        let (top, bottom) = (px(W / 2, 2), px(W / 2, H - 3));
        if self.textures {
            // Orange, top and bottom: the texture was bound and sampled.
            for p in [top, bottom] {
                assert!(
                    p.0 > 200 && p.1 > 110 && p.1 < 150 && p.2 < 90,
                    "the texture was not sampled: {p:?}"
                );
            }
            eprintln!("verify: the 1080p texture reads {top:?}, orange");
            return;
        }
        assert!(
            top.0 > 200 && top.2 < 90,
            "fragment did not paint the top: {top:?}"
        );
        assert!(
            bottom.2 > 200 && bottom.0 < 90,
            "fragment did not paint the bottom: {bottom:?}"
        );
        eprintln!("verify: gradient runs {top:?} to {bottom:?}, top to bottom");
    }

    /// What a fragment costs the first time it is seen: wgpu parses and
    /// validates the WGSL (naga again, after the core already did at
    /// registration), then the driver compiles it. Eight distinct sources,
    /// so nothing is cached. Run before any drawing, or the first row
    /// absorbs whatever the GPU still owes.
    fn compile_cost(&self) {
        let attrs = wgpu::vertex_attr_array![
            0 => Float32x2, 1 => Float32x2, 2 => Float32x4,
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
            6 => Float32x4, 7 => Float32x4, 8 => Float32x4,
        ];
        let vbuf = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &attrs,
        };
        println!("{:>4} {:>12} {:>14}", "n", "module_ms", "pipeline_ms");
        for i in 0..8 {
            let src = FRAG_WGSL.replace(
                "let t = clamp(local.y",
                &format!("let unique = {i}.0; let t = clamp(local.y"),
            );
            let t0 = Instant::now();
            let module = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(src.into()),
                });
            let t1 = Instant::now();
            let pipe = make_fragment_pipeline(
                &self.device,
                &self.frag_layout,
                &self.quad_module,
                &module,
                self.format,
                &vbuf,
            );
            let t2 = Instant::now();
            println!(
                "{i:>4} {:>12.3} {:>14.3}",
                (t1 - t0).as_secs_f32() * 1e3,
                (t2 - t1).as_secs_f32() * 1e3
            );
            drop(pipe);
        }
    }
}

fn make_fragment_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    vertex: &wgpu::ShaderModule,
    fragment: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    vbuf: &wgpu::VertexBufferLayout<'_>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("kui.fragment"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: vertex,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(vbuf.clone())],
        },
        fragment: Some(wgpu::FragmentState {
            module: fragment,
            entry_point: Some("fs_frag"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                // Premultiplied over: what every fragment blends as.
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
    })
}

fn best_and_median(mut v: Vec<f32>) -> (f32, f32) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[0], v[v.len() / 2])
}

fn main() {
    let bench = Bench::new();
    if std::env::var("COMPILE").is_ok() {
        bench.compile_cost();
        return;
    }
    bench.verify();
    let n_quads: usize = std::env::args()
        .skip(1)
        .find_map(|s| s.parse().ok())
        .unwrap_or(10_000);
    let (fw, fh) = frag_size();
    let what = if bench.textures {
        "texture"
    } else {
        "fragment"
    };
    println!("\n{n_quads} quads, {W}x{H}, offscreen; {what} boxes {fw}x{fh}\n");
    println!(
        "{:>9} {:>7} {:>10} {:>10} {:>12} {:>12} {:>11}",
        "fragments", "draws", "cpu_best", "cpu_med", "frame_best", "frame_med", "saturated"
    );
    for &f in &[0usize, 1, 8, 32, 100] {
        let (instances, runs) = scene(n_quads, f);
        let draws = runs.len();
        for _ in 0..60 {
            bench.frame(&instances, &runs);
        }
        let (mut cpu, mut frame) = (Vec::new(), Vec::new());
        for _ in 0..300 {
            let (c, t) = bench.frame(&instances, &runs);
            cpu.push(c);
            frame.push(t);
        }
        let (cb, cm) = best_and_median(cpu);
        let (fb, fm) = best_and_median(frame);
        let mut sat: Vec<f32> = (0..5)
            .map(|_| bench.saturated(&instances, &runs, 60))
            .collect();
        sat.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{f:>9} {draws:>7} {cb:>10.3} {cm:>10.3} {fb:>12.3} {fm:>12.3} {:>11.3}",
            sat[0]
        );
    }
}
