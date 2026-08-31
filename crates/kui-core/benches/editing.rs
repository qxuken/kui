//! Editing latency: what one keystroke costs end-to-end (apply + full frame)
//! at several document sizes.
//!
//! Run: cargo bench -p kui-core --bench editing

use kui_core::{Core, EditOptions, FontFamily, InputEvent, NodeSpec, Size, Sizing, TextStyle};

fn doc(lines: usize) -> String {
    let mut s = String::new();
    for i in 0..lines {
        s.push_str(&format!(
            "line {i}: the quick brown fox jumps over the lazy dog {i}\n"
        ));
    }
    s
}

fn frame(core: &mut Core, initial: &str) {
    let mut ui = core.frame(Size::new(1200.0, 800.0), 2.0);
    ui.configure_root(NodeSpec::column().fill().scroll_y());
    ui.text_edit(
        "doc",
        initial,
        &EditOptions {
            style: TextStyle::new(14.0).family(FontFamily::Mono),
            multiline: true,
            autofocus: true,
            ..Default::default()
        },
        NodeSpec::column().width(Sizing::Grow(1.0)).pad(20.0),
    );
    ui.finish();
}

fn bench_size(lines: usize) -> (f64, f64, usize) {
    let text = doc(lines);
    let mut core = Core::new();
    frame(&mut core, &text);
    frame(&mut core, &text); // warm caches

    let n = 30;
    let mut apply_total = 0.0;
    let mut frame_total = 0.0;
    for _ in 0..n {
        let t0 = std::time::Instant::now();
        core.handle_input(InputEvent::Text("x".to_string()));
        apply_total += t0.elapsed().as_secs_f64();

        let t1 = std::time::Instant::now();
        frame(&mut core, &text);
        frame_total += t1.elapsed().as_secs_f64();
    }
    let (dl, _) = core.output();
    (apply_total / n as f64 * 1e3, frame_total / n as f64 * 1e3, dl.quads.len())
}

/// Isolate cosmic-text costs from kui's: what do insert and reshape cost raw?
fn bench_cosmic_raw() {
    use cosmic_text::{Attrs, Buffer, Edit as _, Editor, FontSystem, Metrics, Motion, Shaping};
    let mut fs = FontSystem::new();
    let mut buffer = Buffer::new(&mut fs, Metrics::new(28.0, 44.0));
    buffer.set_size(&mut fs, Some(2000.0), None);
    buffer.set_text(
        &mut fs,
        &doc(500),
        Attrs::new().family(cosmic_text::Family::Monospace),
        Shaping::Advanced,
    );
    buffer.shape_until_scroll(&mut fs, false);
    let mut editor = Editor::new(buffer);
    editor.action(&mut fs, cosmic_text::Action::Motion(Motion::BufferEnd));

    let n = 50;
    let mut t_insert = 0.0;
    let mut t_shape = 0.0;
    for _ in 0..n {
        let t0 = std::time::Instant::now();
        editor.insert_string("x", None);
        t_insert += t0.elapsed().as_secs_f64();
        let t1 = std::time::Instant::now();
        editor.shape_as_needed(&mut fs, false);
        t_shape += t1.elapsed().as_secs_f64();
    }
    println!(
        "cosmic raw (500 lines): insert {:.3} ms, shape_as_needed {:.3} ms",
        t_insert / n as f64 * 1e3,
        t_shape / n as f64 * 1e3
    );
}

fn main() {
    bench_cosmic_raw();
    println!("{:>8} | {:>12} | {:>12} | {:>8}", "lines", "apply (ms)", "frame (ms)", "quads");
    for lines in [50, 500, 2000, 10000, 100000] {
        let (apply, frame, quads) = bench_size(lines);
        println!("{lines:>8} | {apply:>12.3} | {frame:>12.3} | {quads:>8}");
    }
}
