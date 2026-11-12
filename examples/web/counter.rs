//! The shared Rust counter in the Canvas 2D browser host.

#![cfg(target_arch = "wasm32")]

#[path = "../rust/apps/counter/app.rs"]
mod app;

use app::Counter;
type AppModel = Counter;
use kui_core::{Ui, UiEvent};
use kui_web::{App, Runner};

impl App for Counter {
    fn view(&mut self, ui: &mut Ui<'_>) {
        Counter::view(self, ui);
    }

    fn on_event(&mut self, event: UiEvent) {
        Counter::on_event(self, event);
    }
}

mod host;

fn make_runner() -> Runner<Counter> {
    Runner::new(Counter::default(), &host::session())
}
