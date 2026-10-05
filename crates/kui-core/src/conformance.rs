//! The scene corpus: test infrastructure, behind the `conformance` feature
//! (off by default), that every binding replays to prove it lowers props
//! and elements the way `kui-core` does.
//!
//! Each [`Scene`](crate::conformance::Scene) is a small view plus the
//! input to replay and the [`Expect`](crate::conformance::Expect)ed
//! outcome; [`drive`](crate::conformance::drive) runs one against a `Core`
//! and [`report`](crate::conformance::report)
//! renders a text block that four languages can produce byte for byte.
//! The crate's own tests assert the font-independent part (access rows,
//! events, warnings, quad counts); the Lua, C and Node suites rebuild the
//! scenes through their public APIs and compare their reports against a
//! dump made on the same machine (`cargo run -p kui-core --features
//! conformance --example conformance-dump`). A prop or element visible to
//! a binding has to appear in a scene here, or the build fails. Nothing in
//! a shipped app needs this module.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

use crate::access::{AccessTree, Role};
use crate::audio::{AudioCommand, AudioSpec};
use crate::color::Color;
use crate::display::{Clip, Quad, QuadKind};
use crate::edit::EditOptions;
use crate::enter::Enter;
use crate::geom::{Edges, Rect, Size, Vec2};
use crate::input::{EditKey, InputEvent, KeyCode, KeyMods, KeyPress, Mods, OptionAsAlt};
use crate::key::Key;
use crate::line::Stroke;
use crate::menu::{BarMenu, MenuBar, MenuItem, MenuRole};
use crate::resources::{ImageId, SoundId};
use crate::runtime::Core;
use crate::spec::{
    Align, Dir, FloatAnchor, FloatConfig, Min, NodeSpec, Overscroll, PadShorthand, ScrollAxes,
    ScrollbarMode, Sizing, TextStyle, TextWrap,
};
use crate::text::Span;
use crate::tree::{NodeContent, Tree};
use crate::ui::Ui;
use crate::value::Value;
use crate::widgets;
use crate::window::{DismissReason, WindowCommand, WindowConfig, WindowEnv, WindowId, WindowRole};

/// Every scene is built at this viewport and scale. A binding that drives
/// its own frames has to use the same numbers or nothing lines up.
pub const VIEWPORT: Size = Size { w: 320.0, h: 240.0 };
pub const SCALE: f32 = 1.0;

/// The host window facts a scene is driven under — the third thing a scene
/// declares to its adapters, beside the tree and the steps.
///
/// A frame driver pushes these in; a headless `Core` has them at their
/// defaults, which is why `widgets::window_buttons` drew nothing in any
/// scene until the corpus could say otherwise (backlog P9). Only the
/// *window* half travels: `refresh_hz` and `focused` change no scene's
/// output, and leaving them out keeps the report line short.
///
/// The controls rect is carried as a `w`/`h` extent at the window origin,
/// not as a free rect, because that is the shape all four bindings can
/// express — C's `kui_env_set_window` takes two numbers, and the one real
/// instance (the macOS traffic lights) sits at the origin.
pub const NATIVE_CHROME: WindowEnv = WindowEnv {
    id: WindowId::MAIN,
    custom_chrome: false,
    maximized: false,
    fullscreen: false,
    always_on_top: false,
    native_controls: None,
};

/// The app draws its own chrome, and the OS draws nothing over it — so
/// `widgets::window_buttons` builds its three buttons.
pub const CUSTOM_CHROME: WindowEnv = WindowEnv {
    id: WindowId::MAIN,
    custom_chrome: true,
    maximized: false,
    fullscreen: false,
    always_on_top: false,
    native_controls: None,
};

/// Custom chrome *and* controls the OS keeps drawing over our content: the
/// macOS traffic lights, at the rect the runner reported before it
/// measured (78x28 logical px at the window origin — gpui's measured
/// `TRAFFIC_LIGHT_PADDING` under the macOS 26 SDK over the 28 px titlebar
/// macOS 26 drew; macOS 27 measures 78x32, and `kui_native::macos_chrome` now
/// asks the window). The corpus keeps the older pair as its fixture: the
/// same tree that builds two button clusters under [`CUSTOM_CHROME`]
/// builds none under this one, its titlebar starts at 78 instead of the
/// bare 12pt margin, and the strip is the keep-out's 28 tall rather than
/// the metric's 34 — which is the whole of what `widgets::titlebar`
/// adapting "per platform by itself" means.
pub const CUSTOM_CHROME_INSET: WindowEnv = WindowEnv {
    id: WindowId::MAIN,
    custom_chrome: true,
    maximized: false,
    fullscreen: false,
    always_on_top: false,
    native_controls: Some(Rect {
        x: 0.0,
        y: 0.0,
        w: 78.0,
        h: 28.0,
    }),
};

/// The fixture image: 4x4 opaque white RGBA. Every binding registers the
/// same bytes in the same order, so the handles match and the image quad
/// comes out of the same atlas slot.
pub const IMAGE_W: u32 = 4;
pub const IMAGE_H: u32 = 4;

pub fn image_pixels() -> Vec<u8> {
    vec![0xff; (IMAGE_W * IMAGE_H * 4) as usize]
}

/// The fixture stream: registered as a copy of the fixture image and then
/// **updated in place** to 8×2 opaque grey before the first frame, so
/// every adapter's `update_image` door is exercised — the dimensions
/// change, the handle does not, and from then on the image is
/// texture-backed (ADR 0025, decisions 1 and 2).
pub const STREAM_W: u32 = 8;
pub const STREAM_H: u32 = 2;

pub fn stream_pixels() -> Vec<u8> {
    let mut px = vec![0x80; (STREAM_W * STREAM_H * 4) as usize];
    for a in px.iter_mut().skip(3).step_by(4) {
        *a = 0xff;
    }
    px
}

/// The fixture sound. Not decodable audio — an `<audio>` node draws nothing
/// and the corpus never plays it; it only has to be a registrable handle.
pub const SOUND_BYTES: &[u8] = b"RIFF....WAVE";

/// The `wrap` scene's boxes, as (width, height). Shared so every adapter
/// writes the same four and a typo cannot pass as a wrapping difference.
pub const WRAP_BOXES: &[(f32, f32)] = &[(30.0, 12.0), (40.0, 16.0), (50.0, 20.0), (20.0, 24.0)];

/// The `tabs` scene's roomy bar: each tab's stand-in label as (width,
/// height). Two tabs in a 200-wide bar split it 100/100, so neither fit
/// floor binds; the 12-tall label lifts a 50%-of-20 tab to 12, the 8-tall
/// one does not.
pub const TAB_ROOMY: &[(f32, f32)] = &[(30.0, 12.0), (50.0, 8.0)];
/// The crowded bar's label widths: 300 of fit floor in the same 200-wide
/// bar, so every tab sits at its label and the bar scrolls by 100.
pub const TAB_CROWDED: &[f32] = &[60.0, 70.0, 80.0, 90.0];

/// The corpus's fragment source, registered as a fixture and mirrored
/// character for character by every adapter. A vertical gradient between
/// two params, plus a ring from a third, so the scene exercises both the
/// parameter block and the prelude's `kui_sd_rounded_box`.
pub const FRAGMENT_WGSL: &str = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let t = clamp(in.local.y / max(in.size.y, 1.0), 0.0, 1.0);
    let base = mix(params[0], params[1], t);
    let d = kui_sd_rounded_box(in.local - in.size * 0.5, in.size * 0.5, vec4<f32>(params[2].x));
    let ring = 1.0 - smoothstep(-KUI_AA, KUI_AA, abs(d) - params[2].y);
    return vec4<f32>(mix(base.rgb, params[3].rgb, ring), base.a);
}";

/// The `fragments` scene's parameters, mirrored by every adapter: two
/// gradient stops, a corner radius and a ring width, and a ring colour.
pub const FRAGMENT_PARAMS: [f32; 16] = [
    0.85, 0.30, 0.25, 1.0, // params[0]: the top of the gradient
    0.20, 0.45, 0.90, 1.0, // params[1]: the bottom
    10.0, 2.0, 0.0, 0.0, // params[2]: the ring's radius and width
    1.0, 1.0, 1.0, 1.0, // params[3]: the ring's colour
];

/// The corpus's second fragment source, one that reads its `image`
/// (backlog V1, ADR 0025 decision 7): the image stretched over the box
/// through `kui_sample`, tinted by the first param, with the image's
/// texel size — `in.image.zw` — folded into the alpha so a source that
/// reads the rect is validated and not just one that samples.
pub const FRAGMENT_IMAGE_WGSL: &str = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let uv = in.local / max(in.size, vec2<f32>(1.0));
    let c = kui_sample(uv) * params[0];
    return vec4<f32>(c.rgb, c.a * step(1.0, in.image.z));
}";

/// The `fragments` scene's tint for the two image-reading fragments.
pub const FRAGMENT_IMAGE_PARAMS: [f32; 4] = [1.0, 0.5, 0.25, 1.0];

/// Eighteen numbers, so the last two are dropped with a warning.
pub const FRAGMENT_PARAMS_LONG: [f32; 18] = [
    0.1, 0.2, 0.3, 1.0, 0.4, 0.5, 0.6, 1.0, 4.0, 1.0, 0.0, 0.0, 0.9, 0.9, 0.2, 1.0, 7.0, 8.0,
];

/// Handles a scene's builder needs, registered before the first frame.
#[derive(Clone, Copy)]
pub struct Fixtures {
    pub image: ImageId,
    /// See [`stream_pixels`]: an image whose pixels were replaced once.
    pub stream: ImageId,
    pub sound: SoundId,
    pub fragment: crate::resources::FragmentId,
    /// See [`FRAGMENT_IMAGE_WGSL`]: the one that samples its `image`.
    pub sampler: crate::resources::FragmentId,
    /// An image registered and removed before any scene builds: the handle
    /// ADR 0025's dead-handle rule is pinned on (draws nothing, warns
    /// nothing). Removed rather than made up, because no raw number is
    /// dead in every process — `from_ffi` reads every handle at an odd
    /// generation, so raw 1 is index 1 at generation 1, the first key the
    /// process's mint hands out, and it stays live — *foreign*, in a Node
    /// process whose earlier sessions the GC has not collected — until
    /// that session drops it. Raw 0 is different: index 0 is the
    /// slot the mint never fills, and "no image" at every door.
    pub dead: ImageId,
}

/// Registers the corpus fixtures on a fresh core, in this order.
pub fn fixtures(core: &mut Core) -> Fixtures {
    let image = core.resources.add_image(IMAGE_W, IMAGE_H, image_pixels());
    let stream = core.resources.add_image(IMAGE_W, IMAGE_H, image_pixels());
    core.update_image(stream, STREAM_W, STREAM_H, stream_pixels());
    let sound = core.add_sound(SOUND_BYTES.to_vec());
    let fragment = core
        .add_fragment(FRAGMENT_WGSL)
        .expect("the corpus fragment must compile");
    let sampler = core
        .add_fragment(FRAGMENT_IMAGE_WGSL)
        .expect("the corpus image fragment must compile");
    let dead = core.resources.add_image(IMAGE_W, IMAGE_H, image_pixels());
    core.resources.remove_image(dead);
    Fixtures {
        image,
        stream,
        sound,
        fragment,
        sampler,
        dead,
    }
}

/// The named keys a [`Step::KeyAtDown`] / [`Step::KeyAtUp`] step names,
/// by index — the way [`ARROWS`] spells an arrow. A
/// binding's adapter keeps the same list.
pub const STEP_KEYS: &[&str] = &["shift", "enter", "capslock"];

/// The press a key-at step makes: `STEP_KEYS[k]` at location `l`.
fn step_key(k: u32, l: u32) -> KeyPress {
    let code = KeyCode::from_name(STEP_KEYS[k as usize]).expect("a step key");
    KeyPress::new(code, KeyMods::default()).with_location(crate::input::KeyLocation::from_bits(
        l << crate::input::KeyLocation::SHIFT,
    ))
}

/// The arrow keys [`Step::Arrow`] indexes, in the order a step line
/// carries. Left / Up move to the previous item of a composite, Right /
/// Down to the next; in a wrapped container the cross-axis pair moves by
/// a line instead.
pub const ARROWS: [EditKey; 4] = [EditKey::Left, EditKey::Right, EditKey::Up, EditKey::Down];
/// The reasons [`Step::WindowDismissed`] indexes, in the order a step line
/// carries — an index, like [`ARROWS`], so every argument stays an integer.
pub const DISMISS_REASONS: [DismissReason; 2] = [DismissReason::Outside, DismissReason::Escape];
/// [`DISMISS_REASONS`] positions, for a scene to read as words.
pub const OUTSIDE: u32 = 0;
pub const ESCAPE: u32 = 1;
/// [`ARROWS`] positions, for a scene to read as words.
pub const LEFT: u32 = 0;
pub const RIGHT: u32 = 1;
pub const UP: u32 = 2;
pub const DOWN: u32 = 3;

/// One replayed input. Values are integers so every adapter can print and
/// parse the step list without agreeing on float formatting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Logical coordinates.
    Cursor(i32, i32),
    CursorLeft,
    MouseDown,
    MouseUp,
    /// The secondary (context-menu) button.
    SecondaryDown,
    SecondaryUp,
    /// The middle button, which only an `onButton` node hears (backlog
    /// F105).
    MiddleDown,
    MiddleUp,
    /// Wheel delta in logical px; positive y scrolls up.
    Scroll(i32, i32),
    /// Tab and Shift-Tab: one step along the focus ring, forwards and
    /// backwards. Spelled as their own step kinds rather than a `key`
    /// step with an argument, so the report's step lines stay one word.
    Tab,
    ShiftTab,
    /// The modifier state, as `KeyMods::bits` — Shift 1, Ctrl 2, Alt 4,
    /// Super 8 — the way `Arrow` is an index: one step kind with an
    /// integer rather than a down and an up per key, so a chord is one
    /// line and every argument stays an integer. `InputEvent::Modifiers`.
    /// What a Shift-press reads: a press while `1` is set extends the
    /// selection from its anchor instead of starting over (ADR 0029,
    /// decision 3).
    Modifiers(u32),
    /// Escape: lets go of a focused control, or asks a modal to go away.
    Escape,
    /// An arrow key, as an index into [`ARROWS`] — inside a composite it
    /// moves the inner selection
    /// (`docs/adr/0007-composite-keyboard-patterns.md`), and a slider it
    /// nudges. An index rather than four step kinds of its own, so every
    /// argument in a step line stays an integer.
    Arrow(u32),
    /// Home and End: the first and last item of a composite.
    Home,
    End,
    /// One printable character, as its Unicode scalar value — the same
    /// integer discipline, since a report has to be produced byte-identically
    /// by four languages. Inside a composite it searches the items by name;
    /// a space presses the focused item unless a search is under way.
    Type(u32),
    /// A raw key going down and coming up on the focused `on_key` sink,
    /// as the character's Unicode scalar value like [`Step::Type`] —
    /// `InputEvent::KeyDown` / `KeyUp` rather than the editing keys above,
    /// which is what a keymap or a held-key binding is routed. No text,
    /// no modifiers, no repeat.
    KeyDown(u32),
    KeyUp(u32),
    /// A named key going down or up at a place on the keyboard (backlog
    /// F108): the key as an index into [`STEP_KEYS`], the place as
    /// `KeyLocation::bits`'s number (0 standard, 1 left, 2 right, 3
    /// numpad) — integers, as every argument is. No modifiers, no text.
    KeyAtDown(u32, u32),
    KeyAtUp(u32, u32),
    /// An IME composing one character (its Unicode scalar value, the caret
    /// at its end) on whatever holds focus — a stock editor shows it
    /// inline, an `on_key` sink hears `{kind="preedit"}`.
    /// Zero is the composition ending without a commit: empty text, no
    /// cursor.
    Preedit(u32),
    /// The IME committing one character: `InputEvent::Commit`, which a
    /// stock editor takes as typed text and a sink hears as
    /// `{kind="text"}`.
    Commit(u32),
    /// The clipboard answering a paste with one character and the
    /// pasteboard's markers as [`crate::input::ClipboardMarks::bits`] —
    /// 1 concealed, 2 transient: `InputEvent::Paste`, which
    /// a stock editor takes as typed text and a sink hears as
    /// `{kind="text"}` with `concealed` / `transient` beside it.
    Paste(u32, u32),
    /// Not an input: the view is a function of a phase, and this is the
    /// view changing its mind. Every scene but `exit` builds the same tree
    /// for every phase; a departing node is one the later phases stop
    /// declaring, which is the only way to ask for an exit at all.
    Phase(u32),
    /// Not an input either: the frame clock, in milliseconds from an
    /// origin the scene picks. A core with no clock snaps every transition
    /// (and an exit that snaps is the plain disappearance it always was),
    /// so a scene that never sets one is a scene where time does not pass.
    Time(u32),
    /// Not an input: the driver reporting that the OS closed the window
    /// with this id (`Core::window_closed`) — the user pressed its close
    /// button. What a declared window does after that is the half of ADR
    /// 0004's edge rule a headless core can pin.
    WindowClosed(u32),
    /// Not an input either: the driver reporting that the window with this
    /// id was asked to go away (`Core::dismiss_window`), with the reason as
    /// an index into [`DISMISS_REASONS`]. A press outside a window and a
    /// key routed to a non-activating popup are both facts only an OS has,
    /// so this is the only way the event exists headlessly — and the event
    /// is the whole contract, since the core closes nothing in answer.
    WindowDismissed(u32, u32),
    /// Not an input: the OS appearance changing under the app
    /// (`Core::set_system`), as an index into `Appearance::ALL` — 0
    /// unknown, 1 light, 2 dark. What flips a themed token's half and the
    /// theme's base without the view changing (ADR 0019, ADR 0027).
    Appearance(u32),
    /// Files dragged in from the OS over the window at a point
    /// (`InputEvent::DragFiles`, ADR 0031): `n` files, spelled
    /// `/drop/1.txt` … `/drop/n.txt` by every adapter (see [`drop_paths`])
    /// so the argument stays an integer. Entering and moving alike.
    DragFiles(u32, i32, i32),
    /// The same files released at a point (`InputEvent::DropFiles`).
    DropFiles(u32, i32, i32),
    /// The files left the window (`InputEvent::DragCancel`).
    DragCancel,
}

/// The paths a [`Step::DragFiles`] / [`Step::DropFiles`] with `n` files
/// carries: `/drop/1.txt` … `/drop/n.txt`. Every adapter spells them so.
pub fn drop_paths(n: u32) -> Vec<String> {
    (1..=n).map(|k| format!("/drop/{k}.txt")).collect()
}

impl Step {
    /// The step's spelling in a report (`step cursor 50 30`).
    pub fn write(&self, out: &mut String) {
        match *self {
            Step::Cursor(x, y) => {
                let _ = writeln!(out, "step cursor {x} {y}");
            }
            Step::CursorLeft => out.push_str("step cursorleft\n"),
            Step::MouseDown => out.push_str("step mousedown\n"),
            Step::MouseUp => out.push_str("step mouseup\n"),
            Step::SecondaryDown => out.push_str("step secondarydown\n"),
            Step::SecondaryUp => out.push_str("step secondaryup\n"),
            Step::MiddleDown => out.push_str("step middledown\n"),
            Step::MiddleUp => out.push_str("step middleup\n"),
            Step::Scroll(x, y) => {
                let _ = writeln!(out, "step scroll {x} {y}");
            }
            Step::Tab => out.push_str("step tab\n"),
            Step::ShiftTab => out.push_str("step shifttab\n"),
            Step::Modifiers(m) => {
                let _ = writeln!(out, "step modifiers {m}");
            }
            Step::Escape => out.push_str("step escape\n"),
            Step::Arrow(d) => {
                let _ = writeln!(out, "step arrow {d}");
            }
            Step::Home => out.push_str("step home\n"),
            Step::End => out.push_str("step end\n"),
            Step::Type(c) => {
                let _ = writeln!(out, "step type {c}");
            }
            Step::KeyDown(c) => {
                let _ = writeln!(out, "step keydown {c}");
            }
            Step::KeyUp(c) => {
                let _ = writeln!(out, "step keyup {c}");
            }
            Step::KeyAtDown(k, l) => {
                let _ = writeln!(out, "step keyatdown {k} {l}");
            }
            Step::KeyAtUp(k, l) => {
                let _ = writeln!(out, "step keyatup {k} {l}");
            }
            Step::Preedit(c) => {
                let _ = writeln!(out, "step preedit {c}");
            }
            Step::Commit(c) => {
                let _ = writeln!(out, "step commit {c}");
            }
            Step::Paste(c, marks) => {
                let _ = writeln!(out, "step paste {c} {marks}");
            }
            Step::Phase(n) => {
                let _ = writeln!(out, "step phase {n}");
            }
            Step::Time(ms) => {
                let _ = writeln!(out, "step time {ms}");
            }
            Step::WindowClosed(id) => {
                let _ = writeln!(out, "step windowclosed {id}");
            }
            Step::WindowDismissed(id, reason) => {
                let _ = writeln!(out, "step windowdismissed {id} {reason}");
            }
            Step::Appearance(n) => {
                let _ = writeln!(out, "step appearance {n}");
            }
            Step::DragFiles(n, x, y) => {
                let _ = writeln!(out, "step dragfiles {n} {x} {y}");
            }
            Step::DropFiles(n, x, y) => {
                let _ = writeln!(out, "step dropfiles {n} {x} {y}");
            }
            Step::DragCancel => out.push_str("step dragcancel\n"),
        }
    }

    /// The input this step replays, or `None` for the two that move the
    /// world around the view rather than poking it — see [`Step::Phase`]
    /// and [`Step::Time`].
    pub fn event(&self) -> Option<InputEvent> {
        Some(match *self {
            Step::Phase(_)
            | Step::Time(_)
            | Step::WindowClosed(_)
            | Step::WindowDismissed(..)
            | Step::Appearance(_) => {
                return None;
            }
            Step::Cursor(x, y) => InputEvent::CursorMoved(Vec2::new(x as f32, y as f32)),
            Step::CursorLeft => InputEvent::CursorLeft,
            Step::MouseDown => InputEvent::mouse_down(1),
            Step::MouseUp => InputEvent::mouse_up(),
            Step::SecondaryDown => InputEvent::MouseDown {
                button: crate::input::MouseButton::Secondary,
                clicks: 1,
            },
            Step::SecondaryUp => InputEvent::MouseUp {
                button: crate::input::MouseButton::Secondary,
            },
            Step::MiddleDown => InputEvent::MouseDown {
                button: crate::input::MouseButton::Middle,
                clicks: 1,
            },
            Step::MiddleUp => InputEvent::MouseUp {
                button: crate::input::MouseButton::Middle,
            },
            Step::Scroll(x, y) => InputEvent::Scroll(Vec2::new(x as f32, y as f32)),
            Step::Tab => InputEvent::Key(EditKey::Tab, Mods::default()),
            Step::ShiftTab => InputEvent::Key(EditKey::Tab, Mods::NONE.with_shift()),
            Step::Modifiers(m) => InputEvent::Modifiers(KeyMods::from_bits(m)),
            Step::Escape => InputEvent::Key(EditKey::Escape, Mods::default()),
            Step::Arrow(d) => InputEvent::Key(ARROWS[d as usize], Mods::default()),
            Step::Home => InputEvent::Key(EditKey::Home, Mods::default()),
            Step::End => InputEvent::Key(EditKey::End, Mods::default()),
            Step::Type(c) => InputEvent::Text(
                char::from_u32(c)
                    .expect("a printable step character")
                    .to_string(),
            ),
            Step::KeyDown(c) => InputEvent::KeyDown(KeyPress::new(
                KeyCode::Char(char::from_u32(c).expect("a printable step character")),
                KeyMods::default(),
            )),
            Step::KeyUp(c) => InputEvent::KeyUp(KeyPress::new(
                KeyCode::Char(char::from_u32(c).expect("a printable step character")),
                KeyMods::default(),
            )),
            Step::KeyAtDown(k, l) => InputEvent::KeyDown(step_key(k, l)),
            Step::KeyAtUp(k, l) => InputEvent::KeyUp(step_key(k, l)),
            Step::Preedit(0) => InputEvent::Preedit(String::new(), None),
            Step::Preedit(c) => {
                let s = char::from_u32(c)
                    .expect("a printable step character")
                    .to_string();
                let len = s.len();
                InputEvent::Preedit(s, Some((0, len)))
            }
            Step::Commit(c) => InputEvent::Commit(
                char::from_u32(c)
                    .expect("a printable step character")
                    .to_string(),
            ),
            Step::Paste(c, marks) => InputEvent::Paste {
                text: char::from_u32(c)
                    .expect("a printable step character")
                    .to_string(),
                marks: crate::input::ClipboardMarks::from_bits(marks),
            },
            Step::DragFiles(n, x, y) => InputEvent::DragFiles {
                paths: drop_paths(n),
                at: Vec2::new(x as f32, y as f32),
            },
            Step::DropFiles(n, x, y) => InputEvent::DropFiles {
                paths: drop_paths(n),
                at: Vec2::new(x as f32, y as f32),
            },
            Step::DragCancel => InputEvent::DragCancel,
        })
    }
}

/// What a scene must produce, in the parts that do not depend on which
/// fonts are installed. Checked in; asserted by `tests/conformance.rs`.
pub struct Expect {
    /// Exact solid-quad count (backgrounds, borders, scrollbars, rings):
    /// geometry-driven, so it does not move with the font.
    pub solid: usize,
    /// Exact drop-shadow-quad count.
    pub shadows: usize,
    /// Exact image-quad count.
    pub images: usize,
    /// Exact segment-quad count: one per straight piece of every `line`,
    /// so a curve's flattening is pinned too. A lower bound instead where
    /// [`Expect::segments_follow_text`] says so.
    pub segments: usize,
    /// Whether the segments include a wavy or dotted underline's pieces,
    /// whose number follows the text's advance and the face's underline
    /// stroke, so the installed fonts: 32 on the Mac the `underlines`
    /// scene was written on, 30 under Windows' Cascadia Mono. Then
    /// `segments` is a lower bound, as `glyphs_min` is; the other three
    /// adapters still match this machine's report exactly, and `deco.rs`'s
    /// tests pin the pieces a width makes.
    pub segments_follow_text: bool,
    /// Exact fragment-quad count: one per `fragment` node that resolved
    /// its handle. A node whose handle is dead emits none, which is how
    /// the scene pins that too.
    pub fragments: usize,
    /// Exact texture-quad count: one per `image` node drawn from a texture
    /// of its own rather than the atlas — one that was updated,
    /// or that no page could hold.
    pub textures: usize,
    /// Glyph quads are one per rendered glyph — a lower bound keeps a font
    /// that maps a run differently from failing the build.
    pub glyphs_min: usize,
    /// The access tree in tree order, one `depth role name|description|value`
    /// per node (see [`report`]'s `node` lines, without the key and flags).
    pub access: &'static [&'static str],
    /// Events in order, `kind tag`.
    pub events: &'static [&'static str],
    /// Announcements in order, `politeness text` (see
    /// `docs/adr/0008-live-regions-and-announcements.md`).
    pub announcements: &'static [&'static str],
    /// Diagnostic codes, in order.
    pub warnings: &'static [&'static str],
    /// Window commands in order, as [`write_command`] spells them without
    /// the `cmd ` prefix: `drag 0`, `open 1 0 0 0 400 300 1 0 0 0 0`,
    /// `close 1`.
    pub commands: &'static [&'static str],
    /// Audio commands in order, as [`write_audio_command`] spells them
    /// without the `audio ` prefix: `play 1 1`, `stop 2`. Empty for every
    /// scene that declares no playback — which is all of them but `media`.
    pub audio: &'static [&'static str],
    pub title: Option<&'static str>,
    /// Whether the frame asked for the window on top (`alwaysOnTop`).
    pub always_on_top: bool,
    /// Whether the frame asked for secure keyboard entry (`secureInput`,
    /// backlog F85).
    pub secure_input: bool,
    /// Which Option keys the frame asked to act as Alt (`optionAsAlt`,
    /// backlog F113).
    pub option_as_alt: OptionAsAlt,
}

