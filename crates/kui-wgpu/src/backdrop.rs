//! A node's backdrop blur (backlog F129): `QuadKind::Backdrop`.
//!
//! A frame with none draws as it always has, straight to the surface. A
//! frame with one draws into an offscreen copy of the surface instead, so
//! what it has drawn can be read back: the render pass ends at each
//! backdrop quad, the region under it (the node, clipped, plus three
//! sigmas around it) is copied out, downsampled, and blurred along each
//! axis in three small passes, and the pass resumes where it stopped,
//! writing the blur back inside the node's rounded rect before it draws
//! on. The frame is then drawn onto the surface in one blit.
//!
//! Only the offscreen frame is the surface's size: it is the whole frame.
//! The scratch is as big as the largest region of a frame and its
//! downsampled image (backlog RG150), grown by half again when a frame
//! needs more, so a sheet sliding open does not remake it every frame.
//! The three scratch passes load what was there rather than clear it, as
//! the shaders never read past the image they are given: on a tiler a
//! cleared and stored attachment writes back every tile, scissor or not.
//!
//! Everything here but the pipelines is made the first time a frame needs
//! it and dropped by the first frame that does not. kui draws no frames
//! while nothing changes, so a frame counted or a clock checked after the
//! last blur goes might never come; the frame that stops drawing the blur
//! always does. Coming back costs a few allocations: 0.1 to 0.2 ms of CPU
//! at 2560 x 1600 on an M-series Mac (measured in RG150).

use kui_core::{Clip, Quad};

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
    pub sharp_size: [f32; 4],
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
            // The scratch's sizes are filled in by `Scratch::params`.
            sizes: [down[0] as f32, down[1] as f32, 0.0, 0.0],
            sharp_size: [0.0; 4],
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

/// What a frame's blurs need of the scratch, each side the largest of
/// any blur's: the region `sharp` holds, and the image `ping` and `pong`
/// blur.
pub(crate) fn need(blurs: &[Blur]) -> Need {
    blurs.iter().fold(
        Need {
            sharp: (1, 1),
            down: (1, 1),
        },
        |n, b| Need {
            sharp: (n.sharp.0.max(b.region[2]), n.sharp.1.max(b.region[3])),
            down: (n.down.0.max(b.down[0]), n.down.1.max(b.down[1])),
        },
    )
}

/// The scratch sizes a frame's blurs need: see [`need`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Need {
    pub sharp: (u32, u32),
    pub down: (u32, u32),
}

