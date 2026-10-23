//! What a shadow costs the renderer since its node is knocked out of it
//! (backlog V10): the real `Renderer`, windowless, drawing a frame of
//! shadowed cards into a texture with `render_to`.
//!
//! Two lists from one core frame: as emitted, each shadow carrying its
//! node's box, and the same quads with that box made empty, which the
//! shader skips — so the gap between them is the knock-out alone, on the
//! CPU (`render_to`'s encode and submit) and on the GPU (frames back to
//! back, waited on once). Not a divan bench: it prints a table.
//!
//!   cargo bench -p kui-wgpu --bench shadows            # 10k cards
//!   cargo bench -p kui-wgpu --bench shadows -- 1000

use std::time::Instant;

use kui_core::{Color, Core, DisplayList, NodeSpec, QuadKind, Shadow, Size};
use kui_wgpu::{Gpu, Renderer, wgpu};

/// A grid of `n` white rounded cards with a soft shadow, filling 1728x1117
/// logical px at 2x (a 3456x2234 frame).
fn frame(core: &mut Core, n: usize) {
    let cols = (n as f32).sqrt().ceil() as usize;
    let (w, h) = (1728.0, 1117.0);
    let mut ui = core.frame(Size::new(w, h), 2.0);
    ui.configure_root(NodeSpec::column().fill().bg(Color::hex(0xf2efe6ff)));
    let (cw, ch) = (w / cols as f32, h / n.div_ceil(cols) as f32);
    for r in 0..n.div_ceil(cols) {
        ui.with(NodeSpec::row().height(ch), |ui| {
            for c in 0..cols.min(n - r * cols) {
                let _ = c;
                ui.leaf(
                    NodeSpec::column()
                        .size(cw - 6.0, ch - 6.0)
                        .bg(Color::hex(0xffffffff))
                        .radius(6.0)
                        .shadow(Shadow {
                            color: Color::hex(0x00000055),
                            dx: 0.0,
                            dy: 2.0,
                            blur: 6.0,
                            spread: 0.0,
                        }),
                );
            }
        });
    }
    ui.finish();
}

fn main() {
    let n: usize = std::env::args()
        .skip(1)
        .find_map(|s| s.parse().ok())
        .unwrap_or(10_000);
    let gpu = pollster::block_on(Gpu::windowless()).expect("an adapter");
    eprintln!("adapter: {}", gpu.adapter().get_info().name);
    let format = wgpu::TextureFormat::Bgra8Unorm;
    let mut r = Renderer::windowless(&gpu, format).expect("a renderer");
    let target = gpu.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d {
            width: 3456,
            height: 2234,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let mut core = Core::new();
    frame(&mut core, n);
    // Two lists of the same frame: as emitted, and with every knock-out
    // made empty — insets the quad's whole size, which the shader reads
    // as no box and skips. Only quads, clips and the viewport are read by
    // a frame with no fragments or textures.
    let copy = |dl: &DisplayList| DisplayList {
        quads: dl.quads.clone(),
        clips: dl.clips.clone(),
        viewport: dl.viewport,
        scale: dl.scale,
        ..Default::default()
    };
    let (dl, atlas) = core.output();
    let list = copy(dl);
    let mut bare = copy(dl);
    for q in bare.quads.iter_mut().filter(|q| q.kind == QuadKind::Shadow) {
        q.uv = [0.0, 0.0, q.rect.w, q.rect.h].map(f32::to_bits);
    }
    let shadows = list.quads.iter().filter(|q| q.kind == QuadKind::Shadow).count();
    println!("\n{shadows} shadows of {} quads, 3456x2234\n", list.quads.len());
    println!("{:>12} {:>10} {:>10} {:>11}", "", "cpu_med", "cpu_best", "gpu_frame");
    for (name, dl) in [("knocked out", &list), ("not", &bare)] {
        for _ in 0..30 {
            r.render_to(dl, atlas, &target).unwrap();
        }
        let wait = || {
            let _ = gpu.device().poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
        };
        wait();
        let mut cpu = Vec::new();
        for _ in 0..200 {
            let t = Instant::now();
            r.render_to(dl, atlas, &target).unwrap();
            cpu.push(t.elapsed().as_secs_f64() * 1e3);
            wait();
        }
        cpu.sort_by(f64::total_cmp);
        let mut sat = Vec::new();
        for _ in 0..5 {
            let t = Instant::now();
            for _ in 0..60 {
                r.render_to(dl, atlas, &target).unwrap();
            }
            wait();
            sat.push(t.elapsed().as_secs_f64() * 1e3 / 60.0);
        }
        sat.sort_by(f64::total_cmp);
        println!(
            "{name:>12} {:>10.3} {:>10.3} {:>11.3}",
            cpu[cpu.len() / 2],
            cpu[0],
            sat[0]
        );
    }
}
