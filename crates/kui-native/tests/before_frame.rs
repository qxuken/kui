//! Queued input must reach the last tree before a fresh frame resets it.
use kui_native::{App, Core, InputEvent, Message, MouseButton, NodeSpec, Ui, UiEvent, Vec2};

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Add,
}

#[derive(Default)]
struct Queued {
    send: bool,
    count: u64,
    built: u64,
}
impl App for Queued {
    fn before_frame(&mut self, core: &mut Core) {
        if !std::mem::take(&mut self.send) {
            return;
        }
        for input in [
            InputEvent::CursorMoved(Vec2::new(20.0, 20.0)),
            InputEvent::MouseDown {
                button: MouseButton::Primary,
                clicks: 1,
            },
            InputEvent::MouseUp {
                button: MouseButton::Primary,
            },
        ] {
            for event in core.handle_input(input) {
                self.on_event_with(event, core);
            }
        }
    }
    fn view(&mut self, ui: &mut Ui<'_>) {
        self.built = self.count;
        ui.leaf_keyed("add", NodeSpec::row().size(80.0, 40.0).on_click(Msg::Add));
    }
    fn view_state(&self) -> Option<u64> {
        Some(self.count)
    }
    fn on_event(&mut self, event: UiEvent) {
        if event.message::<Msg>() == Some(Msg::Add) {
            self.count += 1;
        }
    }
}

#[test]
fn queued_input_uses_last_tree_and_updates_the_frame_about_to_draw() {
    let mut runner =
        kui_native::app("queued input").open_headless(Queued::default(), 200.0, 100.0, 1.0);
    runner.turn();
    assert_eq!(runner.app_mut().count, 0);
    runner.app_mut().send = true;
    runner.waker().wake();
    runner.turn();
    assert_eq!(
        runner.app_mut().count,
        1,
        "the click reached the preceding frame's node"
    );
    assert_eq!(
        runner.app_mut().built,
        1,
        "the next view sees the edit, including with view_state caching"
    );
    runner.waker().wake();
    runner.turn();
    assert_eq!(runner.app_mut().count, 1, "queued input is drained once");
}
