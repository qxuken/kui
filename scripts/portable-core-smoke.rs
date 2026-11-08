use kui_core::perf::{PerfClock, PerfHost};
use kui_core::{
    Color, Core, Easing, FileDialog, InputEvent, NodeSpec, Session, Size, TextStyle, Transition,
};
use std::{cell::Cell, rc::Rc};

struct Clock(Cell<u64>);

impl PerfClock for Clock {
    fn now_us(&self) -> u64 {
        self.0.get()
    }
}

fn draw(core: &mut Core, time: f64, color: Color) -> usize {
    core.set_time(time);
    let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
    ui.leaf_keyed(
        "animated",
        NodeSpec::row()
            .size(20.0, 20.0)
            .bg(color)
            .transition_with(Transition::ms(100.0).easing(Easing::Linear)),
    );
    ui.text("Portable core", TextStyle::new(16.0));
    ui.finish();
    core.output().0.quads.len()
}

#[unsafe(no_mangle)]
pub extern "C" fn portable_core_smoke() -> u32 {
    let session = Session::with_fonts(
        "en-US",
        [include_bytes!("../crates/kui-cli/assets/fonts/NotoSans-Regular.ttf").to_vec()],
    );
    let clock = Rc::new(Clock(Cell::new(100)));
    session.set_perf_host(PerfHost::new(Some(clock.clone()), None));
    let mut core = Core::new_in(&session);
    assert_eq!(core.platform().name(), "other");

    core.set_perf_recording(true);
    let span = core.span_begin("portable");
    clock.0.set(125);
    core.span_end(span);

    let first = draw(&mut core, 1.0, Color::hex(0xff0000ff));
    assert!(first > 1, "font supplied by the host draws glyphs");
    assert!(core.request_files(FileDialog::open().tag("import")));
    assert_eq!(core.take_file_requests().len(), 1);
    let events = core.handle_input(InputEvent::Files(vec!["picked.txt".into()]));
    assert_eq!(events[0].event.name(), "files");
    assert!(!core.awaiting_files());

    clock.0.set(150);
    let second = draw(&mut core, 1.01, Color::hex(0x0000ffff));
    assert!(core.animating());
    clock.0.set(350);
    let third = draw(&mut core, 1.20, Color::hex(0x0000ffff));
    assert!(!core.animating());

    let trace = core.perf_trace_read();
    let recorded = trace
        .records
        .iter()
        .find_map(|record| match record {
            kui_core::kui_trace::Record::Span(recorded) if recorded.id == span.0 => Some(recorded),
            _ => None,
        })
        .expect("host-timed span");
    assert_eq!(recorded.dur, 25);
    (first * 10_000 + second * 100 + third) as u32
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("{}", portable_core_smoke());
}
