//! A node's backdrop blur (backlog F129): `QuadKind::Backdrop`.
//!
//! A frame with none draws as it always has, straight to the surface. A
//! frame with one draws into an offscreen copy of the surface instead, so
//! what it has drawn can be read back: the render pass ends at each
//! backdrop quad, the region under it (the node, clipped, plus three
//! sigmas around it) is copied out, downsampled, blurred along each axis
//! and written back inside the node's rounded rect, and the pass resumes
//! where it stopped. The frame is then drawn onto the surface in one blit.
//! Everything here is made the first time a frame needs it, kept while
//! frames go on needing it, and dropped after [`IDLE_FRAMES`] without one.

use kui_core::{Clip, Quad};

/// How many frames without a backdrop blur keep the offscreen textures
/// alive: a second at 120 Hz, so a blur that comes and goes with a panel
/// does not reallocate each time, and a window that had one once does not
/// hold four screens of memory for the rest of its life.
pub(crate) const IDLE_FRAMES: u32 = 120;

/// The deepest downsample: past it the blurred image is too coarse for a
/// bilinear upsample to hide.
const MAX_DOWN: u32 = 16;

/// The kernel's half width in taps, at most: three sigmas of the largest
/// sigma left after downsampling, with room.
const MAX_TAPS: f32 = 24.0;

/// One backdrop quad's numbers, as `backdrop.wgsl`'s `Params` reads them.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Params {
    pub region: [f32; 4],
    pub rect: [f32; 4],
    pub radii: [f32; 4],
    pub clip: [f32; 4],
    pub clip_radii: [f32; 4],
    pub blur: [f32; 4],
    pub sizes: [f32; 4],
}

/// One blur of a frame: its numbers, and the whole pixels the steps read
/// and write.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Blur {
    /// The quad's index in the display list: where the pass breaks.
    pub quad: u32,
    pub params: Params,
    /// The region read back, target px: x, y, w, h.
    pub region: [u32; 4],
    /// The blurred image's size in texels.
    pub down: [u32; 2],
    /// Where the composite writes: the node, clipped, target px.
    pub scissor: [u32; 4],
}

/// Plans the blur of quad `index` over a `width` x `height` target: `None`
/// when it would change nothing — no radius, nothing visible of the node
/// inside its clip and the target, or an opacity of zero.
pub(crate) fn plan(index: u32, q: &Quad, clip: Clip, width: u32, height: u32) -> Option<Blur> {
    let sigma = q.blur;
    let opacity = q.color.a;
    if sigma.is_nan() || sigma <= 0.0 || opacity.is_nan() || opacity <= 0.0 {
        return None;
    }
    let (w, h) = (width as f32, height as f32);
    // The node inside its clip and the target: all the composite writes.
    let x0 = q.rect.x.max(clip.rect.x).max(0.0);
    let y0 = q.rect.y.max(clip.rect.y).max(0.0);
    let x1 = (q.rect.x + q.rect.w).min(clip.rect.x + clip.rect.w).min(w);
    let y1 = (q.rect.y + q.rect.h).min(clip.rect.y + clip.rect.h).min(h);
    if !(x1 > x0 && y1 > y0) {
        return None;
    }
    let scissor = [
        x0.floor() as u32,
        y0.floor() as u32,
        (x1.ceil() as u32).min(width),
        (y1.ceil() as u32).min(height),
    ];
    // Three sigmas around it: what the blur at its edge pulls in.
    let margin = (sigma * 3.0).ceil();
    let region = [
        (x0 - margin).max(0.0).floor() as u32,
        (y0 - margin).max(0.0).floor() as u32,
        ((x1 + margin).min(w).ceil() as u32).min(width),
        ((y1 + margin).min(h).ceil() as u32).min(height),
    ];
    let (rw, rh) = (region[2] - region[0], region[3] - region[1]);
    if rw == 0 || rh == 0 {
        return None;
    }
    // Downsample by the largest power of two under sigma / 2, so the
    // kernel left is two to four texels wide whatever the radius; the box
    // mean and the bilinear upsample add some blur of their own, taken off
    // the sigma that is left.
    let mut d = 1u32;
    while d * 2 <= MAX_DOWN && (d * 2) as f32 <= sigma / 2.0 {
        d *= 2;
    }
    let df = d as f32;
    let left = (sigma * sigma - df * df / 6.0).max(0.0).sqrt() / df;
    let s = left.max(0.5);
    let taps = (s * 3.0).ceil().min(MAX_TAPS);
    let down = [rw.div_ceil(d), rh.div_ceil(d)];
    Some(Blur {
        quad: index,
        params: Params {
            region: [region[0] as f32, region[1] as f32, rw as f32, rh as f32],
            rect: [q.rect.x, q.rect.y, q.rect.w, q.rect.h],
            radii: q.radius,
            clip: [clip.rect.x, clip.rect.y, clip.rect.w, clip.rect.h],
            clip_radii: clip.radius,
            blur: [df, s, opacity.min(1.0), taps],
            // The scratch size is filled in by the renderer, which owns it.
            sizes: [down[0] as f32, down[1] as f32, 0.0, 0.0],
        },
        region: [region[0], region[1], rw, rh],
        down,
        scissor: [
            scissor[0],
            scissor[1],
            scissor[2] - scissor[0],
            scissor[3] - scissor[1],
        ],
    })
}

