//! Recording may begin before the native window has supplied its context.
use kui_core::{Core, NodeSpec, Size};

fn draw(core: &mut Core, scale: f32) {
    let mut ui = core.frame(Size::new(100.0, 100.0), scale);
    ui.leaf(NodeSpec::column().size(20.0, 20.0));
    ui.finish();
}

#[test]
fn first_recorded_frame_replaces_startup_display_defaults_without_changing_the_snapshot_later() {
    let mut core = Core::new();
    core.set_perf_recording(true);
    core.env.refresh_hz = Some(60.0);
    draw(&mut core, 2.0);
    let first = kui_trace::Trace::from_bytes(&core.perf_trace())
        .unwrap()
        .session
        .unwrap();
    assert_eq!(first.refresh_hz, 60.0);
    assert_eq!(first.scale, 2.0);
    assert!((first.budget_ms - 1000.0 / 60.0).abs() < 0.001);
    // A session is a snapshot, not a retroactive relabelling of older frames.
    core.env.refresh_hz = Some(120.0);
    draw(&mut core, 1.0);
    let later = kui_trace::Trace::from_bytes(&core.perf_trace())
        .unwrap()
        .session
        .unwrap();
    assert_eq!(later, first);
}

#[test]
fn streamed_capture_publishes_first_frame_context_to_readers() {
    let path =
        std::env::temp_dir().join(format!("kui-perf-context-{}.kuitrace", std::process::id()));
    let mut core = Core::new();
    core.perf_trace_to(path.to_str().unwrap()).unwrap();
    core.env.refresh_hz = Some(144.0);
    draw(&mut core, 2.0);
    core.perf_flush().unwrap();
    let trace = kui_trace::Trace::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
    let session = trace.session.unwrap();
    assert_eq!(session.refresh_hz, 144.0);
    assert_eq!(session.scale, 2.0);
    assert!((session.budget_ms - 1000.0 / 144.0).abs() < 0.001);
    let _ = std::fs::remove_file(path);
}
