//! A window nobody can see still runs its app (backlog F169): a wake, or
//! a time the view asked for, builds the frame of a minimized or covered
//! window and draws nothing, and the frame that brings it back draws what
//! the app came to meanwhile. Through the headless `Drive`, whose
//! `hide`, `wake`, `due` and `show` are what the windowed runner does
//! (`mod unseen`).

use std::sync::{Arc, Mutex};

use kui_native::testing::Drive;
use kui_native::{App, Core, FrameCause, NodeSpec, Size, TextStyle, Ui, UiEvent, WindowEvents};

/// Notes another thread hands over, as a sync engine or a folder watcher
/// does, taken in by the view; and a sweep the view asks to run every two
/// seconds.
#[derive(Default)]
struct Inbox {
    arrived: Arc<Mutex<Vec<String>>>,
    notes: Vec<String>,
    sweeps: u32,
    next_sweep: f64,
    /// Why each frame ran, as the view read it.
    causes: Vec<FrameCause>,
    resizes: Vec<(f64, f64)>,
}

impl App for Inbox {
    fn view(&mut self, ui: &mut Ui<'_>) {
        self.causes.push(ui.core().frame_cause());
        self.notes.append(&mut self.arrived.lock().unwrap());
        let now = ui.now();
        if now >= self.next_sweep {
            self.sweeps += 1;
            self.next_sweep = now + 2.0;
        }
        ui.request_frame_at(self.next_sweep);
        ui.window_events(WindowEvents::default().on_resize("resized"));
        ui.with_keyed("list", NodeSpec::column().fill(), |ui| {
            for (i, n) in self.notes.iter().enumerate() {
                ui.text_in_keyed(&format!("n{i}"), NodeSpec::row(), n, TextStyle::new(12.0));
            }
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        if let kui_native::Event::Resize(r) = ev.read() {
            self.resizes
                .push((f64::from(r.size.w), f64::from(r.size.h)));
        }
    }
}

#[test]
fn a_hidden_window_runs_on_a_wake_and_a_due_time_and_draws_when_back() {
    let mut app = Inbox::default();
    let inbox = app.arrived.clone();
    let mut d = Drive::new(Core::new(), 320.0, 240.0);
    d.frame(&mut app);
    assert_eq!((d.drawn(), app.sweeps), (1, 1));

    // Minimized: on Windows the window reports no size at all.
    d.hide();
    d.viewport = Size::new(0.0, 0.0);

    // Another thread hands a note over and wakes the loop.
    inbox.lock().unwrap().push("from another device".into());
    d.wake(&mut app);
    assert_eq!(app.notes, ["from another device"], "the view took it in");
    assert!(app.causes.last().unwrap().contains(FrameCause::WAKE));
    assert_eq!(d.drawn(), 1, "and nothing was drawn");
    assert!(
        app.resizes.is_empty(),
        "nor a resize heard: {:?}",
        app.resizes
    );

    // Not yet two seconds: nothing due.
    d.advance(1.0);
    assert!(!d.due(&mut app));
    assert_eq!(app.sweeps, 1);
    // Two seconds on: the sweep the view asked for runs, unseen.
    d.advance(1.0);
    assert!(d.due(&mut app));
    assert_eq!(app.sweeps, 2);
    inbox.lock().unwrap().push("and another".into());
    d.advance(2.0);
    assert!(d.due(&mut app));
    assert_eq!(app.sweeps, 3);
    assert_eq!(app.notes.len(), 2, "a due time takes in what arrived too");
    assert_eq!(d.drawn(), 1, "still nothing drawn");
    assert!(d.hidden());

    // Restored at the size it had: the frame that brings it back draws
    // both notes, and no resize went by.
    d.viewport = Size::new(320.0, 240.0);
    d.show(&mut app);
    assert_eq!(d.drawn(), 2);
    assert!(app.causes.last().unwrap().contains(FrameCause::OCCLUSION));
    assert_eq!(
        d.texts_under("list"),
        ["from another device", "and another"]
    );
    let (list, _) = d.core.output();
    assert!(!list.quads.is_empty(), "drawn");
    assert!(app.resizes.is_empty(), "{:?}", app.resizes);
    assert!(d.warnings().is_empty(), "{:?}", d.warnings());

    // Shown, a wake draws as it always did.
    inbox.lock().unwrap().push("third".into());
    d.wake(&mut app);
    assert_eq!((d.drawn(), app.notes.len()), (3, 3));
}

/// Restored at another size, the frame that brings it back is a resize,
/// as the window's is.
#[test]
fn a_window_restored_at_another_size_hears_one_resize() {
    let mut app = Inbox::default();
    let mut d = Drive::new(Core::new(), 320.0, 240.0);
    d.frame(&mut app);
    d.hide();
    d.viewport = Size::new(0.0, 0.0);
    d.wake(&mut app);
    d.wake(&mut app);
    assert!(app.resizes.is_empty());
    d.viewport = Size::new(400.0, 300.0);
    d.show(&mut app);
    assert_eq!(app.resizes, [(400.0, 300.0)]);
}
