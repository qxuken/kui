//! One sample per process. Build with the release library, then run with
//! an installed font family and a scratch path ending in `.kuitrace`.

use kui_core::{Core, Size, TextStyle};
use std::hint::black_box;
use std::time::Instant;

#[cfg(kui_native_host)]
use kui_native::system::new_core;

#[cfg(not(kui_native_host))]
fn new_core() -> Core {
    Core::new()
}

const FRAME_COUNT: usize = 2_000;
const TEXT: &str = "Sphinx of black quartz, judge my vow. 0123456789";

fn frame(core: &mut Core, time: f64) {
    core.set_time(time);
    let mut ui = core.frame(Size::new(800.0, 600.0), 2.0);
    for _ in 0..24 {
        ui.text(TEXT, TextStyle::new(16.0));
    }
    ui.finish();
    black_box(core.output().0.quads.len());
}

fn micros(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1_000_000.0
}

fn frames(core: &mut Core, start_frame: usize, mark: bool) -> (f64, f64) {
    let mut samples = Vec::with_capacity(FRAME_COUNT);
    for i in start_frame..start_frame + FRAME_COUNT {
        let start = Instant::now();
        if mark {
            let span = core.span_begin("probe handler");
            core.perf_mark("probe input");
            core.span_end(span);
        }
        frame(core, i as f64 / 60.0);
        samples.push(micros(start));
    }
    samples.sort_unstable_by(f64::total_cmp);
    (samples[FRAME_COUNT / 2], samples[FRAME_COUNT * 95 / 100])
}

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let family = args.next().expect("installed font family");
    let path = args.next().expect("scratch .kuitrace path");
    assert!(path.ends_with(".kuitrace"));
    assert!(!std::path::Path::new(&path).exists(), "use a fresh path");

    let start = Instant::now();
    let mut core = new_core();
    let first_core = micros(start);

    let start = Instant::now();
    frame(&mut core, 0.0);
    let first_frame = micros(start);

    let start = Instant::now();
    let other = black_box(new_core());
    let second_core = micros(start);
    drop(other);

    let start = Instant::now();
    let font = core
        .add_system_font(&family)
        .expect("installed font family");
    let register = micros(start);

    let start = Instant::now();
    black_box(core.measure_text(TEXT, &TextStyle::new(16.0).font(font), None));
    let measure = micros(start);

    let start = Instant::now();
    assert_eq!(core.reload_system_fonts(), 0, "font catalog changed");
    let rescan = micros(start);
    let families = core.system_font_families().len();

    let (frame_p50, frame_p95) = frames(&mut core, 1, false);
    core.perf_trace_to(&path)?;
    let (stream_p50, stream_p95) = frames(&mut core, FRAME_COUNT + 1, true);
    let start = Instant::now();
    core.perf_flush()?;
    let flush = micros(start);
    let trace_bytes = std::fs::metadata(&path)?.len();
    drop(core);
    std::fs::remove_file(&path)?;

    println!(
        "{first_core:.2},{first_frame:.2},{second_core:.2},{register:.2},{measure:.2},{rescan:.2},\
         {frame_p50:.2},{frame_p95:.2},{stream_p50:.2},{stream_p95:.2},{flush:.2},\
         {families},{trace_bytes}"
    );
    Ok(())
}
