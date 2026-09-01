//! wgpu backend: one über-pipeline drawing instanced quads (rounded rects,
//! borders, and atlas glyphs unified), so a whole UI is a single draw call.
//! Consumes `kui_core::DisplayList` and mirrors the core's glyph atlas.

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
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    viewport: [f32; 2],
    atlas_size: [f32; 2],
}

fn instance_of(q: &Quad) -> Instance {
    let kind = match q.kind {
        QuadKind::Solid => 0.0,
        QuadKind::GlyphMask => 1.0,
        QuadKind::GlyphColor => 2.0,
        QuadKind::Image => 3.0,
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
        params: [q.radius, q.border_w, kind, 0.0],
        uv: [
            q.uv[0] as f32,
            q.uv[1] as f32,
            q.uv[2] as f32,
            q.uv[3] as f32,
        ],
        clip: [q.clip.x, q.clip.y, q.clip.w, q.clip.h],
    }
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
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
    pub clear_color: wgpu::Color,
}

impl Renderer {
    pub async fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(target)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: vec![],
            // One queued frame: measurably lower input-to-photon latency at
            // the cost of less slack for slow frames.
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kui"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

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

        let instance_attrs = wgpu::vertex_attr_array![
            0 => Float32x2, 1 => Float32x2, 2 => Float32x4,
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
            6 => Float32x4,
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kui.quads"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &instance_attrs,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
        let atlas_tex = create_atlas_texture(&device, atlas_size);
        let bind_group = create_bind_group(&device, &bind_layout, &globals_buf, &atlas_tex);

        let instance_cap = 4096;
        let instance_buf = create_instance_buffer(&device, instance_cap);

        Ok(Self {
            surface,
            device,
            queue,
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
            clear_color: wgpu::Color {
                r: 0.06,
                g: 0.065,
                b: 0.08,
                a: 1.0,
            },
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    fn sync_atlas(&mut self, atlas: &mut GlyphAtlas) {
        if atlas.size != self.atlas_size {
            self.atlas_size = atlas.size;
            self.atlas_tex = create_atlas_texture(&self.device, atlas.size);
            self.bind_group = create_bind_group(
                &self.device,
                &self.bind_layout,
                &self.globals_buf,
                &self.atlas_tex,
            );
            self.atlas_epoch = u64::MAX;
        }
        if atlas.dirty || self.atlas_epoch != atlas.epoch {
            self.queue.write_texture(
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
            self.instance_buf = create_instance_buffer(&self.device, self.instance_cap);
        }
        if !self.instances.is_empty() {
            self.queue
                .write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(&self.instances));
        }
        let globals = Globals {
            viewport: [dl.viewport.w.max(1.0), dl.viewport.h.max(1.0)],
            atlas_size: [self.atlas_size as f32, self.atlas_size as f32],
        };
        self.queue
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

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
            .device
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
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.instance_buf.slice(..));
                pass.draw(0..6, 0..self.instances.len() as u32);
            }
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
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