/// One scene: a builder every binding re-expresses, the input to replay,
/// and the rows of `schema::CUSTOM` / `schema::ELEMENTS` it pins.
pub struct Scene {
    pub name: &'static str,
    pub doc: &'static str,
    /// `schema::CUSTOM` names this scene exercises. Hand-written, but not
    /// taken on trust: `observe` derives the same set from the tree the
    /// builder produces, and the Rust adapter fails a claim that is not in
    /// it (see [`Coverage`]).
    pub custom: &'static [&'static str],
    /// `schema::ELEMENTS` names this scene exercises, checked the same way.
    pub elements: &'static [&'static str],
    /// The reference lowering. Every other binding expresses the same tree
    /// in its own surface; the report says whether it did. The `u32` is
    /// the phase [`Step::Phase`] leaves behind — 0 until a step says
    /// otherwise, and ignored by every scene that never changes its mind.
    pub build: fn(&mut Ui<'_>, &Fixtures, u32),
    /// The host window facts to drive under, declared before the first
    /// frame. [`NATIVE_CHROME`] for all but the two scenes that are about
    /// chrome; it travels to the other adapters as the report's `env` line,
    /// so a third one costs them nothing.
    pub env: WindowEnv,
    pub steps: &'static [Step],
    pub expect: Expect,
}

pub fn scene(name: &str) -> Option<&'static Scene> {
    SCENES.iter().find(|s| s.name == name)
}

// ---------------------------------------------------------------------------
// The corpus

