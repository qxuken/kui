//! What drawing faded subtrees as pictures costs (backlog V11, ADR
//! 0056): a grid of disabled-looking cards — a fill and a label each, at
//! half opacity — emitted and drawn with groups on and off. The core's
//! side is the frame's emit; the renderer's is `render_to`'s encode and
//! submit on the CPU, and frames back to back on the GPU. Not a divan
//! bench: it prints a table.
//!
//!   cargo bench -p kui-wgpu --bench groups            # 10, 100, 1000 cards
//!   cargo bench -p kui-wgpu --bench groups -- 300

use std::time::Instant;

use kui_core::{Color, Core, NodeSpec, Size, TextStyle};
use kui_wgpu::{Gpu, Renderer, wgpu};

const VIEW: Size = Size {
    w: 1728.0,
    h: 1117.0,
};

fn frame(core: &mut Core, n: usize) {
    let cols = (n as f32).sqrt().ceil() as usize;
    let rows = n.div_ceil(cols);
    let mut ui = core.frame(VIEW, 2.0);
    ui.configure_root(NodeSpec::column().fill().bg(Color::hex(0x14161cff)));
    let (cw, ch) = (VIEW.w / cols as f32, VIEW.h / rows as f32);
    for r in 0..rows {
        ui.with(NodeSpec::row().height(ch), |ui| {
            for _ in 0..cols.min(n - r * cols) {
                ui.with(
                    NodeSpec::column()
                        .size(cw - 6.0, ch - 6.0)
                        .pad(4.0)
                        .radius(6.0)
                        .bg(Color::hex(0x3b5bd4ff))
                        .opacity(0.5),
                    |ui| ui.text("Disabled", TextStyle::new(12.0).color(Color::WHITE)),
                );
            }
        });
    }
    ui.finish();
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() {
    let counts: Vec<usize> = {
        let asked: Vec<usize> = std::env::args()
            .skip(1)
            .filter_map(|s| s.parse().ok())
            .collect();
        if asked.is_empty() {
            vec![10, 100, 1000]
        } else {
            asked
        }
    };
    let gpu = pollster::block_on(Gpu::windowless()).expect("an adapter");
    eprintln!("adapter: {}", gpu.adapter().get_info().name);
    let format = wgpu::TextureFormat::Bgra8Unorm;
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
    let wait = || {
        let _ = gpu.device().poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
    };
    println!(
        "\n{:>6} {:>7} {:>10} {:>10} {:>10}",
        "cards", "groups", "emit_ms", "cpu_ms", "gpu_ms"
    );
    for &n in &counts {
        for groups in [false, true] {
            let mut core = Core::new();
            core.set_opacity_groups(groups);
            let mut r = Renderer::windowless(&gpu, format).expect("a renderer");
            let mut emit = Vec::new();
            for _ in 0..40 {
                let t = Instant::now();
                frame(&mut core, n);
                emit.push(t.elapsed().as_secs_f64() * 1e3);
            }
            let (dl, atlas) = core.output();
            for _ in 0..20 {
                r.render_to(dl, atlas, &target).unwrap();
            }
            wait();
            let mut cpu = Vec::new();
            for _ in 0..100 {
                let t = Instant::now();
                r.render_to(dl, atlas, &target).unwrap();
                cpu.push(t.elapsed().as_secs_f64() * 1e3);
                wait();
            }
            let mut gpu_ms = Vec::new();
            for _ in 0..5 {
                let t = Instant::now();
                for _ in 0..30 {
                    r.render_to(dl, atlas, &target).unwrap();
                }
                wait();
                gpu_ms.push(t.elapsed().as_secs_f64() * 1e3 / 30.0);
            }
            println!(
                "{n:>6} {:>7} {:>10.3} {:>10.3} {:>10.3}",
                if groups { "on" } else { "off" },
                median(emit),
                median(cpu),
                gpu_ms.iter().cloned().fold(f64::MAX, f64::min),
            );
        }
    }
}
