//! One line of 100k characters in a horizontally scrolling view (backlog
//! C19): what it costs to open, to scroll through, and to type into. Before
//! chunking it was 662 ms / 0.18 ms / 102 ms with 47 MB cached; a chunk is
//! ~1 KB, so each should be the cost of a screenful.
//!
//! Run: cargo bench -p kui-core --bench long_line

use kui_core::{Color, Core, Key, NodeSpec, Size, Sizing, TextStyle, TextWrap, Vec2};

const N: usize = 100_000;

fn line(n: usize, salt: u64) -> String {
    let mut x = 0x9E37_79B9_7F4A_7C15u64 ^ salt;
    let mut s = String::with_capacity(n);
    while s.len() < n {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.push(if s.len() % 11 == 10 {
            ' '
        } else {
            (b'!' + (x % 93) as u8) as char
        });
    }
    s
}

fn frame(core: &mut Core, text: &str, scroll_x: f32) -> usize {
    let view = Key::ROOT.str("view");
    core.set_scroll(view, Vec2::new(scroll_x, 0.0));
    let mut ui = core.frame(Size::new(1200.0, 400.0), 2.0);
    ui.configure_root(NodeSpec::column().fill().bg(Color::rgb8(0, 0, 0)));
    ui.with_keyed(
        "view",
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(300.0))
            .scroll_x(),
        |ui| {
            ui.with(NodeSpec::row().height(Sizing::Fixed(18.0)), |ui| {
                ui.text(
                    text,
                    TextStyle::new(13.0)
                        .mono()
                        .line_height(18.0)
                        .wrap(TextWrap::None),
                )
            });
        },
    );
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

/// Opening: a line nobody has shaped, drawn at its start. A new line each
/// time (its content is its key) on one core, so the font system's start-up
/// is not in the number.
#[divan::bench(sample_count = 20)]
fn long_line_100k_first_frame(bencher: divan::Bencher) {
    let mut core = Core::new();
    frame(&mut core, &line(N, 0), 0.0);
    let mut salt = 0u64;
    bencher.bench_local(|| {
        salt += 1;
        let text = line(N, salt);
        frame(&mut core, &text, 0.0)
    });
}

/// Scrolling: a viewport's width per frame through a line already open.
#[divan::bench]
fn long_line_100k_scroll(bencher: divan::Bencher) {
    let mut core = Core::new();
    let text = line(N, 0);
    frame(&mut core, &text, 0.0);
    let mut x = 0.0f32;
    bencher.bench_local(|| {
        x = (x + 1100.0) % 300_000.0;
        frame(&mut core, &text, x)
    });
}

/// Typing: one character inserted in the middle, the view held there.
#[divan::bench(sample_count = 30)]
fn long_line_100k_edit(bencher: divan::Bencher) {
    let mut core = Core::new();
    let text = line(N, 0);
    let x = 50_000.0 * 7.8;
    frame(&mut core, &text, x);
    let mut edited = text.clone();
    let mut n = 0usize;
    bencher.bench_local(|| {
        n += 1;
        edited.insert(50_000, if n.is_multiple_of(2) { 'X' } else { 'Y' });
        let q = frame(&mut core, &edited, x);
        edited.remove(50_000);
        q
    });
}

fn main() {
    divan::main();
}
