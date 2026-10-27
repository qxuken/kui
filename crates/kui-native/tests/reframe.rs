//! A frame only an animation of the core's asks for is built from the last
//! tree, with no view, for an app that says what its view reads
//! (`App::view_state`, backlog C138) — and the view runs again as soon as
//! anything else asks.

use std::time::{Duration, Instant};

use kui_native::{
    App, Color, InputEvent, Keyframe, Message, MouseButton, NodeSpec, Transition, Ui, UiEvent, Vec2,
};

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Add,
}

struct Pulse {
    /// What `view_state` answers; `None` is an app that says nothing.
    state: Option<u64>,
    views: u32,
    count: u32,
}

impl App for Pulse {
    fn view(&mut self, ui: &mut Ui<'_>) {
        self.views += 1;
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.leaf_keyed(
                "pulse",
                NodeSpec::column()
                    .size(40.0, 40.0)
                    .bg(Color::hex(0x000000ff))
                    .transition_with(Transition::ms(400.0))
                    .keyframes(vec![Keyframe::default().at(0.5).bg(Color::hex(0xffffffff))]),
            );
            ui.leaf_keyed(
                "add",
                NodeSpec::row()
                    .size(80.0, 30.0)
                    .on_click(Msg::Add)
                    .label(format!("Add {}", self.count)),
            );
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.message::<Msg>() == Some(Msg::Add) {
            self.count += 1;
            if let Some(s) = &mut self.state {
                *s += 1;
            }
        }
    }

    fn view_state(&self) -> Option<u64> {
        self.state
    }
}

fn run(state: Option<u64>) -> (u32, u64, kui_native::HeadlessRunner<Pulse>) {
    let app = Pulse {
        state,
        views: 0,
        count: 0,
    };
    let mut r = kui_native::app("reframe").open_headless(app, 320.0, 200.0, 1.0);
    // Until a dozen frames are drawn, not for a fixed time: the cycle
    // draws at the display's pace, and a hosted CI Mac drew 6 in the
    // 300 ms this once waited. The deadline only stops a run that draws
    // nothing.
    let deadline = Instant::now() + Duration::from_secs(5);
    while r.frames() < 12 && Instant::now() < deadline {
        r.turn_until((Instant::now() + Duration::from_millis(20)).min(deadline));
    }
    (r.app_mut().views, r.frames(), r)
}

#[test]
fn an_app_that_says_its_state_is_not_asked_for_its_view_while_only_the_cycle_animates() {
    let (views, frames, mut r) = run(Some(0));
    assert!(
        frames >= 12,
        "the cycle drew at the display's pace: {frames} frames in 5 s"
    );
    assert!(views <= 3, "the view ran {views} times in {frames} frames");
    // A click moves the app's state: the next frame runs the view, and
    // the frame after the click shows what it did.
    let at = Vec2::new(40.0, 55.0);
    let input = r.input();
    input.send(InputEvent::CursorMoved(at));
    input.send(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    input.send(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    let before = r.app_mut().views;
    let deadline = Instant::now() + Duration::from_secs(5);
    while (r.app_mut().count == 0 || r.app_mut().views == before) && Instant::now() < deadline {
        r.turn_until((Instant::now() + Duration::from_millis(20)).min(deadline));
    }
    assert_eq!(
        r.app_mut().count,
        1,
        "the click on a frame built again landed"
    );
    assert!(r.app_mut().views > before, "and its answer ran the view");
}

#[test]
fn an_app_that_says_nothing_runs_its_view_every_frame() {
    let (views, frames, _) = run(None);
    assert_eq!(u64::from(views), frames);
}