pub const SCENES: &[Scene] = &[
    Scene {
        name: "layout",
        doc: "Containers: both directions, the pad shorthand — four explicit \
              edges on the card, and an axis-and-override box whose whole \
              size is what the fallback resolved to — a border, a stable \
              key, and plain and rich text at a declared size. The card also \
              carries the paint props that fade and lift it: group opacity \
              and a drop shadow.",
        custom: &["dir", "pad", "border", "key", "size"],
        elements: &["box", "text"],
        build: build_layout,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 3,
            shadows: 1,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 7,
            access: &[
                "0 window ||",
                "1 staticText ab||",
                "1 staticText cd||",
                "1 staticText a b c||",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "sizing",
        doc: "One row of a known width holding a child in each sizing mode, \
              so the four resolve to four different widths: fixed 30, 25% \
              of 200 = 50, fit around a 20-wide child, and grow taking the \
              100 that is left. Inside a fit-sized parent — where the \
              generic prop tests live — a percent and a grow both collapse \
              to the same geometry, so nothing there can tell the modes \
              apart. The outer pad is the `padX`/`padY` shorthand pair, \
              spelled unequally so a binding cannot fall back from one to \
              the other unnoticed.",
        custom: &["pad", "key"],
        elements: &["box"],
        build: build_sizing,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 6,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "wrap",
        doc: "A wrapping row: four fixed boxes broken onto two lines by a \
              width they don't fit, with a gap along a line and a cross gap \
              between them. All geometry, no text, so the digest is the \
              same wherever it runs.",
        custom: &["dir", "pad"],
        elements: &["box"],
        build: build_wrap,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 5,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "align",
        doc: "The main axis's free space dealt out and the cross axis's \
              baselines lined up (backlog C13), and a ratio sizing each \
              axis (C14): three 120-wide rows of three 10 px boxes under \
              `spaceBetween`, `spaceAround` and `spaceEvenly`; a baseline \
              row of a 12 and a 20 px text and a box, so the small text \
              drops to the large one's baseline and the box sits on it; \
              and a `grow`-wide box at 4:1 (120 x 30) above a 12-high box \
              at 2:1 (24 x 12).",
        custom: &["dir", "pad", "size"],
        elements: &["box", "text"],
        build: build_align,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 13,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 4,
            access: &["0 window ||", "1 staticText ab||", "1 staticText cd||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "stock-controls",
        doc: "The stock controls (ADR 0034): a slider over 0..100 by 10 at \
              30, 216 wide at the origin so its track — the content box \
              the pointer reads — runs from x 16 to 216; under it a \
              checkbox, a checked one, a mixed one, a radio group of two with the \
              second checked and a switch that is on; the checked box's mark is \
              a drawn stroke. The pointer presses \
              the track at 70, drags to 90 and lets go — two `move` rows \
              and an `end` — then the keys move the slider from the 30 \
              the view still declares: Right to 40, Home to 0, End to \
              100; then the checkbox is clicked. The event rows carry each \
              change's phase and value, which is the arithmetic the core \
              took on; the access rows carry the checked, mixed and slider \
              states.",
        custom: &["key"],
        elements: &["checkbox", "radio", "radioGroup", "switch", "slider"],
        build: build_stock_controls,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(156, 16),
            Step::MouseDown,
            Step::Cursor(196, 16),
            Step::MouseUp,
            Step::Arrow(1),
            Step::Home,
            Step::End,
            Step::Cursor(16, 40),
            Step::MouseDown,
            Step::MouseUp,
        ],
        expect: Expect {
            solid: 12,
            shadows: 0,
            images: 0,
            segments: 2,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 20,
            access: &[
                "0 window ||",
                "1 slider Volume||",
                "1 checkbox Mute||",
                "1 checkbox Sync||",
                "1 checkbox All||",
                "1 radioGroup Theme||",
                "2 radio Light||",
                "2 radio Dark||",
                "1 switch Wi-Fi||",
            ],
            events: &[
                "change vol move 70",
                "change vol move 90",
                "change vol end 90",
                "change vol end 40",
                "change vol end 0",
                "change vol end 100",
                "mute -",
            ],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "table",
        doc: "A table (ADR 0033): three rows of a fixed-width table, the \
              first a header, in three columns — a bare text label column \
              (its cells three, five and two characters, so the column is \
              the widest and the shorter labels are held to it), a fixed \
              column of boxes whose widths differ per row (30, 50, 20: the \
              column is 50), and a grow column that takes the rest. The \
              rows are grow rows with a gap between their cells, so a \
              cell's x is the same in every row; the header row is a fit \
              row with two cells, held to the columns like the others.",
        custom: &["dir", "pad", "key", "size"],
        elements: &["table", "box", "text"],
        build: build_table,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            // The table's own background, a box per cell of the fixed and
            // grow columns in the three body rows, and five rules.
            solid: 12,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 14,
            access: &[
                "0 window ||",
                "1 staticText name||",
                "1 staticText w||",
                "1 staticText abc||",
                "1 staticText abcde||",
                "1 staticText ab||",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "tabs",
        doc: "An i3-style tab bar twice: `grow` tabs with a `minWidth` of \
              \"fit\", in a bar with room (two tabs split it evenly, the \
              floor idle) and in one without (four tabs sit at their \
              labels' widths and the bar scrolls x, scrolled once). The \
              roomy tabs also carry a 50% height under a fit floor, so \
              both axes and the floor's binding and idle cases are in one \
              frame. All geometry, no text: the labels are fixed boxes.",
        custom: &["overflow", "key"],
        elements: &["box"],
        build: build_tabs,
        env: NATIVE_CHROME,
        steps: &[Step::Cursor(100, 38), Step::Scroll(-40, 0)],
        expect: Expect {
            solid: 15,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||", "1 scrollView ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "overflow",
        doc: "A clipping wrapper around a rounded scrolling list, scrolled \
              once: the overflow composite, its retained offset, its \
              scrollbar, and the rounded clip its items inherit.",
        custom: &["overflow", "key"],
        elements: &["box"],
        build: build_overflow,
        env: NATIVE_CHROME,
        steps: &[Step::Cursor(40, 40), Step::Scroll(0, -30)],
        expect: Expect {
            solid: 5,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||", "1 scrollView ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "tokens",
        doc: "Tokens beside the theme (ADR 0027): a table declared every \
              frame — an unthemed peach, a themed ink, two lengths, and a \
              name a role owns, refused with `reserved-token`, then the \
              derived tokens of ADR 0028 (`TOKEN_DERIVED`: a lift, a \
              two-step chain, a `raise` off the `surface` role, a step off \
              a derived token, a `readable` against the themed ink, and one \
              whose source is nothing, dropped with `unknown-token`) — then \
              a row referencing them by name in every slot kind: a `bg`, a `width`, \
              a `pad` edge, a `border`'s width and colour, a text's `size` \
              and `color`, a span's `color`; `$surface` and `$radius` as the \
              roles by the same spelling; and `$nothing`, which paints \
              nothing and raises `unknown-token`. The one step flips the \
              appearance to light, so the frame the report keeps is the \
              themed token's light half and the theme's light base, both \
              without the view changing.",
        custom: &["pad", "border", "key", "size"],
        elements: &["box", "text"],
        build: build_tokens,
        env: NATIVE_CHROME,
        steps: &[Step::Appearance(1)],
        expect: Expect {
            solid: 8,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 6,
            access: &["0 window ||", "1 staticText tokensx||"],
            // The OS setting moving under the app is itself an event.
            events: &["system -"],
            announcements: &[],
            warnings: &["reserved-token", "unknown-token", "unknown-token"],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "anchor",
        doc: "Scroll anchoring (backlog C26 step 3): two scrollers of the \
              same six rows, wheeled to the same offset, one declared \
              `anchor`; phase 1 prepends a taller row to both. The anchored \
              list keeps the row that was at its top at its top — the offset \
              moved by the new row's height — and the other shows its content \
              slid down under an offset that stayed. One frame is the whole \
              property, once the frame before is given.",
        custom: &["overflow", "key"],
        elements: &["box"],
        build: build_anchor,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(50, 40),
            Step::Scroll(0, -40),
            Step::Cursor(150, 40),
            Step::Scroll(0, -40),
            Step::Phase(1),
        ],
        expect: Expect {
            // Two scroller boxes and two thumbs; the anchored list shows
            // three rows (offset 70 of a 150 content in 60), the other
            // four (offset 40, two of them partial).
            solid: 11,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||", "1 scrollView ||", "1 scrollView ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "scrollbar",
        doc: "The four scrollbar rows on three scrollers of the same \
              overflowing list: one `hidden` (no thumb, and its track is \
              content), one styled (an 8 px thumb in its own two colours, \
              held active by the pointer at the end so the wide active \
              width and the active colour are what the report carries), \
              one `auto` — shown when first seen, gone two seconds later, \
              back on a wheel, and gone again by the last frame, which is \
              the frame the report keeps.",
        custom: &["overflow", "key"],
        elements: &["box"],
        build: build_scrollbar,
        env: NATIVE_CHROME,
        steps: &[
            Step::Time(0),
            Step::Time(2000),
            // The wheel over the auto scroller: its bar comes back.
            Step::Cursor(250, 40),
            Step::Scroll(0, -10),
            Step::Time(2100),
            Step::Time(3500),
            // Onto the styled scroller's track, which grew for the wide thumb.
            Step::Cursor(195, 40),
        ],
        expect: Expect {
            // Three scrollers, three rows showing in each — four in the
            // one the wheel moved off a row boundary — and one thumb.
            solid: 14,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &[
                "0 window ||",
                "1 scrollView ||",
                "1 scrollView ||",
                "1 scrollView ||",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "float",
        doc: "Every float spelling: the \"below\" shorthand, a preset base \
              with one override declared (the untouched offset keeps the \
              preset's gap), and the full config (anchor, at, self, dx/dy, \
              fit). The full one is asymmetric per axis and would land \
              off-screen unclamped, so swapping at with self, dx with dy, \
              or dropping fit all move it.",
        custom: &["float", "key"],
        elements: &["box"],
        build: build_float,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 5,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "clip-float",
        doc: "A float that takes its parent's clip (backlog F90): a \
              toolbar over a `clip` canvas, and two nodes on the canvas \
              anchored to it and panned half past its top edge — one \
              declaring `clip`, one not. The clipped one is cut at the \
              canvas's edge and a press over the toolbar where its cut \
              half would be reaches the toolbar; the other escapes as \
              every float did, drawn over the toolbar and taking the \
              press there. A binding that drops the bit sends the first \
              press to the node.",
        custom: &["float", "key", "overflow"],
        elements: &["box"],
        build: build_clip_float,
        env: NATIVE_CHROME,
        steps: &[
            // Over the toolbar, where the clipped node's cut half is.
            Step::Cursor(60, 30),
            Step::MouseDown,
            Step::MouseUp,
            // Its half inside the canvas.
            Step::Cursor(60, 50),
            Step::MouseDown,
            Step::MouseUp,
            // Over the toolbar, on the node that escapes.
            Step::Cursor(180, 30),
            Step::MouseDown,
            Step::MouseUp,
        ],
        expect: Expect {
            // The toolbar, the canvas and the two nodes.
            solid: 4,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &[
                "0 window ||",
                "1 button Toolbar||",
                "1 button Node||",
                "1 button Free||",
            ],
            events: &["toolbar -", "node -", "free -"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "pixel-snap",
        doc: "Boxes painted on whole pixels (`pixelSnap`): a row of three               at 40.5 by 20.25, the first two snapped and the first with a               hard shadow, the third drawn where layout put it. The               snapped two meet on one pixel line, 41 px from the row's               start, and are 20 tall, and the shadow is snapped with its               box. The third keeps its fractional rect, which is what               every box was. A binding that drops the flag draws the first               two at 40.5 as well.",
        custom: &["dir"],
        elements: &["box"],
        build: build_pixel_snap,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 3,
            shadows: 1,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "clip-access",
        doc: "An access rect is what is drawn (backlog F93): a toolbar over \
              a `clip` canvas beside a scroller. On the canvas, a `clip` \
              float straddling its top edge, one wholly past it, and one \
              that escapes; in the scroller, three rows, the second half \
              out of its bottom and the third wholly out. The report's \
              `node` lines carry each rect: cut at the edge, a zero-size \
              point on it, the whole box. A binding that drops `clip` from \
              a float leaves the first two whole.",
        custom: &["float", "key", "overflow"],
        elements: &["box"],
        build: build_clip_access,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            // The toolbar, the canvas, the two nodes drawn on it, the
            // scroller, its two rows in view and its bar; the node wholly
            // past the canvas's edge and the row wholly out are culled.
            solid: 8,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &[
                "0 window ||",
                "1 button Toolbar||",
                "1 button Cut||",
                "1 button Past||",
                "1 button Free||",
                "1 scrollView ||",
                "2 button Row 0||",
                "2 button Row 1||",
                "2 button Row 2||",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "tooltip",
        doc: "The tooltip prop: hover tracking, the accessible description \
              it sets, and the hint that floats only while hovered — beside \
              the `description` prop on its own, which sets the same slot \
              and draws nothing.",
        custom: &["tooltip", "key"],
        elements: &["tooltip", "box", "text"],
        build: build_tooltip,
        env: NATIVE_CHROME,
        steps: &[Step::Cursor(50, 30)],
        expect: Expect {
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 10,
            access: &[
                "0 window ||",
                "1 group |a hint|",
                "2 staticText badge||",
                "1 button Save|Nothing to save yet|",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "select",
        doc: "The stock select (backlog F72, F73): a field over four options \
              — three by label and one a menu-item object posting an `id` of \
              its own, disabled — with the second in force. The steps click \
              the field, which opens the core's menu under it and reaches \
              the app as nothing; choose the first row, which is the one \
              event the app hears — the row's label, on the field's key — \
              and closes the menu; then click the field again, so the frame \
              the report keeps has the field *and* the menu: the current \
              row checked, the dead one dimmed. Three clicks, one event.",
        custom: &["key", "pad", "size"],
        elements: &["select", "box", "text"],
        build: build_select,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(60, 22),
            Step::MouseDown,
            Step::MouseUp,
            Step::Cursor(60, 62),
            Step::MouseDown,
            Step::MouseUp,
            Step::Cursor(60, 22),
            Step::MouseDown,
            Step::MouseUp,
        ],
        expect: Expect {
            // The field's box and the menu's panel; the rows are washes
            // only while hovered or focused, and the checkmark is a glyph.
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 30,
            access: &[
                "0 window ||",
                "1 button language|Deutsch|",
                "1 staticText body||",
                "1 menu Menu||",
                "2 menuItem English||",
                "2 menuItem Deutsch||",
                "2 menuItem Français||",
                "2 menuItem Latin||",
            ],
            // The field's two clicks are the core's; the row's choice is
            // the app's one event, a `menu` on the field.
            events: &["menu -"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "menubar",
        doc: "The application menu bar (ADR 0018): the `menuBar` element \
              declares one and draws it, because this is a machine \
              with no menu bar of its own — on macOS the driver would hand \
              the same declaration to the OS and the element would draw \
              nothing, which is the one thing about it a headless corpus \
              cannot see. The steps open the File menu the way a user does, \
              so the frame the report keeps has the bar *and* its dropped \
              menu in it: a row with an accelerator, a separator, a checked \
              row and a dead one. Nothing reaches the app — a press on a \
              title is the bar's own, taken back by key — which is what the \
              empty event list says.",
        custom: &["key", "pad", "size"],
        elements: &["menuBar", "box", "text"],
        build: build_menu_bar,
        env: NATIVE_CHROME,
        steps: &[Step::Cursor(20, 13), Step::MouseDown, Step::MouseUp],
        expect: Expect {
            // The bar, the open menu's panel, the open title's accent
            // background, and the separator's rule.
            solid: 4,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 20,
            access: &[
                "0 window ||",
                "1 menu Menu bar||",
                "2 menuItem File||",
                "2 menu File||",
                "3 menuItem New||",
                "3 menuItem Wrap||",
                "3 menuItem Print||",
                "2 menuItem Edit||",
                "1 staticText body||",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "chrome",
        doc: "Window chrome, driven under a declared custom chrome — the \
              only scene that departs from NATIVE_CHROME, and the reason \
              the env line exists. The frame's declared title, its ask \
              for the window above every other app's (alwaysOnTop — a \
              declaration with no node, like the title), its ask for \
              secure keyboard entry (secureInput, the same) and for the \
              left Option key as Alt (optionAsAlt, the same), an adaptive \
              titlebar hosting custom content and appending its own \
              buttons, a hand-laid strip holding a second cluster through \
              the windowButtons element itself, and a focusable box that \
              claims key focus while it is declared.",
        custom: &[
            "title",
            "alwaysOnTop",
            "secureInput",
            "optionAsAlt",
            "keyFocus",
            "size",
        ],
        elements: &["titlebar", "windowButtons", "box", "text"],
        build: build_chrome,
        env: CUSTOM_CHROME,
        steps: &[],
        expect: Expect {
            // The sink, plus each cluster's two drawn glyphs: the minimize
            // bar and the maximize outline are boxes, and only the close
            // cross is text.
            solid: 5,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 3,
            access: &[
                "0 window kui conformance||",
                "1 titleBar ||",
                "2 staticText app||",
                "2 button Minimize||",
                "2 button Maximize||",
                "2 button Close||",
                // The hand-laid strip is a plain row, so its buttons sit
                // directly under the window rather than under a role.
                "1 button Minimize||",
                "1 button Maximize||",
                "1 button Close||",
                "1 group Sink||",
            ],
            // The sink takes the keyboard as the view declares it, and
            // its `onFocus` hears it.
            events: &["focus sink"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: Some("kui conformance"),
            always_on_top: true,
            secure_input: true,
            option_as_alt: OptionAsAlt::Left,
        },
    },
    Scene {
        name: "chrome-inset",
        doc: "The same tree as `chrome`, under the same custom chrome plus \
              the macOS traffic lights: env is the only difference between \
              the two scenes, and it takes both button clusters away and \
              moves the title from the bare 12pt margin out to the controls' \
              right edge. `widgets::titlebar` adapting per platform by \
              itself, pinned across four bindings instead of described.",
        custom: &[
            "title",
            "alwaysOnTop",
            "secureInput",
            "optionAsAlt",
            "keyFocus",
            "size",
        ],
        // Not `windowButtons`: the element is called and builds nothing,
        // which is the behaviour under test. `chrome` is where that row is
        // claimed, and `observe` would not derive it here.
        elements: &["titlebar", "box", "text"],
        build: build_chrome,
        env: CUSTOM_CHROME_INSET,
        steps: &[],
        expect: Expect {
            // Just the sink: the OS draws the controls, so neither cluster
            // draws its glyph boxes.
            solid: 1,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 3,
            access: &[
                "0 window kui conformance||",
                "1 titleBar ||",
                "2 staticText app||",
                "1 group Sink||",
            ],
            // The sink takes the keyboard as the view declares it, and
            // its `onFocus` hears it.
            events: &["focus sink"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: Some("kui conformance"),
            always_on_top: true,
            secure_input: true,
            option_as_alt: OptionAsAlt::Left,
        },
    },
    Scene {
        name: "controls",
        doc: "A clicked button, a keyed editor and a slider, inside a panel \
              that asks for a context menu: the secondary press routes to \
              the panel and moves neither focus nor the caret — from the \
              panel's own body, and from the button over it that offers no \
              menu of its own, since an unclaimed press reaches the \
              enclosing menu as an unclaimed key reaches the enclosing sink \
              (backlog T1). The panel claims the middle button with \
              `onButton` and `buttons=\"middle\"` (backlog F105), so the \
              secondary presses stay its context menu's while a middle \
              press over the button reaches the panel, and the pointer \
              moved while it is held and its release go there too — the \
              phases in order, whichever binding spelled the mask. The slider \
              names its own reading (`valueText`), which lands in the value \
              column beside the editor's text — a node has one string slot, \
              and a slider that named its reading reads as that instead of \
              its number (backlog F8). The clicked button says what it will \
              do (`description`), and a second stock button takes the other \
              rows the composite admits at once — a `label` past its text, \
              `disabled`, and a `tooltip` whose description reaches the \
              access row while its float never draws, since nothing hovers \
              it — through each binding's own button, not a box. The editor \
              is a single-line field with `wrap` declared and a seed wider \
              than its box (backlog F44): it folds onto two lines where a \
              plain field would scroll one, and the quad digest is what \
              pins that across the bindings.",
        custom: &["key", "size", "tooltip"],
        elements: &["button", "edit", "box", "text"],
        build: build_controls,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(30, 24),
            Step::MouseDown,
            Step::MouseUp,
            Step::Cursor(4, 4),
            Step::SecondaryDown,
            Step::SecondaryUp,
            Step::Cursor(30, 24),
            Step::SecondaryDown,
            Step::SecondaryUp,
            Step::MiddleDown,
            Step::Cursor(32, 25),
            Step::MiddleUp,
        ],
        expect: Expect {
            // Two buttons: the disabled one is dimmed, and a quad at half
            // alpha is still one solid quad.
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 30,
            access: &[
                "0 window ||",
                "1 button go|Starts the run|",
                "1 button Stop the run|Nothing is running|",
                "1 textInput Note||hello, on two lines in a narrow field",
                "1 slider Focus length||25 minutes",
            ],
            events: &[
                "go -",
                "contextmenu menu",
                "contextmenu menu",
                "button panel press middle",
                "button panel move middle",
                "button panel release middle",
            ],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "keys",
        doc: "Two key sinks, clicked into focus in turn and each pressed and \
              released once. The first says only `on_key` and hears the press \
              alone — the keymap default, so a binding runs once per key; the \
              second says `key_up` too and hears both halves. Then a third \
              sink with a button inside it, holding focus from the frame it \
              was declared in: a shell over a ring \
              (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`). The \
              letter reaches the shell in both halves, because the button \
              claims no letter; the space presses the button and never \
              reaches the shell, because the button has something to \
              activate; and Tab moves focus off it, because Tab is the \
              ring\'s wherever focus is.",
        custom: &["key", "keyFocus"],
        elements: &["box"],
        build: build_keys,
        env: NATIVE_CHROME,
        steps: &[
            // The shell, first, while the button still holds the focus it
            // was declared with.
            Step::KeyDown('m' as u32),
            Step::KeyUp('m' as u32),
            Step::Type(' ' as u32),
            Step::Tab,
            // The two leaf sinks, each clicked into focus in turn.
            Step::Cursor(60, 22),
            Step::MouseDown,
            Step::MouseUp,
            Step::KeyDown('a' as u32),
            Step::KeyUp('a' as u32),
            Step::Cursor(60, 52),
            Step::MouseDown,
            Step::MouseUp,
            Step::KeyDown('b' as u32),
            Step::KeyUp('b' as u32),
        ],
        expect: Expect {
            solid: 4,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &[
                "0 window ||",
                "1 group press||",
                "1 group held||",
                "1 group shell||",
                "2 button Go||",
            ],
            events: &[
                "key down", "key up", "go -", "key down", "key down", "key up",
            ],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "modifier-keys",
        doc: "Where a key is, and the modifier keys as keys (backlog F108). \
              Two key sinks asking for releases, clicked into focus in \
              turn; the second also says `modifier_keys`. Each is pressed \
              the left Shift, the keypad's Enter and Caps Lock. The first \
              hears the Enter alone, both halves, its place said \
              (`numpad`) while its code stays `enter`; to it the Shift and \
              the lock are only ever held. The second hears the Shift's \
              press and release with its side (`left`), the lock's press, \
              and the Enter.",
        custom: &["key"],
        elements: &["box"],
        build: build_modifier_keys,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(60, 22),
            Step::MouseDown,
            Step::MouseUp,
            Step::KeyAtDown(0, 1),
            Step::KeyAtDown(1, 3),
            Step::KeyAtUp(1, 3),
            Step::KeyAtUp(0, 1),
            Step::KeyAtDown(2, 0),
            Step::Cursor(60, 52),
            Step::MouseDown,
            Step::MouseUp,
            Step::KeyAtDown(0, 1),
            Step::KeyAtUp(0, 1),
            Step::KeyAtDown(2, 0),
            Step::KeyAtDown(1, 3),
        ],
        expect: Expect {
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||", "1 group plain||", "1 group mods||"],
            events: &[
                "key plain down enter numpad",
                "key plain up enter numpad",
                "key mods down shift left",
                "key mods up shift left",
                "key mods",
                "key mods down enter numpad",
            ],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "ime",
        doc: "An IME against both kinds of editor (backlog C17). A custom \
              editor — an `on_key` sink drawing a `line` with a caret — is \
              clicked into focus, composed into, committed to, and left with \
              the composition ended: it hears preedit, text and preedit as \
              data on its tag. Then the stock editor takes the same two \
              steps itself and reports a change. The commit is its own \
              input, since a sink already hears typing as the key's text.",
        custom: &["key", "size"],
        elements: &["box", "text", "edit"],
        build: build_ime,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(60, 22),
            Step::MouseDown,
            Step::MouseUp,
            Step::Preedit('x' as u32),
            Step::Commit('y' as u32),
            Step::Preedit(0),
            Step::Cursor(60, 48),
            Step::MouseDown,
            Step::MouseUp,
            Step::Preedit('x' as u32),
            Step::Commit('y' as u32),
        ],
        expect: Expect {
            // The sink's background and the stock editor's caret.
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 3,
            access: &[
                "0 window ||",
                "1 multilineTextInput Buffer||ab",
                "1 textInput Note||y",
            ],
            events: &["preedit ed", "text ed", "preedit ed", "changed -"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "paste",
        doc: "A paste's answer with the pasteboard's markers (backlog F84), \
              against the `ime` scene's two editors. The custom editor is \
              clicked into focus and answered three times: a secret a \
              password manager copied (concealed and transient), a \
              transient one, and a bare commit — the answer an older \
              driver sends — each heard as `text` on its tag with the \
              markers that were set and no others. Then the stock editor \
              takes a concealed paste as typed text and reports a change, \
              the markers being the sink's to read.",
        custom: &["key", "size"],
        elements: &["box", "text", "edit"],
        build: build_ime,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(60, 22),
            Step::MouseDown,
            Step::MouseUp,
            Step::Paste('s' as u32, 3),
            Step::Paste('t' as u32, 2),
            Step::Commit('u' as u32),
            Step::Cursor(60, 48),
            Step::MouseDown,
            Step::MouseUp,
            Step::Paste('v' as u32, 1),
        ],
        expect: Expect {
            // The sink's background and the stock editor's caret.
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 3,
            access: &[
                "0 window ||",
                "1 multilineTextInput Buffer||ab",
                "1 textInput Note||v",
            ],
            events: &[
                "text ed concealed transient",
                "text ed transient",
                "text ed",
                "changed -",
            ],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "cells",
        doc: "A terminal's screen as one node (backlog C20): one row of \
              eleven cells, three of them over a background, a block cursor \
              on the fourth, then a click on that cell. Every binding hands \
              the cells over in its own shape — a `Uint32Array`, a row string \
              with colour runs, a `KuiCell` array — and the same screen has \
              to come out: the same glyphs at `col × cell_w`, one background \
              quad for the run, the cursor under its glyph, the click's \
              payload naming the cell, and the row as the terminal's value.",
        custom: &["key"],
        elements: &["cells"],
        build: build_cells,
        env: NATIVE_CHROME,
        steps: &[Step::Cursor(38, 19), Step::MouseDown, Step::MouseUp],
        expect: Expect {
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 10,
            access: &["0 window ||", "1 terminal term||hello world"],
            events: &["hit -"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "media",
        doc: "The non-text leaves: a registered image (deliberately unnamed, \
              so the diagnostic shows up too), the same image again as \
              `contain` in a box twice its aspect — the painted rect \
              shrinks and centres, the box does not — then the *stream* \
              fixture, an image whose pixels were replaced before the first \
              frame and so draws from a texture of its own (ADR 0025): \
              once plain, once `nearest`, once `cover` in a square box, \
              whose crop is the report's `texture` line. Three retained \
              audio playbacks that draw nothing, and the latency graph's \
              empty chrome. Phase 1 drops two of the playbacks, which is what \
              `finish` is about: a removal releases the one that asked for \
              it — no `stop` reaches the driver and the sound plays itself \
              out — and stops the one that did not. The looped playback \
              stays declared throughout, since a loop is stopped on removal \
              whatever it asked for and this scene keeps one frame.",
        custom: &["size"],
        elements: &["image", "audio", "latencyGraph"],
        build: build_media,
        env: NATIVE_CHROME,
        steps: &[Step::Phase(1)],
        expect: Expect {
            solid: 2,
            shadows: 0,
            images: 2,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 3,
            glyphs_min: 20,
            access: &[
                "0 window ||",
                "1 image ||",
                "1 image Icon||",
                "1 image Stream||",
                "1 image Crisp||",
                "1 image Cropped||",
            ],
            events: &[],
            announcements: &[],
            warnings: &["image-without-label"],
            commands: &[],
            // `chime` (2) asked to finish and its removal says nothing;
            // `blip` (3) did not and is stopped. `music` (1) is the loop,
            // still declared, so it has no departure to describe.
            audio: &["play 1 1", "play 2 0", "play 3 0", "stop 3"],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "lines",
        doc: "The stroke primitive (`docs/adr/0010-a-segment-primitive.md`): \
              a diagonal segment, an orthogonal elbow through three points, \
              and a faded curve through four knots — 8, 9 and 14 pieces by \
              the core's flattening, so the segment count pins it — and a \
              dash-dot round a corner, 3 px into its pattern (backlog V2): \
              ten more segments, one per mark and two for the mark that \
              turns the corner — beside \
              a box, which the lines paint over because a line is a float. \
              A line with no input is elided from the access tree; the \
              elbow declares a click and a label, so it is a button, and \
              it is hit by its stroke and not its box (ADR 0026): a press \
              on the horizontal piece clicks it, a press inside its \
              bounding box but off the stroke reaches nothing.",
        custom: &["key"],
        elements: &["box", "line"],
        build: build_lines,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(120, 20),
            Step::MouseDown,
            Step::MouseUp,
            Step::Cursor(110, 50),
            Step::MouseDown,
            Step::MouseUp,
        ],
        expect: Expect {
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 44,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||", "1 button Elbow||"],
            events: &["elbow -"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "polygon",
        doc: "Five fills in a 200×120 canvas (ADR 0025, decision 6): a \
              triangle that declares a click and a label, so it is a button \
              hit by its outline and not its box (ADR 0026) — a press inside \
              it clicks, a press in its bounding box past the hypotenuse \
              reaches nothing; a concave arrowhead; an eight-point star, the \
              most a polygon takes; a nine-point outline whose ninth is \
              dropped with a warning; and a faded quad. Each is one \
              `fragment` quad whose sixteen params are the vertices \
              normalised to the shape's own padded box — the `fragment` \
              lines of the report pin them to the bit — painted by the stock \
              source every binding gets from the core, so no adapter writes \
              WGSL here. A fill with no input is elided like a stroke.",
        custom: &["key"],
        elements: &["polygon", "box"],
        build: build_polygon,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(28, 26),
            Step::MouseDown,
            Step::MouseUp,
            Step::Cursor(55, 45),
            Step::MouseDown,
            Step::MouseUp,
        ],
        expect: Expect {
            solid: 1,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 5,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||", "1 button Triangle||"],
            events: &["tri -"],
            announcements: &[],
            warnings: &["polygon-points-truncated"],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "path",
        doc: "Seven paths in a 200×120 canvas (ADR 0040): two quarter wedges \
              of one pie sharing a radial edge, round by their arcs — the \
              first declares a click and a label, so it is a button hit by \
              its outline: a press inside it clicks, a press in its bounding \
              box past the arc reaches nothing — the second keyed; an \
              even-odd ring, declared in the flat op form by the bindings \
              that take it; a stroked cubic with no fill; a triangle filled \
              and stroked, faded; a bar turned an eighth of a turn about \
              a pivot it names, by the quad that draws it (ADR 0041); \
              and a `d` that does not parse, keyed, \
              which raises its warning and draws nothing. Every paint is one \
              glyph-mask quad from the atlas, so the glyph lines of the \
              report pin the masks' slots; nothing is a texture.",
        custom: &["key"],
        elements: &["path", "box"],
        build: build_path,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(75, 75),
            Step::MouseDown,
            Step::MouseUp,
            Step::Cursor(98, 98),
            Step::MouseDown,
            Step::MouseUp,
        ],
        expect: Expect {
            solid: 1,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 7,
            access: &["0 window ||", "1 button Wedge||"],
            events: &["wedge -"],
            announcements: &[],
            warnings: &["path-malformed"],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "fragments",
        doc: "A box a registered WGSL function paints \
              (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`): \
              a plain gradient, a rounded and faded one with a child painted \
              over it, one whose handle is dead — which draws nothing, the \
              documented fallback for every resource — and one that declares \
              eighteen params, so the truncation warning is pinned. Then the \
              image input (backlog V1, ADR 0025 decision 7): the sampling \
              fixture reading the atlas-backed icon, the same reading the \
              texture-backed stream — which takes the frame's one `texture` \
              entry, as an `image` node of it would — and the same naming an \
              image that is live in no session, which draws nothing: the \
              removal order ADR 0015 asked to see pinned. The parameters ride \
              a side list, not the quad, so the report carries them as bits \
              on their own lines, and a draw's image as a `fragment-image` \
              line naming where its texels are.",
        custom: &["key"],
        elements: &["box", "fragment"],
        build: build_fragments,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 5,
            // The stream's side entry is there (the `texture 0` line); no
            // texture *quad* is, since the fragment's quad is what draws.
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &["fragment-params-truncated"],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "modal",
        doc: "A floated modal over an app with window chrome \
              (`docs/adr/0003-modal-surfaces.md`). `modal` is a schema row, \
              so every transport lowers it mechanically; what a scene pins \
              is the behaviour around it, and the four things here are the \
              parts no binding gets for free. A press on the button behind \
              emits no click, only the modal's `dismiss` — the app is inert. \
              A press on the titlebar emits nothing at all — it asks the \
              driver for a window drag instead: window chrome stays live, so \
              it is not \"outside\" and asks for no dismissal. \
              A press on the dialog's own OK button does click, and Escape \
              asks it to go away a second time, so both dismiss reasons are \
              in the event list with a live event between them. Then the \
              ring: focus enters the dialog by itself, and Shift-Tab, \
              Shift-Tab, Tab walk it — over the dialog's two stops they land \
              back on Cancel, over the whole tree's three they would land on \
              Open — and a Space presses where they landed, so the walk is \
              an event rather than a fact about the last frame. Last, the \
              way out (backlog F4): the app owns its keyboard, so it \
              declares `open` focused every frame the dialog is shut, and \
              the frame that drops the dialog declares the freshly created \
              `note` instead. That change is an edge, and an edge on the \
              closing frame stands — the report's last frame has focus on \
              `note`, where the restore alone would have put it back on \
              `open`.",
        custom: &["float", "key", "keyFocus"],
        elements: &["box", "text", "titlebar"],
        build: build_modal,
        env: NATIVE_CHROME,
        steps: &[
            // The app behind: inert, and the press asks the modal to go.
            Step::Cursor(50, 50),
            Step::MouseDown,
            Step::MouseUp,
            // The titlebar: the platform's, so it stays live.
            Step::Cursor(160, 16),
            Step::MouseDown,
            Step::MouseUp,
            // The dialog's own button: live, and it clicks.
            Step::Cursor(250, 160),
            Step::MouseDown,
            Step::MouseUp,
            // The ring, inside the dialog and nowhere else.
            Step::ShiftTab,
            Step::ShiftTab,
            Step::Tab,
            // Space presses where the ring landed, which is how the walk
            // survives into a report that keeps one frame: the last frame
            // has no dialog in it any more, and `cancel` in the event list
            // is the whole of the claim the three steps make.
            Step::Type(' ' as u32),
            Step::Escape,
            // The app drops the dialog and declares the node it was
            // renaming focused: a `keyFocus` edge on the closing frame
            // stands, and the focus the modal displaced (`open`) is not
            // handed back over it.
            Step::Phase(1),
        ],
        expect: Expect {
            // The frame the report keeps is the one after the dialog: the
            // two app buttons and the focus ring on `note`.
            solid: 3,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 3,
            access: &[
                "0 window ||",
                "1 titleBar ||",
                "2 staticText app||",
                "1 button Open||",
                "1 button Note||",
            ],
            events: &["dismiss dlg", "ok -", "cancel -", "dismiss dlg"],
            announcements: &[],
            warnings: &[],
            // The titlebar press: chrome stays live under a modal, and a
            // live drag strip asks the driver to move the window.
            commands: &["drag 0"],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "composite",
        doc: "Composite keyboard patterns \
              (`docs/adr/0007-composite-keyboard-patterns.md`). Nothing \
              here is declared: a `tabList` and a `list` whose items are \
              focusable *are* composites, so what a scene pins is the \
              behaviour the derivation produces, and none of it is \
              mechanical lowering. The tab bar is one Tab stop and it \
              enters on the selected tab, not the first; an arrow and End \
              move focus inside it and each emits the tab's own click \
              payload, because `tab` is one of the two roles whose pattern \
              defines selection as following focus. The `add` button after \
              it is one Tab away — over three tabs collapsed to one stop, \
              not over two more tabs — and the next Tab enters the list, \
              whose rows move under the arrows and emit *nothing*, since a \
              list activates manually. Then type-ahead: `b` finds Bravo by \
              name, a space extends that search instead of pressing (the \
              one interaction with ADR 0002's Space-activates), the clock \
              moves past a second so the buffer ages at the next frame, \
              and the same space now presses. A last Up shows the list \
              clamping where the tab bar wrapped. The frame the report \
              keeps has focus on Alpha, the tab bar still showing the tab \
              the *view* selected — the core moved focus and never wrote \
              `selected` — and the two containers' derived orientations, \
              horizontal and vertical, on their nodes.",
        custom: &["key", "size"],
        elements: &["box", "text"],
        build: build_composite,
        env: NATIVE_CHROME,
        steps: &[
            // A clock, so type-ahead can age (decision 9). Without one
            // every keystroke starts a fresh search and the two spaces
            // below could not differ.
            Step::Time(0),
            // Into the tab bar: one stop, entered on the selected tab —
            // the second, so Home moves and pins that the entry was not
            // simply the first item.
            Step::Tab,
            Step::Home,
            Step::Arrow(RIGHT),
            Step::End,
            // Out of it in one step, over the button beside it, and into
            // the list — which enters on its first row, nothing selected.
            Step::Tab,
            Step::Tab,
            Step::Arrow(DOWN),
            // Type-ahead: a name, then a space that extends the search
            // rather than pressing.
            Step::Type('a' as u32),
            Step::Type(' ' as u32),
            // Past a second: the buffer ages at the next frame, so the
            // same space presses instead.
            Step::Time(2000),
            Step::Type(' ' as u32),
            // A list clamps where a tab bar wraps.
            Step::Arrow(UP),
        ],
        expect: Expect {
            solid: 7,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 20,
            access: &[
                "0 window ||",
                "1 tabList ||",
                "2 tab One||",
                "2 tab Two||",
                "2 tab Three||",
                "1 button Add||",
                "1 list ||",
                "2 listItem ||",
                "3 staticText Alpha||",
                "2 listItem ||",
                "3 staticText Bravo||",
            ],
            events: &["one -", "two -", "three -", "alpha -"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "exit",
        doc: "Exit transitions (`docs/adr/0005-the-paint-vocabulary.md`): \
              the first feature that lets a node outlive the frame that \
              declared it. `exit` is a schema row and lowers mechanically \
              everywhere; what no binding gets for free is how the ghost \
              behaves, and a report keeps one frame, so the steps end where \
              the four claims differ. `fade` is 80ms into a 400ms exit: it \
              still draws (with its text, which is the previous frame's \
              text list and not just its tree), eased to neither end of the \
              run — and it is inert three ways, since a press on ground \
              covered by both where the node was and where its ghost now \
              is emits nothing, two Tabs walk past it from `A` to `B`, and \
              the access tree does not list it. `blink` ran 50ms and is \
              over, so it is gone by the same frame `fade` is not. `flash` \
              left and came back mid-flight, so the frame holds one picture \
              of it, not two. The budget is judged per frame and whole \
              (`docs/adr/0012-the-exit-budget.md`): `bulk`, one subtree a \
              node past it, leaves in a frame of its own and is refused \
              whole — no ghost, an `exit-budget` warning — and then 4200 \
              one-node rows leave together and are refused the same way, \
              where admitting subtrees one at a time would have kept 4096 \
              of them. Neither refusal touches `fade`'s ghost, which is \
              smaller than the room either would have needed.",
        custom: &["key", "size"],
        elements: &["box", "text"],
        build: build_exit,
        env: NATIVE_CHROME,
        steps: &[
            // Start the clock. Without one every transition snaps and the
            // store is cleared, so there would be no ghost to look at.
            Step::Time(0),
            // The press that proves `fade`'s click is live while it is.
            Step::Cursor(58, 42),
            Step::MouseDown,
            Step::MouseUp,
            // The view stops declaring three subtrees at once: four nodes,
            // admitted whole.
            Step::Phase(1),
            // 80ms in: `fade` and `flash` are mid-flight, `blink` is over.
            Step::Time(80),
            // `flash` comes back while its own exit is still running.
            Step::Phase(2),
            // A second press, on ground covered by both where the node was
            // and where its ghost now is: neither is a hit region.
            Step::Cursor(90, 42),
            Step::MouseDown,
            Step::MouseUp,
            // The ring is `A`, `B` and nothing between them.
            Step::Tab,
            Step::Tab,
            // `bulk` leaves alone: 4097 nodes in one subtree, refused whole.
            Step::Phase(3),
            // The rows leave together: 4200 nodes in 4200 subtrees, refused
            // whole — the frame that separates whole-or-nothing admission
            // from the per-subtree kind, which would keep 4096 of them.
            Step::Phase(4),
        ],
        expect: Expect {
            // Eight live boxes and the focus ring on `B`, plus exactly one
            // ghost: `flash`'s was retired by its return and `blink`'s
            // expired, so a store that kept either would count eleven,
            // both twelve — and one that admitted the rows one at a time
            // would count 4106.
            solid: 10,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 3,
            access: &["0 window ||", "1 group A||", "1 group B||"],
            events: &["hit -"],
            announcements: &[],
            warnings: &["exit-budget", "exit-budget"],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "windows",
        doc: "The declared window set (`docs/adr/0004-multi-window.md`, \
              decisions 4-6), which is the half of multi-window a headless \
              core can pin: the diff, its commands, its events and its two \
              warnings. `windows` lowers mechanically in every binding; what \
              the scene pins is the edge rule. The first frame declares \
              `palette` twice with different sizes, so it opens once, with \
              the first config, and `duplicate-window-config` is raised on \
              that edge. The user then closes it: a `closed` event, and the \
              next frame — still declaring it — reopens nothing and raises \
              `window-declared-while-closed`. Phase 1 stops declaring, \
              which is what lets phase 2's declaration *start*: a second \
              `Open`, with a new id, because a closed window's identity did \
              not survive its lapse. Phase 3 stops again and the diff \
              closes it. Four `window` events, three commands, two \
              warnings, and the report keeps the frame with nothing open.",
        custom: &["windows", "size"],
        elements: &["box", "text"],
        build: build_windows,
        env: NATIVE_CHROME,
        steps: &[
            // The user closes the window the first frame opened.
            Step::WindowClosed(1),
            // Still declared after the close: closed it stays, with a line
            // saying why. Then the declaration lapses ...
            Step::Phase(1),
            // ... and starts again, which is the only thing that reopens.
            Step::Phase(2),
            // And stops, so the diff closes what it opened.
            Step::Phase(3),
        ],
        expect: Expect {
            solid: 1,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 6,
            access: &["0 window ||", "1 staticText closed||"],
            events: &[
                "window opened",
                "window closed",
                "window opened",
                "window closed",
            ],
            announcements: &[],
            warnings: &["duplicate-window-config", "window-declared-while-closed"],
            commands: &[
                "open 1 0 0 0 400 300 1 0 0 0 0",
                "open 2 0 0 0 400 300 1 0 0 0 0",
                "close 2",
            ],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "popup",
        doc: "A popup window (`docs/adr/0004-multi-window.md`, decision 9) \
              and its `dismiss`, which is the half of step 4 a headless core \
              can pin — a borderless non-activating surface placed in screen \
              coordinates has no headless equivalent, so the OS half is a \
              smoke job. The declaration differs from a normal window's in \
              four numbers and a kind: the `open` line carries kind 1, \
              `activates` 0, the owner that declared it (main, 0) and the \
              anchor rect an `onLayout` node reported for the field the menu \
              belongs to. Then the driver reports the two dismissals, and \
              **neither closes anything**: the window is still open, still \
              declared, and no command follows — the app stops declaring it \
              on the frame it chooses, which is phase 1, and only that \
              closes it (ADR 0003 decision 6, one level up). The last step \
              dismisses a window that no longer exists and gets nothing, so \
              a driver reporting a stale id cannot invent an event.",
        custom: &["windows", "size"],
        elements: &["box", "text"],
        build: build_popup,
        env: NATIVE_CHROME,
        steps: &[
            // A press landed outside it, then Escape reached it. Two
            // events, no commands, and the window stays up.
            Step::WindowDismissed(1, OUTSIDE),
            Step::WindowDismissed(1, ESCAPE),
            // The app answers the way it answers a modal's dismiss.
            Step::Phase(1),
            // And a dismissal of what is no longer there says nothing.
            Step::WindowDismissed(1, OUTSIDE),
        ],
        expect: Expect {
            solid: 1,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 6,
            access: &["0 window ||", "1 staticText closed||"],
            events: &[
                "window opened",
                "dismiss outside",
                "dismiss escape",
                "window closed",
            ],
            // A popup says nothing to a screen reader by opening: it is a
            // window, and a window announcing itself is what the access
            // tree's `window` role already does.
            announcements: &[],
            warnings: &[],
            commands: &["open 1 0 0 1 160 320 0 12 40 160 24", "close 1"],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "live",
        doc: "Both halves of \
              `docs/adr/0008-live-regions-and-announcements.md`. The \
              **row**: a `live` box holding a result count. It is a plain \
              box, so eliding it would drop the liveness on the floor — \
              the scene pins that it is a `group` in the tree instead, \
              with the counted text read as part of it: a live region is \
              one message, and its name is what moves when the message \
              does. Phase 1 changes the count, which is the whole event: \
              nothing else about the frame moves. The **queue**: \
              the same phase announces \"Saved\" once, with no node behind \
              it, and the report carries it on an `announce` line the way \
              it carries a warning — drained over the scene, not read off \
              the last frame. And the defect: a second live box with \
              nothing inside it can never say anything, which is \
              `live-region-without-name`.",
        custom: &["key", "pad"],
        elements: &["box", "text"],
        build: build_live,
        env: NATIVE_CHROME,
        steps: &[Step::Phase(1)],
        expect: Expect {
            solid: 1,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 8,
            access: &["0 window ||", "1 group 3 results||", "1 group ||"],
            events: &[],
            announcements: &["assertive Saved"],
            warnings: &["live-region-without-name"],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "drag",
        doc: "A pointer-captured drag on an `onDrag` handle, and what its \
              deltas mean: `dx`/`dy` are the displacement from the press \
              point in every phase (backlog F2). The pointer presses at \
              (40, 20), moves 2 px, 2 px and 4 px, and lets go: the first \
              move sits inside the 3 px slop and emits nothing, the second \
              is 4 px from the press — past the slop, measured from the \
              press and not per event — and carries all 4, the third \
              carries 8, and `end` carries 8 too — not zero — so a handler \
              can commit from it. The event rows carry the deltas, which \
              is what pins them across the four transports.",
        custom: &["key"],
        elements: &["box"],
        build: build_drag,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(40, 20),
            Step::MouseDown,
            Step::Cursor(42, 20),
            Step::Cursor(44, 20),
            Step::Cursor(48, 20),
            Step::MouseUp,
        ],
        expect: Expect {
            solid: 1,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[
                "drag split start 0 0",
                "drag split move 4 0",
                "drag split move 8 0",
                "drag split end 8 0",
            ],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "selection",
        doc: "A `selectable` card and a drag across it (ADR 0017): the \
              pointer presses inside the first label, moves into the third \
              and lets go, so the selection covers the tail of one run, all \
              of the next and the head of the last. The three highlight \
              quads that leaves are what pins it — they are solid quads \
              under the glyphs, at the tint every binding must produce and \
              at the geometry cosmic-text resolved the two ends to, so an \
              adapter that started the selection at the wrong byte, painted \
              over the text instead of under it, or missed a run in the \
              middle disagrees on both the count and the digest. The card \
              also fixes its own width, so the runs do not depend on the \
              window.",
        custom: &["key", "pad", "size"],
        elements: &["box", "text"],
        build: build_selection,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(14, 16),
            Step::MouseDown,
            Step::Cursor(30, 60),
            Step::MouseUp,
        ],
        expect: Expect {
            solid: 4,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 11,
            access: &[
                "0 window ||",
                "1 staticText one||",
                "1 staticText two||",
                "1 staticText three||",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "menu",
        doc: "The stock context menu (ADR 0017, decision 5), opened the way \
              a user opens one: a secondary press inside the `selectable` \
              card of the `selection` scene, which nothing claimed with \
              `onContextMenu`, so the core offers Copy and Select All \
              itself. Copy is dead — nothing is selected — and drawn dimmed \
              rather than left out, so the row a reader reaches for is \
              where it was last time. Same tree as `selection`; only the \
              steps differ, which is the point: no binding declares this \
              menu, and all four have to end up with the same quads under \
              the same access rows anyway. Two solid quads and not four: \
              the card and the menu's own panel, since a row paints a \
              background only while it is hovered or *visibly* focused, and \
              a menu opened by a press has taken no keyboard focus to \
              show.",
        custom: &["key", "pad", "size"],
        elements: &["box", "text"],
        build: build_selection,
        env: NATIVE_CHROME,
        steps: &[Step::Cursor(30, 16), Step::SecondaryDown],
        expect: Expect {
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 11,
            access: &[
                "0 window ||",
                "1 staticText one||",
                "1 staticText two||",
                "1 staticText three||",
                "1 menu Menu||",
                "2 menuItem Copy||",
                "2 menuItem Select All||",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "selection-extend",
        doc: "The `selection` card, and a Shift-press keeping the anchor \
              (ADR 0029, decision 3): a click in the first run places both \
              ends together, a Shift-click in the third extends from that \
              anchor to the press, and a Shift-press-drag back into the \
              second goes on from the same anchor — by characters, whatever \
              the click count — so what is left is the tail of the first \
              run and the head of the second. An adapter that read the \
              press without the modifier starts over at the third click and \
              leaves one highlight quad where two are pinned.",
        custom: &["key", "pad", "size"],
        elements: &["box", "text"],
        build: build_selection,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(14, 16),
            Step::MouseDown,
            Step::MouseUp,
            Step::Modifiers(KeyMods::SHIFT),
            Step::Cursor(30, 60),
            Step::MouseDown,
            Step::MouseUp,
            Step::Cursor(30, 38),
            Step::MouseDown,
            Step::Cursor(14, 38),
            Step::MouseUp,
            Step::Modifiers(0),
        ],
        expect: Expect {
            solid: 3,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 11,
            access: &[
                "0 window ||",
                "1 staticText one||",
                "1 staticText two||",
                "1 staticText three||",
            ],
            // The modifier going down and up is itself an event on the
            // root — the one an app keeps in its model for a held-key
            // overlay — so the two steps leave two rows.
            events: &["modifiers -", "modifiers -"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "selection-scroll",
        doc: "A `selectable` card two runs tall over six, and a press held \
              past its bottom edge (ADR 0029, decisions 1 and 2): the \
              pointer presses in the first run and moves 60 px below the \
              card, so the core scrolls it toward the pointer — ten px on \
              the clockless frames, then the clock's own share on each \
              `time` step at ten px a second per px past the edge — until \
              the card is at its end, re-placing the live end under the \
              pointer as the runs move; then the pointer comes back inside \
              and one wheel notch up under the still-held press moves the \
              text back and the end follows a frame later; then the \
              release. The offset at the end and the \
              highlight quads the moved runs leave are what pins the rate, \
              the cap, the clamp and the re-hit. Both clocks are pinned: an \
              adapter that stepped by a fixed amount per frame agrees on \
              the first frames and not on the timed ones.",
        custom: &["key", "pad", "size", "overflow"],
        elements: &["box", "text"],
        build: build_selection_scroll,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(14, 16),
            Step::MouseDown,
            Step::Cursor(30, 100),
            Step::Time(0),
            Step::Time(50),
            Step::Time(100),
            Step::Time(150),
            Step::Time(200),
            Step::Cursor(30, 20),
            Step::Scroll(0, 30),
            Step::Time(250),
            Step::MouseUp,
        ],
        expect: Expect {
            solid: 3,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 6,
            access: &[
                "0 window ||",
                "1 scrollView ||",
                "2 staticText one||",
                "2 staticText two||",
                "2 staticText three||",
                "2 staticText four||",
                "2 staticText five||",
                "2 staticText six||",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "scroll-gestures",
        doc: "Where a wheel goes when a scroller cannot use it (backlog \
              F107): a page scrolling y holds a strip scrolling x, and in \
              the strip a list scrolling y that says `overscroll` \
              `contain` and a terminal-like node hearing the wheel \
              (`onScroll`) on `y` only (`scrollAxes`). A notch runs the \
              list to its end, and the next, over the list at its limit, \
              moves nothing — without `contain` it would move the page. \
              Over the terminal a vertical notch is its event, and a \
              sideways one passes it by and moves the strip — without \
              `scrollAxes` the terminal would hear it too. Each `Scroll` \
              is a gesture of its own; latching across a gesture is a \
              driver's gesture boundaries, pinned in kui-core's tests.",
        custom: &["overflow", "key", "pad"],
        elements: &["box"],
        build: build_scroll_gestures,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(30, 30),
            Step::Scroll(0, -80),
            Step::Scroll(0, -20),
            Step::Cursor(150, 30),
            Step::Scroll(0, -10),
            Step::Scroll(-30, 0),
        ],
        expect: Expect {
            solid: 12,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &[
                "0 window ||",
                "1 scrollView ||",
                "2 scrollView ||",
                "3 scrollView ||",
            ],
            events: &["scroll term -"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "scroll-handler-room",
        doc: "A wheel handler that is a scroll container too is answered \
              by its room on the axes it scrolls (backlog F118): a strip \
              scrolling x holds a code box that scrolls x and hears the \
              wheel (`onScroll`), its offset at its start, then a spacer. \
              A notch over the spacer moves the strip; one back toward \
              the start over the code box, which has no room that way, \
              passes it by and moves the strip back; one the other way \
              is its event. Without F118 the second notch would be its \
              event too, and the strip would stay where the first left \
              it.",
        custom: &["overflow", "key", "pad"],
        elements: &["box"],
        build: build_scroll_handler_room,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(150, 30),
            Step::Scroll(-30, 0),
            Step::Cursor(40, 30),
            Step::Scroll(20, 0),
            Step::Scroll(-10, 0),
        ],
        expect: Expect {
            solid: 5,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||", "1 scrollView ||", "2 scrollView ||"],
            events: &["scroll code -"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "cells-scroll",
        doc: "The `cells` screen three rows tall, `selectable` and hearing \
              the wheel (`onScroll`, ADR 0029, decision 4): a notch of two \
              and a half lines is two whole lines out and a half carried, \
              the next half-line notch is the carried half made whole, and \
              the view answers each by moving `originLine` (the phase is \
              the app's answer). Then a press in the first row is held 60 \
              px below the grid: each frame's step arrives as the lines it \
              covers, the view answers again, and the selection's ends keep \
              their absolute lines through it — the anchor on the line the \
              press took, the live end on the last row of the moved screen. \
              The event rows carry the lines, which is what pins the carry \
              and the rate across the four transports.",
        custom: &["key", "pad"],
        elements: &["cells"],
        build: build_cells_scroll,
        env: NATIVE_CHROME,
        steps: &[
            Step::Cursor(38, 19),
            Step::Scroll(0, -45),
            Step::Phase(2),
            Step::Scroll(0, -9),
            Step::Phase(3),
            Step::MouseDown,
            Step::Cursor(38, 124),
            Step::Time(0),
            Step::Time(100),
            Step::Phase(7),
            Step::Time(200),
            Step::Phase(10),
            Step::MouseUp,
        ],
        expect: Expect {
            solid: 3,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 18,
            access: &["0 window ||", "1 terminal term||hello world\nbrave\nbye"],
            // Two and a half lines, then the half made whole; then the
            // held press: 10 px on the clockless frames (a fifth of a
            // line and a half, carried), 60 px on each 100 ms step.
            events: &[
                "scroll term 2",
                "scroll term 1",
                "scroll term 0",
                "scroll term 1",
                "scroll term 3",
                "scroll term 3",
            ],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "virtual",
        doc: "A virtualised list, which is what the `index` row exists for: \
              the rows a long list can show, each opened at its *data* index \
              rather than at the position it happens to occupy, between the \
              two spacers that hold the height of the rows nobody built. \
              The rows are numbered 100..103 under five children, so their \
              keys are ones auto-keying could not have produced — which is \
              how a binding that dropped the row (and auto-keyed them 1, 2, \
              3 instead) is caught, and what pins that a row's identity \
              follows the row rather than the slot. The list declares its \
              whole size with `rowCount`, the row Select All spans by; a \
              binding that does not know the row warns, and the warning \
              list is pinned empty.",
        custom: &["index", "rowCount", "key", "overflow"],
        elements: &["box"],
        build: build_virtual,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 4,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &[
                "0 window ||",
                "1 list log||",
                "2 listItem row 100||",
                "2 listItem row 101||",
                // Built and clipped away: three rows are declared and two
                // draw, so the count of quads and the count of access rows
                // deliberately disagree.
                "2 listItem row 102||",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "layers",
        doc: "The paint order as a stack of layers \
              (`docs/adr/0023-layers-stack-in-the-order-they-open.md`): a \
              scroller with its bar, two floats over it that overlap each \
              other, and a click where each of them is the topmost thing. \
              The popover covers the scroller's bar and takes the press \
              there (the bar is the in-flow layer's chrome, under every \
              float); the toast is over the popover in phase 0 because it \
              is later in the tree, and under it in phase 2 because the \
              popover closed and reopened — so the same press at the same \
              point reaches a different float. The digest carries the \
              order; the events pin what it decides.",
        custom: &["float", "key", "overflow"],
        elements: &["box"],
        build: build_layers,
        env: NATIVE_CHROME,
        steps: &[
            // Where the two floats overlap: the toast, later in the tree.
            Step::Cursor(230, 90),
            Step::MouseDown,
            Step::MouseUp,
            // On the scroller's track, under the popover: the popover.
            Step::Cursor(315, 80),
            Step::MouseDown,
            Step::MouseUp,
            Step::Phase(1),
            Step::Phase(2),
            // The same overlap, with the popover reopened over the toast.
            Step::Cursor(230, 90),
            Step::MouseDown,
            Step::MouseUp,
        ],
        expect: Expect {
            // The page, the eight rows the viewport shows, its bar, and
            // the two floats.
            solid: 12,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &[
                "0 window ||",
                "1 scrollView ||",
                "1 button Popover||",
                "1 button Toast||",
            ],
            events: &["toast -", "popover -", "popover -"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "drop",
        doc: "Files dragged in from the OS \
              (`docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md`): \
              two zones side by side, a button inside the first, and \
              across the phases the two things decision 2 looks past or \
              stops at — a float over the first zone that is no zone \
              (phase 1), and a modal over it (phase 2). The files enter \
              the first zone, move over its button (the button is the \
              zone's), move over the overlay (looked past), leave for \
              the second zone and land there with no leave after; under \
              the modal the first zone is no target and a cancel with \
              nothing lit is nothing. The first zone's `dropBg` lights \
              while they are over it, which the digest carries. The \
              `leave` carries the paths of the `enter` it was prepared \
              at (one file), not the two the last move reported: it is \
              built once, so a zone the view stops declaring still gets \
              it, and the paths cannot change within one OS drag.",
        custom: &["float", "key"],
        elements: &["box"],
        build: build_drop,
        env: NATIVE_CHROME,
        steps: &[
            Step::DragFiles(1, 50, 50),
            // Over the button inside the zone: a move on the zone.
            Step::DragFiles(1, 30, 30),
            Step::Phase(1),
            // Over the overlay, with two files now: still the zone's.
            Step::DragFiles(2, 60, 60),
            // The same point again: nothing.
            Step::DragFiles(2, 60, 60),
            // Into the second zone: leave, then enter.
            Step::DragFiles(1, 250, 50),
            Step::DropFiles(1, 250, 50),
            Step::Phase(2),
            // Under the modal the first zone is no target.
            Step::DragFiles(1, 50, 50),
            Step::DragCancel,
        ],
        expect: Expect {
            // The two zones, the button, the modal.
            solid: 4,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||", "1 button Pick||", "1 dialog ||"],
            events: &[
                "drop files enter 1",
                "drop files move 1",
                "drop files move 2",
                "drop files leave 1",
                "drop other enter 1",
                "drop other drop 1",
            ],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "underlines",
        doc: "An underline's own colour and shape (backlog K4): a rich text \
              whose span is underlined in red by a wave, a text underlined \
              in green by a solid line through its style rows, a text \
              underlined by dots in its own colour, and a 1×3 cell grid \
              whose cells carry the wave bit and a red underline colour. \
              A solid line is the one rect C22 drew; a wave and dots are \
              segments — the capsule a `line` draws — so the segment count \
              pins the shapes and no backend learned a kind. How many \
              pieces a run makes follows its advance and the face's \
              stroke, so the checked-in count is a lower bound (RG38) and \
              each adapter matches this machine's report exactly.",
        custom: &["key"],
        elements: &["box", "text", "cells"],
        build: build_underlines,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 1,
            shadows: 0,
            images: 0,
            // One piece at least for each of the three runs that is not
            // solid; how many more follows the face (see `Expect`).
            segments: 3,
            segments_follow_text: true,
            fragments: 0,
            textures: 0,
            glyphs_min: 12,
            access: &[
                "0 window ||",
                "1 staticText let value||",
                "1 staticText warn||",
                "1 staticText dots||",
                "1 terminal term||abc",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "joined-backgrounds",
        doc: "Rounded span backgrounds joined into one shape (backlog \
              F101): four texts of mono no-wrap spans stacked with no gap, \
              the first three's selected spans in one translucent colour \
              with `bgRadius` 4 — the tail of the first line, the whole \
              second, the head of the third — and the fourth's in another \
              colour, meeting the third but not joined with it. Each \
              rounded background is one `fragment` quad of the stock join \
              source the core registers, as the polygon's is, so no \
              adapter writes WGSL here; its sixteen params are its own \
              extent, the extents of the pieces it meets above and below, \
              the radius and which of the two there are, in physical px, \
              and the `fragment` lines of the report pin them to the bit. \
              A binding that drops the radius draws four solids instead.",
        custom: &[],
        elements: &["box", "text"],
        build: build_joined_backgrounds,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 0,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 4,
            textures: 0,
            glyphs_min: 20,
            access: &[
                "0 window ||",
                "1 staticText let a = 1;||",
                "1 staticText let b = 22;||",
                "1 staticText c + d||",
                "1 staticText find||",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "break-spaces",
        doc: "Whitespace that takes its room (backlog F106): a mono text \
              `ab  c` with `wrap` `break-spaces` in a box 4 px wide, its two \
              spaces under a background. Every glyph is wider than the box, \
              so each is a row of its own — the spaces too, which under \
              `word` hang past the edge of the row before them — and the \
              background is one solid on each space's row. A binding that \
              drops the value wraps by word and draws the spaces elsewhere.",
        custom: &[],
        elements: &["box", "text"],
        build: build_break_spaces,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 3,
            access: &["0 window ||", "1 staticText ab  c||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "relative-shrink",
        doc: "A share of the room gives when the room is spent (backlog \
              F110): a row 200 px wide with a 20 px gap holds two `50%` \
              bars, 90 each rather than 100 and overflowing; a row 300 px \
              wide with a 20 px gap holds two `clamp(100px, 60%, 400px)` \
              bars, 140 each rather than 180. A binding whose layout does \
              not shrink the shares draws them wider, past their rows.",
        custom: &[],
        elements: &["box"],
        build: build_relative_shrink,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 4,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "column-squeeze",
        doc: "A column of fit children gives none less height than its \
              content (backlog F114): in a column 50 px tall, three fit \
              rows each holding a bar 20 px tall stand at 0, 20 and 40, \
              overflowing the column, where they were squeezed to 16.7 px \
              each and their bars overlapped; beside them, under a 20 px \
              row, a column that clips gives to the 30 px left; and in a \
              third, a wrapping row of two 60 px chips, 10 apart, keeps \
              its two lines and the 5 px between them, 45 px, over a bar \
              at 45. A binding whose layout squeezes the rows draws the \
              bars closer.",
        custom: &[],
        elements: &["box"],
        build: build_column_squeeze,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 9,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "fit-across",
        doc: "A fit box across a column is no wider than the column's \
              box (backlog F116): in a card capped at 100, a fit column \
              holding a wrapping row of three 40 px chips, 10 apart, is \
              held to 100, and the row breaks there — two chips on the \
              first line, the third 25 px under them, 5 apart, the column \
              100 x 45; beside it at 120, the same card whose column \
              declares `minWidth: fit` keeps its 140 and one line, \
              running past the card. A binding whose layout leaves the \
              first column at its content draws its chips on one line.",
        custom: &[],
        elements: &["box"],
        build: build_fit_across,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 8,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "size-expressions",
        doc: "Size expressions (backlog F109), resolved against the \
              parent's content box: in a column 400 px wide, four bars 10 \
              px tall — `clamp(100px, 50%, 150px)` (150), `min(80%, 300px)` \
              (300), a 900 px bar under a `25%` `maxWidth` (100), and a fit \
              bar under a `max(40%, 50px)` `minWidth` (160). Each binding \
              writes some as spellings and some as data; the quads' \
              digest holds the four widths. A binding that drops an \
              expression draws a bar of another width, or none.",
        custom: &[],
        elements: &["box"],
        build: build_size_expressions,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 4,
            shadows: 0,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
    Scene {
        name: "sampler",
        doc: "The generic rows no other scene declares, on four nodes \
              (backlog AR47): a card carrying the size ceilings, `center`, \
              the four corner radii, an offset shadow, \
              the hover / pressed / focus colours, a hover group, \
              `initial_focus`, `accent`, a pointer cursor, `selected` and \
              `expanded`, the hover and layout and force-click tags, a \
              click and a hover sound, `animate`, and an eased, delayed, \
              alternating keyframe run with an entrance — read 60 ms in, so \
              the easing, the delay and the direction are in the quads; a \
              window-drag strip; a focus region aligned both ways with one \
              stop; a text with \
              `max_lines`, `ellipsis`, both decorations and a feature \
              string; and a `line` with a caret and a selection anchor. \
              The card is a lone `tab`, so the one warning is \
              `item-outside-container`. \
              The pointer enters the card (its hover tag and sound) and \
              clicks it (its sound), and the layout tag reports its rect. \
              The Node suite holds every generic prop to some scene's \
              source; this is the scene the rest land in.",
        custom: &["key", "size"],
        elements: &["box", "text"],
        build: build_sampler,
        env: NATIVE_CHROME,
        steps: &[
            Step::Time(0),
            Step::Cursor(40, 30),
            Step::MouseDown,
            Step::MouseUp,
            Step::Time(60),
        ],
        expect: Expect {
            // The card, the strip, the region's stop, and the line's
            // caret and its selection.
            solid: 5,
            shadows: 1,
            images: 0,
            segments: 0,
            segments_follow_text: false,
            fragments: 0,
            textures: 0,
            glyphs_min: 6,
            access: &[
                "0 window ||",
                "1 tab Card||",
                "1 titleBar ||",
                "1 button Stop||",
                "1 staticText a long line that is cut short||",
                "1 staticText sel||",
            ],
            // The layout tag from the first frame, the hover tag as the
            // pointer entered, and the click; the hover sound and the
            // click sound as two playbacks.
            events: &["layout lay", "hover hov", "card -"],
            announcements: &[],
            // The card is a `tab` with no `tabList` above it, and every
            // binding says so the same way.
            warnings: &["item-outside-container"],
            commands: &[],
            audio: &["play 1 0", "play 2 0"],
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: OptionAsAlt::None,
        },
    },
];

/// The rows a virtual list builds, at the data indices it builds them at.
/// A binding that lowers `index` correctly reproduces these keys wherever it
/// puts the rows; one that ignores the row auto-keys them by position and
/// every quad in the scene lands the same while the access tree and the hit
/// keys do not — which is why the rows carry roles and names.
pub const VIRTUAL_ROWS: [u64; 3] = [100, 101, 102];
/// How many rows the `virtual` scene's list declares it has (`rowCount`).
pub const VIRTUAL_ROW_COUNT: u64 = 109;

fn build_virtual(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with_keyed(
        "list",
        NodeSpec::column()
            .size(120.0, 60.0)
            .gap(0.0)
            .scroll_y()
            .bg(Color::hex(0x101018ff))
            .role(Role::List)
            .label("log"),
        |ui| {
            // How many rows the list has, built or not.
            ui.row_count(VIRTUAL_ROW_COUNT);
            // The height of the rows above the built range, and below it.
            ui.leaf_keyed("lead", virtual_spacer(20.0));
            for i in VIRTUAL_ROWS {
                ui.leaf_indexed(
                    i,
                    NodeSpec::column()
                        .grow_width()
                        .height(20.0)
                        .bg(Color::hex(0x30344aff))
                        .role(Role::ListItem)
                        .label(match i {
                            100 => "row 100",
                            101 => "row 101",
                            _ => "row 102",
                        }),
                );
            }
            ui.leaf_keyed("tail", virtual_spacer(100.0));
        },
    );
}

fn virtual_spacer(h: f32) -> NodeSpec {
    NodeSpec::column().grow_width().height(h)
}

fn build_layout(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(
        NodeSpec::column()
            .pad(8.0)
            .gap(6.0)
            .bg(Color::hex(0x14161eff)),
        |ui| {
            ui.with_keyed(
                "card",
                NodeSpec::row()
                    .padding(Edges {
                        l: 12.0,
                        r: 10.0,
                        t: 6.0,
                        b: 4.0,
                    })
                    .gap(4.0)
                    .bg(Color::hex(0x202030ff))
                    .border(2.0, Color::hex(0x2a2d3aff))
                    .radius(5.0)
                    .opacity(0.75)
                    .shadow_color(Color::hex(0x00000066))
                    .shadow_blur(8.0)
                    .shadow_y(3.0)
                    .shadow_spread(1.0)
                    .size(180.0, 40.0),
                |ui| {
                    ui.text("ab", TextStyle::new(12.0));
                    ui.text("cd", TextStyle::new(12.0));
                },
            );
            // Padding only, no children: the box's own size *is* the
            // resolved shorthand, so a binding that fell back differently
            // draws a differently sized quad. `padY` gives the top, `padB`
            // overrides the bottom, `padX` both sides.
            ui.leaf(
                NodeSpec::column()
                    .padding(
                        PadShorthand {
                            x: Some(9.0),
                            y: Some(3.0),
                            b: Some(1.0),
                            ..PadShorthand::default()
                        }
                        .resolve(),
                    )
                    .bg(Color::hex(0x2a2d3aff)),
            );
            ui.rich_text(
                &[
                    Span::new("a "),
                    Span::new("b").bold().color(Color::hex(0x73d98cff)),
                    Span::new(" c").italic(),
                ],
                TextStyle::new(13.0),
            );
        },
    );
}

/// The four sizing modes side by side in a parent whose width is known, so
/// each resolves to a width no other mode produces.
fn build_sizing(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(
        NodeSpec::column().padding(Edges {
            l: 14.0,
            r: 14.0,
            t: 6.0,
            b: 6.0,
        }),
        |ui| {
            ui.with_keyed(
                "bar",
                NodeSpec::row().size(200.0, 40.0).bg(Color::hex(0x101018ff)),
                |ui| {
                    let cell = |bg: u32| NodeSpec::column().height(20.0).bg(Color::hex(bg));
                    ui.leaf(cell(0x30344aff).width(30.0));
                    ui.with(cell(0x3b5bd4ff).width(Sizing::Percent(0.25)), |ui| {
                        // Nothing inside: a percent is the parent's, not
                        // the content's.
                        let _ = ui;
                    });
                    ui.with(cell(0x73d98cff).width(Sizing::Fit), |ui| {
                        ui.leaf(
                            NodeSpec::column()
                                .size(20.0, 10.0)
                                .bg(Color::hex(0xff0000ff)),
                        );
                    });
                    ui.leaf(cell(0xffcc00ff).grow_width());
                },
            );
        },
    );
}

/// 92px of content, a 6px gap: 30 + 40 fit, 50 + 20 go to the second line.
/// The heights differ per box so the two lines have different cross
/// extents and a binding that dropped `crossGap` lands them elsewhere.
fn build_wrap(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(
        NodeSpec::row()
            .wrap()
            .pad(4.0)
            .gap(6.0)
            .cross_gap(10.0)
            .width(100.0)
            .bg(Color::hex(0x101018ff)),
        |ui| {
            for (w, h) in WRAP_BOXES {
                ui.leaf(NodeSpec::column().size(*w, *h).bg(Color::hex(0x30344aff)));
            }
        },
    );
}

/// The `align` scene's three spreads, in the order its rows declare them.
pub const ALIGN_SPREADS: [Align; 3] = [Align::SpaceBetween, Align::SpaceAround, Align::SpaceEvenly];

fn build_align(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let square = || {
        NodeSpec::column()
            .size(10.0, 10.0)
            .bg(Color::hex(0x30344aff))
    };
    ui.with(
        NodeSpec::column()
            .pad(4.0)
            .gap(6.0)
            .width(128.0)
            .bg(Color::hex(0x101018ff)),
        |ui| {
            for a in ALIGN_SPREADS {
                ui.with(NodeSpec::row().width(120.0).main_align(a), |ui| {
                    for _ in 0..3 {
                        ui.leaf(square());
                    }
                });
            }
            ui.with(
                NodeSpec::row().gap(4.0).cross_align(Align::Baseline),
                |ui| {
                    ui.text("ab", TextStyle::new(12.0));
                    ui.text("cd", TextStyle::new(20.0));
                    ui.leaf(square());
                },
            );
            ui.leaf(
                NodeSpec::column()
                    .grow_width()
                    .aspect_ratio(4.0)
                    .bg(Color::hex(0x3b5bd4ff)),
            );
            ui.leaf(
                NodeSpec::column()
                    .height(12.0)
                    .aspect_ratio(2.0)
                    .bg(Color::hex(0x73d98cff)),
            );
        },
    );
}

fn build_stock_controls(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let m = ui.metrics();
    let tag = |k: &str| Value::map([("kind", Value::str(k))]);
    ui.with(NodeSpec::column().pad(8.0).gap(8.0), |ui| {
        widgets::slider_with(
            ui,
            "Volume",
            widgets::slider_spec(&m)
                .width(216.0)
                .value_now(30.0)
                .value_min(0.0)
                .value_max(100.0)
                .value_step(10.0)
                .on_change(tag("vol")),
            None,
        );
        widgets::checkbox(ui, "Mute", false, tag("mute"));
        widgets::checkbox(ui, "Sync", true, tag("sync"));
        widgets::toggle_with(
            ui,
            widgets::Toggle::Checkbox,
            "All",
            "All",
            widgets::toggle_spec(&m).mixed(true).on_click(tag("all")),
            None,
        );
        widgets::radio_group_with(ui, "Theme", widgets::radio_group_spec(&m), |ui| {
            widgets::radio(ui, "Light", false, tag("light"));
            widgets::radio(ui, "Dark", true, tag("dark"));
        });
        widgets::switch(ui, "Wi-Fi", true, tag("wifi"));
    });
}

/// The rows of the `table` scene: a label (a bare text cell), the width
/// of the fixed cell beside it, and the grow cell's height.
pub const TABLE_ROWS: &[(&str, f32, f32)] = &[
    ("abc", 30.0, 10.0),
    ("abcde", 50.0, 12.0),
    ("ab", 20.0, 8.0),
];

fn build_table(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with_keyed(
        "table",
        NodeSpec::table()
            .width(200.0)
            .pad(4.0)
            .gap(2.0)
            .bg(Color::hex(0x101018ff))
            // Grid rules: one across each of the three
            // gaps between the four rows, one down each of the two gaps
            // between the body rows' three columns.
            .rules(Color::hex(0x2b3350ff))
            .rule_width(1.0),
        |ui| {
            // The header: a fit row of two cells, held to the columns.
            ui.with(NodeSpec::row().gap(6.0), |ui| {
                ui.text("name", TextStyle::new(12.0));
                ui.text("w", TextStyle::new(12.0));
            });
            for (label, w, h) in TABLE_ROWS {
                ui.with(NodeSpec::row().grow_width().gap(6.0), |ui| {
                    ui.text(label, TextStyle::new(12.0));
                    ui.leaf(NodeSpec::column().size(*w, *h).bg(Color::hex(0x30344aff)));
                    ui.leaf(
                        NodeSpec::column()
                            .grow_width()
                            .height(*h)
                            .bg(Color::hex(0x3b5bd4ff)),
                    );
                });
            }
        },
    );
}

/// A card that scopes one selection over the three runs inside it. Fixed
/// width so the runs sit where the steps expect whatever the window is,
/// and one style for all three so a binding cannot pass by getting one
/// size right and another wrong.
fn build_selection(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with_keyed(
        "card",
        NodeSpec::column()
            .width(200.0)
            .pad(8.0)
            .gap(4.0)
            .bg(Color::hex(0x14161eff))
            .selectable(),
        |ui| {
            for line in SELECTION_LINES {
                ui.text(line, TextStyle::new(13.0));
            }
        },
    );
}

fn build_tabs(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let bar = || NodeSpec::row().size(200.0, 20.0).bg(Color::hex(0x101018ff));
    let tab = || {
        NodeSpec::column()
            .grow_width()
            .min_width(Min::FIT)
            .bg(Color::hex(0x30344aff))
    };
    let label = |w: f32, h: f32| NodeSpec::column().size(w, h).bg(Color::hex(0x3b5bd4ff));
    ui.with(NodeSpec::column().pad(4.0).gap(4.0), |ui| {
        ui.with_keyed("roomy", bar(), |ui| {
            for (w, h) in TAB_ROOMY {
                ui.with(
                    tab().height(Sizing::Percent(0.5)).min_height(Min::FIT),
                    |ui| {
                        ui.leaf(label(*w, *h));
                    },
                );
            }
        });
        ui.with_keyed("crowded", bar().scroll_x(), |ui| {
            for w in TAB_CROWDED {
                ui.with(tab().grow_height(), |ui| {
                    ui.leaf(label(*w, 12.0));
                });
            }
        });
    });
}

fn build_overflow(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(NodeSpec::column().pad(4.0).clip(), |ui| {
        ui.with_keyed(
            "list",
            NodeSpec::column()
                .size(120.0, 60.0)
                .gap(4.0)
                .scroll_y()
                // Rounded and clipping: the items inside inherit the
                // rounded clip, so every adapter has to carry a quad's
                // `clip_radius` (which the digest hashes) and not just its
                // `clip`.
                .radius(8.0)
                .bg(Color::hex(0x101018ff)),
            |ui| {
                for key in ITEM_KEYS {
                    ui.leaf_keyed(
                        key,
                        NodeSpec::column()
                            .size(100.0, 20.0)
                            .bg(Color::hex(0x30344aff)),
                    );
                }
            },
        );
    });
}

/// Item labels as constants: a binding building this scene has to use the
/// same strings, since keys are hashes of the path.
pub const ITEM_KEYS: [&str; 6] = ["i0", "i1", "i2", "i3", "i4", "i5"];

/// The `scrollbar` scene's three scrollers, keyed in this order: the
/// `hidden` one, the styled one, the `auto` one.
pub const SCROLLBAR_KEYS: [&str; 3] = ["hidden", "styled", "auto"];

fn build_scrollbar(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(NodeSpec::row().pad(10.0).gap(10.0), |ui| {
        for (n, key) in SCROLLBAR_KEYS.iter().enumerate() {
            let list = NodeSpec::column()
                .size(90.0, 60.0)
                .gap(0.0)
                .scroll_y()
                .bg(Color::hex(0x101018ff));
            let list = match n {
                0 => list.scrollbar(ScrollbarMode::Hidden),
                1 => list
                    .scrollbar_width(8.0)
                    .scrollbar_color(Color::hex(0x3b5bd4ff))
                    .scrollbar_active_color(Color::hex(0xffcc00ff)),
                _ => list.scrollbar(ScrollbarMode::Auto),
            };
            ui.with_keyed(key, list, |ui| {
                for item in ITEM_KEYS {
                    ui.leaf_keyed(
                        item,
                        NodeSpec::column()
                            .size(80.0, 20.0)
                            .bg(Color::hex(0x30344aff)),
                    );
                }
            });
        }
    });
}

/// The `tokens` scene's table, as every adapter declares it: the names
/// and values, with `surface` in it on purpose.
pub const TOKEN_COLORS: [(&str, u32, u32); 3] = [
    ("peach", 0xffcc99ff, 0xffcc99ff),
    ("ink", 0x202020ff, 0xe0e0e0ff),
    ("surface", 0xff0000ff, 0xff0000ff),
];
pub const TOKEN_LENGTHS: [(&str, f32); 3] = [("side_w", 60.0), ("gap", 8.0), ("big", 16.0)];

/// The scene's derived tokens, declared after the values in
/// this order: the name, its source, and the chain as `(verb, colour
/// operand or "", number)` tuples. `lit` is one step off a value; `dim`
/// two steps that do not commute, so the fold's order is pinned; `up`
/// derives from `surface`, which the declaration above lost to the role,
/// so its source *is* the role and `raise` turns with the base; `deep`
/// derives from a derived token; `read` is the contrast loop against
/// the themed `ink`, which it has to move for on the dark base only; and
/// `bad` names a source nothing declared, so it is dropped with
/// `unknown-token` at declaration and never referenced.
/// One step of a corpus recipe: the verb, the colour operand or `""`, and
/// the number.
pub type TokenOp = (&'static str, &'static str, f32);
pub const TOKEN_DERIVED: [(&str, &str, &[TokenOp]); 6] = [
    ("lit", "peach", &[("lift", "", 0.3)]),
    ("dim", "ink", &[("mix", "peach", 0.5), ("darken", "", 0.5)]),
    ("up", "surface", &[("raise", "", 0.25)]),
    ("deep", "lit", &[("alpha", "", 0.5)]),
    ("read", "peach", &[("readable", "ink", 4.5)]),
    ("bad", "nothing", &[("lift", "", 0.1)]),
];

/// The keyed boxes of the `tokens` scene, in order: the four of ADR 0027
/// and one per derived token that resolved.
pub const TOKEN_KEYS: [&str; 9] = [
    "peach", "ink", "role", "missing", "lit", "dim", "up", "deep", "read",
];

fn build_tokens(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let mut t = crate::tokens::Tokens::new();
    for (name, light, dark) in TOKEN_COLORS {
        t = t.color_themed(name, Color::hex(light), Color::hex(dark));
    }
    for (name, px) in TOKEN_LENGTHS {
        t = t.length(name, px);
    }
    for (name, from, ops) in TOKEN_DERIVED {
        let ops = ops.iter().map(|(verb, color, n)| {
            let color = (!color.is_empty()).then_some(*color);
            crate::tokens::ColorOp::parse(verb, color, *n).expect("a corpus verb")
        });
        t = t.derive(name, from, ops);
    }
    ui.set_tokens(t);
    // Rust holds no reference: it reads each value by name and writes it,
    // which is what the other adapters' `$name` resolves to — and a name
    // that resolves to nothing leaves the row undeclared, as their
    // `$nothing` does (AR14: one miss policy).
    let peach = ui.token_color("peach").expect("declared");
    let ink = ui.token_color("ink").expect("declared");
    let surface = ui.token_color("surface").expect("a role");
    let nothing = ui.token_color("nothing");
    let derived: Vec<Option<Color>> = TOKEN_KEYS[4..]
        .iter()
        .map(|name| ui.token_color(name))
        .collect();
    let side_w = ui.token_length("side_w").expect("declared");
    let gap = ui.token_length("gap").expect("declared");
    let big = ui.token_length("big").expect("declared");
    let radius = ui.token_length("radius").expect("a role");
    let cell = |bg: Option<Color>| {
        let spec = NodeSpec::column().size(side_w, 30.0);
        match bg {
            Some(bg) => spec.bg(bg),
            None => spec,
        }
    };
    ui.with(
        NodeSpec::row()
            .padding(Edges {
                l: gap,
                r: 10.0,
                t: 10.0,
                b: 10.0,
            })
            .gap(gap),
        |ui| {
            ui.leaf_keyed(TOKEN_KEYS[0], cell(Some(peach)));
            ui.leaf_keyed(TOKEN_KEYS[1], cell(Some(ink)).border(gap, peach));
            ui.leaf_keyed(TOKEN_KEYS[2], cell(Some(surface)).radius(radius));
            ui.leaf_keyed(TOKEN_KEYS[3], cell(nothing));
            for (key, c) in TOKEN_KEYS[4..].iter().zip(derived) {
                ui.leaf_keyed(key, cell(c));
            }
            ui.rich_text(
                &[Span::new("tokens"), Span::new("x").color(ink)],
                TextStyle::new(big).color(peach),
            );
        },
    );
}

/// The `anchor` scene's two scrollers, by key: the anchored one and the
/// control.
pub const ANCHOR_KEYS: [&str; 2] = ["anchored", "plain"];

fn build_anchor(ui: &mut Ui<'_>, _f: &Fixtures, phase: u32) {
    ui.with(NodeSpec::row().pad(10.0).gap(10.0), |ui| {
        for (n, key) in ANCHOR_KEYS.iter().enumerate() {
            let list = NodeSpec::column()
                .size(90.0, 60.0)
                .scroll_y()
                .bg(Color::hex(0x101018ff));
            let list = if n == 0 { list.anchor() } else { list };
            ui.with_keyed(key, list, |ui| {
                let row = |ui: &mut Ui<'_>, key: &str, h: f32| {
                    ui.leaf_keyed(
                        key,
                        NodeSpec::column().size(80.0, h).bg(Color::hex(0x30344aff)),
                    );
                };
                if phase >= 1 {
                    row(ui, "new", 30.0);
                }
                for item in ITEM_KEYS {
                    row(ui, item, 20.0);
                }
            });
        }
    });
}

/// The three runs of the `selection` scene, in order. Short and distinct
/// so a report shows at a glance which run a highlight belongs to, and
/// three of them so the drag has a run to cover *whole* between its two
/// partial ends.
pub const SELECTION_LINES: [&str; 3] = ["one", "two", "three"];

/// The six runs of the `selection-scroll` scene: the three above and
/// three more, so the card has somewhere to scroll to.
pub const SELECTION_SCROLL_LINES: [&str; 6] = ["one", "two", "three", "four", "five", "six"];

/// The `selection-scroll` card's height: two runs and a bit, so four of
/// the six are past its edge.
pub const SELECTION_SCROLL_HEIGHT: f32 = 40.0;

/// The `selection` card, forty px tall and scrolling, over six runs.
fn build_selection_scroll(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with_keyed(
        "card",
        NodeSpec::column()
            .size(200.0, SELECTION_SCROLL_HEIGHT)
            .pad(8.0)
            .gap(4.0)
            .bg(Color::hex(0x14161eff))
            .scroll_y()
            .selectable(),
        |ui| {
            for line in SELECTION_SCROLL_LINES {
                ui.text(line, TextStyle::new(13.0));
            }
        },
    );
}

/// The three rows of the `cells-scroll` screen, and the absolute line row
/// 0 is at phase 0: the phase is added to it, which is how the scene's
/// view "answers" a scroll event.
pub const CELLS_SCROLL_ROWS: [&str; 3] = ["hello world", "brave", "bye"];
pub const CELLS_SCROLL_ORIGIN: u64 = 100;

/// The `cells` screen, three rows, `selectable` and hearing the wheel,
/// with row 0 at `CELLS_SCROLL_ORIGIN + phase`.
/// `scroll-gestures`: a page (y) holding a strip (x) holding a contained
/// list (y) and a y-only wheel handler, then a spacer each.
fn build_scroll_gestures(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(NodeSpec::column().pad(4.0), |ui| {
        ui.with_keyed(
            "page",
            NodeSpec::column()
                .size(200.0, 100.0)
                .scroll_y()
                .bg(Color::hex(0x101018ff)),
            |ui| {
                ui.with_keyed(
                    "strip",
                    NodeSpec::row().size(200.0, 80.0).scroll_x(),
                    |ui| {
                        ui.with_keyed(
                            "list",
                            NodeSpec::column()
                                .size(100.0, 80.0)
                                .gap(4.0)
                                .scroll_y()
                                .overscroll(Overscroll::Contain)
                                .bg(Color::hex(0x161820ff)),
                            |ui| {
                                for key in ITEM_KEYS {
                                    ui.leaf_keyed(
                                        key,
                                        NodeSpec::column()
                                            .size(90.0, 20.0)
                                            .bg(Color::hex(0x30344aff)),
                                    );
                                }
                            },
                        );
                        ui.leaf_keyed(
                            "term",
                            NodeSpec::column()
                                .size(100.0, 80.0)
                                .bg(Color::hex(0x3b5bd4ff))
                                .on_scroll(Value::map([("kind", Value::str("term"))]))
                                .scroll_axes(ScrollAxes::Y),
                        );
                        ui.leaf(
                            NodeSpec::column()
                                .size(100.0, 80.0)
                                .bg(Color::hex(0x2a2d3aff)),
                        );
                    },
                );
                ui.leaf(
                    NodeSpec::column()
                        .size(200.0, 60.0)
                        .bg(Color::hex(0x22252fff)),
                );
            },
        );
    });
}

/// `scroll-handler-room`: a strip (x) holding a code box that scrolls x
/// and hears the wheel, its content three times its width, then a spacer.
fn build_scroll_handler_room(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(NodeSpec::column().pad(4.0), |ui| {
        ui.with_keyed(
            "strip",
            NodeSpec::row()
                .size(200.0, 80.0)
                .scroll_x()
                .bg(Color::hex(0x101018ff)),
            |ui| {
                ui.with_keyed(
                    "code",
                    NodeSpec::column()
                        .size(100.0, 80.0)
                        .scroll_x()
                        .on_scroll(Value::map([("kind", Value::str("code"))]))
                        .bg(Color::hex(0x161820ff)),
                    |ui| {
                        ui.leaf(
                            NodeSpec::column()
                                .size(300.0, 80.0)
                                .bg(Color::hex(0x3b5bd4ff)),
                        );
                    },
                );
                ui.leaf(
                    NodeSpec::column()
                        .size(200.0, 80.0)
                        .bg(Color::hex(0x2a2d3aff)),
                );
            },
        );
    });
}

fn build_cells_scroll(ui: &mut Ui<'_>, _f: &Fixtures, phase: u32) {
    use crate::cells::{Cell, CellGrid};
    let cols = 11;
    let mut cells = vec![Cell::new(' ', 0xd6d8e0ff, 0); 3 * cols];
    for (r, row) in CELLS_SCROLL_ROWS.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            cells[r * cols + c] = Cell::new(ch, 0xd6d8e0ff, 0);
        }
    }
    ui.with(NodeSpec::column().pad(10.0), |ui| {
        ui.cells_keyed(
            "term",
            &CellGrid {
                rows: 3,
                cols,
                cells: &cells,
                style: TextStyle::new(13.0).mono().line_height(18.0),
                cursor: None,
                origin_line: CELLS_SCROLL_ORIGIN + phase as u64,
            },
            NodeSpec::default()
                .selectable()
                .on_scroll(Value::map([("kind", Value::str("term"))]))
                .label("term"),
        );
    });
}

fn build_float(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(NodeSpec::column().pad(20.0).gap(4.0), |ui| {
        ui.with_keyed(
            "anchor",
            NodeSpec::column()
                .size(80.0, 24.0)
                .bg(Color::hex(0x333333ff)),
            |ui| {
                ui.leaf(
                    NodeSpec::column()
                        .float(FloatConfig::below())
                        .size(40.0, 12.0)
                        .bg(Color::hex(0xff0000ff)),
                );
            },
        );
        // A preset as a base, with one override declared: `dx` moves it
        // sideways and the untouched `dy` keeps "below"'s own 6px gap. A
        // binding that wrote a whole offset instead of the piece it saw
        // lands this quad 6px too high.
        ui.with_keyed(
            "nudged",
            NodeSpec::column()
                .size(60.0, 20.0)
                .bg(Color::hex(0x444444ff)),
            |ui| {
                ui.leaf(
                    NodeSpec::column()
                        .float(FloatConfig::build(
                            FloatConfig::below(),
                            None,
                            None,
                            Some(6.0),
                            None,
                            false,
                            false,
                        ))
                        .size(30.0, 10.0)
                        .bg(Color::hex(0x0000ffff)),
                );
            },
        );
        ui.leaf(
            NodeSpec::column()
                // Deliberately asymmetric in every axis, and placed so
                // that `fit` has to do something: `at` differs from
                // `self_at` per axis, `dx` from `dy`, and the attachment
                // lands at (-16, 254) — off the viewport on both sides —
                // so a dropped `fit` moves it. Symmetric values here (the
                // `at == self_at`, `dx == dy` this used to have) let a
                // binding swap either pair with no visible effect.
                .float(
                    FloatConfig::viewport()
                        .at(Align::Start, Align::End)
                        .self_at(Align::End, Align::Start)
                        .offset(-6.0, 14.0)
                        .fit(),
                )
                .size(10.0, 10.0)
                .bg(Color::hex(0x00ff00ff)),
        );
    });
}

/// The tooltip prop, spelled out: the bindings' parsers turn `tooltip`
/// into `hoverable` plus an accessible `description`, and draw
/// `widgets::hover_hint` as the node's last child while it is hovered —
/// the stock tooltip's chrome under `role="none"`, so the hint's text is
/// read once, as the description, and never as the group's content.
///
/// The second node is the `description` prop on its own — the same slot
/// with neither the hover tracking nor the float, which is what a hint
/// that is spoken and never drawn looks like. It sits here rather than in
/// a scene of its own so the two are read side by side: the same string
/// arrives in the same column of the access dump, and only the tooltip
/// draws anything for it.
fn build_select(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let options = [
        MenuItem::new("English"),
        MenuItem::new("Deutsch"),
        MenuItem::new("Français"),
        MenuItem::new("Latin").id(Value::str("la")).enabled(false),
    ];
    ui.with(NodeSpec::column().pad(10.0).gap(6.0), |ui| {
        widgets::select_items(ui, "language", &options, Some(1));
        ui.text("body", TextStyle::new(12.0));
    });
}

fn build_tooltip(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(NodeSpec::column().pad(10.0), |ui| {
        let key = ui.child_key("tip");
        ui.with_keyed(
            "tip",
            NodeSpec::row()
                .size(100.0, 40.0)
                .bg(Color::hex(0x333333ff))
                .role(Role::Group)
                .hoverable()
                .description("a hint"),
            |ui| {
                ui.text("badge", TextStyle::new(12.0));
                if ui.is_hovered(key) {
                    widgets::hover_hint(ui, "a hint");
                }
            },
        );
        ui.leaf(
            NodeSpec::row()
                .size(100.0, 20.0)
                .role(Role::Button)
                .label("Save")
                .description("Nothing to save yet"),
        );
    });
}

/// The application menu bar: a declaration, and the widget that
/// draws it. Two menus, so the second's title is somewhere to hover; a
/// standard role, a separator, a checked row and an accelerator, so every
/// part of a row is in the frame the report keeps.
fn build_menu_bar(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let bar = MenuBar::new(vec![
        BarMenu::new(
            "File",
            vec![
                MenuItem::new("New")
                    .id(Value::str("file.new"))
                    .accel("mod+n"),
                MenuItem::separator(),
                MenuItem::new("Wrap")
                    .id(Value::str("file.wrap"))
                    .checked(true),
                MenuItem::new("Print")
                    .id(Value::str("file.print"))
                    .enabled(false),
            ],
        ),
        BarMenu::new("Edit", vec![MenuItem::role(MenuRole::Copy)]),
    ]);
    // Grow, because the bar is a full-width strip: in a `fit` parent it
    // would be squeezed to the widest thing beside it and its titles would
    // wrap, which is ordinary flex and worth a scene not tripping over.
    ui.with(NodeSpec::column().gap(6.0).grow_width(), |ui| {
        widgets::menu_bar(ui, bar);
        ui.text("body", TextStyle::new(12.0));
    });
}

fn build_chrome(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.window_title("kui conformance");
    // The other root declaration with no node: the frame
    // asks for the window on top, and the report says it asked.
    ui.always_on_top(true);
    // And the third: the frame asks for secure keyboard
    // entry, which a runner applies while the window has the keyboard.
    ui.secure_input(true);
    // And the fourth: the left Option key as Alt, which a
    // runner applies to the window on change.
    ui.option_as_alt(OptionAsAlt::Left);
    ui.with(NodeSpec::column().gap(6.0), |ui| {
        // The adaptive form: content between the platform inset and the
        // cluster `titlebar_with` appends by itself.
        widgets::titlebar_with(ui, |ui| ui.text("app", TextStyle::new(12.0)));
        // And the fully-custom form the `windowButtons` element is *for*
        // (`schema::ELEMENTS`: "just the min/max/close buttons, for fully
        // custom titlebars") — a strip an app laid out itself. Two clusters
        // is the point: they are the two ways an app gets the buttons, and
        // only the second one goes through each binding's own element.
        // The strip is a plain row rather than a second `window_drag`: the
        // element under test is the cluster, and a drag handle would derive
        // a second `titleBar` role, which is a thing to tell a screen reader
        // rather than a side effect of where the corpus put a box. It keeps
        // focus (`keepFocus`), as a strip of buttons beside
        // an editor would, which changes nothing drawn.
        ui.with(
            NodeSpec::row().grow_width().keep_focus(),
            widgets::window_buttons,
        );
        let sink = ui.open_keyed(
            "sink",
            NodeSpec::column()
                .size(40.0, 16.0)
                .bg(Color::hex(0x22242cff))
                .focusable()
                .on_focus(Value::map([("kind", Value::str("sink"))]))
                .label("Sink"),
        );
        ui.take_key_focus(sink);
        ui.close();
    });
}

fn build_controls(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let panel = NodeSpec::column()
        .pad(10.0)
        .gap(6.0)
        .on_context_menu(Value::map([("kind", Value::str("menu"))]))
        .on_button(Value::map([("kind", Value::str("panel"))]))
        .buttons(crate::input::Buttons::MIDDLE);
    ui.with(panel, |ui| {
        widgets::button_with(
            ui,
            "go",
            "go",
            widgets::button_spec(&ui.theme(), &ui.metrics())
                .on_click(Value::map([("kind", Value::str("go"))]))
                .description("Starts the run"),
            None,
        );
        // Every row the stock button admits, on a button the steps never
        // touch. The spec is what a binding's parser builds from
        // `tooltip = "…"` — `apply_tooltip` for the hover tracking and the
        // description — plus the hint it floats while hovered, and the
        // access row is the proof: a name that is not the text, the
        // disabled column, and the tooltip's string in the description
        // column with nothing drawn for it.
        widgets::button_with(
            ui,
            "stop",
            "stop",
            widgets::button_spec(&ui.theme(), &ui.metrics())
                .on_click(Value::map([("kind", Value::str("stop"))]))
                .label("Stop the run")
                .disabled(true)
                .apply_tooltip("Nothing is running"),
            Some("Nothing is running"),
        );
        // A field with `wrap` declared: a field's keyboard on a document's
        // layout. The seed is wider than the box, so the
        // quad digest pins the second line — and a binding that dropped
        // the row would lay the draft out on one line, scrolled.
        ui.text_edit(
            "note",
            "hello, on two lines in a narrow field",
            &EditOptions {
                style: TextStyle::new(13.0),
                wrap: true,
                ..Default::default()
            },
            NodeSpec::column().width(160.0).label("Note"),
        );
        // A slider that says what its position reads as. Without
        // `value_text` a reader has only the three numbers and says a
        // percentage — 25 in [5..60] is "36 percent" — and the reading
        // travels in the access row's value column, the one string slot a
        // node has. No background, so the scene's quad counts
        // are the button's and the editor's as before.
        ui.leaf_keyed(
            "focus",
            NodeSpec::row()
                .role(Role::Slider)
                .label("Focus length")
                .value_now(25.0)
                .value_min(5.0)
                .value_max(60.0)
                .value_text("25 minutes")
                .size(120.0, 12.0),
        );
    });
}

/// The keys scene: two sinks at known rows — (60, 22) is the press-only
/// one, (60, 52) the one that hears releases. The tag is an integer, not
/// a map with a `kind`, so the report's event column falls back to the
/// phase — which is what the scene is about — and Lua, which has no
/// spelling for a null tag, can declare the same sink.
fn build_ime(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(NodeSpec::column().pad(10.0).gap(6.0), |ui| {
        ui.with_keyed(
            "buffer",
            NodeSpec::column()
                .size(200.0, 24.0)
                .bg(Color::hex(0x1b1d27ff))
                .on_key(Value::map([("kind", Value::str("ed"))]))
                .role(Role::MultilineTextInput)
                .label("Buffer"),
            |ui| {
                ui.text_in_keyed(
                    "l0",
                    NodeSpec::row().height(20.0).role(Role::Line).caret(1),
                    "ab",
                    TextStyle::new(13.0).mono(),
                );
            },
        );
        ui.text_edit(
            "note",
            "",
            &EditOptions {
                style: TextStyle::new(13.0),
                ..Default::default()
            },
            NodeSpec::column().width(200.0).label("Note"),
        );
    });
}

fn build_cells(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    use crate::cells::{Cell, CellGrid, CursorShape};
    let cells: Vec<Cell> = "hello world"
        .chars()
        .enumerate()
        .map(|(i, ch)| Cell::new(ch, 0xd6d8e0ff, if i < 3 { 0x1a1d27ff } else { 0 }))
        .collect();
    ui.with(NodeSpec::column().pad(10.0), |ui| {
        ui.cells_keyed(
            "term",
            &CellGrid {
                rows: 1,
                cols: 11,
                cells: &cells,
                style: TextStyle::new(13.0).mono().line_height(18.0),
                cursor: Some((0, 3, CursorShape::Block, Color::hex(0x6a8bffff))),
                origin_line: 0,
            },
            NodeSpec::default()
                .on_click(Value::map([("kind", Value::str("hit"))]))
                .label("term"),
        );
    });
}

fn build_keys(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let sink = |label: &str| {
        NodeSpec::row()
            .size(100.0, 24.0)
            .bg(Color::hex(0x1b1d27ff))
            .on_key(Value::Int(1))
            .role(Role::Group)
            .label(label)
    };
    ui.with(NodeSpec::column().pad(10.0).gap(6.0), |ui| {
        ui.leaf_keyed("press", sink("press"));
        ui.leaf_keyed("held", sink("held").key_up());
        // A shell over a ring: the sink hears what the button inside it
        // does not claim, and the button holds focus from the first frame.
        ui.with_keyed("shell", sink("shell").key_up(), |ui| {
            let go = ui.leaf_keyed(
                "go",
                NodeSpec::row()
                    .size(80.0, 16.0)
                    .bg(Color::hex(0x3b5bd4ff))
                    .on_click(Value::map([("kind", Value::str("go"))]))
                    .label("Go"),
            );
            ui.take_key_focus(go);
        });
    });
}

/// Two sinks asking for releases, tagged by kind so the report tells
/// them apart; the second asks for the modifier keys.
fn build_modifier_keys(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let sink = |label: &str| {
        NodeSpec::row()
            .size(100.0, 24.0)
            .bg(Color::hex(0x1b1d27ff))
            .on_key(Value::map([("kind", Value::str(label))]))
            .key_up()
            .role(Role::Group)
            .label(label)
    };
    ui.with(NodeSpec::column().pad(10.0).gap(6.0), |ui| {
        ui.leaf_keyed("plain", sink("plain"));
        ui.leaf_keyed("mods", sink("mods").modifier_keys());
    });
}

/// The three playbacks are declared in this order, so the ids the report
/// names are 1 (`music`), 2 (`chime`) and 3 (`blip`) in every binding.
/// Phase 1 stops declaring the last two, which is the scene's whole point:
/// `chime` asked to [`AudioSpec::finish`] and leaves no command behind,
/// `blip` did not and is stopped.
fn build_media(ui: &mut Ui<'_>, f: &Fixtures, phase: u32) {
    use crate::resources::{ImageFit, ImageOpts, Sampling};
    ui.with(NodeSpec::column().pad(6.0).gap(4.0), |ui| {
        ui.image(f.image, NodeSpec::column().width(16.0).radius(2.0));
        // The 4×4 icon in a 32×16 box: `contain` paints a 16×16 rect,
        // centred, and the box (the access rect, the hit region) stays 32.
        ui.image_with(
            f.image,
            ImageOpts {
                fit: ImageFit::Contain,
                ..ImageOpts::default()
            },
            NodeSpec::column().size(32.0, 16.0).label("Icon"),
        );
        ui.image(f.stream, NodeSpec::column().width(16.0).label("Stream"));
        ui.image_with(
            f.stream,
            ImageOpts {
                sampling: Sampling::Nearest,
                ..ImageOpts::default()
            },
            NodeSpec::column().width(16.0).label("Crisp"),
        );
        // The 8×2 stream in a 12×12 box: `cover` keeps the box and shows
        // the middle 2×2 texels — `texture 2 3 0 2 2` in the report.
        ui.image_with(
            f.stream,
            ImageOpts {
                fit: ImageFit::Cover,
                ..ImageOpts::default()
            },
            NodeSpec::column().size(12.0, 12.0).label("Cropped"),
        );
        ui.audio_keyed("music", AudioSpec::new(f.sound).volume(0.5).looped());
        widgets::latency_graph(ui);
        // Last, and in this order: an `audio` element draws nothing, so a
        // Lua table can end on the two the phase drops without a nil in
        // the middle of its sequence.
        if phase == 0 {
            ui.audio_keyed("chime", AudioSpec::new(f.sound).finish());
            ui.audio_keyed("blip", AudioSpec::new(f.sound));
        }
    });
}

/// The `polygon` scene's outlines, in the canvas's box space, shared so
/// every adapter draws the same vertices and a typo cannot pass as a
/// normalisation difference. The last is nine points on purpose.
pub const POLYGON_TRIANGLE: &[(f32, f32)] = &[(10.0, 10.0), (60.0, 20.0), (20.0, 50.0)];
pub const POLYGON_ARROW: &[(f32, f32)] = &[(80.0, 10.0), (130.0, 30.0), (80.0, 50.0), (95.0, 30.0)];
pub const POLYGON_STAR: &[(f32, f32)] = &[
    (170.0, 10.0),
    (176.0, 24.0),
    (190.0, 30.0),
    (176.0, 36.0),
    (170.0, 50.0),
    (164.0, 36.0),
    (150.0, 30.0),
    (164.0, 24.0),
];
pub const POLYGON_NINE: &[(f32, f32)] = &[
    (10.0, 70.0),
    (30.0, 65.0),
    (50.0, 70.0),
    (70.0, 65.0),
    (90.0, 70.0),
    (90.0, 110.0),
    (50.0, 100.0),
    (10.0, 110.0),
    (5.0, 90.0),
];
pub const POLYGON_QUAD: &[(f32, f32)] =
    &[(110.0, 70.0), (190.0, 70.0), (180.0, 110.0), (120.0, 110.0)];

/// The `path` scene's outlines as SVG path data, shared so every adapter
/// draws the same ops. The ring is also spelled in the flat op form
/// ([`PATH_RING_OPS`]) for the bindings that take it, and the two must be
/// the same ops to the bit — a test in `tests/conformance.rs` holds them
/// to it. The last is malformed on purpose.
pub const PATH_WEDGE: &str = "M60 60 L100 60 A40 40 0 0 1 60 100 Z";
pub const PATH_WEDGE2: &str = "M60 60 L60 100 A40 40 0 0 1 20 60 Z";
pub const PATH_RING: &str = "M120 10 H190 V80 H120 Z M140 30 H170 V60 H140 Z";
pub const PATH_RING_OPS: &[f32] = &[
    0.0, 120.0, 10.0, 1.0, 190.0, 10.0, 1.0, 190.0, 80.0, 1.0, 120.0, 80.0, 5.0, 0.0, 140.0, 30.0,
    1.0, 170.0, 30.0, 1.0, 170.0, 60.0, 1.0, 140.0, 60.0, 5.0,
];
pub const PATH_CURVE: &str = "M110 90 C130 70 150 110 190 90";
pub const PATH_TRI: &str = "M20 10 L50 10 L35 40 Z";
/// The turned bar, and its turn: turns, then the pivot.
pub const PATH_BAR: &str = "M30 104 H50 V110 H30 Z";
pub const PATH_BAR_TURN: (f32, f32, f32) = (0.125, 40.0, 107.0);
pub const PATH_BAD: &str = "M10 10 L20";

fn build_path(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    use crate::path::{FillRule, Path};
    ui.with(
        NodeSpec::column()
            .size(200.0, 120.0)
            .bg(Color::hex(0x14161eff)),
        |ui| {
            ui.path_d(
                PATH_WEDGE,
                FillRule::NonZero,
                None,
                None,
                NodeSpec::column()
                    .bg(Color::hex(0x7f9cf5ff))
                    .on_click(Value::map([("kind", Value::str("wedge"))]))
                    .label("Wedge"),
            );
            ui.path_d_keyed(
                "wedge2",
                PATH_WEDGE2,
                FillRule::NonZero,
                None,
                None,
                NodeSpec::column().bg(Color::hex(0xd8863bff)),
            );
            // The flat form, as C always and the others may declare it.
            let ring = Path::from_floats(PATH_RING_OPS)
                .expect("the ring's ops")
                .fill_rule(FillRule::EvenOdd);
            ui.path(&ring, NodeSpec::column().bg(Color::hex(0xf5d67fff)));
            ui.path_d(
                PATH_CURVE,
                FillRule::NonZero,
                // Dashed: 8 px marks, 4 px gaps, 3 px in.
                Some(
                    Stroke::new(2.0, Color::hex(0x9ad9a0ff))
                        .dash(8.0, 4.0)
                        .dash_offset(3.0),
                ),
                None,
                NodeSpec::column(),
            );
            ui.path_d(
                PATH_TRI,
                FillRule::NonZero,
                Some(Stroke::new(1.5, Color::hex(0xffffffff))),
                None,
                NodeSpec::column().bg(Color::hex(0xe07a8aff)).opacity(0.5),
            );
            // Turned by its quad, about a pivot it names (ADR 0041).
            ui.path_d(
                PATH_BAR,
                FillRule::NonZero,
                None,
                Some(crate::path::Turn {
                    turns: PATH_BAR_TURN.0,
                    pivot: Some(Vec2::new(PATH_BAR_TURN.1, PATH_BAR_TURN.2)),
                }),
                NodeSpec::column().bg(Color::hex(0x7fd6f5ff)),
            );
            ui.path_d_keyed(
                "bad",
                PATH_BAD,
                FillRule::NonZero,
                None,
                None,
                NodeSpec::column().bg(Color::hex(0xffffffff)),
            );
        },
    );
}

fn polygon_points(pts: &[(f32, f32)]) -> Vec<Vec2> {
    pts.iter().map(|&(x, y)| Vec2::new(x, y)).collect()
}

fn build_polygon(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(
        NodeSpec::column()
            .size(200.0, 120.0)
            .bg(Color::hex(0x14161eff)),
        |ui| {
            ui.polygon(
                &polygon_points(POLYGON_TRIANGLE),
                NodeSpec::column()
                    .bg(Color::hex(0x7f9cf5ff))
                    .on_click(Value::map([("kind", Value::str("tri"))]))
                    .label("Triangle"),
            );
            ui.polygon(
                &polygon_points(POLYGON_ARROW),
                NodeSpec::column().bg(Color::hex(0xd8863bff)),
            );
            ui.polygon_keyed(
                "star",
                &polygon_points(POLYGON_STAR),
                NodeSpec::column().bg(Color::hex(0xf5d67fff)),
            );
            ui.polygon(
                &polygon_points(POLYGON_NINE),
                NodeSpec::column().bg(Color::hex(0x9ad9a0ff)),
            );
            ui.polygon(
                &polygon_points(POLYGON_QUAD),
                NodeSpec::column().bg(Color::hex(0xe07a8aff)).opacity(0.5),
            );
        },
    );
}

/// The `lines` scene: a 200×120 canvas holding three strokes and one box.
/// Points are in the canvas's box space, the way a floated card's offset
/// is. The curve's chords are 44.7, 50 and 82.5, which
/// `line::flatten_curve` cuts into 8, 9 and 14 pieces at `CURVE_STEP` 6.
fn build_drop(ui: &mut Ui<'_>, _f: &Fixtures, phase: u32) {
    ui.with(NodeSpec::row().fill(), |ui| {
        ui.with_keyed(
            "files",
            NodeSpec::column()
                .width(200.0)
                .grow_height()
                .pad(10.0)
                .bg(Color::hex(0x22242cff))
                .drop_bg(Color::hex(0x2b3350ff))
                .on_drop(Value::map([("kind", Value::str("files"))])),
            |ui| {
                // Inside the zone: files over it are the zone's.
                ui.leaf_keyed(
                    "pick",
                    NodeSpec::row()
                        .size(60.0, 40.0)
                        .bg(Color::hex(0x3b5bd4ff))
                        .on_click(Value::map([("kind", Value::str("pick"))]))
                        .label("Pick"),
                );
            },
        );
        ui.leaf_keyed(
            "other",
            NodeSpec::column()
                .fill()
                .bg(Color::hex(0x30344aff))
                .on_drop(Value::map([("kind", Value::str("other"))])),
        );
        let over = FloatConfig::viewport()
            .inside(Align::Start, Align::Start)
            .offset(20.0, 20.0);
        // What an app shows in answer to `enter`: a hoverable float
        // over the zone that takes no files, and is looked past.
        if phase == 1 {
            ui.leaf_keyed(
                "overlay",
                NodeSpec::column()
                    .float(over)
                    .size(160.0, 160.0)
                    .hoverable(),
            );
        }
        // A modal over the zone: the zone's region is not emitted, so
        // the files find nothing there.
        if phase == 2 {
            ui.leaf_keyed(
                "confirm",
                NodeSpec::column()
                    .float(over)
                    .size(160.0, 160.0)
                    .bg(Color::hex(0x101018ff))
                    .modal(Value::map([("kind", Value::str("dismiss"))])),
            );
        }
    });
}

fn build_underlines(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    use crate::cells::{Cell, CellGrid, flags};
    use crate::spec::UnderlineStyle;
    let mono = TextStyle::new(14.0).mono().line_height(20.0);
    ui.with(NodeSpec::column().pad(10.0).gap(4.0), |ui| {
        ui.rich_text(
            &[
                Span::new("let "),
                Span::new("value")
                    .underline_color(Color::hex(0xff0000ff))
                    .underline_style(UnderlineStyle::Wavy),
            ],
            mono,
        );
        ui.text("warn", mono.underline_color(Color::hex(0x00ff00ff)));
        ui.text(
            "dots",
            mono.underline_color(Color::hex(0x7f9cf5ff))
                .underline_style(UnderlineStyle::Dotted),
        );
        let cells: Vec<Cell> = "abc"
            .chars()
            .map(|ch| {
                Cell::new(ch, 0xd6d8e0ff, 0)
                    .with(flags::WAVY)
                    .underline_color(0xff0000ff)
            })
            .collect();
        ui.cells_keyed(
            "term",
            &CellGrid {
                rows: 1,
                cols: 3,
                cells: &cells,
                style: mono,
                cursor: None,
                origin_line: 0,
            },
            NodeSpec::default().label("term"),
        );
    });
}

fn build_joined_backgrounds(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let mono = TextStyle::new(14.0).mono().line_height(20.0);
    let sel = |s| Span::new(s).bg(Color::hex(0x3b5bd466)).bg_radius(4.0);
    ui.with(NodeSpec::column().pad(10.0), |ui| {
        ui.rich_text(&[Span::new("let "), sel("a = 1;")], mono);
        ui.rich_text(&[sel("let b = 22;")], mono);
        ui.rich_text(&[sel("c"), Span::new(" + d")], mono);
        ui.rich_text(
            &[Span::new("find").bg(Color::hex(0xd9738c66)).bg_radius(4.0)],
            mono,
        );
    });
}

fn build_relative_shrink(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let bar = |w: Sizing| {
        NodeSpec::column()
            .width(w)
            .height(10.0)
            .bg(Color::hex(0x3b5bd4ff))
    };
    ui.with(NodeSpec::column().gap(4.0), |ui| {
        ui.with(NodeSpec::row().width(200.0).gap(20.0), |ui| {
            ui.with(bar(Sizing::Percent(0.5)), |_| {});
            ui.with(bar(Sizing::Percent(0.5)), |_| {});
        });
        let clamp = crate::schema::sizing_str("clamp(100px, 60%, 400px)").unwrap();
        ui.with(NodeSpec::row().width(300.0).gap(20.0), |ui| {
            ui.with(bar(clamp), |_| {});
            ui.with(bar(clamp), |_| {});
        });
    });
}

fn build_column_squeeze(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let bar = |ui: &mut Ui<'_>| {
        ui.leaf(
            NodeSpec::column()
                .width(100.0)
                .height(20.0)
                .bg(Color::hex(0x3b5bd4ff)),
        );
    };
    ui.with(NodeSpec::row().gap(20.0), |ui| {
        ui.with(NodeSpec::column().width(100.0).height(50.0), |ui| {
            for _ in 0..3 {
                ui.with(NodeSpec::row(), bar);
            }
        });
        ui.with(NodeSpec::column().width(100.0).height(50.0), |ui| {
            ui.with(NodeSpec::row(), bar);
            ui.with(NodeSpec::column().clip(), |ui| {
                bar(ui);
                bar(ui);
            });
        });
        ui.with(NodeSpec::column().width(100.0).height(50.0), |ui| {
            let chips = NodeSpec::row().grow_width().wrap().gap(10.0).cross_gap(5.0);
            ui.with(chips, |ui| {
                for _ in 0..2 {
                    ui.leaf(
                        NodeSpec::column()
                            .width(60.0)
                            .height(20.0)
                            .bg(Color::hex(0x73d98cff)),
                    );
                }
            });
            ui.with(NodeSpec::row(), bar);
        });
    });
}

fn build_fit_across(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let card = |ui: &mut Ui<'_>, wrapper: NodeSpec| {
        ui.with(NodeSpec::column().max_width(100.0), |ui| {
            ui.with(wrapper.bg(Color::hex(0x30344aff)), |ui| {
                ui.with(NodeSpec::row().wrap().gap(10.0).cross_gap(5.0), |ui| {
                    for _ in 0..3 {
                        ui.leaf(
                            NodeSpec::column()
                                .width(40.0)
                                .height(20.0)
                                .bg(Color::hex(0x73d98cff)),
                        );
                    }
                });
            });
        });
    };
    ui.with(NodeSpec::row().gap(20.0), |ui| {
        card(ui, NodeSpec::column());
        card(ui, NodeSpec::column().min_width(Min::FIT));
    });
}

fn build_size_expressions(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    use crate::schema::{max_str, min_str, sizing_str};
    let bar = |spec: NodeSpec| spec.height(10.0).bg(Color::hex(0x3b5bd4ff));
    ui.with(NodeSpec::column().width(400.0).gap(4.0), |ui| {
        let clamp = sizing_str("clamp(100px, 50%, 150px)").unwrap();
        ui.with(bar(NodeSpec::column().width(clamp)), |_| {});
        let min = crate::calc::sizing_of(crate::calc::Expr::Min(vec![
            crate::calc::Expr::Pct(0.8),
            crate::calc::Expr::Px(300.0),
        ]))
        .unwrap();
        ui.with(bar(NodeSpec::column().width(min)), |_| {});
        let quarter = max_str("25%").unwrap();
        ui.with(
            bar(NodeSpec::column().width(900.0).max_width(quarter)),
            |_| {},
        );
        let floor = min_str("max(40%, 50px)").unwrap();
        ui.with(bar(NodeSpec::column().min_width(floor)), |_| {});
    });
}

fn build_break_spaces(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let mono = TextStyle::new(14.0)
        .mono()
        .line_height(20.0)
        .wrap(TextWrap::BreakSpaces);
    ui.with(NodeSpec::column().pad(10.0), |ui| {
        ui.with(NodeSpec::column().width(4.0), |ui| {
            ui.rich_text(
                &[
                    Span::new("ab"),
                    Span::new("  ").bg(Color::hex(0x3b5bd4ff)),
                    Span::new("c"),
                ],
                mono,
            )
        });
    });
}

fn build_lines(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(
        NodeSpec::column()
            .size(200.0, 120.0)
            .bg(Color::hex(0x14161eff)),
        |ui| {
            ui.line(
                Vec2::new(10.0, 10.0),
                Vec2::new(90.0, 70.0),
                Stroke::new(2.0, Color::hex(0x7f9cf5ff)),
                NodeSpec::column(),
            );
            // An elbow that declares a click, which a line ignores.
            ui.polyline(
                &[
                    Vec2::new(100.0, 20.0),
                    Vec2::new(140.0, 20.0),
                    Vec2::new(140.0, 60.0),
                ],
                Stroke::new(3.0, Color::hex(0xd8863bff)),
                NodeSpec::column()
                    .on_click(Value::map([("kind", Value::str("elbow"))]))
                    .label("Elbow"),
            );
            ui.polyline_keyed(
                "curve",
                &[
                    Vec2::new(20.0, 100.0),
                    Vec2::new(60.0, 80.0),
                    Vec2::new(100.0, 110.0),
                    Vec2::new(180.0, 90.0),
                ],
                Stroke::new(1.5, Color::hex(0x9ad9a0ff)).curve(),
                NodeSpec::column().opacity(0.5),
            );
            // A dash-dot round a corner, 3 px into its pattern.
            ui.polyline(
                &[
                    Vec2::new(150.0, 70.0),
                    Vec2::new(190.0, 70.0),
                    Vec2::new(190.0, 110.0),
                ],
                Stroke::new(2.0, Color::hex(0xe07a8aff))
                    .dashed(crate::Dash::of(&[10.0, 4.0, 2.0, 4.0]).unwrap())
                    .dash_offset(3.0),
                NodeSpec::column(),
            );
            ui.leaf(
                NodeSpec::column()
                    .size(40.0, 20.0)
                    .bg(Color::hex(0x202030ff)),
            );
        },
    );
}

/// The modal scene's dialog and the app under it. The dialog is a
/// viewport float pinned to the bottom right corner (x 200..320,
/// y 140..240), so the steps' coordinates land on known nodes: (50, 50) is
/// the `Open` button behind, (160, 16) the titlebar strip, and (250, 160)
/// the dialog's OK button. The two buttons inside it are the whole ring
/// while the modal is up. The titlebar is the one platform-dependent
/// height in the tree (34 logical px, 32 on Windows), so the two points
/// above it and below it are chosen to land the same way on either.
/// The `fragments` scene. Sizes are fixed and there is no text, so the
/// digest is geometry alone and holds on every machine.
fn build_fragments(ui: &mut Ui<'_>, f: &Fixtures, _phase: u32) {
    ui.with(
        NodeSpec::column()
            .size(200.0, 120.0)
            .gap(4.0)
            .bg(Color::hex(0x14161eff)),
        |ui| {
            ui.fragment(
                f.fragment,
                &FRAGMENT_PARAMS,
                NodeSpec::column().size(80.0, 40.0),
            );
            // Rounded, faded, and holding a child that paints over it —
            // the child's own solid is the one the fade multiplies too.
            ui.fragment_with_keyed(
                "card",
                f.fragment,
                &FRAGMENT_PARAMS,
                NodeSpec::column()
                    .size(80.0, 40.0)
                    .pad(6.0)
                    .radius(8.0)
                    .opacity(0.5),
                |ui| {
                    ui.leaf(
                        NodeSpec::column()
                            .size(20.0, 10.0)
                            .bg(Color::hex(0x202030ff)),
                    );
                },
            );
            // A handle that is live in no session: draws nothing.
            ui.fragment(
                crate::resources::FragmentId::from_ffi(0),
                &FRAGMENT_PARAMS,
                NodeSpec::column().size(20.0, 10.0),
            );
            // Eighteen params: the last two are dropped, with a warning.
            ui.fragment(
                f.fragment,
                &FRAGMENT_PARAMS_LONG,
                NodeSpec::column().size(30.0, 12.0),
            );
            // The image input: the atlas-backed icon, the texture-backed
            // stream, and an image live in no session (draws nothing).
            ui.with(NodeSpec::row().gap(4.0), |ui| {
                ui.fragment(
                    f.sampler.with_image(f.image),
                    &FRAGMENT_IMAGE_PARAMS,
                    NodeSpec::column().size(24.0, 24.0),
                );
                ui.fragment(
                    f.sampler.with_image(f.stream),
                    &FRAGMENT_IMAGE_PARAMS,
                    NodeSpec::column().size(32.0, 8.0),
                );
                // The removed fixture rather than a raw number: 0 is "no
                // image" at the C, Lua and Node doors, and every other
                // number is the first session's handle in some process
                // (see `Fixtures::dead`).
                ui.fragment(
                    f.sampler.with_image(f.dead),
                    &FRAGMENT_IMAGE_PARAMS,
                    NodeSpec::column().size(24.0, 24.0),
                );
            });
        },
    );
}

fn build_modal(ui: &mut Ui<'_>, _f: &Fixtures, phase: u32) {
    let button = |kind: &str, label: &str| {
        NodeSpec::row()
            .size(100.0, 24.0)
            .bg(Color::hex(0x3b5bd4ff))
            .on_click(Value::map([("kind", Value::str(kind))]))
            .label(label)
    };
    // Grow, not fit: the titlebar is a full-width drag strip, and a fit
    // column would shrink it to its content and leave (160, 16) on
    // nothing at all — which reads as inert chrome.
    ui.with(NodeSpec::column().gap(6.0).grow_width(), |ui| {
        widgets::titlebar_with(ui, |ui| {
            ui.text("app", TextStyle::new(12.0));
        });
        let open = ui.leaf_keyed(
            "open",
            NodeSpec::row()
                .size(100.0, 20.0)
                .bg(Color::hex(0x30344aff))
                .on_click(Value::map([("kind", Value::str("open"))]))
                .label("Open"),
        );
        // Phase 1 is the app with the dialog gone: the node it was opened
        // to rename, created with it and still here, declared focused so
        // that the restore has a declaration to yield to
        // (`docs/adr/0003-modal-surfaces.md`, decision 4).
        if phase != 0 {
            let note = ui.leaf_keyed(
                "note",
                NodeSpec::row()
                    .size(100.0, 20.0)
                    .bg(Color::hex(0x30344aff))
                    .on_click(Value::map([("kind", Value::str("note"))]))
                    .label("Note"),
            );
            ui.take_key_focus(note);
            return;
        }
        // The app owns its keyboard while the dialog is shut, and says so
        // the only way a data view can. Redeclared every frame, so it is
        // an edge once and clobbers nothing afterwards — which is what
        // makes the *change* of declaration above an edge at all.
        ui.take_key_focus(open);
        // The dialog: declared in phase 0 and dropped in phase 1, which is
        // the only way to ask for a modal to close.
        ui.with_keyed(
            "dialog",
            NodeSpec::column()
                .size(120.0, 100.0)
                .pad(8.0)
                .gap(6.0)
                .bg(Color::hex(0x202030ff))
                .float(FloatConfig::viewport().inside(Align::End, Align::End))
                .modal(Value::map([("kind", Value::str("dlg"))]))
                .label("Settings"),
            |ui| {
                ui.leaf_keyed("ok", button("ok", "OK"));
                ui.leaf_keyed("cancel", button("cancel", "Cancel"));
            },
        );
    });
}

/// A tab bar and a picker list, each a composite because its items are
/// focusable, with an ordinary button between them that keeps its own Tab
/// stop. The tabs are a row and the list a column, so the two derived
/// orientations differ.
fn build_composite(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(NodeSpec::column().gap(6.0).grow_width(), |ui| {
        ui.with_keyed("tabs", NodeSpec::row().role(Role::TabList).gap(4.0), |ui| {
            for (i, name) in ["One", "Two", "Three"].iter().enumerate() {
                ui.with_keyed(
                    name,
                    NodeSpec::row()
                        .role(Role::Tab)
                        // The view's own selection, and the entry the
                        // ring takes: the second tab, not the first.
                        .selected(i == 1)
                        .size(60.0, 20.0)
                        .bg(Color::hex(0x30344aff))
                        .on_click(Value::map([("kind", Value::str(name.to_lowercase()))])),
                    |ui| ui.text(name, TextStyle::new(12.0)),
                );
            }
        });
        ui.leaf_keyed(
            "add",
            NodeSpec::row()
                .size(40.0, 20.0)
                .bg(Color::hex(0x3b5bd4ff))
                .on_click(Value::map([("kind", Value::str("add"))]))
                .label("Add"),
        );
        ui.with_keyed("rows", NodeSpec::column().role(Role::List).gap(2.0), |ui| {
            for name in ["Alpha", "Bravo"] {
                ui.with_keyed(
                    name,
                    NodeSpec::row()
                        // A picker, not a navigation list: the row
                        // itself is focusable, which is the whole of
                        // what makes this list a composite.
                        .role(Role::ListItem)
                        .focusable()
                        .size(80.0, 18.0)
                        .bg(Color::hex(0x202030ff))
                        .on_click(Value::map([("kind", Value::str(name.to_lowercase()))])),
                    |ui| ui.text(name, TextStyle::new(12.0)),
                );
            }
        });
    });
}

/// How many children `bulk` carries. With its own root that is
/// `depart::MAX_NODES + 1` nodes — one past the budget, so the whole
/// subtree is refused rather than half-retained.
pub const EXIT_BULK_ROWS: usize = crate::depart::MAX_NODES;

/// How many one-node subtrees `slotRows` holds: more than the budget, each
/// declaring its own `exit`, dropped in one frame. Under per-subtree
/// admission the first `MAX_NODES` of them would become ghosts and the
/// rest blink away; under ADR 0012's decision 2 the frame's removal is
/// judged whole and none of them do. The difference is a solid count.
pub const EXIT_ROWS: usize = crate::depart::MAX_NODES + 104;

/// The `exit` scene. Four departing nodes and two that stay, laid out so
/// that dropping one moves nothing else: each departing node sits alone in
/// a fixed-size slot, and `bulk` is last, so the only geometry that changes
/// between phases is the ghosts'.
///
/// - `fade` (phase 1) is the one still in flight at the end: 400ms, and the
///   clock stops 80ms in. It is focusable, clickable and labelled *while
///   live*, so its inertness afterwards is three separate observations.
///   The text inside it is what makes the ghost need the previous frame's
///   text list as well as its tree.
/// - `blink` (phase 1) runs 50ms, so by 80ms it is over and gone.
/// - `flash` (phase 1) leaves and comes back in phase 2 while its exit is
///   still running: the live node wins and the ghost goes, so the frame
///   holds one picture of it and not two.
/// - `bulk` (phase 1) is one node past [`crate::depart::MAX_NODES`], so it
///   is refused whole, with an `exit-budget` warning and no ghost.
fn build_exit(ui: &mut Ui<'_>, _f: &Fixtures, phase: u32) {
    let slot = |h: f32| NodeSpec::column().size(140.0, h).bg(Color::hex(0x101018ff));
    let keep = |label: &'static str| {
        NodeSpec::row()
            .size(60.0, 16.0)
            .bg(Color::hex(0x22242cff))
            .focusable()
            .label(label)
    };
    ui.with(
        NodeSpec::column()
            .fill()
            .pad(8.0)
            .gap(6.0)
            .bg(Color::hex(0x14161eff)),
        |ui| {
            ui.leaf_keyed("a", keep("A"));
            ui.with_keyed("slotFade", slot(40.0), |ui| {
                if phase == 0 {
                    ui.text_in_keyed(
                        "fade",
                        NodeSpec::column()
                            .size(100.0, 24.0)
                            .bg(Color::hex(0x3b5bd4ff))
                            .transition(400.0)
                            .exit(Enter::from(40.0, 0.0).opacity(0.0))
                            .focusable()
                            .label("Fade")
                            .on_click(Value::map([("kind", Value::str("hit"))])),
                        "bye",
                        TextStyle::new(12.0),
                    );
                }
            });
            ui.with_keyed("slotBlink", slot(16.0), |ui| {
                if phase == 0 {
                    ui.leaf_keyed(
                        "blink",
                        NodeSpec::column()
                            .size(100.0, 12.0)
                            .bg(Color::hex(0x73d98cff))
                            .transition(50.0)
                            .exit(Enter::from(20.0, 0.0)),
                    );
                }
            });
            ui.with_keyed("slotFlash", slot(16.0), |ui| {
                if phase != 1 {
                    ui.leaf_keyed(
                        "flash",
                        NodeSpec::column()
                            .size(100.0, 12.0)
                            .bg(Color::hex(0xffcc00ff))
                            .transition(400.0)
                            .exit(Enter::from(-20.0, 0.0)),
                    );
                }
            });
            ui.leaf_keyed("b", keep("B"));
            // More one-node departures than the budget, each a solid quad
            // half a pixel wide, in a slot that keeps its size when they go.
            ui.with_keyed(
                "slotRows",
                NodeSpec::row().size(300.0, 4.0).bg(Color::hex(0x101018ff)),
                |ui| {
                    if phase < 4 {
                        for i in 0..EXIT_ROWS {
                            ui.leaf_indexed(
                                i as u64,
                                NodeSpec::column()
                                    .size(0.5, 4.0)
                                    .bg(Color::hex(0x8a8fa3ff))
                                    .transition(400.0)
                                    .exit(Enter::default().opacity(0.0)),
                            );
                        }
                    }
                },
            );
            // Last, and sized by its children, which have no size: dropping
            // it takes only the trailing gap with it.
            if phase < 3 {
                ui.with_keyed(
                    "bulk",
                    NodeSpec::column()
                        .transition(400.0)
                        .exit(Enter::default().opacity(0.0)),
                    |ui| {
                        for _ in 0..EXIT_BULK_ROWS {
                            ui.leaf(NodeSpec::column());
                        }
                    },
                );
            }
        },
    );
}

// ---------------------------------------------------------------------------
// Derived coverage

/// The `windows` scene: the same tree every phase (a box saying which
/// phase it is in), and a declaration that comes and goes. Phase 0 declares
/// `palette` twice, disagreeing about the size; phases 1 and 3 declare
/// nothing; phase 2 declares it once.
/// The anchor the popup scene is placed against: what an `onLayout` node
/// would have reported for the field the menu belongs to, spelled out so
/// every binding declares the same four numbers rather than reproducing a
/// layout to discover them.
pub const POPUP_ANCHOR: Rect = Rect {
    x: 12.0,
    y: 40.0,
    w: 160.0,
    h: 24.0,
};

fn build_popup(ui: &mut Ui<'_>, _f: &Fixtures, phase: u32) {
    // A list 320 tall under a 240-tall viewport: the case `FloatConfig::fit`
    // cannot place, which is the whole reason for the kind.
    if phase == 0 {
        ui.window("menu", WindowConfig::popup(POPUP_ANCHOR, 160.0, 320.0));
    }
    ui.text_in(
        NodeSpec::column().pad(8.0).bg(Color::hex(0x14161eff)),
        if phase == 0 { "menu" } else { "closed" },
        TextStyle::new(12.0),
    );
}

fn build_drag(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    // A splitter handle: 80x40 at the origin, so (40, 20) is its middle
    // and every step lands on it. Keyed, so the press has a stable node
    // to capture on across the frames the scene drives.
    ui.leaf_keyed(
        "handle",
        NodeSpec::column()
            .size(80.0, 40.0)
            .bg(Color::hex(0x30344aff))
            .on_drag(Value::map([("kind", Value::str("split"))])),
    );
}

fn build_live(ui: &mut Ui<'_>, _f: &Fixtures, phase: u32) {
    if phase == 1 {
        // The queue half: one thing to say, on the frame it happened.
        // Phase 1 is the last frame the scene builds, so this is called
        // once and `announcement-repeated` stays quiet.
        ui.announce("Saved", crate::access::Live::Assertive);
    }
    ui.with(
        NodeSpec::column()
            .pad(8.0)
            .gap(4.0)
            .bg(Color::hex(0x14161eff)),
        |ui| {
            // The row half. A plain box that would otherwise be elided:
            // `live` is what keeps it in the tree.
            ui.with_keyed(
                "status",
                NodeSpec::column().live(crate::access::Live::Polite),
                |ui| {
                    let n = if phase == 0 { "0" } else { "3" };
                    ui.text(&format!("{n} results"), TextStyle::new(12.0));
                },
            );
            // Live, and with nothing to be live about.
            ui.leaf_keyed(
                "empty",
                NodeSpec::column().live(crate::access::Live::Polite),
            );
        },
    );
}

fn build_windows(ui: &mut Ui<'_>, _f: &Fixtures, phase: u32) {
    if phase == 0 || phase == 2 {
        ui.window("palette", WindowConfig::sized(400.0, 300.0));
    }
    if phase == 0 {
        // The same name again with another config: the first declaration
        // wins, and the disagreement is reported on the frame it opens.
        ui.window("palette", WindowConfig::sized(500.0, 500.0));
    }
    ui.with(
        NodeSpec::column().pad(8.0).bg(Color::hex(0x14161eff)),
        |ui| {
            let label = if phase == 0 || phase == 2 {
                "open"
            } else {
                "closed"
            };
            ui.text(label, TextStyle::new(12.0));
        },
    );
}

/// What a built frame actually exercised, read back off the tree.
///
/// [`Scene::custom`] and [`Scene::elements`] are hand-written *claims*, and
/// a claim nothing checks is a claim that rots: a scene could name `float`,
/// stop floating anything, and the corpus would still report full coverage
/// with nothing for the quad digests to compare on that row. `observe`
/// derives the same two sets from what the builder produced, so the Rust
/// adapter can assert derived ⊇ declared and a stale claim fails the build.
///
/// Derived is a superset on purpose. Several `widgets` helpers key their
/// nodes, so scenes touch rows they never claim; only the claims have to
/// be true.
#[derive(Default)]
pub struct Coverage {
    pub custom: BTreeSet<&'static str>,
    pub elements: BTreeSet<&'static str>,
}

/// Rows `observe` cannot derive, with the reason. These are the ones
/// whose coverage the corpus only *declares*; it is the written record of
/// what is left unchecked rather than a silence.
///
/// **It is empty, and that is the interesting state.** Its one entry was
/// `windowButtons`, exempted because `widgets::window_buttons` draws
/// nothing unless `env.window.custom_chrome` is set and no adapter could
/// declare that to a headless core. Every adapter can now (backlog P9;
/// Node was the last, and needed an `Env` surface of its own first — B2),
/// the `chrome` scene drives under [`CUSTOM_CHROME`], and the three
/// buttons are compared byte-for-byte across all four bindings instead of
/// being claimed by a scene that built none. Keep the constant: the test
/// below is what stops the next exemption from being permanent.
pub const UNDERIVED: &[(&str, &str)] = &[];

/// Whether node `i`'s key is one the view spelled (`key`, `with_keyed`)
/// rather than one auto-keying derived from its position — the observable
/// half of the `key` row.
///
/// Auto-keys live in `Key`'s sibling-index namespace and label keys in its
/// string one, and a tag byte separates the two, so the test is whether
/// *any* index under this parent hashes to the node's key. The counter
/// auto-keying draws from advances once per auto-keyed child of that
/// parent — including an `audio` node, which consumes one without leaving
/// a tree node behind — so the search runs past the sibling count. Its
/// exact length does not matter: only a hash collision could put a label
/// key inside the range.
///
/// `auto` holds each parent's auto-range keys once per frame: asked node by
/// node, the sibling count and the range's hashes were a parent's size
/// squared, and the exit scene's 4200 rows under one parent
/// took the suite from 8 s to 46 s.
fn is_label_keyed(
    t: &Tree,
    i: usize,
    auto: &mut HashMap<u32, std::collections::HashSet<Key>>,
) -> bool {
    let parent = t.parent[i];
    let keys = auto.entry(parent).or_insert_with(|| {
        let parent_key = t.keys[parent as usize];
        let siblings = t.children(parent).count() as u64;
        (0..siblings + 16).map(|j| parent_key.index(j)).collect()
    });
    !keys.contains(&t.keys[i])
}

/// Whether node `i` was opened at a data index the *view* chose rather than
/// the one its position would have given it — the observable half of the
/// `index` row. Both live in `Key`'s sibling-index namespace, so the only
/// difference there is *which* index: auto-keying draws 0, 1, 2 … from a
/// counter that advances once per auto-keyed child, so an index past
/// anything that counter could have reached is one nothing but a
/// declaration produces. An `index` inside the auto range is invisible
/// here, and deliberately — it is exactly the key auto-keying would have
/// given, which is the row's whole point. So a scene that means to pin the
/// row numbers its rows past the sibling count, as a virtual list does.
///
/// Only asked of nodes [`is_label_keyed`] has already put outside the auto
/// range: an auto-keyed node cannot be far-indexed, and it is the common
/// case by a wide margin — asking every node cost the conformance suite
/// 8.5 s of the 12 it took.
fn is_far_indexed(t: &Tree, i: usize) -> bool {
    let parent = t.parent[i];
    let parent_key = t.keys[parent as usize];
    let siblings = t.children(parent).count() as u64;
    let auto_max = siblings + 16;
    // A window rather than an unbounded scan: the corpus is the only caller
    // and its indices are row numbers, so this is generous by three orders
    // of magnitude and still a few thousand hashes on a handful of nodes.
    (auto_max..auto_max + 4096).any(|j| parent_key.index(j) == t.keys[i])
}

/// Derives the `CUSTOM` / `ELEMENTS` rows the frame `core` just built
/// exercises, unioning into `cov` (a scene is driven over several frames,
/// and a hover-gated tooltip only exists on some of them).
fn observe(core: &Core, cov: &mut Coverage) {
    // Each parent's auto-range keys, built on first ask (`is_label_keyed`).
    let mut auto = HashMap::new();
    if core.window_title().is_some() {
        cov.custom.insert("title");
    }
    // The level is a declaration the same way: no node, the frame's flag.
    if core.always_on_top() {
        cov.custom.insert("alwaysOnTop");
    }
    if core.secure_input() {
        cov.custom.insert("secureInput");
    }
    if core.option_as_alt() != OptionAsAlt::None {
        cov.custom.insert("optionAsAlt");
    }
    // `keyFocus` leaves no mark on the tree: the focus it takes looks
    // exactly like the focus a click takes, so the frame's declaration
    // list is the only trace of one.
    if !core.declared_focus().is_empty() {
        cov.custom.insert("keyFocus");
    }
    // A window declaration is not a node either; the frame's list is the
    // only trace of one.
    if !core.declared_windows().is_empty() {
        cov.custom.insert("windows");
    }
    // The `menuBar` element declares a menu and may draw nothing at all, so
    // neither its rows nor its nodes are a reliable trace of it; the
    // declaration is. (The bar it draws is told from a hand-built row of
    // `menuItem`s by the core having recorded its nodes, which only the
    // widget does — but on a host with a platform bar there are none.)
    if core.menu_bar().is_some() {
        cov.elements.insert("menuBar");
    }
    // An `audio` element builds no node either — it declares a playback
    // the audio store reconciles — so it is read off the store.
    if core.audio.any_mounted(core.env.window.id) {
        cov.elements.insert("audio");
    }

    let t = &core.tree;
    for i in 0..t.len() {
        let spec = &t.specs[i];
        let l = &spec.layout;

        // Column is the default, so a row is the only observable `dir` —
        // and a table, which is `dir="table"` in JSX and the element too.
        if l.dir == Dir::Row {
            cov.custom.insert("dir");
        }
        if l.is_table() {
            cov.custom.insert("dir");
            cov.elements.insert("table");
        }
        if l.padding != Edges::default() {
            cov.custom.insert("pad");
        }
        if l.clips() {
            cov.custom.insert("overflow");
        }
        if let Some(f) = l.float {
            cov.custom.insert("float");
            // The `tooltip` *element* is by definition a float hanging
            // below its parent, centered (`FloatConfig::below`), which is
            // the chrome `widgets::tooltip_with` draws.
            if f.anchor == FloatAnchor::Parent
                && f.anchor_point == (Align::Center, Align::End)
                && f.self_point == (Align::Center, Align::Start)
            {
                cov.elements.insert("tooltip");
            }
        }
        if spec.style.border_w > 0.0 && spec.style.border_color.is_visible() {
            cov.custom.insert("border");
        }
        // What every binding's `tooltip` *prop* lowers to.
        if spec.hoverable && spec.access().description.is_some() {
            cov.custom.insert("tooltip");
        }
        // `widgets::button_spec` plus a payload: the stock button is the
        // node that declares a click and both interaction backgrounds.
        if spec.events().on_click.is_some()
            && spec.interact().hover_bg.is_some()
            && spec.interact().pressed_bg.is_some()
        {
            cov.elements.insert("button");
        }
        // The stock controls are the roles they declare; a
        // slider is the stock one when it asked the core for its changes,
        // since a hand-drawn slider (the `controls` scene's) nudges.
        match spec.access().role {
            Some(Role::Checkbox) => {
                cov.elements.insert("checkbox");
            }
            Some(Role::Radio) => {
                cov.elements.insert("radio");
            }
            Some(Role::RadioGroup) => {
                cov.elements.insert("radioGroup");
            }
            Some(Role::Switch) => {
                cov.elements.insert("switch");
            }
            Some(Role::Slider) if spec.events().on_change.is_some() => {
                cov.elements.insert("slider");
            }
            _ => {}
        }
        // The stock select is the node whose click carries the `select`
        // tag the core takes back (`widgets::select_tag`); nothing else
        // declares that payload.
        if spec
            .events()
            .on_click
            .as_ref()
            .is_some_and(|v| v.get_bool("select") == Some(true))
        {
            cov.elements.insert("select");
        }
        match spec.window {
            Some(WindowRole::Drag) => {
                cov.elements.insert("titlebar");
            }
            Some(WindowRole::Button(_)) => {
                cov.elements.insert("windowButtons");
            }
            None => {}
        }
        // `widgets::latency_graph` hides its subtree from assistive
        // technology, which is the one thing the graph declares about
        // itself. The `tooltip` prop's hint is the corpus's other
        // `Role::None`, and a float, which the graph is not.
        if spec.access().role == Some(Role::None) && l.float.is_none() {
            cov.elements.insert("latencyGraph");
        }

        match t.content[i] {
            // The root is the core's own, not a `box` the view declared.
            NodeContent::Container => {
                if i > 0 {
                    cov.elements.insert("box");
                }
            }
            NodeContent::Cells(_) => {
                cov.elements.insert("cells");
            }
            // `size` is as far as the tree goes: the frame's text list
            // keeps a cache key and a color, not the `TextStyle` it was
            // shaped from, so the font size cannot be read back — only
            // that the scene declared text at a style at all.
            NodeContent::Text(_) => {
                cov.elements.insert("text");
                cov.custom.insert("size");
            }
            NodeContent::Edit(_) => {
                cov.elements.insert("edit");
                cov.custom.insert("size");
            }
            NodeContent::Image(..) => {
                cov.elements.insert("image");
            }
            NodeContent::Line(_) => {
                cov.elements.insert("line");
            }
            NodeContent::Fragment(_) => {
                cov.elements.insert("fragment");
            }
            NodeContent::Polygon(_) => {
                cov.elements.insert("polygon");
            }
            NodeContent::Path(_) => {
                cov.elements.insert("path");
            }
        }

        // A key outside the auto range is one the view spelled, and it is
        // one of the two namespaces or the other — never both, so an
        // index-keyed row must not be credited to `key`. Nesting them also
        // keeps `is_far_indexed`'s scan off the auto-keyed nodes, which are
        // nearly all of them and cannot be far-indexed by construction.
        if i > 0 && is_label_keyed(t, i, &mut auto) {
            if is_far_indexed(t, i) {
                cov.custom.insert("index");
            } else {
                cov.custom.insert("key");
            }
        }
    }
    if !t.row_counts.is_empty() {
        cov.custom.insert("rowCount");
    }
}

// ---------------------------------------------------------------------------
// Running a scene

/// What one scene produced: quads as counts and a digest, the access tree,
/// the events the replay emitted, the diagnostics, the declared title.
pub struct Output {
    pub quad_count: usize,
    pub quad_digest: u64,
    /// Per [`QuadKind`], in its discriminant order.
    pub kinds: [usize; 9],
    /// Every `FragmentDraw` the frame emitted, in the order the quads
    /// index them. The parameters are not on the quad, so the digest
    /// cannot reach them; the report carries them instead, as bits, so no
    /// adapter has to agree on how a float prints.
    pub fragments: Vec<[f32; 16]>,
    /// Beside each entry of [`Self::fragments`], where its `image` is:
    /// `None` for a draw with no image (the line is omitted), an atlas
    /// texel rect, or the index of the `texture` entry it reads and the
    /// rect in that texture. What pins a fragment's binding to the texel
    /// (backlog V1).
    pub fragment_images: Vec<crate::display::FragmentImage>,
    /// Every `TextureDraw`'s texel rect, in the order the quads index
    /// them; the handle is minted, not declared, and is skipped as atlas
    /// `uv` is. What pins a `fit="cover"` crop to the texel.
    pub textures: Vec<[u32; 4]>,
    pub nodes: Vec<NodeRow>,
    pub events: Vec<(String, String)>,
    /// Everything `announce` queued over the whole scene, in order —
    /// drained once at the end, the way warnings are, because an
    /// announcement is something that *happened* during the scene and the
    /// report keeps only one frame.
    pub announcements: Vec<crate::access::Announcement>,
    pub warnings: Vec<&'static str>,
    /// Every window command the replay drained, in order — what a frame
    /// driver would have applied to real windows.
    pub commands: Vec<WindowCommand>,
    /// Every audio command the replay drained, in order — what a frame
    /// driver would have applied to a real device. In the report because
    /// an `audio` element builds no tree node: the commands are the only
    /// thing a playback leaves behind for a binding to be diffed on.
    pub audio: Vec<AudioCommand>,
    pub title: Option<String>,
    /// Whether the last frame asked for the window on top: like the
    /// title, a declaration with no node, so the report carries it.
    pub always_on_top: bool,
    /// Whether the last frame asked for secure keyboard entry, the same
    /// way.
    pub secure_input: bool,
    /// Which Option keys the last frame asked to act as Alt, the same way.
    pub option_as_alt: OptionAsAlt,
    /// The `CUSTOM` / `ELEMENTS` rows the frames actually exercised (see
    /// [`Coverage`]). Not part of the [`report`]: it is derived from the
    /// tree a builder produced, which is a question about the builder, not
    /// about the protocol the other bindings reproduce.
    pub coverage: Coverage,
}

/// One access-tree node, flattened to what four languages can all report.
pub struct NodeRow {
    pub depth: usize,
    pub key: Key,
    pub role: &'static str,
    pub focused: bool,
    pub disabled: bool,
    /// `-` / `0` / `1`, or `m` for a mixed checkbox.
    pub checked: Option<bool>,
    pub mixed: bool,
    /// `-` / `0` / `1`. In the report because the core moving focus inside
    /// a composite must be visible *not* to have moved this
    /// (`docs/adr/0007-composite-keyboard-patterns.md`, decision 10).
    pub selected: Option<bool>,
    /// `-` / `h` / `v`: how a composite container arranges its items.
    pub orientation: &'static str,
    /// `-` / `p` / `a`: the liveness the node declared.
    pub live: &'static str,
    pub scrollable: bool,
    /// The access rect as f32 bits, `[x, y, w, h]`: font-dependent where
    /// text sizes the node, so in the report and never in [`Expect`].
    /// What a reader's hover and highlight go by, cut to the node's clip.
    pub rect: [u32; 4],
    /// Action names in `AccessAction::ALL` (bit) order, comma-joined.
    pub actions: String,
    pub name: String,
    pub description: String,
    pub value: String,
}

/// FNV-1a over the little-endian bytes of a quad's fields, `uv` excluded:
/// atlas coordinates depend on glyph insertion order, which a binding is
/// free to reach by a different route. Field order is `KuiQuad`'s: x, y, w,
/// h, `color[4]`, `border_color[4]`, `radius[4]`, border_w, blur, kind, `clip[4]`,
/// `clip_radius[4]` — words 0..=18 and 23..=30 of the 31-word struct. Every
/// adapter hashes the same words, so a geometry difference is one
/// mismatched hex string, and a mirror of `KuiQuad` that missed a field
/// mismatches on every scene rather than on none.
pub const FNV_OFFSET: u64 = crate::key::FNV_OFFSET;
pub const FNV_PRIME: u64 = crate::key::FNV_PRIME;

fn mix(h: &mut u64, word: u32) {
    *h = crate::key::fnv(*h, &word.to_le_bytes());
}

/// The clip is digested resolved rather than as the index it now rides as
/// (`display::ClipId`), so the number a report carries is the geometry a
/// backend clips by and not how this frame happened to intern it. That is
/// what keeps a report comparable across a change to the table — and what
/// the C and Node mirrors have to do too, walking `clips[q.clip]`.
pub fn quad_digest(quads: &[Quad], clips: &[Clip]) -> u64 {
    let mut h = FNV_OFFSET;
    for q in quads {
        for v in [
            q.rect.x,
            q.rect.y,
            q.rect.w,
            q.rect.h,
            q.color.r,
            q.color.g,
            q.color.b,
            q.color.a,
            q.border_color.r,
            q.border_color.g,
            q.border_color.b,
            q.border_color.a,
            q.radius[0],
            q.radius[1],
            q.radius[2],
            q.radius[3],
            q.border_w,
            q.blur,
        ] {
            mix(&mut h, v.to_bits());
        }
        mix(&mut h, q.kind as u32);
        // `uv` is skipped because atlas coordinates follow glyph insertion
        // order; on a segment it is the endpoints, which are geometry —
        // the two diagonals of one box would otherwise digest the same.
        // Mixed here, in the struct's word order, so the C and Node
        // mirrors can walk the words in sequence.
        // A texture quad's `uv[0]` is an index into a side list whose
        // entry is declared geometry too (the texel rect a `fit="cover"`
        // cropped); the index is mixed here and the entry is the report's
        // `texture` line, so a crop that moved is a report that moved.
        if q.kind == QuadKind::Segment || q.kind == QuadKind::Texture {
            for v in q.uv {
                mix(&mut h, v);
            }
        }
        let clip = clips.get(q.clip as usize).copied().unwrap_or(Clip::NONE);
        for v in [
            clip.rect.x,
            clip.rect.y,
            clip.rect.w,
            clip.rect.h,
            clip.radius[0],
            clip.radius[1],
            clip.radius[2],
            clip.radius[3],
        ] {
            mix(&mut h, v.to_bits());
        }
    }
    h
}

fn rows(tree: &AccessTree) -> Vec<NodeRow> {
    let mut depth_of: HashMap<Key, usize> = HashMap::new();
    tree.nodes
        .iter()
        .map(|n| {
            let depth = n
                .parent
                .and_then(|p| depth_of.get(&p).copied())
                .map_or(0, |d| d + 1);
            depth_of.insert(n.key, depth);
            NodeRow {
                depth,
                key: n.key,
                role: n.role.name(),
                focused: n.focused,
                disabled: n.disabled,
                checked: n.checked,
                mixed: n.mixed,
                selected: n.selected,
                orientation: match n.orientation {
                    Some(crate::access::Orientation::Horizontal) => "h",
                    Some(crate::access::Orientation::Vertical) => "v",
                    None => "-",
                },
                live: match n.live {
                    crate::access::Live::Off => "-",
                    crate::access::Live::Polite => "p",
                    crate::access::Live::Assertive => "a",
                },
                scrollable: n.scroll.is_some(),
                rect: [n.rect.x, n.rect.y, n.rect.w, n.rect.h].map(f32::to_bits),
                actions: n
                    .action_list()
                    .into_iter()
                    .map(|a| a.name())
                    .collect::<Vec<_>>()
                    .join(","),
                name: n.name.clone().unwrap_or_default(),
                description: n.description.clone().unwrap_or_default(),
                value: n.value.clone().unwrap_or_default(),
            }
        })
        .collect()
}

/// The one event shape the corpus compares: a payload's `kind`, and the
/// `kind` inside its `tag` when the core merged one in. Anything richer
/// would need a map walker in the C API, which there is none of.
fn event_row(payload: &Value) -> (String, String) {
    let kind = payload.get_str("kind").unwrap_or("-").to_string();
    // The second column is the event's tag. The two window-level events
    // carry none — they are on the window and not on a node — and without
    // a fallback two `window` lines or two `dismiss` lines are the same
    // line, which is exactly the difference those scenes are about. So an
    // untagged payload shows the field that tells it from its siblings:
    // `phase` for a `window`, `reason` for a `dismiss`.
    let mut tag = payload
        .get("tag")
        .and_then(|t| t.get("kind"))
        .or_else(|| payload.get("phase"))
        .or_else(|| payload.get("reason"))
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
    // A drag's phase and deltas ride in the tag column (`split move 8 0`),
    // because the deltas are the contract: `dx`/`dy` are measured from the
    // press point in every phase, and a binding that summed
    // steps instead would agree on the kind and disagree here. Printed as
    // integers — the steps are integers, so the deltas are exact.
    // A slider's change carries its phase and the value the core worked
    // out, the arithmetic being what is pinned. The corpus
    // steps land on whole values, so they print as integers.
    if kind == "change" {
        let phase = payload.get_str("phase").unwrap_or("-");
        let v = payload.get_float("value").unwrap_or(0.0) as i64;
        let _ = write!(tag, " {phase} {v}");
    }
    if kind == "drag" {
        let num = |k: &str| payload.get(k).and_then(Value::as_float).unwrap_or(0.0) as i64;
        let phase = payload.get_str("phase").unwrap_or("-");
        let _ = write!(tag, " {phase} {} {}", num("dx"), num("dy"));
    }
    // A scroll's lines ride the same way (`scroll term 2`): the whole
    // lines are the contract for a grid, and a binding that lost the
    // carried fraction between two notches would agree on the kind and
    // disagree here. `-` for a node that is not a grid.
    // A held button's phase and which button ride the same way
    // (`button panel press middle`): the capture is the
    // contract — a move and a release on the owner wherever the pointer
    // went — and a binding that lost the mask would claim the secondary
    // presses and disagree here.
    if kind == "button" {
        let phase = payload.get_str("phase").unwrap_or("-");
        match payload.get("button") {
            Some(Value::Str(b)) => {
                let _ = write!(tag, " {phase} {b}");
            }
            Some(Value::Int(n)) => {
                let _ = write!(tag, " {phase} {n}");
            }
            _ => {
                let _ = write!(tag, " {phase} -");
            }
        }
    }
    // A key from one of a key's twins says which, with its phase and
    // code (`key mods down shift left`): the place is the
    // contract, and a binding that dropped it would agree on the kind and
    // disagree here. A key from the standard place prints as it did.
    if kind == "key"
        && let Some(loc) = payload.get_str("location")
        && loc != "standard"
    {
        let phase = payload.get_str("phase").unwrap_or("-");
        let code = payload.get_str("code").unwrap_or("-");
        let _ = write!(tag, " {phase} {code} {loc}");
    }
    if kind == "scroll" {
        match payload.get_int("lines") {
            Some(n) => {
                let _ = write!(tag, " {n}");
            }
            None => tag.push_str(" -"),
        }
    }
    // A drop's phase and how many paths reached it ride the same way
    // (`drop files enter 2`): the phase is the contract (ADR 0031,
    // decision 1), and a binding that dropped the list on the wire would
    // agree on the kind and disagree here.
    if kind == "drop" {
        let phase = payload.get_str("phase").unwrap_or("-");
        let n = payload
            .get("paths")
            .and_then(Value::as_list)
            .map_or(0, <[Value]>::len);
        let _ = write!(tag, " {phase} {n}");
    }
    // A paste's markers ride the same way (`text ed concealed transient`),
    // each only when it is set, as the payload carries them (backlog
    // F84): a binding that dropped them on the wire would agree on the
    // kind and disagree here, and an unmarked commit's line is the one it
    // always was.
    if kind == "text" {
        for marker in ["concealed", "transient"] {
            if payload.get(marker).and_then(Value::as_bool) == Some(true) {
                let _ = write!(tag, " {marker}");
            }
        }
    }
    (kind, tag)
}

/// Drives one scene's protocol against `core`, which must be fresh and
/// carry [`fixtures`]. Every binding reimplements exactly this:
///
/// 1. turn diagnostics on;
/// 2. declare the window facts (step 0, and the reason `env` is a
///    parameter: `widgets::window_buttons` and the titlebar's inset read
///    them while the *first* frame builds, so a driver that pushed them
///    afterwards would compare a different tree);
/// 3. build a frame, collect the events it left pending and the window
///    commands it queued;
/// 4. for each step, apply the input (or the clock, the phase, or the OS
///    close), collect its events and commands, build again;
/// 5. read the quads, access tree, warnings and title of the last frame.
///
/// The [`Coverage`] it also returns is Rust-only bookkeeping over the tree
/// each frame built, not part of the protocol: the other bindings have
/// nothing to reproduce there.
pub fn drive(
    core: &mut Core,
    env: WindowEnv,
    steps: &[Step],
    mut build: impl FnMut(&mut Ui<'_>, u32),
) -> Output {
    core.set_diagnostics(true);
    core.env.window = env;
    let mut events = Vec::new();
    let mut commands = Vec::new();
    let mut audio = Vec::new();
    let mut coverage = Coverage::default();
    let mut phase = 0u32;
    let mut frame = |core: &mut Core,
                     phase: u32,
                     events: &mut Vec<(String, String)>,
                     commands: &mut Vec<WindowCommand>,
                     audio: &mut Vec<AudioCommand>,
                     coverage: &mut Coverage| {
        let mut ui = core.frame(VIEWPORT, SCALE);
        build(&mut ui, phase);
        ui.finish();
        observe(core, coverage);
        events.extend(
            core.take_pending_events()
                .iter()
                .map(|e| event_row(&e.payload)),
        );
        commands.extend(core.take_window_commands());
        audio.extend(core.take_audio_commands());
    };
    frame(
        core,
        phase,
        &mut events,
        &mut commands,
        &mut audio,
        &mut coverage,
    );
    for step in steps {
        match *step {
            Step::Phase(n) => phase = n,
            Step::Time(ms) => core.set_time(ms as f64 / 1000.0),
            Step::WindowClosed(id) => {
                core.window_closed(WindowId(id));
                events.extend(
                    core.take_pending_events()
                        .iter()
                        .map(|e| event_row(&e.payload)),
                );
                commands.extend(core.take_window_commands());
                audio.extend(core.take_audio_commands());
            }
            Step::WindowDismissed(id, reason) => {
                core.dismiss_window(WindowId(id), DISMISS_REASONS[reason as usize]);
                events.extend(
                    core.take_pending_events()
                        .iter()
                        .map(|e| event_row(&e.payload)),
                );
            }
            Step::Appearance(n) => {
                let appearance = crate::env::Appearance::ALL[n as usize];
                core.set_system(crate::env::SystemEnv {
                    appearance,
                    ..core.env.system
                });
            }
            _ => {
                let evs = core.handle_input(step.event().expect("an input step"));
                events.extend(evs.iter().map(|e| event_row(&e.payload)));
                commands.extend(core.take_window_commands());
                audio.extend(core.take_audio_commands());
            }
        }
        frame(
            core,
            phase,
            &mut events,
            &mut commands,
            &mut audio,
            &mut coverage,
        );
    }

    let title = core.window_title().map(str::to_string);
    let always_on_top = core.always_on_top();
    let secure_input = core.secure_input();
    let option_as_alt = core.option_as_alt();
    let announcements = core.take_announcements();
    let warnings = core.take_warnings().into_iter().map(|w| w.code).collect();
    let nodes = rows(core.access_tree());
    let dl = core.output().0;
    let fragment_params: Vec<[f32; 16]> = dl.fragments.iter().map(|f| f.params).collect();
    let fragment_images = dl.fragments.iter().map(|f| f.image).collect();
    let quads = &dl.quads;
    let clips = &dl.clips;
    let mut kinds = [0usize; 9];
    for q in quads.iter() {
        kinds[match q.kind {
            QuadKind::Solid => 0,
            QuadKind::GlyphMask => 1,
            QuadKind::GlyphColor => 2,
            QuadKind::Image => 3,
            QuadKind::GlyphSubpixel => 4,
            QuadKind::Shadow => 5,
            QuadKind::Segment => 6,
            QuadKind::Fragment => 7,
            QuadKind::Texture => 8,
        }] += 1;
    }
    Output {
        quad_count: quads.len(),
        quad_digest: quad_digest(quads, clips),
        textures: dl.textures.iter().map(|t| t.uv).collect(),
        kinds,
        fragments: fragment_params,
        fragment_images,
        nodes,
        events,
        announcements,
        warnings,
        commands,
        audio,
        title,
        always_on_top,
        secure_input,
        option_as_alt,
        coverage,
    }
}

/// Runs a corpus scene through its own reference builder.
pub fn run(scene: &Scene) -> Output {
    let mut core = Core::new();
    let f = fixtures(&mut core);
    drive(&mut core, scene.env, scene.steps, |ui, phase| {
        (scene.build)(ui, &f, phase)
    })
}

// ---------------------------------------------------------------------------
// The report

/// The `env` line: the five numbers `kui_env_set_window` takes, in its
/// order. Written only when a scene departs from [`NATIVE_CHROME`], so the
/// scenes that are not about chrome carry no line at all and an adapter
/// that sees none drives under the defaults it already had.
pub fn write_env(env: WindowEnv, out: &mut String) {
    if env == NATIVE_CHROME {
        return;
    }
    let r = env.native_controls.unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
    let _ = writeln!(
        out,
        "env {} {} {} {} {}",
        env.custom_chrome as u8, env.maximized as u8, env.fullscreen as u8, r.w as i32, r.h as i32,
    );
}

/// The `audio` line for one audio command: the verb, and the playback it
/// names — `audio play 1 0`, `audio stop 1`. What a driver would send a
/// device, minus everything that cannot travel: volumes and fades are
/// floats, and a `sound` is a process-unique handle a binding mints for
/// itself, so neither would compare across four runs. A play carries its
/// `looped` bit because that is the one thing about a playback the report
/// otherwise could not see, and it is what decides whether a departure
/// stops the playback or releases it (`AudioSpec::finish`).
/// `master` and `unload` name no playback at all.
pub fn write_audio_command(cmd: &AudioCommand, out: &mut String) {
    let _ = match *cmd {
        AudioCommand::Play {
            playback, looped, ..
        } => writeln!(out, "audio play {} {}", playback.0, looped as u8),
        AudioCommand::Stop { playback, .. } => writeln!(out, "audio stop {}", playback.0),
        AudioCommand::SetVolume { playback, .. } => writeln!(out, "audio volume {}", playback.0),
        AudioCommand::Pause { playback, .. } => writeln!(out, "audio pause {}", playback.0),
        AudioCommand::Resume { playback, .. } => writeln!(out, "audio resume {}", playback.0),
        AudioCommand::MasterVolume { .. } => writeln!(out, "audio master"),
        AudioCommand::Unload { .. } => writeln!(out, "audio unload"),
    };
}

/// The `cmd` line for one window command: the verb, the window, and what
/// else that verb carries — for an `Open` the rest of what the driver reads
/// (the owner window, the origin, the kind as its index with 0 for normal
/// and 1 for popup, the initial width and height, whether it activates, and
/// the anchor rect a popup is placed against), for a `SetSize` the size
/// asked for. All integers, like every other line.
pub fn write_command(cmd: &WindowCommand, out: &mut String) {
    let _ = match *cmd {
        WindowCommand::StartDrag(w) => writeln!(out, "cmd drag {}", w.0),
        WindowCommand::Close(w) => writeln!(out, "cmd close {}", w.0),
        WindowCommand::Minimize(w) => writeln!(out, "cmd minimize {}", w.0),
        WindowCommand::ToggleMaximize(w) => writeln!(out, "cmd maximize {}", w.0),
        WindowCommand::Open {
            id,
            owner,
            origin,
            config,
        } => writeln!(
            out,
            "cmd open {} {} {} {} {} {} {} {} {} {} {}",
            id.0,
            owner.0,
            origin.0,
            config.kind as u32,
            config.size.w as i32,
            config.size.h as i32,
            config.activates as u8,
            config.anchor.x as i32,
            config.anchor.y as i32,
            config.anchor.w as i32,
            config.anchor.h as i32,
        ),
        WindowCommand::SetSize { window, size } => writeln!(
            out,
            "cmd setsize {} {} {}",
            window.0, size.w as i32, size.h as i32
        ),
        WindowCommand::Focus(w) => writeln!(out, "cmd focus {}", w.0),
        WindowCommand::Redraw(w) => writeln!(out, "cmd redraw {}", w.0),
    };
}

/// Renders a scene's block of the conformance report. The format is
/// line-oriented and carries no formatted floats — only integers, hex and
/// strings — so Rust, C and JavaScript produce the same bytes:
///
/// ```text
/// scene <name>
/// env <customChrome> <maximized> <fullscreen> <controlsW> <controlsH>
///                            the window facts to drive under, omitted at their defaults
/// step <...>                 the replayed input, so an adapter need not restate it
/// title <text|->
/// always-on-top <0|1>      whether the frame asked for the window above every other app's
/// secure-input <0|1>       whether the frame asked for secure keyboard entry
/// option-as-alt <none|left|right|both>
///                            which Option keys the frame asked to act as Alt
/// quads <count> <digest:016x>
/// kinds <solid> <glyphMask> <glyphColor> <image> <glyphSubpixel> <shadow> <segment> <fragment> <texture>
/// fragment <i> <16 × params as f32 bits>
/// fragment-image <i> <atlas|texture> <texture index|-> <x> <y> <w> <h>
///                            where a fragment's `image` is; omitted with none
/// texture <i> <x> <y> <w> <h>   the texel rect a texture quad shows
/// node <depth> <key:016x> <role> <focused> <disabled> <checked|m> <selected> <orientation> <live> <scroll>
///      <x> <y> <w> <h> <actions> <name> | <description> | <value>
///                            one line; the rect as f32 bits
/// event <kind> <tag>
/// cmd <verb> <window> [...]  a window command the driver would have applied
/// warn <code>
/// end
/// ```
pub fn report(name: &str, env: WindowEnv, steps: &[Step], out: &Output) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "scene {name}");
    write_env(env, &mut s);
    for step in steps {
        step.write(&mut s);
    }
    let _ = writeln!(s, "title {}", out.title.as_deref().unwrap_or("-"));
    let _ = writeln!(s, "always-on-top {}", out.always_on_top as u8);
    let _ = writeln!(s, "secure-input {}", out.secure_input as u8);
    let _ = writeln!(s, "option-as-alt {}", out.option_as_alt.name());
    let _ = writeln!(s, "quads {} {:016x}", out.quad_count, out.quad_digest);
    let _ = writeln!(
        s,
        "kinds {} {} {} {} {} {} {} {} {}",
        out.kinds[0],
        out.kinds[1],
        out.kinds[2],
        out.kinds[3],
        out.kinds[4],
        out.kinds[5],
        out.kinds[6],
        out.kinds[7],
        out.kinds[8]
    );
    for (i, params) in out.fragments.iter().enumerate() {
        let _ = write!(s, "fragment {i}");
        for v in params {
            let _ = write!(s, " {:08x}", v.to_bits());
        }
        let _ = writeln!(s);
    }
    for (i, image) in out.fragment_images.iter().enumerate() {
        use crate::display::FragmentImage;
        let (from, index, uv) = match *image {
            FragmentImage::None => continue,
            FragmentImage::Atlas(uv) => ("atlas", "-".to_string(), uv),
            FragmentImage::Texture { index, uv } => ("texture", index.to_string(), uv),
        };
        let _ = writeln!(
            s,
            "fragment-image {i} {from} {index} {} {} {} {}",
            uv[0], uv[1], uv[2], uv[3]
        );
    }
    for (i, uv) in out.textures.iter().enumerate() {
        let _ = writeln!(s, "texture {i} {} {} {} {}", uv[0], uv[1], uv[2], uv[3]);
    }
    for n in &out.nodes {
        let _ = writeln!(
            s,
            "node {} {:016x} {} {} {} {} {} {} {} {} {:08x} {:08x} {:08x} {:08x} {} {} | {} | {}",
            n.depth,
            n.key.0,
            n.role,
            n.focused as u8,
            n.disabled as u8,
            if n.mixed {
                "m".to_string()
            } else {
                n.checked.map_or("-".to_string(), |c| (c as u8).to_string())
            },
            n.selected
                .map_or("-".to_string(), |c| (c as u8).to_string()),
            n.orientation,
            n.live,
            n.scrollable as u8,
            n.rect[0],
            n.rect[1],
            n.rect[2],
            n.rect[3],
            if n.actions.is_empty() {
                "-"
            } else {
                &n.actions
            },
            n.name,
            n.description,
            n.value,
        );
    }
    for (kind, tag) in &out.events {
        let _ = writeln!(s, "event {kind} {tag}");
    }
    for a in &out.announcements {
        let _ = writeln!(s, "announce {} {}", a.live.name(), a.text);
    }
    for c in &out.commands {
        write_command(c, &mut s);
    }
    for c in &out.audio {
        write_audio_command(c, &mut s);
    }
    for w in &out.warnings {
        let _ = writeln!(s, "warn {w}");
    }
    s.push_str("end\n");
    s
}

/// The whole corpus as one report — the reference the other three bindings
/// are diffed against.
pub fn reference_report() -> String {
    SCENES
        .iter()
        .map(|s| report(s.name, s.env, s.steps, &run(s)))
        .collect()
}

/// Splits a report into `(scene name, block)` pairs. Adapters use it to
/// find their scene's expected block, and the steps to replay.
pub fn blocks(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut name = String::new();
    let mut block = String::new();
    for line in text.lines() {
        if let Some(n) = line.strip_prefix("scene ") {
            name = n.to_string();
            block.clear();
        }
        block.push_str(line);
        block.push('\n');
        if line == "end" {
            out.push((std::mem::take(&mut name), std::mem::take(&mut block)));
        }
    }
    out
}

/// The rows the `layers` scene's scroller holds: enough to overflow the
/// viewport, so it has a bar for the popover to cover.
pub const LAYERS_ROWS: usize = 16;

fn build_sampler(ui: &mut Ui<'_>, f: &Fixtures, _phase: u32) {
    use crate::anim::{Easing, Repeat};
    use crate::keyframes::Keyframe;
    let tag = |k: &str| Value::map([("kind", Value::str(k))]);
    ui.with(NodeSpec::column().pad(8.0).gap(6.0), |ui| {
        let card = NodeSpec::row()
            .size(120.0, 40.0)
            .max_width(100.0)
            .max_height(30.0)
            .center()
            .bg(Color::hex(0x1b1d27ff))
            .radius_tl(8.0)
            .radius_tr(2.0)
            .radius_br(8.0)
            .radius_bl(2.0)
            .shadow_color(Color::hex(0x00000080))
            .shadow_x(3.0)
            .shadow_y(2.0)
            .shadow_blur(2.0)
            .hoverable()
            .hover_bg(Color::hex(0x262a3aff))
            .pressed_bg(Color::hex(0x30364aff))
            .hover_group("cards")
            .focusable()
            .focus_bg(Color::hex(0x2b3350ff))
            .initial_focus()
            .accent()
            .cursor(crate::cursor::CursorShape::Pointer)
            .selected(true)
            .expanded(true)
            .on_click(tag("card"))
            .on_hover(tag("hov"))
            .on_layout(tag("lay"))
            .on_force_click(tag("force"))
            .click_sound(f.sound)
            .hover_sound(f.sound)
            .animate()
            .transition(100.0)
            .easing(Easing::EaseInOut)
            // A bounce on a timed easing: a spring (`Transition::curve`).
            .bounce(0.3)
            .slide()
            .delay(20.0)
            .repeat(Repeat::Alternate)
            .keyframes(vec![
                Keyframe::default().bg(Color::hex(0x1b1d27ff)),
                Keyframe::default()
                    .at(1.0)
                    .bg(Color::hex(0x3b5bd4ff))
                    .radius(12.0),
            ])
            .enter(Enter::from(-12.0, 0.0).opacity(0.0))
            // A tab reports `selected` either way, where a button keeps
            // the state to itself — so the row shows the flag landed.
            .role(Role::Tab)
            .label("Card");
        ui.text_in_keyed("card", card, "ab", TextStyle::new(12.0));
        ui.leaf_keyed(
            "strip",
            NodeSpec::row()
                .size(60.0, 10.0)
                .bg(Color::hex(0x3a3f52ff))
                .window_drag(),
        );
        ui.with_keyed(
            "dock",
            NodeSpec::row()
                .focus_region()
                .gap(4.0)
                .height(30.0)
                .main_align(Align::Center)
                .cross_align(Align::End),
            |ui| {
                ui.leaf_keyed(
                    "stop",
                    NodeSpec::row()
                        .size(20.0, 20.0)
                        .bg(Color::hex(0x2a2d3aff))
                        .focusable()
                        .role(Role::Button)
                        .label("Stop"),
                );
            },
        );
        ui.text_in(
            NodeSpec::column().width(60.0),
            "a long line that is cut short",
            TextStyle::new(12.0)
                .max_lines(1)
                .ellipsis()
                .underline()
                .strikethrough()
                .features(crate::spec::FontFeatures::parse("liga=0")),
        );
        ui.text_in_keyed(
            "line",
            NodeSpec::row()
                .height(16.0)
                .role(Role::Line)
                .caret(2)
                .selection_anchor(0)
                .caret_solid(),
            "sel",
            TextStyle::new(12.0),
        );
    });
}

fn build_clip_access(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let button = |w: f32, h: f32, kind: &str, label: &str, bg: u32| {
        NodeSpec::column()
            .size(w, h)
            .bg(Color::hex(bg))
            .on_click(Value::map([("kind", Value::str(kind))]))
            .label(label)
    };
    ui.with(NodeSpec::column().fill(), |ui| {
        ui.leaf_keyed(
            "toolbar",
            button(0.0, 40.0, "toolbar", "Toolbar", 0x3a3f52ff).grow_width(),
        );
        ui.with(NodeSpec::row().fill(), |ui| {
            ui.with_keyed(
                "canvas",
                NodeSpec::column().fill().clip().bg(Color::hex(0x101018ff)),
                |ui| {
                    // 20 px past the canvas's top, 60 px past it,
                    // and 60 px past it escaping.
                    for (key, label, dx, dy, clip) in [
                        ("cut", "Cut", 20.0, -20.0, true),
                        ("past", "Past", 100.0, -60.0, true),
                        ("free", "Free", 140.0, -60.0, false),
                    ] {
                        let float = FloatConfig::parent().offset(dx, dy);
                        let float = if clip { float.clipped() } else { float };
                        ui.leaf_keyed(key, button(60.0, 40.0, key, label, 0x3b5bd4ff).float(float));
                    }
                },
            );
            ui.with_keyed(
                "list",
                NodeSpec::column()
                    .size(100.0, 50.0)
                    .scroll_y()
                    .bg(Color::hex(0x202030ff)),
                |ui| {
                    for (key, label) in [("row0", "Row 0"), ("row1", "Row 1"), ("row2", "Row 2")] {
                        ui.leaf_keyed(key, button(100.0, 30.0, key, label, 0x73d98cff));
                    }
                },
            );
        });
    });
}

fn build_pixel_snap(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let cell = |bg: u32| NodeSpec::column().size(40.5, 20.25).bg(Color::hex(bg));
    ui.with(NodeSpec::row(), |ui| {
        ui.leaf(
            cell(0xd9738cff)
                .pixel_snap()
                .shadow_color(Color::hex(0x000000ff)),
        );
        ui.leaf(cell(0x73d98cff).pixel_snap());
        ui.leaf(cell(0x3b5bd4ff));
    });
}

fn build_clip_float(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(NodeSpec::column().fill(), |ui| {
        ui.leaf_keyed(
            "toolbar",
            NodeSpec::row()
                .grow_width()
                .height(40.0)
                .bg(Color::hex(0x3a3f52ff))
                .on_click(Value::map([("kind", Value::str("toolbar"))]))
                .label("Toolbar"),
        );
        ui.with_keyed(
            "canvas",
            NodeSpec::column().fill().clip().bg(Color::hex(0x101018ff)),
            |ui| {
                // 20 px above the canvas's top: half past its edge.
                ui.leaf_keyed(
                    "node",
                    NodeSpec::column()
                        .float(FloatConfig::parent().offset(40.0, -20.0).clipped())
                        .size(80.0, 40.0)
                        .bg(Color::hex(0x3b5bd4ff))
                        .on_click(Value::map([("kind", Value::str("node"))]))
                        .label("Node"),
                );
                ui.leaf_keyed(
                    "free",
                    NodeSpec::column()
                        .float(FloatConfig::parent().offset(160.0, -20.0))
                        .size(80.0, 40.0)
                        .bg(Color::hex(0x73d98cff))
                        .on_click(Value::map([("kind", Value::str("free"))]))
                        .label("Free"),
                );
            },
        );
    });
}

fn build_layers(ui: &mut Ui<'_>, _f: &Fixtures, phase: u32) {
    // One wrapper the size of the window, as every binding's scene returns
    // one node; the page fills it and the floats hang off the viewport.
    ui.with(NodeSpec::column().fill(), |ui| {
        ui.with_keyed(
            "page",
            NodeSpec::column()
                .fill()
                .scroll_y()
                .bg(Color::hex(0x101018ff)),
            |ui| {
                for i in 0..LAYERS_ROWS {
                    ui.leaf_keyed(
                        &format!("row{i}"),
                        NodeSpec::row().grow_width().height(30.0).bg(if i % 2 == 0 {
                            Color::hex(0x22242cff)
                        } else {
                            Color::hex(0x30344aff)
                        }),
                    );
                }
            },
        );
        let at = |x: f32, y: f32| {
            FloatConfig::viewport()
                .inside(Align::Start, Align::Start)
                .offset(x, y)
        };
        // Closed in phase 1, back in phase 2: the reopening is what
        // puts it over the toast, whatever the tree says.
        if phase != 1 {
            ui.leaf_keyed(
                "popover",
                NodeSpec::column()
                    .float(at(200.0, 40.0))
                    .size(120.0, 80.0)
                    .bg(Color::hex(0x3b5bd4ff))
                    .on_click(Value::map([("kind", Value::str("popover"))]))
                    .label("Popover"),
            );
        }
        ui.leaf_keyed(
            "toast",
            NodeSpec::column()
                .float(at(140.0, 60.0))
                .size(120.0, 80.0)
                .bg(Color::hex(0x73d98cff))
                .on_click(Value::map([("kind", Value::str("toast"))]))
                .label("Toast"),
        );
    });
}
