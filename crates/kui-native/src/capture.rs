//! A core's finished frame as pixels (backlog C55, ADR 0054): what a
//! window shows, read back for a test, a screenshot or a smoke run, from
//! a window's core or a headless one alike.
//!
//! The frame is drawn again, from the core's display list, by a renderer
//! of the process's own on a device with no window ([`kui_wgpu::Gpu::windowless`]),
//! opened by the first capture and kept for the next. Not the window's
//! swapchain: that is a render attachment only, and making it readable
//! would cost every frame for a rare capture. So a headless core, which
//! never had a renderer, captures the same way a window's does.
//!
//! What the window shows and the capture does not: what the OS draws
//! behind a translucent window (a backdrop's material, the desktop), and
//! the wallpaper kui draws where there is no material. The capture is
//! cleared to the theme's `bg`, which is what an opaque window is cleared
//! to.

use kui_core::Core;
use kui_wgpu::wgpu;

/// A frame read back by [`capture`]: `width` by `height` physical pixels,
/// four bytes each (RGBA), row by row from the top left, alpha not
/// premultiplied — the shape `Core::add_image` and [`crate::decode_image`]
/// use — and the scale the frame was drawn at.
#[derive(Clone, Debug, PartialEq)]
pub struct Capture {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub rgba: Vec<u8>,
}

/// The process's capture renderer, opened by the first capture. A
/// `Mutex` because a capture may come from any thread a binding calls on;
/// one at a time is what a GPU readback is anyway.
static RENDERER: std::sync::Mutex<Option<kui_wgpu::Renderer>> = std::sync::Mutex::new(None);

/// The core's last finished frame, drawn again and read back.
///
/// Call it between frames: while one builds, the list is the one the
/// frame began by emptying. The first capture in a process opens a GPU
/// device of its own and builds its pipelines: about 40–100 ms on an M3
/// Pro. Later ones cost the draw, the wait for the GPU and the 30 MB copy
/// out: about 10 ms for a 3456 by 2234 frame. The window's own renderer is
/// not touched: its atlas changes stay its to upload.
///
/// Fails, saying why, for a core that has not finished a frame, a frame
/// larger than the device's largest texture, or a machine with no GPU
/// adapter (a container with no software Vulkan); after a lost device
/// the next capture opens a new one.
pub fn capture(core: &Core) -> Result<Capture, String> {
    capture_begin(core)?.wait()
}

/// A frame [`capture_begin`] drew and asked back, not yet waited for:
/// the GPU's wait and the copy out are [`PendingFrame::wait`]'s, on
/// whichever thread calls it.
pub(crate) struct PendingFrame {
    pending: kui_wgpu::PendingCapture,
    scale: f32,
}

impl PendingFrame {
    pub(crate) fn wait(self) -> Result<Capture, String> {
        match self.pending.wait() {
            Ok(c) => Ok(Capture {
                width: c.width,
                height: c.height,
                scale: self.scale,
                rgba: c.rgba,
            }),
            // The next capture finds the device lost and opens another.
            Err(kui_wgpu::RenderError::DeviceLost) => {
                Err("the GPU device was lost; the next capture opens another".into())
            }
            Err(_) => Err("the GPU could not read the frame back".into()),
        }
    }
}

/// [`capture`]'s first half, on the thread that has the core: the frame
/// drawn on the process's capture device and its readback submitted,
/// about half a millisecond, with nothing waited for.
pub(crate) fn capture_begin(core: &Core) -> Result<PendingFrame, String> {
    let (dl, atlas) = core.drawn();
    if dl.viewport.w < 1.0 || dl.viewport.h < 1.0 {
        return Err("no finished frame to capture".into());
    }
    let mut slot = RENDERER.lock().unwrap_or_else(|e| e.into_inner());
    if slot.as_ref().is_some_and(|r| r.gpu().lost()) {
        *slot = None;
    }
    if slot.is_none() {
        let gpu = pollster::block_on(kui_wgpu::Gpu::windowless())
            .map_err(|e| format!("no GPU to draw the frame on: {e}"))?;
        let r = kui_wgpu::Renderer::windowless(&gpu, wgpu::TextureFormat::Rgba8Unorm)
            .map_err(|e| format!("no renderer to draw the frame with: {e}"))?;
        *slot = Some(r);
    }
    let r = slot.as_mut().expect("opened above");
    // What the runner clears an opaque window to (ADR 0019).
    let bg = core.theme().bg;
    r.clear_color = wgpu::Color {
        r: bg.r as f64,
        g: bg.g as f64,
        b: bg.b as f64,
        a: bg.a as f64,
    };
    match r.capture_begin(dl, atlas) {
        Ok(pending) => Ok(PendingFrame {
            pending,
            scale: dl.scale,
        }),
        Err(kui_wgpu::RenderError::DeviceLost) => {
            *slot = None;
            Err("the GPU device was lost; the next capture opens another".into())
        }
        Err(_) => Err(format!(
            "the device refused a {} by {} frame (its largest texture is {})",
            dl.viewport.w,
            dl.viewport.h,
            r.gpu().device().limits().max_texture_dimension_2d
        )),
    }
}