/// The size a scratch texture of size `have` is remade at to hold `need`,
/// or `None` when it already does. Each side that is short grows to what
/// is needed or by half again, whichever is more, rounded up to 64 and no
/// larger than `cap` (the surface, which no region exceeds): a region that
/// grows a pixel a frame across a 2560 x 1600 surface remakes it some
/// fifteen times in those 2560 frames, not each one.
pub(crate) fn grow(have: (u32, u32), need: (u32, u32), cap: (u32, u32)) -> Option<(u32, u32)> {
    if need.0 <= have.0 && need.1 <= have.1 {
        return None;
    }
    let side = |have: u32, need: u32, cap: u32| {
        if need <= have {
            have
        } else {
            need.max(have + have / 2)
                .next_multiple_of(64)
                .min(cap)
                .max(need)
        }
    };
    Some((side(have.0, need.0, cap.0), side(have.1, need.1, cap.1)))
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

    /// A bind group of `src` and `sharp` over the parameter buffer.
    fn bind(
        &self,
        device: &wgpu::Device,
        src: &wgpu::TextureView,
        sharp: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kui.backdrop"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.params,
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
                    resource: wgpu::BindingResource::TextureView(sharp),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
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

fn texture(
    device: &wgpu::Device,
    label: &str,
    format: wgpu::TextureFormat,
    (width, height): (u32, u32),
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
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
}

fn view(t: &wgpu::Texture) -> wgpu::TextureView {
    t.create_view(&wgpu::TextureViewDescriptor::default())
}

/// The offscreen frame, at the surface's size — the whole frame is drawn
/// into it — and the bind group that blits it onto the surface.
pub(crate) struct Frame {
    pub size: (u32, u32),
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub blit: wgpu::BindGroup,
}

impl Frame {
    pub fn new(
        device: &wgpu::Device,
        pipes: &Pipes,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> Self {
        use wgpu::TextureUsages as U;
        // COPY_DST for nothing the renderer does: a test writes a picture
        // into it to blur.
        let texture = texture(
            device,
            "kui.backdrop.frame",
            format,
            size,
            U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC | U::COPY_DST,
        );
        let view = view(&texture);
        // The blit reads nothing but `src`; the frame fills the other slot.
        let blit = pipes.bind(device, &view, &view);
        Self {
            size,
            texture,
            view,
            blit,
        }
    }

    /// The bytes it holds.
    #[cfg(test)]
    pub fn bytes(&self) -> u64 {
        u64::from(self.size.0) * u64::from(self.size.1) * 4
    }
}

/// The scratch a blur goes through: `sharp`, the region copied out of the
/// frame at its own origin, and `ping` and `pong`, its downsampled image
/// blurred one axis at a time — each as big as the largest of a frame's
/// regions and images asks (see [`grow`]), not the surface.
pub(crate) struct Scratch {
    /// `sharp`'s size.
    pub sharp_size: (u32, u32),
    /// `ping`'s and `pong`'s.
    pub down_size: (u32, u32),
    pub sharp: wgpu::Texture,
    pub ping: wgpu::TextureView,
    pub pong: wgpu::TextureView,
    pub down: wgpu::BindGroup,
    pub blur_h: wgpu::BindGroup,
    pub blur_v: wgpu::BindGroup,
    pub composite: wgpu::BindGroup,
}

impl Scratch {
    pub fn new(
        device: &wgpu::Device,
        pipes: &Pipes,
        format: wgpu::TextureFormat,
        sharp_size: (u32, u32),
        down_size: (u32, u32),
    ) -> Self {
        use wgpu::TextureUsages as U;
        let sharp = texture(
            device,
            "kui.backdrop.sharp",
            format,
            sharp_size,
            U::TEXTURE_BINDING | U::COPY_DST,
        );
        let ping = texture(
            device,
            "kui.backdrop.ping",
            format,
            down_size,
            U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
        );
        let pong = texture(
            device,
            "kui.backdrop.pong",
            format,
            down_size,
            U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
        );
        let (sharp_view, ping, pong) = (view(&sharp), view(&ping), view(&pong));
        Self {
            sharp_size,
            down_size,
            down: pipes.bind(device, &sharp_view, &sharp_view),
            blur_h: pipes.bind(device, &ping, &sharp_view),
            blur_v: pipes.bind(device, &pong, &sharp_view),
            composite: pipes.bind(device, &ping, &sharp_view),
            sharp,
            ping,
            pong,
        }
    }

    /// `b`'s numbers with this scratch's sizes in: what its slot holds.
    pub fn params(&self, b: &Blur) -> Params {
        let mut p = b.params;
        p.sizes[2] = self.down_size.0 as f32;
        p.sizes[3] = self.down_size.1 as f32;
        p.sharp_size = [self.sharp_size.0 as f32, self.sharp_size.1 as f32, 0.0, 0.0];
        p
    }

    /// The bytes it holds.
    #[cfg(test)]
    pub fn bytes(&self) -> u64 {
        let (s, d) = (self.sharp_size, self.down_size);
        (u64::from(s.0) * u64::from(s.1) + 2 * u64::from(d.0) * u64::from(d.1)) * 4
    }
}

/// What a renderer keeps for its blurs from one frame to the next: the
/// offscreen frame and the scratch, both made by the first frame that
/// blurs and both dropped by the first that does not.
#[derive(Default)]
pub(crate) struct Targets {
    pub frame: Option<Frame>,
    pub scratch: Option<Scratch>,
}

impl Targets {
    /// Makes, grows or drops the frame and the scratch for a frame of
    /// `blurs` over a `size` surface. No blurs drops both there and then:
    /// kui draws nothing while nothing changes, so the frame that stops
    /// drawing a blur may be the last for a long while, and textures kept
    /// for a later one would be kept for good.
    pub fn fit(
        &mut self,
        device: &wgpu::Device,
        pipes: &Pipes,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        blurs: &[Blur],
    ) {
        if blurs.is_empty() {
            *self = Self::default();
            return;
        }
        if self.frame.as_ref().is_none_or(|f| f.size != size) {
            self.frame = Some(Frame::new(device, pipes, format, size));
        }
        let need = need(blurs);
        let have = self
            .scratch
            .as_ref()
            .map_or(((0, 0), (0, 0)), |s| (s.sharp_size, s.down_size));
        let sharp = grow(have.0, need.sharp, size);
        let down = grow(have.1, need.down, size);
        if sharp.is_some() || down.is_some() {
            self.scratch = Some(Scratch::new(
                device,
                pipes,
                format,
                sharp.unwrap_or(have.0),
                down.unwrap_or(have.1),
            ));
        }
    }

    /// Both, when a frame blurs.
    pub fn get(&self) -> Option<(&Frame, &Scratch)> {
        self.frame.as_ref().zip(self.scratch.as_ref())
    }

    /// The bytes they hold.
    #[cfg(test)]
    pub fn bytes(&self) -> u64 {
        self.frame.as_ref().map_or(0, Frame::bytes)
            + self.scratch.as_ref().map_or(0, Scratch::bytes)
    }
}

/// Records one blur into `encoder`, between the pass that drew everything
/// before the quad and the one that resumes after it: the copy out of the
/// frame, then the three steps through the scratch. The pass that resumes
/// starts with [`composite`]. `slot` is the blur's parameter slot, a byte
/// offset.
pub(crate) fn record(
    encoder: &mut wgpu::CommandEncoder,
    pipes: &Pipes,
    frame: &Frame,
    s: &Scratch,
    b: &Blur,
    slot: u32,
) {
    let [rx, ry, rw, rh] = b.region;
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &frame.texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: rx, y: ry, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
            texture: &s.sharp,
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
                pipeline: &wgpu::RenderPipeline,
                bind: &wgpu::BindGroup| {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("kui.backdrop"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                // Loaded, not cleared: each step writes every texel of the
                // image in its corner and reads none outside it — `fs_down`
                // clamps to the region, `gauss` to the image, and the
                // composite's bilinear tap to the image's outer texel
                // centres — so what lies past it is never seen.
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
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
        pass.set_scissor_rect(0, 0, dw, dh);
        pass.draw(0..3, 0..1);
    };
    step(encoder, &s.ping, &pipes.down, &s.down);
    step(encoder, &s.pong, &pipes.blur_h, &s.blur_h);
    step(encoder, &s.ping, &pipes.blur_v, &s.blur_v);
}

/// Writes blur `b`, recorded by [`record`], back into the frame: the first
/// draw of the pass that resumes after its quad, scissored to the node, so
/// the frame is not loaded and stored once more for it alone. The caller
/// sets the scissor back to the whole frame before drawing on.
pub(crate) fn composite(
    pass: &mut wgpu::RenderPass<'_>,
    pipes: &Pipes,
    s: &Scratch,
    b: &Blur,
    slot: u32,
) {
    let [sx, sy, sw, sh] = b.scissor;
    pass.set_pipeline(&pipes.composite);
    pass.set_bind_group(0, &s.composite, &[slot]);
    pass.set_scissor_rect(sx, sy, sw, sh);
    pass.draw(0..3, 0..1);
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
            ..Clip::NONE
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
            ..Clip::NONE
        };
        assert!(
            plan(0, &quad(r, 5.0, 1.0), clip, 100, 100).is_none(),
            "clipped away"
        );
    }

    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    /// A device and the pipelines, or `None`, saying so, where the machine
    /// has no adapter.
    struct Rig {
        device: wgpu::Device,
        queue: wgpu::Queue,
        pipes: Pipes,
        align: u32,
    }

    impl Rig {
        fn new() -> Option<Self> {
            let instance = wgpu::Instance::new(
                wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            );
            let Ok(adapter) = pollster::block_on(
                instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
            ) else {
                eprintln!("no adapter: the backdrop blur is not drawn here");
                return None;
            };
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                    .expect("a device");
            let align = device.limits().min_uniform_buffer_offset_alignment;
            let pipes = Pipes::new(&device, FORMAT, align);
            Some(Self {
                device,
                queue,
                pipes,
                align,
            })
        }

        /// Scratch exactly as big as `blurs` need, not rounded up as the
        /// renderer's is, with every texel of it something no blur wrote.
        fn tight_and_dirty(&self, blurs: &[Blur]) -> Scratch {
            let n = need(blurs);
            let s = Scratch::new(&self.device, &self.pipes, FORMAT, n.sharp, n.down);
            let junk = vec![0xc3u8; (n.sharp.0 * n.sharp.1 * 4) as usize];
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &s.sharp,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &junk,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(n.sharp.0 * 4),
                    rows_per_image: Some(n.sharp.1),
                },
                extent(n.sharp),
            );
            let mut enc = self.device.create_command_encoder(&Default::default());
            for view in [&s.ping, &s.pong] {
                let _ = pass(
                    &mut enc,
                    view,
                    wgpu::LoadOp::Clear(wgpu::Color {
                        r: 1.0,
                        g: 0.0,
                        b: 1.0,
                        a: 1.0,
                    }),
                );
            }
            self.queue.submit([enc.finish()]);
            s
        }

        /// Blurs `blurs` over the picture `px` as the renderer does — each
        /// recorded, then written back at the start of the pass after its
        /// quad — through `scratch`, and reads the frame back.
        fn draw(&self, frame: &Frame, scratch: &Scratch, px: &[u8], blurs: &[Blur]) -> Vec<u8> {
            let (w, h) = frame.size;
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &frame.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                px,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(h),
                },
                extent(frame.size),
            );
            let slot = self.align as usize;
            let mut bytes = vec![0u8; blurs.len() * slot];
            for (i, b) in blurs.iter().enumerate() {
                bytes[i * slot..i * slot + std::mem::size_of::<Params>()]
                    .copy_from_slice(bytemuck::bytes_of(&scratch.params(b)));
            }
            self.queue.write_buffer(&self.pipes.params, 0, &bytes);
            let mut enc = self.device.create_command_encoder(&Default::default());
            for (i, b) in blurs.iter().enumerate() {
                let slot = i as u32 * self.align;
                record(&mut enc, &self.pipes, frame, scratch, b, slot);
                let mut pass = pass(&mut enc, &frame.view, wgpu::LoadOp::Load);
                composite(&mut pass, &self.pipes, scratch, b, slot);
            }
            let bpr = (w * 4).next_multiple_of(256);
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (bpr * h) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            enc.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &frame.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bpr),
                        rows_per_image: Some(h),
                    },
                },
                extent(frame.size),
            );
            self.queue.submit([enc.finish()]);
            let slice = buf.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            let _ = self.device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
            let data = slice.get_mapped_range().expect("mapped");
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

    fn pass<'e>(
        enc: &'e mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        load: wgpu::LoadOp<wgpu::Color>,
    ) -> wgpu::RenderPass<'e> {
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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
        })
    }

    /// The blur itself, on whatever GPU the machine has: a black and white
    /// edge under a node comes out softened inside the node and sharp
    /// around it, and half an opacity is half way. Skipped, saying so,
    /// where there is no adapter.
    #[test]
    fn an_edge_under_the_node_is_softened_and_one_beside_it_is_not() {
        let Some(rig) = Rig::new() else { return };
        const W: u32 = 64;
        const H: u32 = 32;
        let frame = Frame::new(&rig.device, &rig.pipes, FORMAT, (W, H));
        // Black on the left half, white on the right, opaque.
        let mut px = vec![0u8; (W * H * 4) as usize];
        for y in 0..H {
            for x in 0..W {
                let o = ((y * W + x) * 4) as usize;
                let v = if x < W / 2 { 0 } else { 255 };
                px[o..o + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let draw = |opacity: f32| -> Vec<u8> {
            let mut q = quad(Rect::new(8.0, 4.0, 48.0, 24.0), 4.0, opacity);
            q.radius = [0.0; 4];
            let b = plan(0, &q, Clip::NONE, W, H).expect("a blur");
            let scratch = rig.tight_and_dirty(&[b]);
            let out = rig.draw(&frame, &scratch, &px, &[b]);
            out.chunks(4).map(|p| p[0]).collect()
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

    const PW: u32 = 200;
    const PH: u32 = 120;

    /// A busy translucent picture: stripes, a gradient, a checker of noise,
    /// premultiplied, its alpha rising left to right.
    fn picture() -> Vec<u8> {
        let mut px = vec![0u8; (PW * PH * 4) as usize];
        let mut s = 0x9e37_79b9u32;
        for y in 0..PH {
            for x in 0..PW {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                let o = ((y * PW + x) * 4) as usize;
                let a = 128 + ((x * 127) / PW);
                let r = if (x / 7) % 2 == 0 { 255u32 } else { 20 };
                let g = (y * 255) / PH;
                let b = if ((x / 16) + (y / 16)) % 2 == 0 {
                    230u32
                } else {
                    s & 0xff
                };
                let pm = |c: u32| ((c * a) / 255) as u8;
                px[o..o + 4].copy_from_slice(&[pm(r), pm(g), pm(b), a as u8]);
            }
        }
        px
    }

    /// Frames of blurs over [`picture`]: each downsample, a rounded clip,
    /// one past two edges, and two of different sizes in one frame.
    fn scenes() -> Vec<Vec<Blur>> {
        let q = |x, y, w, h, sigma, op, radius: [f32; 4]| {
            let mut q = quad(Rect::new(x, y, w, h), sigma, op);
            q.radius = radius;
            q
        };
        let none = Clip::NONE;
        let frames = vec![
            // Small radius, no downsample.
            vec![(q(20.0, 15.0, 40.0, 30.0, 1.5, 1.0, [6.0; 4]), none)],
            // Halved, inside a rounded clip that cuts it.
            vec![(
                q(100.0, 10.0, 70.0, 50.0, 4.0, 1.0, [0.0; 4]),
                Clip {
                    rect: Rect::new(90.0, 20.0, 60.0, 80.0),
                    radius: [10.0; 4],
                    ..Clip::NONE
                },
            )],
            // Quartered, past the right and bottom edges, off the pixel grid.
            vec![(q(150.5, 70.25, 60.0, 60.0, 10.0, 1.0, [12.0; 4]), none)],
            // A toolbar along the top edge, then a larger sheet over part
            // of it, which reads the toolbar's result.
            vec![
                (q(0.0, 0.0, 200.0, 24.0, 6.0, 1.0, [0.0; 4]), none),
                (q(60.0, 10.0, 80.0, 90.0, 20.0, 0.8, [16.0; 4]), none),
            ],
            // The deepest downsample, across nearly all of it, faded.
            vec![(q(10.0, 10.0, 180.0, 100.0, 40.0, 0.7, [8.0; 4]), none)],
        ];
        frames
            .into_iter()
            .map(|f| {
                f.iter()
                    .enumerate()
                    .map(|(i, (q, c))| plan(i as u32, q, *c, PW, PH).expect("a blur"))
                    .collect()
            })
            .collect()
    }

    /// The scratch sized to the regions, and loaded rather than cleared
    /// (backlog RG150), draws what scratch the surface's size, fresh, does:
    /// with every texel past each blur's region and image dirty, not one
    /// pixel of the frame differs, so no step reads past what it was given.
    #[test]
    fn scratch_the_size_of_the_region_draws_what_scratch_the_size_of_the_surface_does() {
        let Some(rig) = Rig::new() else { return };
        let px = picture();
        let frame = Frame::new(&rig.device, &rig.pipes, FORMAT, (PW, PH));
        for (n, blurs) in scenes().iter().enumerate() {
            let roomy = Scratch::new(&rig.device, &rig.pipes, FORMAT, (PW, PH), (PW, PH));
            let want = rig.draw(&frame, &roomy, &px, blurs);
            let tight = rig.tight_and_dirty(blurs);
            assert!(tight.sharp_size.0 < PW || tight.down_size.0 < PW);
            let got = rig.draw(&frame, &tight, &px, blurs);
            let differ = want.iter().zip(&got).filter(|(a, b)| a != b).count();
            assert_eq!(differ, 0, "frame {n}: {differ} bytes differ");
            assert_ne!(got, px, "frame {n} blurred something");
        }
    }

    /// The frame and the scratch are made by a frame that blurs, kept and
    /// not remade while blurs fit them, grown when one does not, and
    /// dropped by the very next frame without one (backlog RG150): kui
    /// draws no frames while nothing changes, so a release that waited for
    /// more frames would never come.
    #[test]
    fn a_frame_without_a_blur_drops_the_frame_and_the_scratch() {
        let Some(rig) = Rig::new() else { return };
        let size = (2560, 1600);
        let toolbar = |w: f32, h: f32| {
            plan(
                0,
                &quad(Rect::new(400.0, 200.0, w, h), 16.0, 1.0),
                Clip::NONE,
                size.0,
                size.1,
            )
            .expect("a blur")
        };
        let mut t = Targets::default();
        assert!(t.get().is_none() && t.bytes() == 0);
        t.fit(
            &rig.device,
            &rig.pipes,
            FORMAT,
            size,
            &[toolbar(300.0, 60.0)],
        );
        let (frame, scratch) = t.get().expect("made by a frame that blurs");
        assert_eq!(frame.size, size, "the frame is the surface's size");
        // A 300 x 60 toolbar's scratch is under a fortieth of the frame's 16 MB.
        assert!(
            scratch.bytes() * 40 < frame.bytes(),
            "{} against {}",
            scratch.bytes(),
            frame.bytes()
        );
        let sharp = scratch.sharp.clone();
        // A smaller blur fits: nothing is remade.
        t.fit(
            &rig.device,
            &rig.pipes,
            FORMAT,
            size,
            &[toolbar(200.0, 40.0)],
        );
        assert!(t.get().expect("kept").1.sharp == sharp, "not remade");
        // A larger one does not: the scratch grows to hold it.
        let wide = toolbar(1200.0, 60.0);
        t.fit(&rig.device, &rig.pipes, FORMAT, size, &[wide]);
        let (_, grown) = t.get().expect("kept");
        assert!(grown.sharp != sharp, "remade");
        assert!(grown.sharp_size.0 >= wide.region[2] && grown.down_size.0 >= wide.down[0]);
        // The first frame without a blur drops the lot.
        t.fit(&rig.device, &rig.pipes, FORMAT, size, &[]);
        assert!(t.get().is_none() && t.frame.is_none() && t.scratch.is_none());
        assert_eq!(t.bytes(), 0);
    }

    /// A region that grows a pixel a frame — a sheet sliding open — remakes
    /// the scratch some fifteen times in 2560 frames, not every frame, and never smaller
    /// than asked or larger than the surface.
    #[test]
    fn a_growing_region_remakes_the_scratch_now_and_then() {
        let cap = (2560, 1600);
        let mut have = (0, 0);
        let mut remade = 0;
        for w in 1..=cap.0 {
            let need = (w, 1 + w / 3);
            if let Some(next) = grow(have, need, cap) {
                assert!(
                    next.0 >= need.0 && next.1 >= need.1,
                    "{next:?} for {need:?}"
                );
                assert!(next.0 <= cap.0 && next.1 <= cap.1, "{next:?}");
                assert!(next.0 >= have.0 && next.1 >= have.1, "never shrinks");
                have = next;
                remade += 1;
            }
        }
        assert!(remade <= 16, "remade {remade} times");
        assert_eq!(grow((64, 64), (64, 10), cap), None, "it fits");
        assert_eq!(grow((0, 0), (300, 60), cap), Some((320, 64)));
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
