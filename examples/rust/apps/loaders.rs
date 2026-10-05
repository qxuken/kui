//! Loaders: ten ways to say "wait", over one job the app owns. The job
//! is a number from 0 to 1 that a clock moves while it runs; three loaders
//! are *determinate* and draw that number — a pie, a ring, a bar — and seven
//! are *indeterminate* and draw only the time — an arc, a chasing pie,
//! spokes, dots, a sweeping bar, a bar going back and forth, a skeleton.
//!
//! What it shows of composing an app:
//!
//! - **The clock is the model's.** `view` is a function of `phase` and
//!   `progress`; `step` is the only thing that moves them. A window steps
//!   by the wall clock and a headless drive by hand, so the drive is exact.
//! - **A frame is asked for only while something moves** (`request_frame`).
//!   Idle, paused or done, the window draws nothing and costs nothing.
//! - **The round loaders are `path`s** (`docs/adr/0040-a-path-is-a-mask-in-the-atlas.md`):
//!   a pie is `Path::sector` with its sweep the progress, a ring the same
//!   with an inner radius, the arc an open stroked `arc_to`. The fills'
//!   shape changes every frame, which is the case the ADR's decision 8 is
//!   for — each leaves the atlas for a texture of its own and is
//!   rasterized per frame, where the track under it, which never changes,
//!   is one mask in the atlas for good. The arc only *turns*, and that is
//!   `rotate` (`docs/adr/0041-a-mask-turns-about-its-centre.md`): one
//!   mask, in the atlas, turned by the quad that draws it. The spokes are `line`s and the rest are boxes:
//!   a loader that is only rectangles needs no path.
//! - **Reduced motion is honoured** (`env.system.motion`): the
//!   indeterminate loaders stand still and the determinate ones still
//!   report, since progress is information and spinning is not.
//!
//! Run: cargo run -p kui-native --example loaders [-- --headless]

use std::f32::consts::TAU;
use std::time::Instant;

use kui_devtools::{Drive, Example};
use kui_native::widgets;
use kui_native::{
    Align, App, Color, Core, FloatConfig, Message, NodeSpec, Path, QuadKind, Role, Stroke,
    TextStyle, Ui, UiEvent, Vec2,
};

/// How long the job takes at speed 1, in seconds.
const JOB_SECS: f32 = 4.0;
/// A loader's stage: the square its shapes are drawn in, and its centre
/// and radius.
const STAGE: f32 = 96.0;
const C: f32 = STAGE / 2.0;
const R: f32 = 34.0;

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    /// Start, pause, resume or run again: whichever the state offers.
    Toggle,
    Reset,
    /// The speed slider's tag; the value rides on the `change` event.
    Speed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Idle,
    Running,
    Paused,
    Done,
}

struct Loaders {
    state: State,
    /// The job, 0 to 1.
    progress: f32,
    /// Seconds the loaders have been moving: what the indeterminate ones
    /// draw. It stops when the job does, so a paused spinner is paused.
    phase: f32,
    speed: f32,
    /// The wall clock's last reading, for a window; `None` under a drive,
    /// which calls `step` itself.
    last: Option<Instant>,
    manual: bool,
    /// Whether the first frame has given the job's button the keyboard.
    focused: bool,
}

impl Loaders {
    fn new() -> Self {
        Self {
            state: State::Idle,
            progress: 0.0,
            phase: 0.0,
            speed: 1.0,
            last: None,
            manual: false,
            focused: false,
        }
    }

    /// Moves the job and the loaders `dt` seconds on. The one place the
    /// model's time changes.
    fn step(&mut self, dt: f32) {
        if self.state != State::Running {
            return;
        }
        self.phase += dt;
        self.progress = (self.progress + dt * self.speed / JOB_SECS).min(1.0);
        if self.progress >= 1.0 {
            self.state = State::Done;
        }
    }

    /// What the one button does next, which is also its label.
    fn action(&self) -> &'static str {
        match self.state {
            State::Idle => "start",
            State::Running => "pause",
            State::Paused => "resume",
            State::Done => "again",
        }
    }
}

impl App for Loaders {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // A window's clock: the time since the last frame, while running.
        if !self.manual {
            let now = Instant::now();
            let dt = self
                .last
                .map_or(0.0, |l| now.duration_since(l).as_secs_f32());
            self.last = (self.state == State::Running).then_some(now);
            // A frame after a long sleep is not a long step.
            self.step(dt.min(0.1));
        }
        // Nothing else wakes the window for the next step.
        if self.state == State::Running {
            ui.request_frame();
        }

