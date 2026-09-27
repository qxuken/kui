//! A pane whose text is new every frame — a terminal streaming, a log
//! tailing, a file scrolled through faster than the cache has seen it.
//! What the frame costs is the shaper's (every line misses the cache and
//! is shaped from scratch; cosmic-text alone on the same fifty lines is the
//! same number), and what the cache holds afterwards is backlog C16's:
//! the byte budget is what keeps `stream_50x200_random` from parking 3 GB
//! of shaped lines by the end of a run. Two vocabularies: `log` is thirty
//! words and numbers, the shape real output has (the shape-run cache hits
//! on the words); `random` is printable ASCII with no repeats.
//!
//! Run: cargo bench -p kui-core --bench stream

use kui_core::{Color, Core, NodeSpec, Size, TextStyle};

const ROWS: usize = 50;
const COLS: usize = 200;
const LH: f32 = 18.0;

fn mono() -> TextStyle {
    TextStyle::new(13.0).mono().line_height(LH)
}

fn rng(row: usize, salt: usize) -> impl FnMut() -> u64 {
    let mut x = (row as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (salt as u64).wrapping_mul(0xD1B5_4A32_D192_ED03)
        | 1;
    move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    }
}

fn random_line(row: usize, salt: usize) -> String {
    let mut r = rng(row, salt);
    let mut s = String::with_capacity(COLS);
    while s.len() < COLS {
        s.push((b' ' + (r() % 95) as u8) as char);
    }
    s
}

const VOCAB: &[&str] = &[
    "INFO",
    "WARN",
    "request",
    "handled",
    "in",
    "ms",
    "GET",
    "/api/v1/items",
    "status=200",
    "user_id=",
    "conn",
    "closed",
    "retry",
    "queue",
    "depth",
    "worker",
    "tick",
    "flush",
    "ok",
    "error:",
    "timeout",
    "cache",
    "hit",
    "miss",
    "bytes",
    "sent",
    "recv",
    "pool",
    "idle",
    "busy",
];

fn log_line(row: usize, salt: usize) -> String {
    let mut r = rng(row, salt);
    let mut s = String::with_capacity(COLS + 8);
    while s.len() < COLS {
        if r().is_multiple_of(4) {
            s.push_str(&format!("{}", r() % 100_000));
        } else {
            s.push_str(VOCAB[(r() % VOCAB.len() as u64) as usize]);
        }
        s.push(' ');
    }
    s.truncate(COLS);
    s
}

fn frame(core: &mut Core, salt: usize, mk: fn(usize, usize) -> String) -> usize {
    let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
    ui.configure_root(NodeSpec::column().fill().bg(Color::rgb8(0, 0, 0)));
    for r in 0..ROWS {
        let t = mk(r, salt);
        ui.text_in(NodeSpec::row().height(LH), &t, mono());
    }
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

/// The same fifty lines every frame: what the cache is for.
#[divan::bench]
fn warm_50x200(bencher: divan::Bencher) {
    let mut core = Core::new();
    frame(&mut core, 0, log_line);
    bencher.bench_local(|| frame(&mut core, 0, log_line));
}

/// Fifty new log-like lines a frame.
#[divan::bench(sample_count = 20)]
fn stream_50x200_log(bencher: divan::Bencher) {
    let mut core = Core::new();
    let mut salt = 0usize;
    frame(&mut core, salt, log_line);
    bencher.bench_local(|| {
        salt += 1;
        frame(&mut core, salt, log_line)
    });
}

/// Fifty new lines of random printable ASCII a frame: nothing for the
/// shape-run cache to hit, so this is the shaper's worst case.
#[divan::bench(sample_count = 10)]
fn stream_50x200_random(bencher: divan::Bencher) {
    let mut core = Core::new();
    let mut salt = 0usize;
    frame(&mut core, salt, random_line);
    bencher.bench_local(|| {
        salt += 1;
        frame(&mut core, salt, random_line)
    });
}

fn main() {
    divan::main();
}
