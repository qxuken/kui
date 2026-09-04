//! Steady-state frame cost: build + layout + emit over a reused Core, the way
//! a real app runs (text caches warm after the first frame).
//!
//! Run: cargo bench -p kui-core

use kui_core::{Color, Core, Key, NodeSpec, Size, Sizing, TextStyle, Ui, Value, Vec2, widgets};

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

fn run_frame(
    core: &mut Core,
    rows: usize,
    cols: usize,
    with_text: bool,
    with_clicks: bool,
) -> usize {
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

/// The same frame with the access tree derived after it — what a frame
/// costs while assistive technology is attached. Every fourth cell is a
/// button (a semantic node); the other rects are elided.
#[divan::bench]
fn frame_10k_rects_with_access_tree(bencher: divan::Bencher) {
    let mut core = Core::new();
    run_frame(&mut core, 100, 100, true, true);
    bencher.bench_local(|| {
        run_frame(&mut core, 100, 100, true, true);
        core.access_tree().nodes.len()
    });
}

#[divan::bench]
fn frame_1k_typical(bencher: divan::Bencher) {
    let mut core = Core::new();
    run_frame(&mut core, 32, 32, true, true);
    bencher.bench_local(|| run_frame(&mut core, 32, 32, true, true));
}

/// The same 10k grid with the paint props that cost extra work: every cell
/// casts a shadow (one more quad each) under a faded root (an alpha
/// multiply over every quad emitted). What group opacity and shadows cost
/// when a frame is made of them.
#[divan::bench]
fn frame_10k_rects_with_shadows_and_opacity(bencher: divan::Bencher) {
    fn frame(core: &mut Core) -> usize {
        let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
        ui.configure_root(NodeSpec::column().fill().pad(8.0).gap(4.0).opacity(0.85));
        for r in 0..100 {
            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)).gap(4.0), |ui| {
                for c in 0..100 {
                    ui.with(
                        NodeSpec::column()
                            .width(Sizing::Grow(1.0))
                            .height(Sizing::Fixed(14.0))
                            .bg(Color::rgb8((r % 255) as u8, (c % 255) as u8, 128))
                            .radius(2.0)
                            .shadow_color(Color::rgba8(0, 0, 0, 96))
                            .shadow_blur(6.0)
                            .shadow_y(2.0),
                        |_| {},
                    );
                }
            });
        }
        ui.finish();
        let (dl, _) = core.output();
        dl.quads.len()
    }
    let mut core = Core::new();
    frame(&mut core);
    bencher.bench_local(|| frame(&mut core));
}

/// The wrapping row at scale: 10k chips of varying width in 100 rows that
/// each break onto several lines. The solver is the hot path, and wrapping
/// adds a break pass plus per-line grow/align walks on top of the flat
/// ones, so this is the shape that would show it — paired with the same
/// tree not wrapping (`frame_10k_chips_unwrapped`) so the difference is
/// the wrapping and not the chips.
fn chips(ui: &mut Ui<'_>, wrap: bool) {
    ui.configure_root(NodeSpec::column().fill().pad(8.0).gap(4.0));
    for r in 0..100 {
        let mut row = NodeSpec::row().width(Sizing::Grow(1.0)).gap(4.0);
        if wrap {
            row = row.wrap().cross_gap(4.0);
        }
        ui.with(row, |ui| {
            for c in 0..100 {
                ui.with(
                    NodeSpec::column()
                        // 40..=110px: several lines per row at 1920 wide.
                        .width(Sizing::Fixed(40.0 + ((r * 100 + c) % 8) as f32 * 10.0))
                        .height(Sizing::Fixed(14.0))
                        .bg(Color::rgb8((r % 255) as u8, (c % 255) as u8, 128))
                        .radius(2.0),
                    |_| {},
                );
            }
        });
    }
}