/// The pipelines and the parameter buffer: made once per renderer, the
/// first time a frame blurs, since they depend on nothing but the format.
pub(crate) struct Pipes {
    pub layout: wgpu::BindGroupLayout,
    pub down: wgpu::RenderPipeline,
    pub blur_h: wgpu::RenderPipeline,
    pub blur_v: wgpu::RenderPipeline,
    pub composite: wgpu::RenderPipeline,
    pub blit: wgpu::RenderPipeline,
    pub sampler: wgpu::Sampler,
    pub params: wgpu::Buffer,
    pub params_cap: usize,
}

impl Pipes {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, align: u32) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kui.backdrop"),
            source: wgpu::ShaderSource::Wgsl(include_str!("backdrop.wgsl").into()),
        });
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kui.backdrop"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: std::num::NonZeroU64::new(
                            std::mem::size_of::<Params>() as u64
                        ),
                    },
                    count: None,
                },
                texture(1),
                texture(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kui.backdrop"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipe = |entry: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("kui.backdrop"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    // Blending off throughout: each step writes what it
                    // means, the composite included, which mixes the sharp
                    // pixel itself.
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
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
        let params_cap = 4;
        Self {
            down: pipe("fs_down"),
            blur_h: pipe("fs_blur_h"),
            blur_v: pipe("fs_blur_v"),
            composite: pipe("fs_composite"),
            blit: pipe("fs_blit"),
            layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("kui.backdrop"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            params: params_buffer(device, params_cap, align),
            params_cap,
        }
    }
}

pub(crate) fn params_buffer(device: &wgpu::Device, cap: usize, align: u32) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("kui.backdrop.params"),
        size: (cap * align as usize) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// The offscreen frame and the three scratch textures, at the surface's
/// size, with the bind group of each step.
pub(crate) struct Targets {
    pub size: (u32, u32),
    pub frame: wgpu::TextureView,
    pub down: wgpu::BindGroup,
    pub blur_h: wgpu::BindGroup,
    pub blur_v: wgpu::BindGroup,
    pub composite: wgpu::BindGroup,
    pub blit: wgpu::BindGroup,
    pub sharp: wgpu::Texture,
    pub ping: wgpu::TextureView,
    pub pong: wgpu::TextureView,
    /// Kept for the views above.
    pub frame_texture: wgpu::Texture,
}

impl Targets {
    pub fn new(
        device: &wgpu::Device,
        pipes: &Pipes,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let make = |label, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        use wgpu::TextureUsages as U;
        // COPY_DST for nothing the renderer does: a test writes a picture
        // into it to blur.
        let frame_texture = make(
            "kui.backdrop.frame",
            U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC | U::COPY_DST,
        );
        let sharp = make("kui.backdrop.sharp", U::TEXTURE_BINDING | U::COPY_DST);
        let ping = make(
            "kui.backdrop.ping",
            U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
        );
        let pong = make(
            "kui.backdrop.pong",
            U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
        );
        let view = |t: &wgpu::Texture| t.create_view(&wgpu::TextureViewDescriptor::default());
        let (frame, sharp_view, ping, pong) =
            (view(&frame_texture), view(&sharp), view(&ping), view(&pong));
        let bind = |src: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("kui.backdrop"),
                layout: &pipes.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &pipes.params,
                            offset: 0,
                            size: std::num::NonZeroU64::new(std::mem::size_of::<Params>() as u64),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(src),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&sharp_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&pipes.sampler),
                    },
                ],
            })
        };
        Self {
            size: (width, height),
            down: bind(&sharp_view),
            blur_h: bind(&ping),
            blur_v: bind(&pong),
            composite: bind(&ping),
            blit: bind(&frame),
            frame,
            sharp,
            ping,
            pong,
            frame_texture,
        }
    }
}

