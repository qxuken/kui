//! The counter: the smallest app that is the whole pattern, and the one
//! example every binding has in the same shape (`docs/adr/0021`, decision
//! 3). `view` rebuilds the tree from the model; a click arrives in
//! `on_event` as data and moves the model; nothing else happens.
//!
//! What every counter holds, so the four stay one shape: the count with
//! `+1`, `-1` and `reset`; a right-click that declares a `modal` menu on
//! the next frame — the core opens nothing, it reports the press and the
//! view puts a float there — with `+10` and `reset` in it; and a headless
//! drive that clicks all of it and exits non-zero on a wrong answer. The
//! messages are a `#[derive(Message)]` enum, the spelling the book's
//! step 3 teaches (`docs/adr/0039`, decision 6); the other bindings
//! build the same `{kind}` maps by hand.
//!
//! Run: cargo run -p kui-native --example counter [-- --headless]

#[path = "counter/app.rs"]
mod app;

use app::Counter;
use kui_devtools::{Drive, Example};
use kui_native::{App, Core, Ui, UiEvent};

impl App for Counter {
    fn view(&mut self, ui: &mut Ui<'_>) {
        Counter::view(self, ui);
    }

    fn on_event(&mut self, ev: UiEvent) {
        Counter::on_event(self, ev);
    }
}

impl Example for Counter {
    const KEYS: &'static [(&'static str, &'static str)] =
        &[("right-click", "the modal menu"), ("Esc", "dismiss it")];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(560.0, 400.0)
    }

    /// The Rosetta drive: every counter clicks its buttons by label, opens
    /// and dismisses its menu, and checks the model after each.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 560.0, 400.0);
        d.frame(self);
        let inc = d.key_of("+1").ok_or("no +1")?;
        let dec = d.key_of("-1").ok_or("no -1")?;
        d.click_key(self, inc);
        d.click_key(self, inc);
        d.click_key(self, dec);
        d.frame(self);
        d.check(self.count == 1, "two +1 and a -1 count to 1")?;

        // The menu: a secondary press anywhere in the example, then the
        // frame that declares the modal, then its +10.
        d.input(
            self,
            kui_native::InputEvent::CursorMoved(kui_native::Vec2::new(40.0, 40.0)),
        );
        d.input(
            self,
            kui_native::InputEvent::MouseDown {
                button: kui_native::MouseButton::Secondary,
                clicks: 1,
            },
        );
        d.input(
            self,
            kui_native::InputEvent::MouseUp {
                button: kui_native::MouseButton::Secondary,
            },
        );
        d.check(self.menu.is_some(), "a right-click asks for the menu")?;
        d.frame(self);
        let add10 = d.key_of("+10").ok_or("the menu did not open")?;
        d.click_key(self, add10);
        d.frame(self);
        d.check(
            self.count == 11 && self.menu.is_none(),
            "+10 counts and closes the menu",
        )?;

        // Escape dismisses a reopened one without choosing.
        self.menu = Some((40.0, 40.0));
        d.frame(self);
        d.key(self, "escape", Default::default());
        d.check(self.menu.is_none(), "escape dismisses the menu")?;
        d.frame(self);
        let gone = d.key_of("+10").is_none();
        d.check(gone, "and the frame after has no menu")
    }
}

kui_devtools::main!(Counter::default());
