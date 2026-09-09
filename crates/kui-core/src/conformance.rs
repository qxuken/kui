//! The scene corpus: what every binding must agree on, as data.
//!
//! `schema::CUSTOM` and `schema::ELEMENTS` name the props and elements each
//! frontend lowers by hand. Naming them makes the transports agree on
//! *identity*; nothing made them agree on *behaviour*, so a binding could
//! (and did) drop a piece of one — `kui_tooltip` set no description, the
//! docs named a Lua float key the parser never read — with a green build.
//!
//! This module is the fix: a list of small named [`Scene`]s, each declaring
//! which `CUSTOM` and `ELEMENTS` rows it exercises — a declaration
//! [`observe`] then derives back off the built tree, so a scene cannot
//! claim a row it stopped touching — plus the input to
//! replay and the [`Expect`]ed semantics. [`drive`] runs one against a
//! `Core` and renders a [`report`] — a line-oriented text block with no
//! floating-point formatting in it, so four languages can produce it
//! byte-identically.
//!
//! Two tiers, because only one of them is portable across machines:
//!
//! - [`Scene::expect`] is checked in and font-independent (access rows,
//!   events, warnings, solid/image quad counts). `kui-core`'s own test
//!   asserts it, which pins the reference behaviour.
//! - The report carries a digest over full quad geometry, which depends on
//!   the installed fonts. It is never checked in: the reference is dumped
//!   at test time (`cargo run -p kui-core --features conformance --example conformance-dump`) and
//!   the other three bindings — Lua, C, Node — reproduce their scenes and
//!   compare against that dump, on the same machine, in the same CI job.
//!
//! Most scenes are one tree replayed against a list of inputs. Two steps
//! are not input at all, and exist because one behaviour needs more than a
//! tree: [`Step::Phase`] is the view changing its mind (a node only departs
//! because the view stopped declaring it) and [`Step::Time`] is the frame
//! clock (without one every transition snaps, so there is nothing to
//! depart). A report keeps one frame, so a scene that uses them has to end
//! its steps where the states it wants to tell apart actually differ.
//!
//! Adding a binding-visible prop or element means adding it to a scene
//! here; a binding that lowers it differently then fails to build.
//!
//! The module is behind the `conformance` feature, off by default: it is
//! test infrastructure, and nothing a shipped binary should carry. The
//! crate's own dev-dependency turns it on for its tests and examples, so
//! does `kui-lua`'s; C and Node need nothing, since both rebuild the scenes
//! through their public APIs and read the reference back as a file.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

use crate::access::{AccessTree, Role};
use crate::audio::{AudioCommand, AudioSpec};
use crate::color::Color;
use crate::display::{Clip, Quad, QuadKind};
use crate::edit::EditOptions;
use crate::enter::Enter;
use crate::geom::{Edges, Rect, Size, Vec2};
use crate::input::{EditKey, InputEvent, KeyCode, KeyMods, KeyPress, Mods};
use crate::key::Key;
use crate::line::Stroke;
use crate::resources::{ImageId, SoundId};
use crate::runtime::Core;
use crate::spec::{
    Align, Dir, FloatAnchor, FloatConfig, Min, NodeSpec, PadShorthand, Sizing, TextStyle,
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
    native_controls: None,
};

/// The app draws its own chrome, and the OS draws nothing over it — so
/// `widgets::window_buttons` builds its three buttons.
pub const CUSTOM_CHROME: WindowEnv = WindowEnv {
    id: WindowId::MAIN,
    custom_chrome: true,
    maximized: false,
    fullscreen: false,
    native_controls: None,
};