        let t = ui.theme();
        let still = ui.env().system.motion.is_reduced();
        // The time the indeterminate loaders draw: none, when the user
        // asked for less motion.
        let phase = if still { 0.0 } else { self.phase };
        let p = self.progress;
        let done = self.state == State::Done;
        let tint = if done { t.success } else { t.accent };
        let muted = TextStyle::new(12.0).color(t.muted);
        // What a loader runs in: a tint of the text colour, so it shows
        // on the card in either appearance.
        let track = track_color(&t);

        ui.with(
            NodeSpec::column().fill().bg(t.bg).pad(24.0).gap(16.0),
            |ui| {
                // The job: its one button, a reset, the speed, what it says.
                ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                    // The job's button is one node whatever it says — a label
                    // that changes would re-key it and drop the keyboard — and
                    // it has the focus from the first frame, so Space starts
                    // the job and pauses it.
                    let (theme, m) = (ui.theme(), ui.metrics());
                    widgets::button_with(
                        ui,
                        "job",
                        self.action(),
                        widgets::button_spec(&theme, &m).on_click(Msg::Toggle),
                        None,
                    );
                    if !self.focused {
                        self.focused = true;
                        let job = ui.child_key("job");
                        ui.focus(job);
                    }
                    widgets::button(ui, "reset", Msg::Reset);
                    ui.leaf(NodeSpec::row().width(12.0));
                    ui.text("speed", muted);
                    widgets::slider(ui, "speed", self.speed, 0.25, 3.0, 0.25, Msg::Speed);
                    ui.text(
                        &format!("{:.2}×", self.speed),
                        TextStyle::new(13.0).color(t.fg),
                    );
                    ui.leaf(NodeSpec::row().width(12.0));
                    let status = match self.state {
                        State::Idle => "idle".to_string(),
                        State::Running => format!("running · {:.0}%", p * 100.0),
                        State::Paused => format!("paused at {:.0}%", p * 100.0),
                        State::Done => "done".to_string(),
                    };
                    ui.with_keyed("status", NodeSpec::row(), |ui| {
                        ui.text(&status, TextStyle::new(13.0).color(t.fg));
                    });
                });

                ui.text("determinate: the job's progress", muted);
                ui.with(NodeSpec::row().gap(12.0), |ui| {
                    // The pie: a disc for the track, and over it a sector from
                    // twelve o'clock whose sweep is the progress. A full turn
                    // is `sector`'s two half arcs, so 100% is a whole disc.
                    card(ui, "pie", &format!("pie, {:.0}%", p * 100.0), |ui| {
                        ui.path(
                            &Path::sector(C, C, R, 0.0, 0.0, 1.0),
                            NodeSpec::column().bg(track),
                        );
                        if p > 0.0 {
                            ui.path_keyed(
                                "fill",
                                &Path::sector(C, C, R, 0.0, -0.25, p),
                                NodeSpec::column().bg(tint).transition(200.0),
                            );
                        }
                    });
                    // The ring: the same sector with an inner radius, and the
                    // number in its hole.
                    card(ui, "ring", &format!("ring, {:.0}%", p * 100.0), |ui| {
                        ui.path(
                            &Path::sector(C, C, R, R - 9.0, 0.0, 1.0),
                            NodeSpec::column().bg(track),
                        );
                        if p > 0.0 {
                            ui.path_keyed(
                                "fill",
                                &Path::sector(C, C, R, R - 9.0, -0.25, p),
                                NodeSpec::column().bg(tint).transition(200.0),
                            );
                        }
                        ui.with(
                            NodeSpec::column()
                                .float(FloatConfig::parent().inside(Align::Center, Align::Center)),
                            |ui| {
                                ui.text(
                                    &format!("{:.0}", p * 100.0),
                                    TextStyle::new(16.0).color(t.fg),
                                );
                            },
                        );
                    });
                    // The bar: two boxes. No path in it.
                    card(ui, "bar", &format!("bar, {:.0}%", p * 100.0), |ui| {
                        ui.with(NodeSpec::column().fill().center(), |ui| {
                            ui.with(
                                NodeSpec::row()
                                    .size(STAGE - 12.0, 8.0)
                                    .bg(track)
                                    .radius(4.0)
                                    .clip(),
                                |ui| {
                                    ui.leaf(
                                        NodeSpec::row()
                                            .size((STAGE - 12.0) * p, 8.0)
                                            .bg(tint)
                                            .radius(4.0),
                                    );
                                },
                            );
                        });
                    });
                });