/// Serves what a core's app asked to keep of its window (ADR 0055):
/// `capture_to`'s stills and `record_to`'s recordings, written to their
/// paths, each answered with a `capture` event in the message it named.
///
/// One per core, kept beside it: it holds a running recording's encoder
/// and the frame it has not shown for long enough yet. The runner keeps
/// one per window and serves it after every frame that window draws; a
/// headless driver serves its own after each frame:
///
/// ```no_run
/// use kui_native::{Captures, Core, NodeSpec, Size, Value};
///
/// let mut core = Core::new();
/// let mut captures = Captures::new();
/// core.capture_to("shot.png", Some(Value::str("saved")));
/// let mut ui = core.frame(Size::new(320.0, 200.0), 2.0);
/// ui.with(NodeSpec::column().fill(), |_| {});
/// ui.finish();
/// captures.serve(&mut core);
/// // `capture: {phase: "saved", path: "shot.png", width: 640, height: 400, …}`
/// let events = core.take_pending_events();
/// ```
///
/// By default the work is done on the thread that serves, and every
/// answer is pending when `serve` returns: the capture (about 10 ms at
/// 3456 by 2234, 2 ms at 800 by 600), a still's encode (12 ms for a
/// retina PNG), a recorded frame's add (2 ms), and a recording's finish
/// (15 ms for a retina APNG, 0.4 s for a GIF). [`Captures::write_on_thread`]
/// moves a recording to a thread of its own, as the runner does for every
/// window: the serving thread only draws each frame again and submits
/// it (about half a millisecond), and the recording's answer comes on a
/// later serve, the thread having asked for the frame that brings it.
#[derive(Default)]
pub struct Captures {
    recording: Option<Recording>,
    /// A recording written on its own thread, running.
    worker: Option<Worker>,
    /// Stopped recordings whose threads are still writing.
    finishing: Vec<Worker>,
    /// What asks for a frame once a thread has written a recording:
    /// `Some` puts recordings on threads.
    wake: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
}

struct Recording {
    path: String,
    message: Option<kui_core::Value>,
    format: crate::AnimationEncoding,
    /// Made with the first frame, which sets the recording's size.
    encoder: Option<crate::AnimationEncoder>,
    /// The frame drawn last and when, on the frame clock and the wall's:
    /// it shows until the next one is drawn.
    held: Option<(Vec<u8>, f64, std::time::Instant)>,
    frames: u32,
    seconds: f64,
}

/// What a written recording reports: its size, frames and seconds.
struct Written {
    width: u32,
    height: u32,
    frames: u32,
    seconds: f64,
}

