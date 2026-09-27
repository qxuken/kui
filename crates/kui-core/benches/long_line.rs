//! One line of 100k characters in a horizontally scrolling view (backlog
//! C19): what it costs to open, to scroll through, and to type into. Before
//! chunking it was 662 ms / 0.18 ms / 102 ms with 47 MB cached; a chunk is
//! ~1 KB, so each should be the cost of a screenful.
//!
//! Run: cargo bench -p kui-core --bench long_line

use kui_core::{Color, Core, Key, NodeSpec, Size, Span, TextStyle, TextWrap, Vec2};

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
        NodeSpec::column().grow_width().height(300.0).scroll_x(),
        |ui| {
            ui.text_in(
                NodeSpec::row().height(18.0),
                text,
                TextStyle::new(13.0)
                    .mono()
                    .line_height(18.0)
                    .wrap(TextWrap::None),
            );
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
    let mut n = 0u32;
    bencher.bench_local(|| {
        // A different insertion every frame: alternating two letters hit
        // the text cache from the third frame on and measured a lookup.
        n += 1;
        let a = char::from_u32(u32::from(b'!') + n % 93).unwrap();
        let b = char::from_u32(u32::from(b'!') + (n / 93) % 93).unwrap();
        edited.insert(50_000, b);
        edited.insert(50_000, a);
        let q = frame(&mut core, &edited, x);
        edited.replace_range(50_000..50_002, "");
        q
    });
}

/// The same line as a paragraph in a vertically scrolling view: `wrap:
/// word` breaks the chunks into rows (C19's step 5).
fn wrapped_frame(core: &mut Core, text: &str, scroll_y: f32) -> usize {
    let view = Key::ROOT.str("view");
    core.set_scroll(view, Vec2::new(0.0, scroll_y));
    let mut ui = core.frame(Size::new(1200.0, 400.0), 2.0);
    ui.configure_root(NodeSpec::column().fill().bg(Color::rgb8(0, 0, 0)));
    ui.with_keyed(
        "view",
        NodeSpec::column().size(1100.0, 300.0).scroll_y(),
        |ui| {
            ui.text_in(
                NodeSpec::column().width(1100.0),
                text,
                TextStyle::new(13.0)
                    .mono()
                    .line_height(18.0)
                    .wrap(TextWrap::Word),
            );
        },
    );
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

/// Opening the paragraph: the rows of the first screenful, and an
/// estimate of the rest.
#[divan::bench(sample_count = 20)]
fn long_line_100k_wrapped_first_frame(bencher: divan::Bencher) {
    let mut core = Core::new();
    wrapped_frame(&mut core, &line(N, 0), 0.0);
    let mut salt = 0u64;
    bencher.bench_local(|| {
        salt += 1;
        let text = line(N, salt);
        wrapped_frame(&mut core, &text, 0.0)
    });
}

/// Scrolling the paragraph: a viewport's height per frame.
#[divan::bench]
fn long_line_100k_wrapped_scroll(bencher: divan::Bencher) {
    let mut core = Core::new();
    let text = line(N, 0);
    wrapped_frame(&mut core, &text, 0.0);
    let mut y = 0.0f32;
    bencher.bench_local(|| {
        y = (y + 280.0) % 20_000.0;
        wrapped_frame(&mut core, &text, y)
    });
}

// ---- rich (C42) and the key (C43) ---------------------------------------

/// The line as three spans — a caret-shaped one at `at` between two plain
/// runs — in the horizontally scrolling view: what an editor's caret row
/// is, past the threshold (backlog C42).
fn rich_frame(core: &mut Core, text: &str, at: usize, scroll_x: f32) -> usize {
    let view = Key::ROOT.str("view");
    core.set_scroll(view, Vec2::new(scroll_x, 0.0));
    let mut ui = core.frame(Size::new(1200.0, 400.0), 2.0);
    ui.configure_root(NodeSpec::column().fill().bg(Color::rgb8(0, 0, 0)));
    let spans = [
        Span::new(&text[..at]).color(Color::rgb8(200, 200, 200)),
        Span::new(&text[at..at + 1]).bg(Color::rgb8(200, 40, 40)),
        Span::new(&text[at + 1..]),
    ];
    ui.with_keyed(
        "view",
        NodeSpec::column().grow_width().height(300.0).scroll_x(),
        |ui| {
            ui.with(NodeSpec::row().height(18.0), |ui| {
                ui.rich_text(
                    &spans,
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

/// Opening a rich line: its chunks on screen, not the paragraph. Before
/// C42 a 100k-character rich line was shaped whole (~660 ms, like the
/// plain one before C19).
#[divan::bench(sample_count = 20)]
fn long_line_100k_rich_first_frame(bencher: divan::Bencher) {
    let mut core = Core::new();
    rich_frame(&mut core, &line(N, 0), 50, 0.0);
    let mut salt = 0u64;
    bencher.bench_local(|| {
        salt += 1;
        let text = line(N, salt);
        rich_frame(&mut core, &text, 50, 0.0)
    });
}

/// A caret span moving one character a frame along a rich line: the
/// chunk it moves in reshapes, the rest hit.
#[divan::bench(sample_count = 30)]
fn long_line_100k_rich_caret(bencher: divan::Bencher) {
    let mut core = Core::new();
    let text = line(N, 0);
    let x = 50_000.0 * 7.8;
    rich_frame(&mut core, &text, 50_000, x);
    let mut at = 50_000usize;
    bencher.bench_local(|| {
        at += 1;
        rich_frame(&mut core, &text, at, x)
    });
}

/// A screenful of long rows, every chunk shaped, nothing changing: what
/// the frame costs when it draws none of the content — the key's hash
/// (backlog C43: 1 ns a byte before, and a newline scan besides).
fn rows_frame(core: &mut Core, rows: &[String]) -> usize {
    let view = Key::ROOT.str("view");
    core.set_scroll(view, Vec2::ZERO);
    let mut ui = core.frame(Size::new(1200.0, 800.0), 2.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("view", NodeSpec::column().fill().clip().scroll_x(), |ui| {
        for r in rows {
            ui.text_in(
                NodeSpec::row().height(18.0),
                r,
                TextStyle::new(13.0)
                    .mono()
                    .line_height(18.0)
                    .wrap(TextWrap::None),
            );
        }
    });
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

#[divan::bench]
fn long_rows_34x500k_steady(bencher: divan::Bencher) {
    let mut core = Core::new();
    let rows: Vec<String> = (0..34).map(|i| line(500_000, 100 + i)).collect();
    rows_frame(&mut core, &rows);
    bencher.bench_local(|| rows_frame(&mut core, &rows));
}

fn main() {
    divan::main();
}
