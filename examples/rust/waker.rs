//! Data arriving off the loop's thread: a "PTY" that produces a line every
//! 40 ms on a thread of its own, and a view that shows the last ten. The
//! loop parks between events (`ControlFlow::Wait`), so without a
//! [`kui::Waker`] the window would show whatever it drew at the last
//! key press. `setup` hands the app the waker once; the thread clones it
//! and calls `wake()` after every line, and the loop draws. Under
//! `KUI_WAKER_LINES=n` the app closes after `n` lines and prints how many
//! frames it drew for them, which is what the by-hand check reads.
//!
//! Run: cargo run -p kui --example waker

use std::sync::{Arc, Mutex};
use std::time::Duration;

use kui::{App, NodeSpec, Sizing, TextStyle, Ui, Waker, WindowCommand};

struct Feed {
    lines: Arc<Mutex<Vec<String>>>,
    frames: usize,
    limit: Option<usize>,
}

impl App for Feed {
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

    fn view(&mut self, ui: &mut Ui<'_>) {
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
        ui.configure_root(NodeSpec::column().fill().pad(16.0).gap(4.0).bg(t.bg));
        ui.text(
            &format!(
                "{} lines, {} frames — no input, a thread woke the loop",
                lines.len(),
                self.frames
            ),
            TextStyle::new(13.0).mono().color(t.muted),
        );
        for line in lines.iter().rev().take(10).rev() {
            ui.with(NodeSpec::row().height(Sizing::Fixed(18.0)), |ui| {
                ui.text(line, TextStyle::new(13.0).mono().color(t.fg))
            });
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let limit = std::env::var("KUI_WAKER_LINES")
        .ok()
        .and_then(|s| s.parse().ok());
    kui::app("waker").size(480.0, 260.0).run(Feed {
        lines: Arc::new(Mutex::new(Vec::new())),
        frames: 0,
        limit,
    })
}