                ui.text(
                    if still {
                        "indeterminate: only the time — standing still, as the system asked"
                    } else {
                        "indeterminate: only the time"
                    },
                    muted,
                );
                ui.with(NodeSpec::row().gap(12.0), |ui| {
                    // The arc: an open path, stroked and not filled, turning
                    // once a second — by `rotate`, about the circle's centre,
                    // so it is one shape and one mask at every angle and the
                    // quad that draws it carries the turn (ADR 0041).
                    card(ui, "arc", "arc spinner", |ui| {
                        let end = 0.3 * TAU;
                        let arc = Path::new()
                            .move_to(C + R, C)
                            .arc_to(R, R, 0.0, false, true, C + R * end.cos(), C + R * end.sin())
                            .stroked(Stroke::new(5.0, t.accent))
                            .pivot(C, C)
                            .rotated(phase);
                        ui.path_keyed("arc", &arc, NodeSpec::column());
                    });
                    // The chase: a pie whose head runs ahead of its tail and
                    // is caught, the sweep breathing between a sliver and
                    // three quarters while the whole turns.
                    card(ui, "chase", "chasing pie", |ui| {
                        let sweep = 0.08 + 0.67 * (0.5 - 0.5 * (phase * 0.5 * TAU).cos());
                        let from = phase * 0.75 - sweep * 0.5;
                        ui.path_keyed(
                            "chase",
                            &Path::sector(C, C, R, R - 12.0, from, sweep),
                            NodeSpec::column().bg(t.accent),
                        );
                    });
                    // The spokes: twelve lines, the brightest stepping round
                    // twelve times a second and the rest fading behind it.
                    card(ui, "spokes", "spokes spinner", |ui| {
                        let head = (phase * 12.0).floor() as usize % 12;
                        for i in 0..12 {
                            let a = i as f32 / 12.0 * TAU;
                            let behind = (head + 12 - i) % 12;
                            let alpha = 1.0 - behind as f32 / 12.0 * 0.85;
                            ui.line_indexed(
                                i as u64,
                                Vec2::new(C + 16.0 * a.cos(), C + 16.0 * a.sin()),
                                Vec2::new(C + 30.0 * a.cos(), C + 30.0 * a.sin()),
                                Stroke::new(4.0, t.fg.with_alpha(alpha)),
                                NodeSpec::column(),
                            );
                        }
                    });
                    // The dots: three round boxes, each a third of a beat
                    // behind the last, lifted by a spacer above it.
                    card(ui, "dots", "bouncing dots", |ui| {
                        ui.with(NodeSpec::column().fill().center(), |ui| {
                            ui.with(NodeSpec::row().gap(8.0).height(36.0), |ui| {
                                for i in 0..3 {
                                    let beat = (phase * 1.4 - i as f32 * 0.15).rem_euclid(1.0);
                                    let lift = (beat * TAU * 0.5).sin().max(0.0) * 20.0;
                                    ui.with(NodeSpec::column().size(12.0, 36.0), |ui| {
                                        ui.leaf(NodeSpec::row().size(12.0, 20.0 - lift));
                                        ui.leaf(
                                            NodeSpec::row()
                                                .size(12.0, 12.0)
                                                .radius(6.0)
                                                .bg(t.accent.with_alpha(0.55 + 0.45 * lift / 20.0)),
                                        );
                                    });
                                }
                            });
                        });
                    });
                    // The sweep: a bar that crosses its track and comes back
                    // in from the left, cut by the track's clip at both ends.
                    card(ui, "sweep", "sweeping bar", |ui| {
                        let len = STAGE - 12.0;
                        let run = len * 0.4;
                        let x = (phase * 0.8).rem_euclid(1.0) * (len + run) - run;
                        ui.with(NodeSpec::column().fill().center(), |ui| {
                            ui.with(
                                NodeSpec::row().size(len, 8.0).bg(track).radius(4.0).clip(),
                                |ui| {
                                    ui.leaf(NodeSpec::row().size(x.max(0.0), 8.0));
                                    ui.leaf(
                                        NodeSpec::row()
                                            .size(run + x.min(0.0), 8.0)
                                            .bg(t.accent)
                                            .radius(4.0),
                                    );
                                },
                            );
                        });
                    });
                    // The loop: the same bar going back and forth, slowing
                    // into each end and out of it, and never leaving its track.
                    card(ui, "loop", "bar going back and forth", |ui| {
                        let len = STAGE - 12.0;
                        let run = len * 0.4;
                        ui.with(NodeSpec::column().fill().center(), |ui| {
                            ui.with(
                                NodeSpec::row().size(len, 8.0).bg(track).radius(4.0).clip(),
                                |ui| {
                                    ui.leaf(
                                        NodeSpec::row().size(loop_offset(phase, len, run), 8.0),
                                    );
                                    ui.leaf(
                                        NodeSpec::row().size(run, 8.0).bg(t.accent).radius(4.0),
                                    );
                                },
                            );
                        });
                    });
                    // The skeleton: the shape of what is coming, each line
                    // dimming and brightening a little after the one above.
                    card(ui, "skeleton", "skeleton", |ui| {
                        ui.with(NodeSpec::column().fill().center().gap(8.0), |ui| {
                            for (i, w) in [72.0, 84.0, 56.0].into_iter().enumerate() {
                                let wave =
                                    0.5 + 0.5 * ((phase * 0.9 - i as f32 * 0.12) * TAU).sin();
                                ui.leaf(
                                    NodeSpec::row()
                                        .size(w, 10.0)
                                        .radius(5.0)
                                        .bg(t.fg.with_alpha(0.10 + 0.14 * wave)),
                                );
                            }
                        });
                    });
                });
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            Some(Msg::Toggle) => {
                self.state = match self.state {
                    State::Idle | State::Paused => State::Running,
                    State::Running => State::Paused,
                    State::Done => {
                        self.progress = 0.0;
                        State::Running
                    }
                }
            }
            Some(Msg::Reset) => {
                self.state = State::Idle;
                self.progress = 0.0;
                self.phase = 0.0;
            }
            // A slider proposes; the app stores, and the next frame draws
            // the slider there.
            Some(Msg::Speed) => {
                if let Some(v) = ev.payload.get_float("value") {
                    self.speed = v as f32;
                }
            }
            None => {}
        }
    }
}

