//! Step 1 — a window with text. The smallest kui program: an `App` whose
//! `view` declares one text node, and a launcher that opens a window
//! around it. Chapter 2 of the book (`docs/book`).
//!
//! Run: cargo run -p kui-native --example tutorial_01_hello

// ANCHOR: all
use kui_native::{App, TextStyle, Ui};

/// The app is any type. It holds the state; this one has none yet.
struct Hello;

impl App for Hello {
    /// Called once per frame. Everything on screen is declared here,
    /// from scratch, every time.
    fn view(&mut self, ui: &mut Ui<'_>) {
        let theme = ui.theme();
        ui.text("Hello, kui", TextStyle::new(24.0).color(theme.fg));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Hello").size(360.0, 200.0).run(Hello)
}
// ANCHOR_END: all
