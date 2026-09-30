//! Step 9 — the world outside the frame. Builds on step 8 with the three
//! doors out of the pure loop: the clipboard (a handler that gets the
//! window's core), a thread that wakes the loop (a clock), and the
//! window itself (its title, and closing it). Chapter 10 of the book.
//!
//! Run: cargo run -p kui-native --example tutorial_09_effects

use std::sync::{Arc, Mutex};
use std::time::Duration;

use kui_native::widgets;
use kui_native::{
    Align, App, Core, Message, NodeSpec, TextStyle, Ui, UiEvent, Waker, WindowCommand,
};

// ANCHOR: message
#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Copy,
    Paste,
    Quit,
    /// The root sink's tag: a paste's text arrives under it.
    Sink,
}
// ANCHOR_END: message

struct Effects {
    /// Seconds since launch, written by a thread.
    ticks: Arc<Mutex<u64>>,
    pasted: String,
    quit: bool,
}

impl App for Effects {
    // ANCHOR: setup
    /// Once, before the window opens. The loop sleeps between events, so
    /// a thread with news calls `wake()` and the loop draws a frame.
    fn setup(&mut self, waker: Waker) {
        let ticks = Arc::clone(&self.ticks);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1));
                *ticks.lock().unwrap() += 1;
                waker.wake();
            }
        });
    }
    // ANCHOR_END: setup

    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let ticks = *self.ticks.lock().unwrap();

        // ANCHOR: window
        // The window is declared like everything else: say the title
        // every frame, and the runner applies it when it changes.
        ui.window_title(&format!("Effects · {ticks}s"));
        if self.quit {
            ui.window_command(WindowCommand::Close(ui.env().window.id));
        }
        // ANCHOR_END: window

        // A key sink with focus, so a paste has somewhere to land.
        let sink = ui.with_keyed(
            "app",
            NodeSpec::column()
                .fill()
                .pad(20.0)
                .gap(12.0)
                .cross_align(Align::Start)
                .bg(t.bg)
                .on_key(Msg::Sink),
            |ui| {
                ui.text(
                    &format!("up for {ticks}s"),
                    TextStyle::new(24.0).color(t.fg),
                );
                ui.with(NodeSpec::row().gap(8.0), |ui| {
                    widgets::button(ui, "Copy the uptime", Msg::Copy);
                    widgets::button(ui, "Paste", Msg::Paste);
                    widgets::button(ui, "Quit", Msg::Quit);
                });
                ui.text(
                    &format!(
                        "pasted: {}",
                        if self.pasted.is_empty() {
                            "nothing yet"
                        } else {
                            &self.pasted
                        }
                    ),
                    TextStyle::new(13.0).color(t.muted),
                );
            },
        );
        ui.take_key_focus(sink);
    }

    // ANCHOR: on_event_with
    /// `on_event` with the core of the window the event came from. What
    /// an app does about an event beyond its model — the clipboard, a
    /// paste, focus — is a call on it here.
    fn on_event_with(&mut self, ev: UiEvent, core: &mut Core) {
        match ev.message::<Msg>() {
            Some(Msg::Copy) => {
                let ticks = *self.ticks.lock().unwrap();
                core.set_clipboard(format!("{ticks}s"), None);
            }
            // The core cannot read the clipboard; the host can. Ask, and
            // the text comes back as an event on the focused sink.
            Some(Msg::Paste) => core.request_paste(),
            Some(Msg::Sink) if ev.kind() == Some("text") => {
                if let Some(text) = ev.payload.get_str("text") {
                    self.pasted = text.to_string();
                }
            }
            Some(Msg::Quit) => self.quit = true,
            _ => {}
        }
    }
    // ANCHOR_END: on_event_with
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Effects").size(420.0, 220.0).run(Effects {
        ticks: Arc::new(Mutex::new(0)),
        pasted: String::new(),
        quit: false,
    })
}