/// Custom chrome *and* controls the OS keeps drawing over our content: the
/// macOS traffic lights, at the rect `kui::MACOS_TRAFFIC_LIGHTS` reports
/// (78x28 logical px at the window origin, gpui's measured
/// `TRAFFIC_LIGHT_PADDING` under the macOS 26 SDK). The same tree that
/// builds two button clusters under [`CUSTOM_CHROME`] builds none under
/// this one, and its titlebar starts at 78 instead of the bare 12pt
/// margin — which is the whole of what `widgets::titlebar` adapting "per
/// platform by itself" means.
pub const CUSTOM_CHROME_INSET: WindowEnv = WindowEnv {
    id: WindowId::MAIN,
    custom_chrome: true,
    maximized: false,
    fullscreen: false,
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

/// Eighteen numbers, so the last two are dropped with a warning.
pub const FRAGMENT_PARAMS_LONG: [f32; 18] = [
    0.1, 0.2, 0.3, 1.0, 0.4, 0.5, 0.6, 1.0, 4.0, 1.0, 0.0, 0.0, 0.9, 0.9, 0.2, 1.0, 7.0, 8.0,
];

/// Handles a scene's builder needs, registered before the first frame.
#[derive(Clone, Copy)]
pub struct Fixtures {
    pub image: ImageId,
    pub sound: SoundId,
    pub fragment: crate::resources::FragmentId,
}

/// Registers the corpus fixtures on a fresh core, in this order.
pub fn fixtures(core: &mut Core) -> Fixtures {
    let image = core.resources.add_image(IMAGE_W, IMAGE_H, image_pixels());
    let sound = core.add_sound(SOUND_BYTES.to_vec());
    let fragment = core
        .add_fragment(FRAGMENT_WGSL)
        .expect("the corpus fragment must compile");
    Fixtures {
        image,
        sound,
        fragment,
    }
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
    /// Wheel delta in logical px; positive y scrolls up.
    Scroll(i32, i32),
    /// Tab and Shift-Tab: one step along the focus ring, forwards and
    /// backwards. Spelled as their own step kinds rather than a `key`
    /// step with an argument, so the report's step lines stay one word.
    Tab,
    ShiftTab,
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
    /// An IME composing one character (its Unicode scalar value, the caret
    /// at its end) on whatever holds focus — a stock editor shows it
    /// inline, an `on_key` sink hears `{kind="preedit"}` (backlog C17).
    /// Zero is the composition ending without a commit: empty text, no
    /// cursor.
    Preedit(u32),
    /// The IME committing one character: `InputEvent::Commit`, which a
    /// stock editor takes as typed text and a sink hears as
    /// `{kind="text"}`.
    Commit(u32),
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
            Step::Scroll(x, y) => {
                let _ = writeln!(out, "step scroll {x} {y}");
            }
            Step::Tab => out.push_str("step tab\n"),
            Step::ShiftTab => out.push_str("step shifttab\n"),
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
            Step::Preedit(c) => {
                let _ = writeln!(out, "step preedit {c}");
            }
            Step::Commit(c) => {
                let _ = writeln!(out, "step commit {c}");
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
        }
    }

    /// The input this step replays, or `None` for the two that move the
    /// world around the view rather than poking it — see [`Step::Phase`]
    /// and [`Step::Time`].
    pub fn event(&self) -> Option<InputEvent> {
        Some(match *self {
            Step::Phase(_) | Step::Time(_) | Step::WindowClosed(_) | Step::WindowDismissed(..) => {
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
            Step::Scroll(x, y) => InputEvent::Scroll(Vec2::new(x as f32, y as f32)),
            Step::Tab => InputEvent::Key(EditKey::Tab, Mods::default()),
            Step::ShiftTab => InputEvent::Key(
                EditKey::Tab,
                Mods {
                    shift: true,
                    ..Default::default()
                },
            ),
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
    /// so a curve's flattening is pinned too.
    pub segments: usize,
    /// Exact fragment-quad count: one per `fragment` node that resolved
    /// its handle. A node whose handle is dead emits none, which is how
    /// the scene pins that too.
    pub fragments: usize,
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
}

/// One scene: a builder every binding re-expresses, the input to replay,
/// and the rows of `schema::CUSTOM` / `schema::ELEMENTS` it pins.
pub struct Scene {
    pub name: &'static str,
    pub doc: &'static str,
    /// `schema::CUSTOM` names this scene exercises. Hand-written, but not
    /// taken on trust: [`observe`] derives the same set from the tree the
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
            fragments: 0,
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
            fragments: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
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
            fragments: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
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
            fragments: 0,
            glyphs_min: 0,
            access: &["0 window ||", "1 scrollView ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
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
            fragments: 0,
            glyphs_min: 0,
            access: &["0 window ||", "1 scrollView ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
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
            fragments: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
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
            fragments: 0,
            glyphs_min: 10,
            access: &[
                "0 window ||",
                "1 group |a hint|",
                "2 staticText badge||",
                "2 staticText a hint||",
                "1 button Save|Nothing to save yet|",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
        },
    },
    Scene {
        name: "chrome",
        doc: "Window chrome, driven under a declared custom chrome — the \
              only scene that departs from NATIVE_CHROME, and the reason \
              the env line exists. The frame's declared title, an adaptive \
              titlebar hosting custom content and appending its own \
              buttons, a hand-laid strip holding a second cluster through \
              the windowButtons element itself, and a focusable box that \
              claims key focus while it is declared.",
        custom: &["title", "keyFocus", "size"],
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
            fragments: 0,
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
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: Some("kui conformance"),
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
        custom: &["title", "keyFocus", "size"],
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
            fragments: 0,
            glyphs_min: 3,
            access: &[
                "0 window kui conformance||",
                "1 titleBar ||",
                "2 staticText app||",
                "1 group Sink||",
            ],
            events: &[],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: Some("kui conformance"),
        },
    },
    Scene {
        name: "controls",
        doc: "A clicked button, a keyed editor and a slider, inside a panel \
              that asks for a context menu: the secondary press routes to \
              the panel and moves neither focus nor the caret. The slider \
              names its own reading (`valueText`), which lands in the value \
              column beside the editor's text — a node has one string slot, \
              and a slider that named its reading reads as that instead of \
              its number (backlog F8). The clicked button says what it will \
              do (`description`), and a second stock button takes the other \
              rows the composite admits at once — a `label` past its text, \
              `disabled`, and a `tooltip` whose description reaches the \
              access row while its float never draws, since nothing hovers \
              it — through each binding's own button, not a box.",
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
        ],
        expect: Expect {
            // Two buttons: the disabled one is dimmed, and a quad at half
            // alpha is still one solid quad.
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 0,
            fragments: 0,
            glyphs_min: 11,
            access: &[
                "0 window ||",
                "1 button go|Starts the run|",
                "1 button Stop the run|Nothing is running|",
                "1 textInput Note||hello",
                "1 slider Focus length||25 minutes",
            ],
            events: &["go -", "contextmenu menu"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
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
            fragments: 0,
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
            fragments: 0,
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
            fragments: 0,
            glyphs_min: 10,
            access: &["0 window ||", "1 terminal term||hello world"],
            events: &["hit -"],
            announcements: &[],
            warnings: &[],
            commands: &[],
            audio: &[],
            title: None,
        },
    },
    Scene {
        name: "media",
        doc: "The non-text leaves: a registered image (deliberately unnamed, \
              so the diagnostic shows up too), three retained audio \
              playbacks that draw nothing, and the latency graph's empty \
              chrome. Phase 1 drops two of the playbacks, which is what \
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
            images: 1,
            segments: 0,
            fragments: 0,
            glyphs_min: 20,
            access: &["0 window ||", "1 image ||"],
            events: &[],
            announcements: &[],
            warnings: &["image-without-label"],
            commands: &[],
            // `chime` (2) asked to finish and its removal says nothing;
            // `blip` (3) did not and is stopped. `music` (1) is the loop,
            // still declared, so it has no departure to describe.
            audio: &["play 1 1", "play 2 0", "play 3 0", "stop 3"],
            title: None,
        },
    },
    Scene {
        name: "lines",
        doc: "The stroke primitive (`docs/adr/0010-a-segment-primitive.md`): \
              a diagonal segment, an orthogonal elbow through three points, \
              and a faded curve through four knots — 8, 9 and 14 pieces by \
              the core's flattening, so the segment count pins it — beside \
              a box, which the lines paint over because a line is a float. \
              Every line is elided from the access tree and takes no input; \
              the elbow declares a click anyway, and the warning says so.",
        custom: &["key"],
        elements: &["box", "line"],
        build: build_lines,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 2,
            shadows: 0,
            images: 0,
            segments: 34,
            fragments: 0,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &["line-ignores-input"],
            commands: &[],
            audio: &[],
            title: None,
        },
    },
    Scene {
        name: "fragments",
        doc: "A box a registered WGSL function paints               (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`):               a plain gradient, a rounded and faded one with a child painted               over it, one whose handle is dead — which draws nothing, the               documented fallback for every resource — and one that declares               eighteen params, so the truncation warning is pinned. The               parameters ride a side list, not the quad, so the report               carries them as bits on their own lines.",
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
            fragments: 3,
            glyphs_min: 0,
            access: &["0 window ||"],
            events: &[],
            announcements: &[],
            warnings: &["fragment-params-truncated"],
            commands: &[],
            audio: &[],
            title: None,
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
            fragments: 0,
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
            fragments: 0,
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
              whole — no ghost, an `exit-budget` warning — and then 600 \
              one-node rows leave together and are refused the same way, \
              where admitting subtrees one at a time would have kept 512 \
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
            // `bulk` leaves alone: 513 nodes in one subtree, refused whole.
            Step::Phase(3),
            // The rows leave together: 600 nodes in 600 subtrees, refused
            // whole — the frame that separates whole-or-nothing admission
            // from the per-subtree kind, which would keep 512 of them.
            Step::Phase(4),
        ],
        expect: Expect {
            // Eight live boxes and the focus ring on `B`, plus exactly one
            // ghost: `flash`'s was retired by its return and `blink`'s
            // expired, so a store that kept either would count eleven,
            // both twelve — and one that admitted the rows one at a time
            // would count 522.
            solid: 10,
            shadows: 0,
            images: 0,
            segments: 0,
            fragments: 0,
            glyphs_min: 3,
            access: &["0 window ||", "1 group A||", "1 group B||"],
            events: &["hit -"],
            announcements: &[],
            warnings: &["exit-budget", "exit-budget"],
            commands: &[],
            audio: &[],
            title: None,
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
            fragments: 0,
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
            fragments: 0,
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
            fragments: 0,
            glyphs_min: 8,
            access: &["0 window ||", "1 group 3 results||", "1 group ||"],
            events: &[],
            announcements: &["assertive Saved"],
            warnings: &["live-region-without-name"],
            commands: &[],
            audio: &[],
            title: None,
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
            fragments: 0,
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
              follows the row rather than the slot.",
        custom: &["index", "key", "overflow"],
        elements: &["box"],
        build: build_virtual,
        env: NATIVE_CHROME,
        steps: &[],
        expect: Expect {
            solid: 4,
            shadows: 0,
            images: 0,
            segments: 0,
            fragments: 0,
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
        },
    },
];

/// The rows a virtual list builds, at the data indices it builds them at.
/// A binding that lowers `index` correctly reproduces these keys wherever it
/// puts the rows; one that ignores the row auto-keys them by position and
/// every quad in the scene lands the same while the access tree and the hit
/// keys do not — which is why the rows carry roles and names.
pub const VIRTUAL_ROWS: [u64; 3] = [100, 101, 102];

fn build_virtual(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with_keyed(
        "list",
        NodeSpec::column()
            .width(Sizing::Fixed(120.0))
            .height(Sizing::Fixed(60.0))
            .gap(0.0)
            .scroll_y()
            .bg(Color::hex(0x101018ff))
            .role(Role::List)
            .label("log"),
        |ui| {
            // The height of the rows above the built range, and below it.
            ui.with_keyed("lead", virtual_spacer(20.0), |_| {});
            for i in VIRTUAL_ROWS {
                ui.with_indexed(
                    i,
                    NodeSpec::column()
                        .width(Sizing::Grow(1.0))
                        .height(Sizing::Fixed(20.0))
                        .bg(Color::hex(0x30344aff))
                        .role(Role::ListItem)
                        .label(match i {
                            100 => "row 100",
                            101 => "row 101",
                            _ => "row 102",
                        }),
                    |_| {},
                );
            }
            ui.with_keyed("tail", virtual_spacer(100.0), |_| {});
        },
    );
}

fn virtual_spacer(h: f32) -> NodeSpec {
    NodeSpec::column()
        .width(Sizing::Grow(1.0))
        .height(Sizing::Fixed(h))
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
                    .width(Sizing::Fixed(180.0))
                    .height(Sizing::Fixed(40.0)),
                |ui| {
                    ui.text("ab", TextStyle::new(12.0));
                    ui.text("cd", TextStyle::new(12.0));
                },
            );
            // Padding only, no children: the box's own size *is* the
            // resolved shorthand, so a binding that fell back differently
            // draws a differently sized quad. `padY` gives the top, `padB`
            // overrides the bottom, `padX` both sides.
            ui.with(
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
                |_| {},
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
                NodeSpec::row()
                    .width(Sizing::Fixed(200.0))
                    .height(Sizing::Fixed(40.0))
                    .bg(Color::hex(0x101018ff)),
                |ui| {
                    let cell = |bg: u32| {
                        NodeSpec::column()
                            .height(Sizing::Fixed(20.0))
                            .bg(Color::hex(bg))
                    };
                    ui.with(cell(0x30344aff).width(Sizing::Fixed(30.0)), |_| {});
                    ui.with(cell(0x3b5bd4ff).width(Sizing::Percent(0.25)), |ui| {
                        // Nothing inside: a percent is the parent's, not
                        // the content's.
                        let _ = ui;
                    });
                    ui.with(cell(0x73d98cff).width(Sizing::Fit), |ui| {
                        ui.with(
                            NodeSpec::column()
                                .width(Sizing::Fixed(20.0))
                                .height(Sizing::Fixed(10.0))
                                .bg(Color::hex(0xff0000ff)),
                            |_| {},
                        );
                    });
                    ui.with(cell(0xffcc00ff).width(Sizing::Grow(1.0)), |_| {});
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
            .width(Sizing::Fixed(100.0))
            .bg(Color::hex(0x101018ff)),
        |ui| {
            for (w, h) in WRAP_BOXES {
                ui.with(
                    NodeSpec::column()
                        .width(Sizing::Fixed(*w))
                        .height(Sizing::Fixed(*h))
                        .bg(Color::hex(0x30344aff)),
                    |_| {},
                );
            }
        },
    );
}

fn build_tabs(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    let bar = || {
        NodeSpec::row()
            .width(Sizing::Fixed(200.0))
            .height(Sizing::Fixed(20.0))
            .bg(Color::hex(0x101018ff))
    };
    let tab = || {
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .min_width(Min::FIT)
            .bg(Color::hex(0x30344aff))
    };
    let label = |w: f32, h: f32| {
        NodeSpec::column()
            .width(Sizing::Fixed(w))
            .height(Sizing::Fixed(h))
            .bg(Color::hex(0x3b5bd4ff))
    };
    ui.with(NodeSpec::column().pad(4.0).gap(4.0), |ui| {
        ui.with_keyed("roomy", bar(), |ui| {
            for (w, h) in TAB_ROOMY {
                ui.with(
                    tab().height(Sizing::Percent(0.5)).min_height(Min::FIT),
                    |ui| {
                        ui.with(label(*w, *h), |_| {});
                    },
                );
            }
        });
        ui.with_keyed("crowded", bar().scroll_x(), |ui| {
            for w in TAB_CROWDED {
                ui.with(tab().height(Sizing::Grow(1.0)), |ui| {
                    ui.with(label(*w, 12.0), |_| {});
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
                .width(Sizing::Fixed(120.0))
                .height(Sizing::Fixed(60.0))
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
                    ui.with_keyed(
                        key,
                        NodeSpec::column()
                            .width(Sizing::Fixed(100.0))
                            .height(Sizing::Fixed(20.0))
                            .bg(Color::hex(0x30344aff)),
                        |_| {},
                    );
                }
            },
        );
    });
}

/// Item labels as constants: a binding building this scene has to use the
/// same strings, since keys are hashes of the path.
pub const ITEM_KEYS: [&str; 6] = ["i0", "i1", "i2", "i3", "i4", "i5"];

fn build_float(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(NodeSpec::column().pad(20.0).gap(4.0), |ui| {
        ui.with_keyed(
            "anchor",
            NodeSpec::column()
                .width(Sizing::Fixed(80.0))
                .height(Sizing::Fixed(24.0))
                .bg(Color::hex(0x333333ff)),
            |ui| {
                ui.with(
                    NodeSpec::column()
                        .float(FloatConfig::below())
                        .width(Sizing::Fixed(40.0))
                        .height(Sizing::Fixed(12.0))
                        .bg(Color::hex(0xff0000ff)),
                    |_| {},
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
                .width(Sizing::Fixed(60.0))
                .height(Sizing::Fixed(20.0))
                .bg(Color::hex(0x444444ff)),
            |ui| {
                ui.with(
                    NodeSpec::column()
                        .float(FloatConfig::build(
                            FloatConfig::below(),
                            None,
                            None,
                            Some(6.0),
                            None,
                            false,
                        ))
                        .width(Sizing::Fixed(30.0))
                        .height(Sizing::Fixed(10.0))
                        .bg(Color::hex(0x0000ffff)),
                    |_| {},
                );
            },
        );
        ui.with(
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
                .width(Sizing::Fixed(10.0))
                .height(Sizing::Fixed(10.0))
                .bg(Color::hex(0x00ff00ff)),
            |_| {},
        );
    });
}

/// The tooltip prop, spelled out: the bindings' parsers turn `tooltip`
/// into `hoverable` plus an accessible `description`, and draw
/// `widgets::tooltip` as the node's last child while it is hovered.
///
/// The second node is the `description` prop on its own — the same slot
/// with neither the hover tracking nor the float, which is what a hint
/// that is spoken and never drawn looks like. It sits here rather than in
/// a scene of its own so the two are read side by side: the same string
/// arrives in the same column of the access dump, and only the tooltip
/// draws anything for it.
fn build_tooltip(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(NodeSpec::column().pad(10.0), |ui| {
        let key = ui.child_key("tip");
        ui.with_keyed(
            "tip",
            NodeSpec::row()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(40.0))
                .bg(Color::hex(0x333333ff))
                .role(Role::Group)
                .hoverable()
                .description("a hint"),
            |ui| {
                ui.text("badge", TextStyle::new(12.0));
                if ui.is_hovered(key) {
                    widgets::tooltip(ui, "a hint");
                }
            },
        );
        ui.with(
            NodeSpec::row()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(20.0))
                .role(Role::Button)
                .label("Save")
                .description("Nothing to save yet"),
            |_| {},
        );
    });
}

fn build_chrome(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.window_title("kui conformance");
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
        // rather than a side effect of where the corpus put a box.
        ui.with(
            NodeSpec::row().width(Sizing::Grow(1.0)),
            widgets::window_buttons,
        );
        let sink = ui.open_keyed(
            "sink",
            NodeSpec::column()
                .width(Sizing::Fixed(40.0))
                .height(Sizing::Fixed(16.0))
                .bg(Color::hex(0x22242cff))
                .focusable()
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
        .on_context_menu(Value::map([("kind", Value::str("menu"))]));
    ui.with(panel, |ui| {
        widgets::button_with(
            ui,
            "go",
            "go",
            widgets::button_spec()
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
            widgets::button_spec()
                .on_click(Value::map([("kind", Value::str("stop"))]))
                .label("Stop the run")
                .disabled(true)
                .apply_tooltip("Nothing is running"),
            Some("Nothing is running"),
        );
        ui.text_edit(
            "note",
            "hello",
            &EditOptions {
                style: TextStyle::new(13.0),
                ..Default::default()
            },
            NodeSpec::column().width(Sizing::Fixed(160.0)).label("Note"),
        );
        // A slider that says what its position reads as. Without
        // `value_text` a reader has only the three numbers and says a
        // percentage — 25 in [5..60] is "36 percent" — and the reading
        // travels in the access row's value column, the one string slot a
        // node has (backlog F8). No background, so the scene's quad counts
        // are the button's and the editor's as before.
        ui.with_keyed(
            "focus",
            NodeSpec::row()
                .role(Role::Slider)
                .label("Focus length")
                .value_now(25.0)
                .value_min(5.0)
                .value_max(60.0)
                .value_text("25 minutes")
                .width(Sizing::Fixed(120.0))
                .height(Sizing::Fixed(12.0)),
            |_| {},
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
                .width(Sizing::Fixed(200.0))
                .height(Sizing::Fixed(24.0))
                .bg(Color::hex(0x1b1d27ff))
                .on_key(Value::map([("kind", Value::str("ed"))]))
                .role(Role::MultilineTextInput)
                .label("Buffer"),
            |ui| {
                ui.with_keyed(
                    "l0",
                    NodeSpec::row()
                        .height(Sizing::Fixed(20.0))
                        .role(Role::Line)
                        .caret(1),
                    |ui| ui.text("ab", TextStyle::new(13.0).mono()),
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
            NodeSpec::column().width(Sizing::Fixed(200.0)).label("Note"),
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
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(24.0))
            .bg(Color::hex(0x1b1d27ff))
            .on_key(Value::Int(1))
            .role(Role::Group)
            .label(label)
    };
    ui.with(NodeSpec::column().pad(10.0).gap(6.0), |ui| {
        ui.with_keyed("press", sink("press"), |_| {});
        ui.with_keyed("held", sink("held").key_up(), |_| {});
        // A shell over a ring: the sink hears what the button inside it
        // does not claim, and the button holds focus from the first frame.
        ui.with_keyed("shell", sink("shell").key_up(), |ui| {
            let go = ui.with_keyed(
                "go",
                NodeSpec::row()
                    .width(Sizing::Fixed(80.0))
                    .height(Sizing::Fixed(16.0))
                    .bg(Color::hex(0x3b5bd4ff))
                    .on_click(Value::map([("kind", Value::str("go"))]))
                    .label("Go"),
                |_| {},
            );
            ui.take_key_focus(go);
        });
    });
}

/// The three playbacks are declared in this order, so the ids the report
/// names are 1 (`music`), 2 (`chime`) and 3 (`blip`) in every binding.
/// Phase 1 stops declaring the last two, which is the scene's whole point:
/// `chime` asked to [`AudioSpec::finish`] and leaves no command behind,
/// `blip` did not and is stopped (backlog F29).
fn build_media(ui: &mut Ui<'_>, f: &Fixtures, phase: u32) {
    ui.with(NodeSpec::column().pad(6.0).gap(4.0), |ui| {
        ui.image(
            f.image,
            NodeSpec::column().width(Sizing::Fixed(16.0)).radius(2.0),
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

/// The `lines` scene: a 200×120 canvas holding three strokes and one box.
/// Points are in the canvas's box space, the way a floated card's offset
/// is. The curve's chords are 44.7, 50 and 82.5, which
/// `line::flatten_curve` cuts into 8, 9 and 14 pieces at `CURVE_STEP` 6.
fn build_lines(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(
        NodeSpec::column()
            .width(Sizing::Fixed(200.0))
            .height(Sizing::Fixed(120.0))
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
                NodeSpec::column().on_click(Value::str("elbow")),
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
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Fixed(40.0))
                    .height(Sizing::Fixed(20.0))
                    .bg(Color::hex(0x202030ff)),
                |_| {},
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
            .width(Sizing::Fixed(200.0))
            .height(Sizing::Fixed(120.0))
            .gap(4.0)
            .bg(Color::hex(0x14161eff)),
        |ui| {
            ui.fragment(
                f.fragment,
                &FRAGMENT_PARAMS,
                NodeSpec::column()
                    .width(Sizing::Fixed(80.0))
                    .height(Sizing::Fixed(40.0)),
            );
            // Rounded, faded, and holding a child that paints over it —
            // the child's own solid is the one the fade multiplies too.
            ui.fragment_with_keyed(
                "card",
                f.fragment,
                &FRAGMENT_PARAMS,
                NodeSpec::column()
                    .width(Sizing::Fixed(80.0))
                    .height(Sizing::Fixed(40.0))
                    .pad(6.0)
                    .radius(8.0)
                    .opacity(0.5),
                |ui| {
                    ui.with(
                        NodeSpec::column()
                            .width(Sizing::Fixed(20.0))
                            .height(Sizing::Fixed(10.0))
                            .bg(Color::hex(0x202030ff)),
                        |_| {},
                    );
                },
            );
            // A handle that is live in no session: draws nothing.
            ui.fragment(
                crate::resources::FragmentId::from_ffi(0),
                &FRAGMENT_PARAMS,
                NodeSpec::column()
                    .width(Sizing::Fixed(20.0))
                    .height(Sizing::Fixed(10.0)),
            );
            // Eighteen params: the last two are dropped, with a warning.
            ui.fragment(
                f.fragment,
                &FRAGMENT_PARAMS_LONG,
                NodeSpec::column()
                    .width(Sizing::Fixed(30.0))
                    .height(Sizing::Fixed(12.0)),
            );
        },
    );
}

fn build_modal(ui: &mut Ui<'_>, _f: &Fixtures, phase: u32) {
    let button = |kind: &str, label: &str| {
        NodeSpec::row()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(24.0))
            .bg(Color::hex(0x3b5bd4ff))
            .on_click(Value::map([("kind", Value::str(kind))]))
            .label(label)
    };
    // Grow, not fit: the titlebar is a full-width drag strip, and a fit
    // column would shrink it to its content and leave (160, 16) on
    // nothing at all — which reads as inert chrome.
    ui.with(NodeSpec::column().gap(6.0).width(Sizing::Grow(1.0)), |ui| {
        widgets::titlebar_with(ui, |ui| {
            ui.text("app", TextStyle::new(12.0));
        });
        let open = ui.with_keyed(
            "open",
            NodeSpec::row()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(20.0))
                .bg(Color::hex(0x30344aff))
                .on_click(Value::map([("kind", Value::str("open"))]))
                .label("Open"),
            |_| {},
        );
        // Phase 1 is the app with the dialog gone: the node it was opened
        // to rename, created with it and still here, declared focused so
        // that the restore has a declaration to yield to
        // (`docs/adr/0003-modal-surfaces.md`, decision 4).
        if phase != 0 {
            let note = ui.with_keyed(
                "note",
                NodeSpec::row()
                    .width(Sizing::Fixed(100.0))
                    .height(Sizing::Fixed(20.0))
                    .bg(Color::hex(0x30344aff))
                    .on_click(Value::map([("kind", Value::str("note"))]))
                    .label("Note"),
                |_| {},
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
                .width(Sizing::Fixed(120.0))
                .height(Sizing::Fixed(100.0))
                .pad(8.0)
                .gap(6.0)
                .bg(Color::hex(0x202030ff))
                .float(
                    FloatConfig::viewport()
                        .at(Align::End, Align::End)
                        .self_at(Align::End, Align::End),
                )
                .modal(Value::map([("kind", Value::str("dlg"))]))
                .label("Settings"),
            |ui| {
                ui.with_keyed("ok", button("ok", "OK"), |_| {});
                ui.with_keyed("cancel", button("cancel", "Cancel"), |_| {});
            },
        );
    });
}

/// A tab bar and a picker list, each a composite because its items are
/// focusable, with an ordinary button between them that keeps its own Tab
/// stop. The tabs are a row and the list a column, so the two derived
/// orientations differ.
fn build_composite(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    ui.with(NodeSpec::column().gap(6.0).width(Sizing::Grow(1.0)), |ui| {
        ui.with_keyed("tabs", NodeSpec::row().role(Role::TabList).gap(4.0), |ui| {
            for (i, name) in ["One", "Two", "Three"].iter().enumerate() {
                ui.with_keyed(
                    name,
                    NodeSpec::row()
                        .role(Role::Tab)
                        // The view's own selection, and the entry the
                        // ring takes: the second tab, not the first.
                        .selected(i == 1)
                        .width(Sizing::Fixed(60.0))
                        .height(Sizing::Fixed(20.0))
                        .bg(Color::hex(0x30344aff))
                        .on_click(Value::map([("kind", Value::str(name.to_lowercase()))])),
                    |ui| ui.text(name, TextStyle::new(12.0)),
                );
            }
        });
        ui.with_keyed(
            "add",
            NodeSpec::row()
                .width(Sizing::Fixed(40.0))
                .height(Sizing::Fixed(20.0))
                .bg(Color::hex(0x3b5bd4ff))
                .on_click(Value::map([("kind", Value::str("add"))]))
                .label("Add"),
            |_| {},
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
                        .width(Sizing::Fixed(80.0))
                        .height(Sizing::Fixed(18.0))
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
pub const EXIT_ROWS: usize = 600;

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
    let slot = |h: f32| {
        NodeSpec::column()
            .width(Sizing::Fixed(140.0))
            .height(Sizing::Fixed(h))
            .bg(Color::hex(0x101018ff))
    };
    let keep = |label: &'static str| {
        NodeSpec::row()
            .width(Sizing::Fixed(60.0))
            .height(Sizing::Fixed(16.0))
            .bg(Color::hex(0x22242cff))
            .focusable()
            .label(label)
    };
    ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .pad(8.0)
            .gap(6.0)
            .bg(Color::hex(0x14161eff)),
        |ui| {
            ui.with_keyed("a", keep("A"), |_| {});
            ui.with_keyed("slotFade", slot(40.0), |ui| {
                if phase == 0 {
                    ui.with_keyed(
                        "fade",
                        NodeSpec::column()
                            .width(Sizing::Fixed(100.0))
                            .height(Sizing::Fixed(24.0))
                            .bg(Color::hex(0x3b5bd4ff))
                            .transition(400.0)
                            .exit(Enter::from(40.0, 0.0).opacity(0.0))
                            .focusable()
                            .label("Fade")
                            .on_click(Value::map([("kind", Value::str("hit"))])),
                        |ui| {
                            ui.text("bye", TextStyle::new(12.0));
                        },
                    );
                }
            });
            ui.with_keyed("slotBlink", slot(16.0), |ui| {
                if phase == 0 {
                    ui.with_keyed(
                        "blink",
                        NodeSpec::column()
                            .width(Sizing::Fixed(100.0))
                            .height(Sizing::Fixed(12.0))
                            .bg(Color::hex(0x73d98cff))
                            .transition(50.0)
                            .exit(Enter::from(20.0, 0.0)),
                        |_| {},
                    );
                }
            });
            ui.with_keyed("slotFlash", slot(16.0), |ui| {
                if phase != 1 {
                    ui.with_keyed(
                        "flash",
                        NodeSpec::column()
                            .width(Sizing::Fixed(100.0))
                            .height(Sizing::Fixed(12.0))
                            .bg(Color::hex(0xffcc00ff))
                            .transition(400.0)
                            .exit(Enter::from(-20.0, 0.0)),
                        |_| {},
                    );
                }
            });
            ui.with_keyed("b", keep("B"), |_| {});
            // More one-node departures than the budget, each a solid quad
            // half a pixel wide, in a slot that keeps its size when they go.
            ui.with_keyed(
                "slotRows",
                NodeSpec::row()
                    .width(Sizing::Fixed(300.0))
                    .height(Sizing::Fixed(4.0))
                    .bg(Color::hex(0x101018ff)),
                |ui| {
                    if phase < 4 {
                        for i in 0..EXIT_ROWS {
                            ui.with_indexed(
                                i as u64,
                                NodeSpec::column()
                                    .width(Sizing::Fixed(0.5))
                                    .height(Sizing::Fixed(4.0))
                                    .bg(Color::hex(0x8a8fa3ff))
                                    .transition(400.0)
                                    .exit(Enter::default().opacity(0.0)),
                                |_| {},
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
                            ui.with(NodeSpec::column(), |_| {});
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
    ui.with(
        NodeSpec::column().pad(8.0).bg(Color::hex(0x14161eff)),
        |ui| {
            ui.text(
                if phase == 0 { "menu" } else { "closed" },
                TextStyle::new(12.0),
            );
        },
    );
}

fn build_drag(ui: &mut Ui<'_>, _f: &Fixtures, _phase: u32) {
    // A splitter handle: 80x40 at the origin, so (40, 20) is its middle
    // and every step lands on it. Keyed, so the press has a stable node
    // to capture on across the frames the scene drives.
    ui.with_keyed(
        "handle",
        NodeSpec::column()
            .width(Sizing::Fixed(80.0))
            .height(Sizing::Fixed(40.0))
            .bg(Color::hex(0x30344aff))
            .on_drag(Value::map([("kind", Value::str("split"))])),
        |_ui| {},
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
            ui.with_keyed(
                "empty",
                NodeSpec::column().live(crate::access::Live::Polite),
                |_ui| {},
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
/// with nothing for the quad digests to compare on that row. [`observe`]
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

/// Rows [`observe`] cannot derive, with the reason. These are the ones
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
fn is_label_keyed(t: &Tree, i: usize) -> bool {
    let parent = t.parent[i];
    let parent_key = t.keys[parent as usize];
    let siblings = t.children(parent).count() as u64;
    !(0..siblings + 16).any(|j| parent_key.index(j) == t.keys[i])
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
    if core.window_title().is_some() {
        cov.custom.insert("title");
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
    // An `audio` element builds no node either — it declares a playback
    // the audio store reconciles — so it is read off the store.
    if core.audio.any_mounted() {
        cov.elements.insert("audio");
    }

    let t = &core.tree;
    for i in 0..t.len() {
        let spec = &t.specs[i];
        let l = &spec.layout;

        // Column is the default, so a row is the only observable `dir`.
        if l.dir == Dir::Row {
            cov.custom.insert("dir");
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
        // itself; nothing else in the corpus asks for `Role::None`.
        if spec.access().role == Some(Role::None) {
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
            NodeContent::Image(_) => {
                cov.elements.insert("image");
            }
            NodeContent::Line(_) => {
                cov.elements.insert("line");
            }
            NodeContent::Fragment(_) => {
                cov.elements.insert("fragment");
            }
        }

        if i > 0 && is_label_keyed(t, i) {
            cov.custom.insert("key");
        }
        if i > 0 && is_far_indexed(t, i) {
            cov.custom.insert("index");
        }
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
    pub kinds: [usize; 8],
    /// Every `FragmentDraw` the frame emitted, in the order the quads
    /// index them. The parameters are not on the quad, so the digest
    /// cannot reach them; the report carries them instead, as bits, so no
    /// adapter has to agree on how a float prints.
    pub fragments: Vec<[f32; 16]>,
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
    /// `-` / `0` / `1`.
    pub checked: Option<bool>,
    /// `-` / `0` / `1`. In the report because the core moving focus inside
    /// a composite must be visible *not* to have moved this
    /// (`docs/adr/0007-composite-keyboard-patterns.md`, decision 10).
    pub selected: Option<bool>,
    /// `-` / `h` / `v`: how a composite container arranges its items.
    pub orientation: &'static str,
    /// `-` / `p` / `a`: the liveness the node declared.
    pub live: &'static str,
    pub scrollable: bool,
    /// Action names in `AccessAction::ALL` (bit) order, comma-joined.
    pub actions: String,
    pub name: String,
    pub description: String,
    pub value: String,
}

/// FNV-1a over the little-endian bytes of a quad's fields, `uv` excluded:
/// atlas coordinates depend on glyph insertion order, which a binding is
/// free to reach by a different route. Field order is `KuiQuad`'s: x, y, w,
/// h, color[4], border_color[4], radius[4], border_w, blur, kind, clip[4],
/// clip_radius[4] — words 0..=18 and 23..=30 of the 31-word struct. Every
/// adapter hashes the same words, so a geometry difference is one
/// mismatched hex string, and a mirror of `KuiQuad` that missed a field
/// mismatches on every scene rather than on none.
pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
pub const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn mix(h: &mut u64, word: u32) {
    for b in word.to_le_bytes() {
        *h ^= b as u64;
        *h = h.wrapping_mul(FNV_PRIME);
    }
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
        if q.kind == QuadKind::Segment {
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
    let kind = payload
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("-")
        .to_string();
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
    // press point in every phase (backlog F2), and a binding that summed
    // steps instead would agree on the kind and disagree here. Printed as
    // integers — the steps are integers, so the deltas are exact.
    if kind == "drag" {
        let num = |k: &str| payload.get(k).and_then(Value::as_float).unwrap_or(0.0) as i64;
        let phase = payload.get("phase").and_then(Value::as_str).unwrap_or("-");
        let _ = write!(tag, " {phase} {} {}", num("dx"), num("dy"));
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
    let announcements = core.take_announcements();
    let warnings = core.take_warnings().into_iter().map(|w| w.code).collect();
    let nodes = rows(core.access_tree());
    let dl = core.output().0;
    let fragment_params: Vec<[f32; 16]> = dl.fragments.iter().map(|f| f.params).collect();
    let quads = &dl.quads;
    let clips = &dl.clips;
    let mut kinds = [0usize; 8];
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
        }] += 1;
    }
    Output {
        quad_count: quads.len(),
        quad_digest: quad_digest(quads, clips),
        kinds,
        fragments: fragment_params,
        nodes,
        events,
        announcements,
        warnings,
        commands,
        audio,
        title,
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
/// stops the playback or releases it (`AudioSpec::finish`, backlog F29).
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
/// quads <count> <digest:016x>
/// kinds <solid> <glyphMask> <glyphColor> <image> <glyphSubpixel> <shadow> <segment>
/// node <depth> <key:016x> <role> <focused> <disabled> <checked> <scroll> <actions> <name> | <description> | <value>
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
    let _ = writeln!(s, "quads {} {:016x}", out.quad_count, out.quad_digest);
    let _ = writeln!(
        s,
        "kinds {} {} {} {} {} {} {} {}",
        out.kinds[0],
        out.kinds[1],
        out.kinds[2],
        out.kinds[3],
        out.kinds[4],
        out.kinds[5],
        out.kinds[6],
        out.kinds[7]
    );
    for (i, params) in out.fragments.iter().enumerate() {
        let _ = write!(s, "fragment {i}");
        for v in params {
            let _ = write!(s, " {:08x}", v.to_bits());
        }
        let _ = writeln!(s);
    }
    for n in &out.nodes {
        let _ = writeln!(
            s,
            "node {} {:016x} {} {} {} {} {} {} {} {} {} {} | {} | {}",
            n.depth,
            n.key.0,
            n.role,
            n.focused as u8,
            n.disabled as u8,
            n.checked.map_or("-".to_string(), |c| (c as u8).to_string()),
            n.selected
                .map_or("-".to_string(), |c| (c as u8).to_string()),
            n.orientation,
            n.live,
            n.scrollable as u8,
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