/// Where the looping bar's left edge is at `phase`, on a track `len`
/// long: from 0 to `len - run` and back every two and a half seconds, on
/// a cosine, so it eases at both ends.
fn loop_offset(phase: f32, len: f32, run: f32) -> f32 {
    (len - run) * (0.5 - 0.5 * (phase * 0.4 * TAU).cos())
}

/// The colour of a loader's track.
fn track_color(t: &kui_native::Theme) -> Color {
    t.fg.with_alpha(0.10)
}

/// One loader's card: the stage its shapes are drawn in — a path or a
/// line is a float in its parent's box space, so the stage is the origin
/// of every coordinate above — and its name under it. The stage is one
/// image to assistive technology, named for what it shows.
fn card(ui: &mut Ui<'_>, name: &str, says: &str, stage: impl FnOnce(&mut Ui<'_>)) {
    let t = ui.theme();
    ui.with_keyed(
        name,
        NodeSpec::column()
            .pad(12.0)
            .gap(8.0)
            .bg(t.surface)
            .border(1.0, t.border)
            .radius(12.0)
            .cross_align(Align::Center),
        |ui| {
            ui.with_keyed(
                "stage",
                NodeSpec::column()
                    .size(STAGE, STAGE)
                    .role(Role::Image)
                    .label(says),
                stage,
            );
            ui.text(name, TextStyle::new(12.0).color(t.muted));
        },
    );
}

