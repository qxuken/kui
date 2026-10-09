//! The runner's decoder (backlog F138): an animated GIF shipped in the
//! binary, decoded by `kui_native::decode_animation` and played on the
//! frame clock — `Animation::at` says which frame `ui.now()` shows and
//! when the next is due, `update_image_with` swaps the pixels when it
//! moves, and `request_frame_at` asks for the frame it is due in. No
//! thread, no decoder of the app's own, and nothing owed between frames:
//! the window sleeps the 80 ms between the spinner's steps. Beside it, the
//! same bytes through `decode_image`, which is the first frame, and a
//! button that pauses the spinner, after which nothing is asked for.
//!
//! Run: cargo run -p kui-native --example decode [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::{
    Animation, App, Core, ImageId, NodeSpec, TextStyle, Ui, UiEvent, decode_animation, decode_image,
};

/// Twelve dots round a ring, 64 px, 80 ms a step, looping for ever.
const SPINNER: &[u8] = include_bytes!("../../assets/spinner.gif");

struct Player {
    gif: Animation,
    /// The animated image's handle, and the frame it holds now.
    playing: Option<(ImageId, usize)>,
    still: Option<ImageId>,
    /// When the spinner started, on the frame clock; moved on by the time
    /// it spent paused, so it resumes on the frame it stopped on.
    started: Option<f64>,
    paused: bool,
    /// When the pause began, on the frame clock.
    paused_at: Option<f64>,
}

impl App for Player {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let now = ui.now();
        match (self.paused, self.paused_at) {
            (true, None) => self.paused_at = Some(now),
            (false, Some(at)) => {
                self.started = self.started.map(|s| s + (now - at));
                self.paused_at = None;
            }
            _ => {}
        }
        let started = *self.started.get_or_insert(now);
        let (w, h) = (self.gif.width, self.gif.height);
        let still = *self.still.get_or_insert_with(|| {
            let p = decode_image(SPINNER).expect("the shipped GIF decodes");
            ui.core().resources.add_image(p.width, p.height, p.rgba)
        });
        let (id, shown) = *self.playing.get_or_insert_with(|| {
            let first = self.gif.frames[0].rgba.clone();
            (ui.core().resources.add_image(w, h, first), 0)
        });
        if !self.paused {
            let at = self.gif.at(now - started);
            if at.index != shown {
                let px = &self.gif.frames[at.index].rgba;
                ui.core()
                    .update_image_with(id, w, h, |out| out.copy_from_slice(px));
                self.playing = Some((id, at.index));
            }
            ui.request_frame_at(started + at.next);
        }

        let label = TextStyle::new(12.0).color(t.muted);
        let shown = self.playing.map_or(0, |(_, i)| i);
        ui.with(
            NodeSpec::column().fill().pad(24.0).gap(16.0).bg(t.bg),
            |ui| {
                ui.with(NodeSpec::row().gap(32.0), |ui| {
                    ui.with(NodeSpec::column().gap(8.0), |ui| {
                        ui.image(id, NodeSpec::column().size(128.0, 128.0).label("Spinner"));
                        ui.text(
                            &format!(
                                "decode_animation: frame {} of {}",
                                shown + 1,
                                self.gif.frames.len()
                            ),
                            label,
                        );
                    });
                    ui.with(NodeSpec::column().gap(8.0), |ui| {
                        ui.image(
                            still,
                            NodeSpec::column()
                                .size(128.0, 128.0)
                                .label("Spinner, first frame"),
                        );
                        ui.text("decode_image: the first frame", label);
                    });
                });
                ui.text(
                    &format!(
                        "{} x {} px, {:.0} ms a step, {}",
                        w,
                        h,
                        self.gif.frames[0].delay * 1000.0,
                        match self.gif.loops {
                            None => "looping for ever".to_string(),
                            Some(n) => format!("{n} times"),
                        }
                    ),
                    TextStyle::new(13.0).color(t.fg),
                );
                ui.text_in_keyed(
                    "pause",
                    NodeSpec::row()
                        .pad_xy(14.0, 6.0)
                        .radius(6.0)
                        .bg(t.surface)
                        .on_click("pause"),
                    if self.paused { "Play" } else { "Pause" },
                    TextStyle::new(13.0).color(t.fg),
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.payload.as_str() == Some("pause") {
            self.paused = !self.paused;
        }
    }
}

impl Example for Player {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(420.0, 300.0)
    }

    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 420.0, 300.0);
        d.frame(self);
        let warnings = d.warnings();
        d.check(warnings.is_empty(), "no warnings")?;
        d.check(
            self.gif.frames.len() == 12 && self.gif.loops.is_none(),
            "the shipped GIF is twelve frames, looping",
        )?;
        let due = d.core.next_frame_at().ok_or("no frame asked for")?;
        d.check(
            (due - 0.08).abs() < 1e-9,
            "the next step is asked for 80 ms on",
        )?;
        d.check(!d.core.animating(), "and nothing is owed until then")?;
        d.advance(0.1);
        d.frame(self);
        d.check(
            self.playing.is_some_and(|(_, i)| i == 1),
            "100 ms on, the second frame shows",
        )?;
        let pause = d.key_named("Pause").ok_or("no button named Pause")?;
        d.click_key(self, pause);
        d.frame(self);
        // The step asked for before the pause is still drawn once.
        d.advance(0.1);
        d.frame(self);
        d.check(
            d.core.next_frame_at().is_none(),
            "paused, nothing more is asked for",
        )?;
        d.check(!d.core.animating(), "and nothing is owed")?;
        d.advance(1.0);
        let play = d.key_named("Play").ok_or("no button named Play")?;
        d.click_key(self, play);
        d.frame(self);
        d.check(
            self.playing.is_some_and(|(_, i)| i == 1),
            "a second later it resumes on the frame it stopped on",
        )?;
        d.check(
            d.core.next_frame_at().is_some(),
            "and asks for the next step again",
        )
    }
}

kui_devtools::main!(Player {
    gif: decode_animation(SPINNER).expect("the shipped GIF decodes"),
    playing: None,
    still: None,
    started: None,
    paused: false,
    paused_at: None,
});
