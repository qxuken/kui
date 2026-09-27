//! `kui_native::testing::Drive` (backlog DX11), driving an app the way an
//! app's own test would: an owned `Core`, gestures through `view` and
//! `on_event`, and the readings a test asserts on.

use kui_native::testing::Drive;
use kui_native::{App, Core, KeyPhase, NodeSpec, TextStyle, Ui, UiEvent, Vec2};

#[derive(Default)]
struct Pad {
    clicks: u32,
    typed: String,
    split: f32,
    drags: Vec<&'static str>,
}

impl App for Pad {
    fn view(&mut self, ui: &mut Ui<'_>) {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.text_in_keyed(
                "add",
                NodeSpec::row().size(80.0, 30.0).on_click("add"),
                &format!("clicked {}", self.clicks),
                TextStyle::new(12.0),
            );
            ui.leaf_keyed("sink", NodeSpec::row().size(80.0, 30.0).on_key("keys"));
            ui.with(NodeSpec::row().size(200.0, 40.0), |ui| {
                ui.leaf_keyed("bar", NodeSpec::row().size(4.0, 40.0).on_drag("bar"));
            });
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        if let Some(d) = ev.drag() {
            self.split = d.ratio().x;
            self.drags.push(match d.phase {
                kui_native::DragPhase::Start => "start",
                kui_native::DragPhase::Move => "move",
                kui_native::DragPhase::End => "end",
            });
        } else if let Some((KeyPhase::Down, k)) = ev.key_press() {
            self.typed.extend(k.text);
        } else if ev.payload.as_str() == Some("add") {
            self.clicks += 1;
        }
    }
}

#[test]
fn an_owned_drive_clicks_types_drags_and_reads_back() {
    let mut app = Pad::default();
    let mut d = Drive::new(Core::new(), 400.0, 300.0).framing();
    d.frame(&mut app);

    let add = d.key_of("add").expect("the button");
    d.click_key(&mut app, add);
    assert_eq!(app.clicks, 1);
    assert_eq!(
        d.texts_under("add"),
        ["clicked 1"],
        "framed after the click"
    );
    let r = d.rect_of(add).expect("a rect");
    d.click(&mut app, r.center().x, r.center().y);
    assert_eq!(app.clicks, 2, "a pointer click at its middle");

    let sink = d.key_of("sink").expect("the sink");
    d.focus(&mut app, sink);
    d.keys(&mut app, "ab c");
    assert_eq!(app.typed, "ab c", "each press carries its text, space too");

    let bar = d.key_of("bar").unwrap();
    let bar = d.rect_of(bar).unwrap();
    d.drag(&mut app, bar.center(), Vec2::new(150.0, bar.center().y));
    assert_eq!(app.drags.first(), Some(&"start"));
    assert_eq!(app.drags.last(), Some(&"end"));
    assert!((app.split - 0.75).abs() < 0.01, "150 of 200: {}", app.split);

    assert!(d.warnings().is_empty(), "{:?}", d.warnings());
    assert!(d.frames() > 5, "framing frames after each gesture");
    assert!(d.log().iter().any(|l| l.contains("add")), "{:?}", d.log());
}

#[test]
fn a_borrowing_drive_frames_only_when_asked() {
    let mut app = Pad::default();
    let mut core = Core::new();
    let mut d = Drive::new(&mut core, 400.0, 300.0);
    d.frame(&mut app);
    let add = d.key_of("add").unwrap();
    d.click_key(&mut app, add);
    assert_eq!(app.clicks, 1);
    assert_eq!(d.texts_under("add"), ["clicked 0"], "no frame since");
    d.frame(&mut app);
    assert_eq!(d.texts_under("add"), ["clicked 1"]);
    assert_eq!(d.frames(), 2);
}

/// ADR 0036: a handler lent its window's core copies and moves focus in
/// answer to the event, where kawoosh parked `clip_out` and
/// `reclaim_focus` in its model for the next `view`.
#[test]
fn a_handler_acts_on_the_core_its_event_came_from() {
    use kui_native::MenuAction;

    struct Copier;
    impl App for Copier {
        fn view(&mut self, ui: &mut Ui<'_>) {
            ui.with(NodeSpec::row(), |ui| {
                ui.leaf_keyed("copy", NodeSpec::row().size(80.0, 30.0).on_click("copy"));
                ui.leaf_keyed("sink", NodeSpec::row().size(80.0, 30.0).on_key("keys"));
            });
        }
        fn on_event_with(&mut self, ev: UiEvent, core: &mut Core) {
            if ev.payload.as_str() == Some("copy") {
                core.set_clipboard("copied", None);
                let sink = core.key_of("sink");
                core.set_key_focus(sink);
            }
        }
    }

    let mut app = Copier;
    let mut d = Drive::new(Core::new(), 400.0, 300.0);
    d.frame(&mut app);
    let copy = d.key_of("copy").unwrap();
    d.click_key(&mut app, copy);
    let actions = d.core.take_menu_actions();
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, MenuAction::SetClipboard { text, .. } if text == "copied")),
        "the write goes out with this turn: {actions:?}"
    );
    d.frame(&mut app);
    let sink = d.key_of("sink");
    assert_eq!(
        d.core.key_focus(),
        sink,
        "the focus lands for the next frame"
    );
}

/// An app that overrides only `on_event` hears everything as before.
#[test]
fn the_plain_handler_still_hears_everything() {
    let mut app = Pad::default();
    let mut d = Drive::new(Core::new(), 400.0, 300.0);
    d.frame(&mut app);
    let add = d.key_of("add").unwrap();
    d.click_key(&mut app, add);
    assert_eq!(app.clicks, 1);
}