fn chip_frame(core: &mut Core, wrap: bool) -> usize {
    let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
    chips(&mut ui, wrap);
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

#[divan::bench]
fn frame_10k_chips_wrapped(bencher: divan::Bencher) {
    let mut core = Core::new();
    chip_frame(&mut core, true);
    bencher.bench_local(|| chip_frame(&mut core, true));
}

#[divan::bench]
fn frame_10k_chips_unwrapped(bencher: divan::Bencher) {
    let mut core = Core::new();
    chip_frame(&mut core, false);
    bencher.bench_local(|| chip_frame(&mut core, false));
}

#[divan::bench]
fn deep_nesting_64_levels(bencher: divan::Bencher) {
    fn nest(ui: &mut Ui<'_>, depth: usize) {
        if depth == 0 {
            ui.text("leaf", TextStyle::default());
            return;
        }
        ui.with(
            NodeSpec::column().pad(1.0).bg(Color::rgb8(20, 20, 30)),
            |ui| nest(ui, depth - 1),
        );
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

// -- Long scrolled lists ----------------------------------------------------
// The case C5 is about: a log viewer or data table whose content is far
// taller than its window. `list_naive` is what a view costs today — every row
// built and laid out, whether or not it can be seen. `list_virtual` is the
// same list through `widgets::virtual_column`, which builds the visible rows
// and two spacers. Both scroll to the middle first, so neither is measuring
// the easy case where the top of the list happens to be on screen.

const LIST_ROW_H: f32 = 24.0;

/// One row's content, the same in both benches: a coloured bar and a label,
/// so a row costs what a real row costs.
fn list_row(ui: &mut Ui<'_>, i: usize) {
    ui.with(
        NodeSpec::row()
            .fill()
            .gap(6.0)
            .pad_xy(8.0, 4.0)
            .bg(Color::rgb8(24, 24, 32))
            .on_click(Value::Int(i as i64)),
        |ui| {
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Fixed(4.0))
                    .height(Sizing::Grow(1.0))
                    .bg(Color::rgb8((i % 255) as u8, 90, 140))
                    .radius(2.0),
                |_| {},
            );
            // 64 distinct strings -> realistic warm-cache text load.
            ui.text(&format!("line {}", i % 64), TextStyle::new(12.0));
        },
    );
}

fn list_naive(core: &mut Core, rows: usize) -> usize {
    let mut ui = core.frame(Size::new(1200.0, 800.0), 2.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("list", NodeSpec::column().fill().scroll_y(), |ui| {
        for i in 0..rows {
            ui.with_indexed(
                i as u64,
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(LIST_ROW_H)),
                |ui| list_row(ui, i),
            );
        }
    });
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

fn list_virtual(core: &mut Core, rows: usize) -> usize {
    let mut ui = core.frame(Size::new(1200.0, 800.0), 2.0);
    ui.configure_root(NodeSpec::column().fill());
    widgets::virtual_column(
        &mut ui,
        "list",
        NodeSpec::column().fill(),
        rows,
        LIST_ROW_H,
        list_row,
    );
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

/// Scrolls the list to its middle, where a virtualizing view has rows both
/// above and below the window to skip.
fn scroll_to_middle(core: &mut Core, rows: usize) {
    core.set_scroll(
        Key::ROOT.str("list"),
        Vec2::new(0.0, rows as f32 * LIST_ROW_H / 2.0),
    );
}

#[divan::bench]
fn list_10k_rows_naive(bencher: divan::Bencher) {
    let mut core = Core::new();
    list_naive(&mut core, 10_000);
    scroll_to_middle(&mut core, 10_000);
    list_naive(&mut core, 10_000);
    bencher.bench_local(|| list_naive(&mut core, 10_000));
}

#[divan::bench]
fn list_10k_rows_virtual(bencher: divan::Bencher) {
    let mut core = Core::new();
    list_virtual(&mut core, 10_000);
    scroll_to_middle(&mut core, 10_000);
    list_virtual(&mut core, 10_000);
    bencher.bench_local(|| list_virtual(&mut core, 10_000));
}

/// The same list an order of magnitude longer. The naive frame grows with
/// it; the virtual one should not notice.
#[divan::bench]
fn list_100k_rows_virtual(bencher: divan::Bencher) {
    let mut core = Core::new();
    list_virtual(&mut core, 100_000);
    scroll_to_middle(&mut core, 100_000);
    list_virtual(&mut core, 100_000);
    bencher.bench_local(|| list_virtual(&mut core, 100_000));
}

fn main() {
    divan::main();
}
