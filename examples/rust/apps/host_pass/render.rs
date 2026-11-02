use std::sync::Arc;

use kui_core::Core;
use kui_wgpu::{Gpu, RenderError, Renderer, wgpu};
use winit::window::Window;

use crate::game::{Game, WorldRect};

const WORLD_WGSL: &str = r#"
struct Globals { size: vec2<f32>, pad: vec2<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;
struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};
@vertex
fn vs(@builtin(vertex_index) vi: u32, @location(0) r: vec4<f32>, @location(1) color: vec4<f32>) -> Out {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let p = r.xy + corners[vi] * r.zw;
    var out: Out;
    out.pos = vec4<f32>(p.x / globals.size.x * 2.0 - 1.0, 1.0 - p.y / globals.size.y * 2.0, 0.0, 1.0);
    out.color = color;
    return out;
}
@fragment
fn fs(in: Out) -> @location(0) vec4<f32> { return in.color; }
"#;

#[derive(Clone, Copy, Default)]
pub struct FrameTimes {
    pub wait_ms: f32,
    pub render_ms: f32,
}

pub struct Render {
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    ui_renderer: Renderer,
    world_pipeline: wgpu::RenderPipeline,
    world_uniform: wgpu::Buffer,
    world_rects: wgpu::Buffer,
    world_bind: wgpu::BindGroup,
}

impl Render {
    pub fn new(
        window: Arc<Window>,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (gpu, surface) = pollster::block_on(Gpu::new(window))?;
        let caps = surface.get_capabilities(gpu.adapter());
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or("the surface has no texture format")?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(gpu.device(), &config);

        let ui_renderer = Renderer::windowless(&gpu, format)?;
        let (world_pipeline, world_uniform, world_rects, world_bind) =
            world_resources(gpu.device(), format);
        Ok(Self {
            gpu,
            surface,
            config,
            ui_renderer,
            world_pipeline,
            world_uniform,
            world_rects,
            world_bind,
        })
    }

    pub fn subpixel_text(&self) -> bool {
        self.ui_renderer.subpixel_text()
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(self.gpu.device(), &self.config);
    }

    pub fn frame(
        &mut self,
        core: &mut Core,
        game: &Game,
        width: f32,
        height: f32,
    ) -> Result<FrameTimes, String> {
        let acquire_started = std::time::Instant::now();
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(self.gpu.device(), &self.config);
                return Ok(FrameTimes::default());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(FrameTimes::default());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("surface validation failed".to_string());
            }
        };
        let wait_ms = acquire_started.elapsed().as_secs_f32() * 1000.0;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.gpu
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("game frame"),
                });
        let render_started = std::time::Instant::now();
        let rects = game.rects();
        let globals = [width, height, 0.0, 0.0];
        self.gpu
            .queue()
            .write_buffer(&self.world_uniform, 0, bytemuck::cast_slice(&globals));
        self.gpu
            .queue()
            .write_buffer(&self.world_rects, 0, bytemuck::cast_slice(&rects));
        {
            let color = wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.05,
                        g: 0.07,
                        b: 0.12,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world"),
                color_attachments: &[Some(color)],
                ..Default::default()
            });
            pass.set_pipeline(&self.world_pipeline);
            pass.set_bind_group(0, &self.world_bind, &[]);
            pass.set_vertex_buffer(0, self.world_rects.slice(..));
            pass.draw(0..6, 0..rects.len() as u32);
        }
        let (list, atlas) = core.output();
        self.ui_renderer
            .render_into(list, atlas, &mut encoder, &view, wgpu::LoadOp::Load)
            .map_err(|error| match error {
                RenderError::DeviceLost => "device lost".to_string(),
                other => format!("Kui render failed: {other:?}"),
            })?;
        self.ui_renderer.submit(encoder.finish());
        self.gpu.queue().present(frame);
        let render_ms = render_started.elapsed().as_secs_f32() * 1000.0;
        Ok(FrameTimes { wait_ms, render_ms })
    }
}

fn world_resources(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> (
    wgpu::RenderPipeline,
    wgpu::Buffer,
    wgpu::Buffer,
    wgpu::BindGroup,
) {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("world shader"),
        source: wgpu::ShaderSource::Wgsl(WORLD_WGSL.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("world pipeline"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<WorldRect>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 16,
                        shader_location: 1,
                    },
                ],
            })],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let world_uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("world globals"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let world_rects = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("world rectangles"),
        size: (256 * size_of::<WorldRect>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let entry = wgpu::BindGroupEntry {
        binding: 0,
        resource: world_uniform.as_entire_binding(),
    };
    let world_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("world globals"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[entry],
    });
    (pipeline, world_uniform, world_rects, world_bind)
}