/// A recording on a thread of its own: the frames go to it as they are
/// drawn, and what came of it comes back once it is written.
struct Worker {
    path: String,
    message: Option<kui_core::Value>,
    /// Dropped to tell the thread the window went.
    jobs: Option<std::sync::mpsc::SyncSender<Job>>,
    done: std::sync::mpsc::Receiver<Result<Written, String>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

enum Job {
    /// A frame drawn at a moment of the frame clock, its pixels on the way.
    Frame(PendingFrame, f64),
    /// The recording stops at a moment: the last frame shows until then.
    Stop(f64),
}

/// How many frames may wait for a recording's thread before the serving
/// thread waits for it in turn: each holds a frame's readback (30 MB at
/// retina), and a thread that keeps up never lets two pile up.
const QUEUED_FRAMES: usize = 4;

impl Captures {
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes recordings on threads of their own from here on (see the
    /// type's docs), `wake` asking for the frame that brings a written
    /// recording's answer back — `Waker::wake`, for the runner. Stills
    /// stay on the serving thread.
    pub fn write_on_thread(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        self.wake = Some(std::sync::Arc::new(wake));
    }

    /// Whether [`Captures::write_on_thread`] was called.
    pub fn writes_on_thread(&self) -> bool {
        self.wake.is_some()
    }

    /// Whether a recording is running: what a driver that draws only on
    /// change may read to keep drawing for one.
    pub fn recording(&self) -> bool {
        self.recording.is_some() || self.worker.is_some()
    }

    /// A driver's turn after a frame is drawn: drains the core's capture
    /// commands, in order, against the frame it finished. A still is
    /// written; a recording that ran before the turn shows its last frame
    /// until now, one asked for starts with this frame, and a stop
    /// writes the file without it. What was written, or why not, is a
    /// `capture` event pending on the core for each ask that named a
    /// message — at once, or on a later turn for a recording written on
    /// its own thread.
    pub fn serve(&mut self, core: &mut Core) {
        self.collect(core);
        let cmds = core.take_capture_commands();
        if cmds.is_empty() && !self.recording() {
            return;
        }
        let now = core.now();
        // The frame, captured once for whatever this turn needs of it.
        let mut shot: Option<Result<Capture, String>> = None;
        // A recording from before this turn: its last frame shows until
        // now, and this one is held next, unless a stop below ends it.
        let mut hold = false;
        if let Some(r) = self.recording.as_mut() {
            if let Err(why) = r.show_held(now) {
                let r = self.recording.take().expect("running");
                fail(core, r.message, &r.path, why);
            } else {
                hold = true;
            }
        }
        hold |= self.worker.is_some();
        for cmd in cmds {
            match cmd {
                kui_core::CaptureCommand::Still { path, message } => {
                    let done = match kui_core::CaptureFormat::of_still(&path) {
                        Err(why) => Err(why),
                        Ok(format) => match shot.get_or_insert_with(|| capture(core)) {
                            Ok(c) => write_still(&path, format, c).map(|()| (c.width, c.height)),
                            Err(why) => Err(why.clone()),
                        },
                    };
                    match done {
                        Ok((w, h)) => saved(core, message, &path, w, h, 1, 0.0),
                        Err(why) => fail(core, message, &path, why),
                    }
                }
                kui_core::CaptureCommand::Clipboard { message } => {
                    // Queued for the driver to apply with its clipboard,
                    // as a copy of text is: the runner after the frame, a
                    // headless host from its own drain.
                    match shot.get_or_insert_with(|| capture(core)) {
                        Ok(c) => {
                            let (w, h, rgba) = (c.width, c.height, c.rgba.clone());
                            core.set_clipboard_image(w, h, rgba);
                            saved(core, message, "", w, h, 1, 0.0);
                        }
                        Err(why) => {
                            let why = why.clone();
                            fail(core, message, "", why);
                        }
                    }
                }
                kui_core::CaptureCommand::Record { path, message } => {
                    let running = self
                        .recording
                        .as_ref()
                        .map(|r| &r.path)
                        .or(self.worker.as_ref().map(|w| &w.path));
                    if let Some(running) = running {
                        let why = format!("already recording to {running:?}");
                        fail(core, message, &path, why);
                        continue;
                    }
                    match kui_core::CaptureFormat::of_recording(&path) {
                        Ok(f) => {
                            let r = Recording::new(path, message, f);
                            match &self.wake {
                                Some(wake) => self.worker = Some(Worker::spawn(r, wake.clone())),
                                None => self.recording = Some(r),
                            }
                            hold = true;
                        }
                        Err(why) => fail(core, message, &path, why),
                    }
                }
                kui_core::CaptureCommand::Stop => {
                    if let Some(r) = self.recording.take() {
                        hold = false;
                        let (message, path) = (r.message.clone(), r.path.clone());
                        match r.write(now) {
                            Ok(w) => {
                                saved(core, message, &path, w.width, w.height, w.frames, w.seconds)
                            }
                            Err(why) => fail(core, message, &path, why),
                        }
                    }
                    if let Some(mut w) = self.worker.take() {
                        hold = false;
                        w.send(Job::Stop(now));
                        self.finishing.push(w);
                    }
                }
            }
        }
        if hold && let Some(r) = self.recording.as_mut() {
            let held = shot
                .take()
                .unwrap_or_else(|| capture(core))
                .and_then(|c| r.hold(c, now));
            if let Err(why) = held {
                let r = self.recording.take().expect("running");
                fail(core, r.message, &r.path, why);
            }
        }
        if hold && let Some(w) = self.worker.as_mut() {
            match capture_begin(core) {
                Ok(frame) => w.send(Job::Frame(frame, now)),
                Err(why) => {
                    let w = self.worker.take().expect("running");
                    fail(core, w.message.clone(), &w.path, why);
                }
            }
        }
    }

    /// The answers of recordings whose threads have finished: written, or
    /// stopped by a failure while still running.
    fn collect(&mut self, core: &mut Core) {
        let mut i = 0;
        while i < self.finishing.len() {
            match self.finishing[i].done.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Empty) => i += 1,
                got => {
                    let mut w = self.finishing.remove(i);
                    w.answer(core, got.ok());
                }
            }
        }
        if let Some(w) = &self.worker
            && let Ok(got) = w.done.try_recv()
        {
            let mut w = self.worker.take().expect("running");
            w.answer(core, Some(got));
        }
    }
}

