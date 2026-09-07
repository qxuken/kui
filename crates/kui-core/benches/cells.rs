//! The cell grid at a terminal's size (backlog C20): 200 × 50 cells as one
//! node. The gate the entry set: under 0.2 ms warm, and flat under
//! streaming, against ~1.8 ms for the same cells as one text node each.
//!
//! Run: cargo bench -p kui-core --bench cells

use kui_core::cells::{Cell, CellGrid, flags};
use kui_core::{Color, Core, NodeSpec, Size, Sizing, TextStyle};

const ROWS: usize = 50;
const COLS: usize = 200;

fn mono() -> TextStyle {
    TextStyle::new(13.0).mono().line_height(18.0)
}

fn rng(seed: u64) -> impl FnMut() -> u64 {
    let mut x = seed | 1;
    move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    }
}

/// A screen of printable ASCII with a few colours and a bold run per row.
fn screen(salt: u64) -> Vec<Cell> {
    let mut r = rng(0x9E37_79B9_7F4A_7C15 ^ salt.wrapping_mul(0xD1B5_4A32_D192_ED03));
    let mut cells = Vec::with_capacity(ROWS * COLS);
    for row in 0..ROWS {
        for col in 0..COLS {
            let x = r();
            let ch = (b' ' + (x % 95) as u8) as char;
            let fg = [0xd6d8e0ff, 0x8a8fa3ff, 0x73d98cff, 0xc78fe8ff][(x >> 8) as usize % 4];
            let bg = if col % 40 < 3 { 0x1a1d27ff } else { 0 };
            let mut c = Cell::new(ch, fg, bg);
            if (row + col) % 17 == 0 {
                c = c.with(flags::BOLD);
            }
            cells.push(c);
        }
    }
    cells
}

fn frame(core: &mut Core, cells: &[Cell]) -> usize {
    let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
    ui.configure_root(NodeSpec::column().fill().bg(Color::rgb8(0, 0, 0)));
    ui.cells(
        &CellGrid {
            rows: ROWS,
            cols: COLS,
            cells,
            style: mono(),
            cursor: Some((
                10,
                20,
                kui_core::CellCursor::Block,
                Color::rgb8(0x6a, 0x8b, 0xff),
            )),
        },
        NodeSpec::default(),
    );
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

/// The same screen every frame.
#[divan::bench]
fn cells_200x50_warm(bencher: divan::Bencher) {
    let mut core = Core::new();
    let cells = screen(0);
    frame(&mut core, &cells);
    bencher.bench_local(|| frame(&mut core, &cells));
}

/// A new screen every frame: every cell's character changed.
#[divan::bench]
fn cells_200x50_streaming(bencher: divan::Bencher) {
    let mut core = Core::new();
    let screens: Vec<Vec<Cell>> = (0..8).map(screen).collect();
    for s in &screens {
        frame(&mut core, s);
    }
    let mut n = 0usize;
    bencher.bench_local(|| {
        n += 1;
        frame(&mut core, &screens[n % screens.len()])
    });
}

/// The same 10k cells as one text node each — the path an app has today.
#[divan::bench]
fn cells_200x50_as_text_nodes(bencher: divan::Bencher) {
    let mut core = Core::new();
    let cells = screen(0);
    let run = |core: &mut Core| {
        let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
        ui.configure_root(NodeSpec::column().fill().bg(Color::rgb8(0, 0, 0)));
        let mut buf = [0u8; 4];
        for row in 0..ROWS {
            ui.with(NodeSpec::row().height(Sizing::Fixed(18.0)), |ui| {
                for col in 0..COLS {
                    let c = cells[row * COLS + col];
                    let s: &str = c.ch.encode_utf8(&mut buf);
                    ui.with(
                        NodeSpec::row()
                            .width(Sizing::Fixed(8.0))
                            .height(Sizing::Fixed(18.0)),
                        |ui| ui.text(s, mono().color(Color::hex(c.fg))),
                    );
                }
            });
        }
        ui.finish();
        let (dl, _) = core.output();
        dl.quads.len()
    };
    run(&mut core);
    bencher.bench_local(|| run(&mut core));
}

fn main() {
    divan::main();
}