/// Records one blur into `encoder`, between the pass that drew everything
/// before the quad and the one that resumes after it: the copy, the three
/// steps into the scratch textures, and the composite into the frame.
/// `slot` is the blur's parameter slot, a byte offset.
pub(crate) fn record(
    encoder: &mut wgpu::CommandEncoder,
    pipes: &Pipes,
    t: &Targets,
    b: &Blur,
    slot: u32,
) {
    let [rx, ry, rw, rh] = b.region;
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &t.frame_texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: rx, y: ry, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
            texture: &t.sharp,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
            width: rw,
            height: rh,
            depth_or_array_layers: 1,
        },
    );
    let [dw, dh] = b.down;
    let step = |encoder: &mut wgpu::CommandEncoder,
                target: &wgpu::TextureView,
                load: wgpu::LoadOp<wgpu::Color>,
                pipeline: &wgpu::RenderPipeline,
                bind: &wgpu::BindGroup,
                (x, y, w, h): (u32, u32, u32, u32)| {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("kui.backdrop"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind, &[slot]);
        pass.set_scissor_rect(x, y, w, h);
        pass.draw(0..3, 0..1);
    };
    // The scratch textures are cleared, which is free on a tiler and
    // costs nothing that matters elsewhere; only their corner is drawn.
    let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
    step(
        encoder,
        &t.ping,
        clear,
        &pipes.down,
        &t.down,
        (0, 0, dw, dh),
    );
    step(
        encoder,
        &t.pong,
        clear,
        &pipes.blur_h,
        &t.blur_h,
        (0, 0, dw, dh),
    );
    step(
        encoder,
        &t.ping,
        clear,
        &pipes.blur_v,
        &t.blur_v,
        (0, 0, dw, dh),
    );
    let [sx, sy, sw, sh] = b.scissor;
    step(
        encoder,
        &t.frame,
        wgpu::LoadOp::Load,
        &pipes.composite,
        &t.composite,
        (sx, sy, sw, sh),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use kui_core::{Color, QuadKind, Rect};

    fn quad(rect: Rect, blur: f32, opacity: f32) -> Quad {
        Quad {
            rect,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: opacity,
            },
            border_color: Color::TRANSPARENT,
            radius: [8.0; 4],
            border_w: 0.0,
            blur,
            kind: QuadKind::Backdrop,
            clip: 0,
            uv: [0; 4],
        }
    }

    #[test]
    fn the_region_is_the_node_and_three_sigmas_around_it_inside_the_target() {
        let b = plan(
            3,
            &quad(Rect::new(100.0, 50.0, 200.0, 40.0), 10.0, 1.0),
            Clip::NONE,
            800,
            600,
        )
        .expect("a blur");
        assert_eq!(b.quad, 3);
        assert_eq!(b.scissor, [100, 50, 200, 40]);
        assert_eq!(b.region, [70, 20, 260, 100]);
        // Sigma 10 downsamples by four, leaving about 2.5 texels.
        assert_eq!(b.params.blur[0], 4.0);
        assert!(
            (b.params.blur[1] - 2.47).abs() < 0.05,
            "{:?}",
            b.params.blur
        );
        assert_eq!(b.down, [65, 25]);
        // At the target's corner the margin is cut off by it.
        let c = plan(
            0,
            &quad(Rect::new(0.0, 0.0, 50.0, 50.0), 10.0, 1.0),
            Clip::NONE,
            800,
            600,
        )
        .expect("a blur");
        assert_eq!(c.region, [0, 0, 80, 80]);
    }

    #[test]
    fn a_clip_narrows_what_is_written_and_what_is_read() {
        let clip = Clip {
            rect: Rect::new(0.0, 0.0, 150.0, 600.0),
            radius: [0.0; 4],
        };
        let b = plan(
            0,
            &quad(Rect::new(100.0, 50.0, 200.0, 40.0), 4.0, 1.0),
            clip,
            800,
            600,
        )
        .expect("a blur");
        assert_eq!(b.scissor, [100, 50, 50, 40]);
        assert_eq!(b.region, [88, 38, 74, 64]);
        assert_eq!(b.params.blur[0], 2.0, "sigma 4 halves");
    }

    #[test]
    fn nothing_to_blur_is_no_blur() {
        let r = Rect::new(10.0, 10.0, 20.0, 20.0);
        assert!(
            plan(0, &quad(r, 0.0, 1.0), Clip::NONE, 100, 100).is_none(),
            "no radius"
        );
        assert!(
            plan(0, &quad(r, 5.0, 0.0), Clip::NONE, 100, 100).is_none(),
            "faded out"
        );
        assert!(plan(0, &quad(r, f32::NAN, 1.0), Clip::NONE, 100, 100).is_none());
        let off = Rect::new(200.0, 10.0, 20.0, 20.0);
        assert!(
            plan(0, &quad(off, 5.0, 1.0), Clip::NONE, 100, 100).is_none(),
            "off target"
        );
        let clip = Clip {
            rect: Rect::new(50.0, 50.0, 10.0, 10.0),
            radius: [0.0; 4],
        };
        assert!(
            plan(0, &quad(r, 5.0, 1.0), clip, 100, 100).is_none(),
            "clipped away"
        );
    }

    /// The blur itself, on whatever GPU the machine has: a black and white
    /// edge under a node comes out softened inside the node and sharp
    /// around it, and half an opacity is half way. Skipped, saying so,
    /// where there is no adapter.
    #[test]
    fn an_edge_under_the_node_is_softened_and_one_beside_it_is_not() {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        else {
            eprintln!("no adapter: the backdrop blur is not drawn here");
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("a device");
        const W: u32 = 64;
        const H: u32 = 32;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let align = device.limits().min_uniform_buffer_offset_alignment;
        let pipes = Pipes::new(&device, format, align);
        let targets = Targets::new(&device, &pipes, format, W, H);
        // Black on the left half, white on the right, opaque.
        let mut px = vec![0u8; (W * H * 4) as usize];
        for y in 0..H {
            for x in 0..W {
                let o = ((y * W + x) * 4) as usize;
                let v = if x < W / 2 { 0 } else { 255 };
                px[o..o + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let size = wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        };
        let draw = |opacity: f32| -> Vec<u8> {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &targets.frame_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &px,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(W * 4),
                    rows_per_image: Some(H),
                },
                size,
            );
            let mut q = quad(Rect::new(8.0, 4.0, 48.0, 24.0), 4.0, opacity);
            q.radius = [0.0; 4];
            let b = plan(0, &q, Clip::NONE, W, H).expect("a blur");
            let mut p = b.params;
            p.sizes[2] = W as f32;
            p.sizes[3] = H as f32;
            queue.write_buffer(&pipes.params, 0, bytemuck::bytes_of(&p));
            let mut enc = device.create_command_encoder(&Default::default());
            record(&mut enc, &pipes, &targets, &b, 0);
            let bpr = (W * 4).next_multiple_of(256);
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (bpr * H) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            enc.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &targets.frame_texture,
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
                size,
            );
            queue.submit([enc.finish()]);
            let slice = buf.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
            let data = slice.get_mapped_range().expect("mapped");
            let mut out = Vec::with_capacity((W * H) as usize);
            for y in 0..H {
                for x in 0..W {
                    out.push(data[(y * bpr + x * 4) as usize]);
                }
            }
            out
        };
        let full = draw(1.0);
        let at = |img: &[u8], x: u32, y: u32| img[(y * W + x) as usize];
        // Inside the node, at the edge: grey either side of it.
        let (l, r) = (at(&full, 31, 16), at(&full, 32, 16));
        assert!(
            (60..=200).contains(&l) && (60..=200).contains(&r),
            "{l} {r}"
        );
        assert!(l < r, "still darker on the dark side: {l} {r}");
        // Inside the node, far from the edge: what was there.
        assert!(at(&full, 12, 16) < 8 && at(&full, 52, 16) > 247);
        // Beside the node, above it: the edge as sharp as it was.
        assert_eq!((at(&full, 31, 1), at(&full, 32, 1)), (0, 255));
        assert_eq!((at(&full, 31, 30), at(&full, 32, 30)), (0, 255));
        // Half the opacity is half way from the sharp pixel to the blurred.
        let half = draw(0.5);
        let mid = at(&half, 32, 16) as i32;
        let want = (255 + r as i32) / 2;
        assert!((mid - want).abs() <= 3, "{mid} against {want}");
    }

    #[test]
    fn a_small_radius_is_not_downsampled_and_a_huge_one_stops_at_sixteen() {
        let r = Rect::new(10.0, 10.0, 20.0, 20.0);
        let small = plan(0, &quad(r, 2.0, 1.0), Clip::NONE, 1000, 1000).unwrap();
        assert_eq!(small.params.blur[0], 1.0);
        let huge = plan(0, &quad(r, 400.0, 1.0), Clip::NONE, 4000, 4000).unwrap();
        assert_eq!(huge.params.blur[0], 16.0);
        assert!(huge.params.blur[3] <= MAX_TAPS);
    }
}
