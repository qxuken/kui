//! Data arriving off the loop's thread: a "PTY" that produces a line every
//! 40 ms on a thread of its own, and a view that shows the last ten. The
//! loop parks between events (`ControlFlow::Wait`), so without a
//! [`kui_native::Waker`] the window would show whatever it drew at the last
//! key press. `setup` hands the app the waker once; the thread clones it
//! and calls `wake()` after every line, and the loop draws. Under
//! `KUI_WAKER_LINES=n` the app closes after `n` lines and prints how many
//! frames it drew for them, which is what the by-hand check reads; and
//! `teardown` prints once more as the window goes — closed by the app,
//! by its button or by Quit alike — which is where an app saves what it
//! would lose (backlog F74).
//!
//! Run: cargo run -p kui-native --example waker

use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use kui_devtools::Example;
#[cfg(not(target_arch = "wasm32"))]
use kui_native::Waker;
use kui_native::{App, NodeSpec, TextStyle, Ui, WindowCommand};

struct Feed {
    lines: Arc<Mutex<Vec<String>>>,
    frames: usize,
    #[cfg(target_arch = "wasm32")]
    started: Option<f64>,
    limit: Option<usize>,
}

impl App for Feed {
    #[cfg(not(target_arch = "wasm32"))]
    fn setup(&mut self, waker: Waker) {
        let lines = Arc::clone(&self.lines);
        let limit = self.limit;
        std::thread::spawn(move || {
            let mut n = 0usize;
            loop {
                std::thread::sleep(Duration::from_millis(40));
                n += 1;
                lines
                    .lock()
                    .unwrap()
                    .push(format!("line {n}: {:>6} bytes", n * 137 % 4096));
                waker.wake();
                if limit.is_some_and(|l| n >= l) {
                    break;
                }
            }
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn teardown(&mut self) {
        println!("teardown after {} frames", self.frames);
    }

    fn view(&mut self, ui: &mut Ui<'_>) {
        #[cfg(target_arch = "wasm32")]
        {
            let now = ui.now();
            let start = *self.started.get_or_insert(now);
            let count = ((now - start) / 0.04) as usize;
            let mut lines = self.lines.lock().unwrap();
            for n in lines.len() + 1..=count {
                lines.push(format!("line {n}: {:>6} bytes", n * 137 % 4096));
            }
            ui.request_frame_at(start + (count + 1) as f64 * 0.04);
        }
        let t = ui.theme();
        self.frames += 1;
        let lines = self.lines.lock().unwrap().clone();
        if let Some(limit) = self.limit
            && lines.len() >= limit
        {
            println!(
                "{} lines arrived, {} frames drawn",
                lines.len(),
                self.frames
            );
            ui.window_command(WindowCommand::Close(ui.env().window.id));
        }
        ui.with(
            NodeSpec::column().fill().pad(16.0).gap(4.0).bg(t.bg),
            |ui| {
                ui.text(
                    &format!(
                        "{} lines, {} frames — {}",
                        lines.len(),
                        self.frames,
                        if cfg!(target_arch = "wasm32") {
                            "a browser timer woke the loop"
                        } else {
                            "a thread woke the loop"
                        }
                    ),
                    TextStyle::new(13.0).mono().color(t.muted),
                );
                for line in lines.iter().rev().take(10).rev() {
                    ui.text_in(
                        NodeSpec::row().height(18.0),
                        line,
                        TextStyle::new(13.0).mono().color(t.fg),
                    );
                }
            },
        );
    }
}

impl Example for Feed {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(480.0, 260.0)
    }
}

kui_devtools::main!(Feed {
    lines: Arc::new(Mutex::new(Vec::new())),
    frames: 0,
    #[cfg(target_arch = "wasm32")]
    started: None,
    limit: std::env::var("KUI_WAKER_LINES")
        .ok()
        .and_then(|s| s.parse().ok()),
});
