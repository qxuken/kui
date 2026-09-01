//! Syntax-highlighted editor frames built the way the `syntax_view` and
//! `modal_editor` examples build them: each
//! visible line is a row of per-color text runs (one text node per token
//! run), gutter numbers, selection segments as bg containers, an inline
//! caret node. Measures whether "highlighting = many small text nodes" holds
//! up, since the text cache is keyed by (content, style, scale) — color
//! excluded — so token runs dedupe across lines and colors.
//!
//! Run: cargo bench -p kui-core --bench highlight

use kui_core::{Color, Core, NodeSpec, Size, Sizing, TextStyle, Ui};

const LH: f32 = 20.0;
const LINES_PER_PANE: usize = 55;
const PANES: usize = 2;

fn mono() -> TextStyle {
    TextStyle::new(13.5).mono().line_height(LH)
}

/// Deterministic "code": ~8 runs per line, realistic token lengths, content
/// unique per (doc line, salt). salt=0 is the steady document; a nonzero
/// salt is "different file" (cold cache).
fn line_runs(line: usize, salt: usize) -> Vec<(String, Color)> {
    let kw = Color::rgb8(0xc7, 0x8f, 0xe8);
    let fg = Color::rgb8(0xd6, 0xd8, 0xe0);
    let ty = Color::rgb8(0x6f, 0xc3, 0xd6);
    let st = Color::rgb8(0x9c, 0xc8, 0x7a);
    let nm = Color::rgb8(0xd9, 0xa1, 0x4d);
    let cm = Color::rgb8(0x5c, 0x66, 0x79);
    match line % 5 {
        0 => vec![
            ("    ".into(), fg),
            ("let ".into(), kw),
            (format!("value_{}_{salt}", line), fg),
            (" = ".into(), fg),
            ("Vec".into(), ty),
            ("::with_capacity(".into(), fg),
            (format!("{}", line * 31 + salt), nm),
            (");".into(), fg),
        ],
        1 => vec![
            ("    ".into(), fg),
            ("if ".into(), kw),
            (format!("count_{line} "), fg),
            ("> ".into(), fg),
            (format!("{}", salt + 7), nm),
            (" { ".into(), fg),
            ("return".into(), kw),
            ("; }".into(), fg),
        ],
        2 => vec![
            (format!("// case {line}: cache keyed on content, salt {salt}"), cm),
        ],
        3 => vec![
            ("    ".into(), fg),
            ("writeln!".into(), Color::rgb8(0xe0, 0x9a, 0x6a)),
            ("(out, ".into(), fg),
            (format!("\"row {line} -> {{}}\""), st),
            (", ".into(), fg),
            (format!("total_{salt}"), fg),
            (")?;".into(), fg),
        ],
        _ => vec![
            ("pub fn ".into(), kw),
            (format!("handle_{}", line), fg),
            ("(&".into(), fg),
            ("mut ".into(), kw),
            ("self".into(), kw),
            (", ev: ".into(), fg),
            ("UiEvent".into(), ty),
            (") {".into(), fg),
        ],
    }
}

/// One editor pane: gutter + highlighted lines, a 10-line selection band,
/// a caret, a statusline — the `syntax_view` example's shape.
fn pane(ui: &mut Ui<'_>, pane_no: usize, top: usize, salt: usize, caret_line: usize) {
    let sel_bg = Color::rgba8(0x3b, 0x5b, 0xd4, 0x55);
    ui.with(
        NodeSpec::column().fill().bg(Color::rgb8(0x14, 0x16, 0x1e)).clip(),
        |ui| {
            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)).height(Sizing::Grow(1.0)), |ui| {
                ui.with(NodeSpec::column().width(Sizing::Fixed(52.0)), |ui| {
                    for ln in top..top + LINES_PER_PANE {
                        ui.with(
                            NodeSpec::row().height(Sizing::Fixed(LH)).main_align(kui_core::Align::End),
                            |ui| ui.text(&format!("{}", ln + 1), TextStyle::new(11.0).mono()),
                        );
                    }
                });
                ui.with(NodeSpec::column().width(Sizing::Grow(1.0)).clip(), |ui| {
                    for ln in top..top + LINES_PER_PANE {
                        let selected = pane_no == 0 && (10..20).contains(&(ln - top));
                        ui.with(NodeSpec::row().height(Sizing::Fixed(LH)), |ui| {
                            for (i, (run, color)) in line_runs(ln, salt).into_iter().enumerate() {
                                let style = mono().color(color);
                                if ln == caret_line && i == 2 {
                                    // Inline caret node between runs.
                                    ui.with(
                                        NodeSpec::column()
                                            .width(Sizing::Fixed(2.0))
                                            .height(Sizing::Fixed(LH - 4.0))
                                            .bg(Color::rgb8(0x6a, 0x8b, 0xff)),
                                        |_| {},
                                    );
                                }
                                if selected && (1..=3).contains(&i) {
                                    ui.with(
                                        NodeSpec::row().height(Sizing::Fixed(LH)).bg(sel_bg),
                                        |ui| ui.text(&run, style),
                                    );
                                } else {
                                    ui.text(&run, style);
                                }
                            }
                        });
                    }
                });
            });
            ui.with(
                NodeSpec::row()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(24.0))
                    .bg(Color::rgb8(0x1a, 0x1d, 0x27)),
                |ui| ui.text("main.rs 12:8 34%", TextStyle::new(11.0).mono()),
            );
        },
    );
}

