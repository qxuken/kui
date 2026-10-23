//! `--shot` and `--record`: the example drawn headlessly and kept as a
//! file, with `kui_native::capture` for the pixels and the runner's own
//! encoder for the file (ADR 0054). What the book's Examples part shows
//! (`nu scripts/book-examples.nu pictures`).
//!
//! A picture is pinned so that a rerun draws the same pixels: a bare core
//! with no dock, the light theme unless `--dark` says otherwise, kui's own
//! accent unless `--accent` names one, full motion, grayscale text, and a
//! stepped clock rather than the wall's. The fonts are the machine's, so
//! another OS draws other glyphs. The example's `setup` is not called,
//! since no runner hands out a waker, so an example fed from a thread is
//! drawn as it starts. It needs a GPU adapter (`kui_native::capture`).

use std::path::Path;

use kui_native::{
    AnimationEncoder, AnimationEncoding, Appearance, Core, ImageEncoding, MotionPref, SystemEnv,
};

use crate::{Cli, Drive, Example};

/// The display scale pictures are drawn at: a retina screen's, so text
/// in the book is as sharp as in the window.
pub const SCALE: f32 = 2.0;
/// The recording's clock steps, a second's worth: a GIF frame's time is
/// in hundredths, and 25 a second is four of them exactly.
const FPS: f64 = 25.0;
/// How long a shot waits on the clock for transitions to finish: a
/// keyframe cycle never does, and a view that asks for a frame on every
/// frame never stops asking.
const SETTLE_SECS: f64 = 2.0;

/// What `--record` does to the example: how many seconds it runs and the
/// cues sent on the way, each at its time from the start.
#[derive(Clone, Copy, Debug)]
pub struct Recording {
    pub secs: f64,
    pub cues: &'static [(f64, Cue)],
}

/// One thing a recording does to the example, as a user would.
#[derive(Clone, Copy, Debug)]
pub enum Cue {
    /// A click on the middle of the node keyed under this label
    /// (`Drive::key_of`).
    Click(&'static str),
    /// A key pressed with no modifiers, by name (`Drive::key`).
    Key(&'static str),
}

/// The settled frame, as a PNG at `path`.
pub fn shot<E: Example>(example: &mut E, cli: &Cli, path: &Path) -> Result<(), String> {
    let mut core = core(cli);
    let mut d = drive(&mut core, example, cli);
    settle(&mut d, example);
    let c = kui_native::capture(&*d.core)?;
    let png = kui_native::encode_image(ImageEncoding::Png, c.width, c.height, &c.rgba)?;
    std::fs::write(path, png).map_err(|e| format!("{}: {e}", path.display()))
}

/// The example's [`Recording`] at `path`, a GIF when the path ends in
/// `.gif` and an animated PNG otherwise: the settled frame, then a frame
/// every step of the clock with the cues sent as it reaches them.
///
/// The book's are GIFs: a page whose keyframe cycles never stop changes
/// most of itself every frame, and 3.6 retina seconds of
/// `features/transition` came to 11 MB as an APNG and 3 MB as a GIF,
/// with no colour a UI on one theme misses from 256.
pub fn record<E: Example>(
    example: &mut E,
    cli: &Cli,
    recording: Recording,
    path: &Path,
) -> Result<(), String> {
    let mut core = core(cli);
    let mut d = drive(&mut core, example, cli);
    settle(&mut d, example);
    let step = 1.0 / FPS;
    let steps = (recording.secs * FPS).round() as u32;
    let gif = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("gif"));
    let format = if gif {
        AnimationEncoding::Gif
    } else {
        AnimationEncoding::Png
    };
    let mut cues = recording.cues.iter().peekable();
    let mut enc: Option<AnimationEncoder> = None;
    for i in 0..=steps {
        let t = i as f64 * step;
        while let Some((_, cue)) = cues.next_if(|(at, _)| *at <= t + step / 2.0) {
            match *cue {
                Cue::Click(label) => {
                    let key = d.key_of(label).ok_or_else(|| {
                        format!("the recording clicks {label:?}, and no node is keyed so")
                    })?;
                    d.click_key(example, key);
                }
                Cue::Key(name) => {
                    d.key(example, name, Default::default());
                }
            }
        }
        d.frame(example);
        let c = kui_native::capture(&*d.core)?;
        let enc = match &mut enc {
            Some(enc) => enc,
            None => enc.insert(AnimationEncoder::new(format, c.width, c.height, None)?),
        };
        enc.add(&c.rgba, step)?;
        d.advance(step);
    }
    let apng = enc.expect("a recording is at least one frame").finish()?;
    std::fs::write(path, apng).map_err(|e| format!("{}: {e}", path.display()))
}

/// A bare core with the pins every picture has.
fn core(cli: &Cli) -> Core {
    let mut core = Core::new();
    core.set_system(SystemEnv {
        appearance: cli.base.unwrap_or(Appearance::Light),
        accent: cli.accent,
        motion: cli.motion.unwrap_or(MotionPref::Full),
        ..Default::default()
    });
    core.set_subpixel_text(false);
    core
}

/// A drive at the example's size and [`SCALE`], its extensions loaded,
/// one frame built.
fn drive<'c, E: Example>(core: &'c mut Core, example: &mut E, cli: &Cli) -> Drive<'c> {
    let (w, h) = cli.size.unwrap_or(example.window().size);
    let mut d = Drive::new(core, w as f32, h as f32);
    d.scale = SCALE;
    d.exts = example.extensions();
    d.frame(example);
    d
}

/// Frames on the stepped clock until nothing but keyframe cycles is
/// owed, or [`SETTLE_SECS`] have gone by — and one more, so a view that
/// sizes itself from the first frame's `resize` or `layout` has drawn
/// what it heard.
fn settle<E: Example>(d: &mut Drive<'_>, example: &mut E) {
    let step = 1.0 / FPS;
    d.advance(step);
    d.frame(example);
    let mut waited = step;
    while d.core.owed().beyond_cycles() && waited < SETTLE_SECS {
        d.advance(step);
        d.frame(example);
        waited += step;
    }
}