impl Drop for Captures {
    /// A recording still running when its window goes is written, its
    /// last frame shown for as long as it was on screen; its core went
    /// with the window, so nothing hears it. A thread still writing one
    /// is waited for, so the file is whole when the process ends.
    fn drop(&mut self) {
        if let Some(r) = self.recording.take()
            && let Err(e) = r.write_dropped()
        {
            eprintln!("kui: writing the recording: {e}");
        }
        for mut w in self
            .worker
            .take()
            .into_iter()
            .chain(self.finishing.drain(..))
        {
            w.jobs = None;
            if let Some(t) = w.thread.take() {
                let _ = t.join();
            }
        }
    }
}

impl Worker {
    fn spawn(r: Recording, wake: std::sync::Arc<dyn Fn() + Send + Sync>) -> Self {
        let (jobs, rx) = std::sync::mpsc::sync_channel(QUEUED_FRAMES);
        let (tx, done) = std::sync::mpsc::channel();
        let (path, message) = (r.path.clone(), r.message.clone());
        let thread = std::thread::Builder::new()
            .name("kui-recording".into())
            .spawn(move || {
                let _ = tx.send(r.record(rx));
                wake();
            })
            .ok();
        Self {
            path,
            message,
            jobs: Some(jobs),
            done,
            thread,
        }
    }

    /// Hands the thread a job; one that has stopped (it failed) is heard
    /// at the next collect.
    fn send(&mut self, job: Job) {
        if let Some(jobs) = &self.jobs
            && jobs.send(job).is_err()
        {
            self.jobs = None;
        }
    }

    /// Joins the thread and says what came of the recording; `None` when
    /// the thread ended without a word (it panicked).
    fn answer(&mut self, core: &mut Core, got: Option<Result<Written, String>>) {
        self.jobs = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let got =
            got.unwrap_or_else(|| Err("the recording's thread ended without writing it".into()));
        match got {
            Ok(w) => saved(
                core,
                self.message.take(),
                &self.path,
                w.width,
                w.height,
                w.frames,
                w.seconds,
            ),
            Err(why) => fail(core, self.message.take(), &self.path, why),
        }
    }
}

impl Recording {
    fn new(path: String, message: Option<kui_core::Value>, f: kui_core::CaptureFormat) -> Self {
        Self {
            path,
            message,
            format: match f {
                kui_core::CaptureFormat::Gif => crate::AnimationEncoding::Gif,
                _ => crate::AnimationEncoding::Png,
            },
            encoder: None,
            held: None,
            frames: 0,
            seconds: 0.0,
        }
    }

    /// A recording's thread: each frame shows the one before until it was
    /// drawn, is waited for and held; a stop writes the file. A window
    /// that went with the recording running (the jobs ending) writes it as
    /// a dropped one does.
    fn record(mut self, jobs: std::sync::mpsc::Receiver<Job>) -> Result<Written, String> {
        for job in jobs {
            match job {
                Job::Frame(frame, at) => {
                    self.show_held(at)?;
                    self.hold(frame.wait()?, at)?;
                }
                Job::Stop(at) => return self.write(at),
            }
        }
        self.write_dropped()
    }