fn frame(core: &mut Core, top: usize, salt: usize, caret_line: usize) -> usize {
    let mut ui = core.frame(Size::new(1600.0, 1200.0), 1.0);
    ui.configure_root(NodeSpec::row().fill().gap(1.0));
    for p in 0..PANES {
        ui.with(NodeSpec::column().fill(), |ui| pane(ui, p, top, salt, caret_line));
    }
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.len()
}

/// Idle/caret-motion frames: content identical, everything cache-warm.
#[divan::bench]
fn highlight_2x55_warm(bencher: divan::Bencher) {
    let mut core = Core::new();
    frame(&mut core, 0, 0, 12);
    bencher.bench_local(|| frame(&mut core, 0, 0, 12));
}

/// A keystroke: one line's content differs every frame (its runs miss the
/// cache and reshape); the other 109 lines stay warm.
#[divan::bench]
fn highlight_2x55_typing(bencher: divan::Bencher) {
    let mut core = Core::new();
    let mut tick = 0usize;
    frame(&mut core, 0, 0, 12);
    bencher.bench_local(|| {
        tick += 1;
        let mut ui = core.frame(Size::new(1600.0, 1200.0), 1.0);
        ui.configure_root(NodeSpec::row().fill().gap(1.0));
        for p in 0..PANES {
            ui.with(NodeSpec::column().fill(), |ui| {
                pane(ui, p, 0, 0, 12);
                // The "edited" line, unique content per frame.
                ui.with(NodeSpec::row().height(Sizing::Fixed(LH)), |ui| {
                    ui.text(&format!("    let typed = \"abc{tick}\";"), mono());
                });
            });
        }
        ui.finish();
        let (dl, _) = core.output();
        dl.quads.len()
    });
}

/// The rich-text alternative: each line is ONE node (a `Span` list shaped as
/// a single paragraph) instead of ~8 run nodes. No inline caret/selection —
/// this is the shape for every line the cursor is NOT on.
#[divan::bench]
fn highlight_2x55_warm_rich(bencher: divan::Bencher) {
    use kui_core::Span;
    let mut core = Core::new();
    let run_frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(1600.0, 1200.0), 1.0);
        ui.configure_root(NodeSpec::row().fill().gap(1.0));
        for _ in 0..PANES {
            ui.with(NodeSpec::column().fill().clip(), |ui| {
                for ln in 0..LINES_PER_PANE {
                    let runs = line_runs(ln, 0);
                    ui.with(NodeSpec::row().height(Sizing::Fixed(LH)), |ui| {
                        let spans: Vec<Span<'_>> =
                            runs.iter().map(|(t, c)| Span::new(t).color(*c)).collect();
                        ui.rich_text(&spans, mono());
                    });
                }
            });
        }
        ui.finish();
        let (dl, _) = core.output();
        dl.quads.len()
    };
    run_frame(&mut core);
    bencher.bench_local(|| run_frame(&mut core));
}

/// Scrolling through a 500-line document, one line per frame: newly revealed
/// lines re-shape once they have been evicted (300-frame TTL), the rest hit.
#[divan::bench]
fn highlight_2x55_scrolling(bencher: divan::Bencher) {
    let mut core = Core::new();
    let mut top = 0usize;
    frame(&mut core, 0, 0, 12);
    bencher.bench_local(|| {
        top = (top + 1) % 500;
        frame(&mut core, top, 0, top + 12)
    });
}

/// Opening a new file: every one of the ~900 runs on screen misses and
/// shapes from scratch (shape-run cache still helps within the frame).
#[divan::bench(sample_count = 30)]
fn highlight_2x55_cold(bencher: divan::Bencher) {
    let mut core = Core::new();
    let mut salt = 0usize;
    bencher.bench_local(|| {
        salt += 1;
        frame(&mut core, 0, salt, 12)
    });
}

fn main() {
    divan::main();
}