impl Example for Loaders {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        (
            "Space",
            "start the job; the same button pauses and resumes it",
        ),
        (
            "speed",
            "how fast the job goes, not how fast the loaders turn",
        ),
    ];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(1030.0, 470.0)
    }

    /// The job by hand: idle asks for no frame; running, the progress is
    /// the time stepped and the moving paths leave the atlas; paused, it
    /// holds and the window sleeps; done, the pie is a whole disc in the
    /// success colour.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        self.manual = true;
        let mut d = Drive::new(core, 1030.0, 470.0);
        let step = |app: &mut Self, d: &mut Drive<'_>, secs: f32, frames: u32| {
            for _ in 0..frames {
                app.step(secs / frames as f32);
                d.advance(f64::from(secs / frames as f32));
                d.frame(app);
            }
        };
        let count = |core: &mut Core, kind: QuadKind, color: Option<Color>| {
            core.output()
                .0
                .quads
                .iter()
                .filter(|q| q.kind == kind && color.is_none_or(|c| q.color == c))
                .count()
        };
        d.frame(self);
        d.check(!d.core.animating(), "idle, no frame is asked for")?;
        // The two tracks, and the still arc and chase: four masks, all in
        // the atlas (told from the text's glyphs by their colours), and
        // the twelve spokes.
        let (track, accent) = (track_color(d.core.theme()), d.core.theme().accent);
        let masks = count(d.core, QuadKind::GlyphMask, Some(track))
            + count(d.core, QuadKind::GlyphMask, Some(accent));
        d.check(masks == 4, "idle, four paths are four atlas masks")?;
        let spokes = count(d.core, QuadKind::Segment, None);
        d.check(spokes == 12, "and the spokes are twelve segments")?;
        d.check(d.core.path_texture_count() == 0, "with no texture")?;

        // A second of a four-second job, in ten frames.
        let job = d.key_of("job").ok_or("no job button")?;
        d.check(
            d.core.focus() == Some(job),
            "the job's button has the keyboard",
        )?;
        d.key(self, "space", Default::default());
        d.check(self.state == State::Running, "and Space starts the job")?;
        step(self, &mut d, 1.0, 10);
        d.check(d.core.animating(), "running, the next frame is asked for")?;
        d.check((self.progress - 0.25).abs() < 1e-3, "a second is a quarter")?;
        let status = d.texts_under("status");
        d.check(
            status.iter().any(|s| s.contains("25%")),
            "and the status says so",
        )?;
        // The pie's fill, the ring's and the chase change shape every
        // frame: each draws from a texture of its own, and the two tracks
        // stay the atlas's. The arc only turns: it is still its one atlas
        // mask, and its quad carries the angle.
        let textures = count(d.core, QuadKind::Texture, None);
        d.check(textures == 3, "the three changing paths are textures")?;
        let turned: Vec<f32> = d
            .core
            .output()
            .0
            .quads
            .iter()
            .filter(|q| q.kind == QuadKind::GlyphMask && q.color == accent)
            .map(|q| q.blur)
            .collect();
        d.check(
            turned.len() == 1 && (turned[0] - self.phase * TAU).abs() < 1e-3,
            "the arc is one atlas mask, turned by its quad",
        )?;
        let masks = count(d.core, QuadKind::GlyphMask, Some(track));
        d.check(masks == 2, "and the two tracks are still masks")?;
        let warned = d.warnings();
        d.check(warned.is_empty(), "nothing warned")?;

        // The looping bar goes out and comes back: at rest on the left,
        // at the far end half a period on, home again a period on.
        let (len, run) = (84.0, 33.6);
        let at = |phase: f32| loop_offset(phase, len, run);
        d.check(
            at(0.0).abs() < 1e-3 && (at(1.25) - (len - run)).abs() < 1e-3 && at(2.5).abs() < 1e-3,
            "the looping bar goes to the far end and back",
        )?;

        // Paused: time passes and nothing moves.
        d.check(d.core.focus() == Some(job), "the button kept the keyboard")?;
        d.key(self, "space", Default::default());
        step(self, &mut d, 1.0, 2);
        d.check(
            self.state == State::Paused && (self.progress - 0.25).abs() < 1e-3,
            "paused, the job holds",
        )?;
        d.check(!d.core.animating(), "and no frame is asked for")?;

        // Twice as fast, the rest takes a second and a half.
        self.speed = 2.0;
        d.click_key(self, job);
        step(self, &mut d, 1.6, 16);
        d.check(
            self.state == State::Done && self.progress == 1.0,
            "the job ends at 1",
        )?;
        // The fill's colour eases to the success role's.
        step(self, &mut d, 0.5, 2);
        d.check(!d.core.animating(), "done, the window sleeps")?;
        let success = d.core.theme().success;
        let green = count(d.core, QuadKind::Texture, Some(success))
            + count(d.core, QuadKind::GlyphMask, Some(success));
        d.check(green == 2, "the pie and the ring are whole and green")?;

        // Reset: back to nothing drawn over the tracks.
        let reset = d.key_of("reset").ok_or("no reset button")?;
        d.click_key(self, reset);
        d.frame(self);
        d.check(
            self.state == State::Idle && self.progress == 0.0 && self.phase == 0.0,
            "reset is idle at zero",
        )?;
        let says = d.texts_under("job");
        d.check(
            says.iter().any(|s| s == "start"),
            "and the button offers start again",
        )
    }
}

kui_devtools::main!(Loaders::new());