    /// The held frame shown until `now`.
    fn show_held(&mut self, now: f64) -> Result<(), String> {
        let Some((px, at, _)) = self.held.take() else {
            return Ok(());
        };
        let secs = (now - at).max(0.0);
        self.encoder
            .as_mut()
            .expect("a frame was held, so the encoder was made")
            .add(&px, secs)?;
        self.seconds += secs;
        Ok(())
    }

    /// Holds `c`, drawn at `now`, to show until the next frame; the first
    /// one sets the recording's size. A frame of another size (the window
    /// resized) is cut at the right and bottom, or padded there with
    /// transparent pixels.
    fn hold(&mut self, c: Capture, now: f64) -> Result<(), String> {
        let enc = match &mut self.encoder {
            Some(e) => e,
            None => self.encoder.insert(crate::AnimationEncoder::new(
                self.format,
                c.width,
                c.height,
                None,
            )?),
        };
        let (w, h) = enc.size();
        let px = if (c.width, c.height) == (w, h) {
            c.rgba
        } else {
            fit(&c, w, h)
        };
        self.held = Some((px, now, std::time::Instant::now()));
        self.frames += 1;
        Ok(())
    }

    /// Writes the file, without the frame drawn at the stop at `now`.
    fn write(mut self, now: f64) -> Result<Written, String> {
        self.show_held(now)?;
        self.write_file()
    }

    /// Writes the file of a recording whose window went: its last frame
    /// shown for as long as it was on screen, by the wall's clock.
    fn write_dropped(mut self) -> Result<Written, String> {
        if let Some((px, _, at)) = self.held.take()
            && let Some(enc) = self.encoder.as_mut()
        {
            let secs = at.elapsed().as_secs_f64();
            enc.add(&px, secs)?;
            self.seconds += secs;
        }
        self.write_file()
    }

    fn write_file(mut self) -> Result<Written, String> {
        let enc = self
            .encoder
            .take()
            .ok_or_else(|| "no frame was drawn while recording".to_string())?;
        let (width, height) = enc.size();
        let bytes = enc.finish()?;
        std::fs::write(&self.path, bytes).map_err(|e| format!("writing {:?}: {e}", self.path))?;
        Ok(Written {
            width,
            height,
            frames: self.frames,
            seconds: self.seconds,
        })
    }
}

/// `c` cut or padded to `w` by `h`, from the top left.
fn fit(c: &Capture, w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0; w as usize * h as usize * 4];
    let cols = w.min(c.width) as usize * 4;
    for y in 0..h.min(c.height) as usize {
        let from = y * c.width as usize * 4;
        let to = y * w as usize * 4;
        out[to..to + cols].copy_from_slice(&c.rgba[from..from + cols]);
    }
    out
}

fn write_still(path: &str, format: kui_core::CaptureFormat, c: &Capture) -> Result<(), String> {
    let format = match format {
        kui_core::CaptureFormat::Png => crate::ImageEncoding::Png,
        kui_core::CaptureFormat::Jpeg => crate::ImageEncoding::Jpeg { quality: 90 },
        kui_core::CaptureFormat::WebP => crate::ImageEncoding::WebP,
        kui_core::CaptureFormat::Gif => crate::ImageEncoding::Gif,
    };
    let bytes = crate::encode_image(format, c.width, c.height, &c.rgba)?;
    std::fs::write(path, bytes).map_err(|e| format!("writing {path:?}: {e}"))
}

fn saved(
    core: &mut Core,
    message: Option<kui_core::Value>,
    path: &str,
    width: u32,
    height: u32,
    frames: u32,
    seconds: f64,
) {
    core.captured(
        message,
        kui_core::Captured {
            phase: kui_core::CapturePhase::Saved,
            path: path.to_string(),
            width,
            height,
            frames,
            seconds,
            error: None,
        },
    );
}

fn fail(core: &mut Core, message: Option<kui_core::Value>, path: &str, why: String) {
    if message.is_none() {
        eprintln!("kui: capturing to {path:?}: {why}");
    }
    core.captured(
        message,
        kui_core::Captured {
            phase: kui_core::CapturePhase::Failed,
            path: path.to_string(),
            width: 0,
            height: 0,
            frames: 0,
            seconds: 0.0,
            error: Some(why),
        },
    );
}
