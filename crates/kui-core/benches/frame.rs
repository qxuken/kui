//! Steady-state frame cost: build + layout + emit over a reused Core, the way
//! a real app runs (text caches warm after the first frame).
//!
//! Run: cargo bench -p kui-core

use kui_core::{Color, Core, NodeSpec, Size, Sizing, TextStyle, Ui, Value};

fn grid(ui: &mut Ui<'_>, rows: usize, cols: usize, with_text: bool, with_clicks: bool) {
    ui.configure_root(NodeSpec::column().fill().pad(8.0).gap(4.0));
    for r in 0..rows {
        ui.with(NodeSpec::row().width(Sizing::Grow(1.0)).gap(4.0), |ui| {
            for c in 0..cols {
                let mut spec = NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(14.0))
                    .bg(Color::rgb8((r % 255) as u8, (c % 255) as u8, 128))
                    .radius(2.0);
                if with_clicks && c % 4 == 0 {
                    spec = spec.on_click(Value::Int((r * cols + c) as i64));
                }
                if with_text && c % 8 == 0 {
                    ui.with(spec, |ui| {
                        // 64 distinct strings -> realistic warm-cache text load.
                        let s = format!("cell {}", (r * cols + c) % 64);
                        ui.text(&s, TextStyle::new(10.0));
                    });
                } else {
                    ui.with(spec, |_| {});
                }
            }
        });
    }
}

fn run_frame(core: &mut Core, rows: usize, cols: usize, with_text: bool, with_clicks: bool) -> usize {
    let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
    grid(&mut ui, rows, cols, with_text, with_clicks);
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

#[divan::bench]
fn frame_10k_rects(bencher: divan::Bencher) {
    let mut core = Core::new();
    run_frame(&mut core, 100, 100, false, false);
    bencher.bench_local(|| run_frame(&mut core, 100, 100, false, false));
}

#[divan::bench]
fn frame_10k_rects_with_text_and_hits(bencher: divan::Bencher) {
    let mut core = Core::new();
    run_frame(&mut core, 100, 100, true, true);
    bencher.bench_local(|| run_frame(&mut core, 100, 100, true, true));
}

#[divan::bench]
fn frame_1k_typical(bencher: divan::Bencher) {
    let mut core = Core::new();
    run_frame(&mut core, 32, 32, true, true);
    bencher.bench_local(|| run_frame(&mut core, 32, 32, true, true));
}

#[divan::bench]
fn deep_nesting_64_levels(bencher: divan::Bencher) {
    fn nest(ui: &mut Ui<'_>, depth: usize) {
        if depth == 0 {
            ui.text("leaf", TextStyle::default());
            return;
        }
        ui.with(NodeSpec::column().pad(1.0).bg(Color::rgb8(20, 20, 30)), |ui| nest(ui, depth - 1));
    }
    let mut core = Core::new();
    bencher.bench_local(|| {
        let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
        for _ in 0..16 {
            nest(&mut ui, 64);
        }
        ui.finish();
        let (dl, _) = core.output();
        dl.quads.len()
    });
}

fn main() {
    divan::main();
}
