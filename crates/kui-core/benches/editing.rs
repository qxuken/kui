//! Editing latency: what one keystroke costs at several document sizes —
//! applying it and the full frame it causes, warm caches — and the edit
//! alone at the largest.
//!
//! Divan, so `scripts/bench-check.nu` reads it (`KUI_BENCH=editing`). It
//! printed a table of its own until the alpha.60 pre-tag pass, where the
//! 100 000-line apply had tripled (RG156) and only a hand-read of that
//! table showed it.
//!
//! Run: cargo bench -p kui-core --bench editing

use std::cell::RefCell;

use kui_core::{Core, EditOptions, FontFamily, InputEvent, NodeSpec, Size, TextStyle, Value};

fn doc(lines: usize) -> String {
    let mut s = String::new();
    for i in 0..lines {
        s.push_str(&format!(
            "line {i}: the quick brown fox jumps over the lazy dog {i}\n"
        ));
    }
    s
}

/// One frame of a focused multiline editor seeded with `initial`, its
/// `on_change` declared when `on_change` says so.
fn frame(core: &mut Core, initial: &str, on_change: bool) {
    let mut ui = core.frame(Size::new(1200.0, 800.0), 2.0);
    ui.configure_root(NodeSpec::column().fill().scroll_y());
    let mut spec = NodeSpec::column().grow_width().pad(20.0);
    if on_change {
        spec = spec.on_change(Value::str("typed"));
    }
    ui.text_edit(
        "doc",
        initial,
        &EditOptions {
            style: TextStyle::new(14.0).family(FontFamily::Mono),
            multiline: true,
            autofocus: true,
            ..Default::default()
        },
        spec,
    );
    ui.finish();
}

/// A core showing the document, its caches warm.
fn warm(lines: usize, on_change: bool) -> (Core, String) {
    let text = doc(lines);
    let mut core = Core::new();
    frame(&mut core, &text, on_change);
    frame(&mut core, &text, on_change);
    (core, text)
}

/// A whole keystroke: the edit, then the frame it causes — what typing
/// costs. Timed together, so divan can batch samples: one short call
/// timed alone reads bimodal on this machine (the 500-line apply 86 to
/// 266 µs), too wide for the guard's 10%.
fn keystroke(bencher: divan::Bencher, lines: usize, on_change: bool) {
    let (mut core, text) = warm(lines, on_change);
    bencher.bench_local(|| {
        core.handle_input(InputEvent::Text("x".to_string()));
        frame(&mut core, &text, on_change);
        core.output().0.quads.len()
    });
}

#[divan::bench]
fn keystroke_500_lines(bencher: divan::Bencher) {
    keystroke(bencher, 500, false);
}

#[divan::bench]
fn keystroke_10k_lines(bencher: divan::Bencher) {
    keystroke(bencher, 10_000, false);
}

#[divan::bench(sample_count = 40)]
fn keystroke_100k_lines(bencher: divan::Bencher) {
    keystroke(bencher, 100_000, false);
}

/// The editor declares `on_change`, so every keystroke's `change: {text}`
/// carries the whole text: ADR 0047's decision 7, and what an editor
/// without the row no longer pays (RG156).
#[divan::bench(sample_count = 40)]
fn keystroke_100k_lines_on_change(bencher: divan::Bencher) {
    keystroke(bencher, 100_000, true);
}

/// The edit alone, the half RG156 tripled: the frame it causes runs before
/// each sample, untimed. Wide, for the reason `keystroke` gives; read it
/// against `keystroke_100k_lines`.
#[divan::bench(sample_count = 40)]
fn keystroke_apply_100k_lines(bencher: divan::Bencher) {
    let (core, text) = warm(100_000, false);
    let core = RefCell::new(core);
    bencher
        .with_inputs(|| frame(&mut core.borrow_mut(), &text, false))
        .bench_local_values(|()| {
            core.borrow_mut()
                .handle_input(InputEvent::Text("x".to_string()))
        });
}

/// cosmic-text alone, for scale: an insert and the reshape it needs in a
/// 500-line buffer, with none of kui around it.
#[divan::bench]
fn cosmic_insert_and_shape_500_lines(bencher: divan::Bencher) {
    use cosmic_text::{Attrs, Buffer, Edit as _, Editor, FontSystem, Metrics, Motion, Shaping};
    let mut fs = FontSystem::new();
    let mut buffer = Buffer::new(&mut fs, Metrics::new(28.0, 44.0));
    buffer.set_size(Some(2000.0), None);
    buffer.set_text(
        &doc(500),
        &Attrs::new().family(cosmic_text::Family::Monospace),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(&mut fs, false);
    let mut editor = Editor::new(buffer);
    editor.action(&mut fs, cosmic_text::Action::Motion(Motion::BufferEnd));
    bencher.bench_local(|| {
        editor.insert_string("x", None);
        editor.shape_as_needed(&mut fs, false);
    });
}

fn main() {
    divan::main();
}
