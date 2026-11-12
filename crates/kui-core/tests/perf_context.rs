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

use kui_core::perf::{PerfHost, TraceOutput};
use kui_trace::{Record, Trace};
use std::{cell::RefCell, io, rc::Rc};

thread_local! {
    static OUTPUT: RefCell<Trace> = RefCell::new(Trace::new());
}

struct TestOutput;

impl TraceOutput for TestOutput {
    fn record(&mut self, record: &Record) -> io::Result<()> {
        OUTPUT.with_borrow_mut(|trace| trace.push(record.clone()));
        Ok(())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn streamed_capture_publishes_first_frame_context_to_readers() {
    let mut core = Core::new();
    core.supply_perf_host(PerfHost::new(None, Some(|_| Ok(Box::new(TestOutput)))));
    core.perf_trace_to("memory").unwrap();
    core.env.refresh_hz = Some(144.0);
    draw(&mut core, 2.0);
    core.perf_flush().unwrap();
    let session = OUTPUT.with_borrow(|trace| trace.session.clone().unwrap());
    assert_eq!(session.refresh_hz, 144.0);
    assert_eq!(session.scale, 2.0);
    assert!((session.budget_ms - 1000.0 / 144.0).abs() < 0.001);
}

#[test]
fn nested_thread_spans_use_the_supplied_clock() {
    struct Clock(std::cell::Cell<u64>);
    impl kui_core::perf::PerfClock for Clock {
        fn now_us(&self) -> u64 {
            self.0.get()
        }
        fn wall_ms(&self) -> u64 {
            123
        }
    }
    let clock = Rc::new(Clock(std::cell::Cell::new(500)));
    let mut core = Core::new();
    core.supply_perf_host(PerfHost::new(Some(clock.clone()), None));
    core.set_perf_recording(true);
    let parent = core.span_begin("parent");
    clock.0.set(510);
    let child = kui_core::perf::span_begin("child");
    clock.0.set(530);
    kui_core::perf::span_end(child);
    clock.0.set(550);
    core.span_end(parent);
    let trace = core.perf_trace_read();
    assert_eq!(trace.session.unwrap().wall_ms, 123);
    let child = trace
        .records
        .iter()
        .find_map(|r| match r {
            Record::Span(span) if span.id == child.0 => Some(span),
            _ => None,
        })
        .unwrap();
    assert_eq!((child.parent, child.start, child.dur), (parent.0, 10, 20));
}
