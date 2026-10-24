//! The core as a host drives it: a view built every frame, and between
//! frames whatever a driver can hand over — pointer, wheel, keys, text,
//! IME, paste, files, assistive requests, a resize, the clock, the OS
//! closing or dismissing a window, the appearance flipping — with a
//! faded subtree drawn as one group or faded per quad (ADR 0056).
//!
//! The view is one of the conformance corpus's scenes
//! (`kui_core::conformance::SCENES`, every element and prop the bindings
//! share), or an editor of the fuzzer's own text — the corpus types into
//! its fields one character at a time, and an editor is where byte offsets
//! go wrong. The scene's own steps are not replayed: the fuzzer's are.
//!
//! Checked after every frame: nothing panicked, and while every number
//! the driver handed over was finite, every quad the frame emitted is.
//! Most pointer moves aim at a node of the last frame
//! (`Action::Aim`), so presses land on what is there to press.
//!
//! Every input gets a new core, built in a session kept from the inputs
//! before it, with the fixtures registered once (`crate::reuse`).

use arbitrary::{Arbitrary, Result, Unstructured};
use kui_core::access::{AccessAction, AccessRequest, TextPos};
use kui_core::conformance::{self, Fixtures, SCENES, Scene};
use kui_core::input::{
    EditKey, KeyCode, KeyLocation, KeyLocks, KeyMods, KeyPress, Mods, MouseButton,
};
use kui_core::{
    ClipboardMarks, Core, DismissReason, EditOptions, InputEvent, Key, LONG_LINE_BYTES, NodeSpec,
    QuadKind, Size, TextStyle, Ui, Vec2, WindowId,
};

use crate::reuse::{self, Kept};

#[derive(Arbitrary, Debug)]
struct Input {
    /// Which view: a corpus scene by index, or past them the editor.
    scene: u8,
    editor: Editor,
    actions: Vec<Action>,
}

/// The editor scene's options.
#[derive(Arbitrary, Debug)]
struct Editor {
    initial: String,
    multiline: bool,
    wrap: bool,
    /// Its width in logical px; 0 grows to the window.
    width: u8,
    /// The text's size, an index into [`SIZES`].
    size: u8,
}

/// Text sizes a binding can pass: none, small, the usual, and far past
/// any screen — every glyph's outline goes to the rasterizer at this size.
const SIZES: &[f32] = &[14.0, 0.0, 1.0, 9.5, 32.0, 200.0, 3000.0, 1.0e6];

#[derive(Arbitrary, Debug)]
enum Action {
    /// The pointer to a node of the last frame (by index into its node
    /// list), offset from its centre.
    Aim(u16, i8, i8),
    Cursor(Coord, Coord),
    CursorLeft,
    Down(Button, u8),
    Up(Button),
    Scroll(Coord, Coord),
    /// A delta in a scroll gesture: pixels, or a wheel's notches.
    Gesture(Coord, Coord, Gesture),
    Text(String),
    Commit(String),
    Paste(String, u8),
    /// A composition and the caret's byte range in it, as the driver
    /// reports it.
    Preedit(String, Option<(u16, u16)>),
    /// An editing key by index into `EditKey::ALL`, the modifiers as
    /// three bits.
    Key(u8, u8),
    KeyDown(Press),
    KeyUp(Press),
    Modifiers(u8),
    ForceClick(Coord, Coord),
    DragFiles(u8, Coord, Coord),
    DropFiles(u8, Coord, Coord),
    DragCancel,
    Files(u8),
    Open(u8),
    /// An assistive request on a node of the last frame.
    Access {
        node: u16,
        action: u8,
        value: Option<String>,
        selection: Option<(u16, u16)>,
    },
    /// The window resized (logical px, a quarter at a time) and its scale.
    Resize(u16, u16, u8),
    /// The clock moving on, in ms.
    Time(u16),
    /// The scene's phase from the low two bits, and from the next one
    /// whether a faded subtree is drawn as one group from here on
    /// (`Core::set_opacity_groups`, ADR 0056).
    Phase(u8),
    Appearance(u8),
    WindowClosed(u8),
    Dismissed(u8, bool),
    /// The editor's text replaced by the app.
    SetText(String),
    /// A frame with nothing new: transitions and exits run.
    Frame,
}

/// A gesture's byte: bit 0 whether it begins one (a `bool` as it was
/// read), bit 7 whether the delta is a wheel's, in lines at 40 px each
/// (ADR 0050, a notch gliding on a scroll container). Read from bits a
/// `bool` left unused, so what was kept before reads as it did but where
/// that bit was set.
#[derive(Arbitrary, Debug, Clone, Copy)]
struct Gesture(u8);

#[derive(Arbitrary, Debug, Clone, Copy)]
enum Button {
    Primary,
    Secondary,
    Middle,
    Other(u8),
}

/// A key as a driver reports it.
#[derive(Arbitrary, Debug)]
struct Press {
    code: Code,
    mods: u8,
    text: Option<String>,
    repeat: bool,
    location: u8,
    caps: bool,
}

#[derive(Arbitrary, Debug)]
enum Code {
    Char(char),
    F(u8),
    /// By index into `KeyCode::named()`.
    Named(u8),
}

/// A number a driver could hand over: in the window most of the time,
/// past its edges sometimes, and now and then one no window produces.
#[derive(Debug, Clone, Copy)]
struct Coord(f32);

impl<'a> Arbitrary<'a> for Coord {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        Ok(Coord(match u.int_in_range(0u8..=31)? {
            0 => f32::NAN,
            1 => f32::INFINITY,
            2 => -1.0e30,
            3 => 1.0e30,
            4..=7 => f32::from(u.arbitrary::<i16>()?) / 4.0,
            _ => f32::from(u.int_in_range(-40i16..=1400)?) / 4.0,
        }))
    }
}

const SCALES: &[f32] = &[1.0, 1.25, 1.5, 2.0, 0.5, 3.0];

pub fn run(data: &[u8]) {
    drive(data, None);
}

/// What [`run`] saw of `data`, line by line — the events, the commands,
/// the warnings, the access tree and the quads after every step — for
/// `tests/reuse.rs` to hold a kept session to a new one. Tracing only
/// reads what `run` reads.
pub fn trace(data: &[u8]) -> Vec<String> {
    let mut lines = Vec::new();
    drive(data, Some(&mut lines));
    lines
}

thread_local! {
    static KEPT: Kept<Fixtures> = const { Kept::new() };
}

fn drive(data: &[u8], trace: Option<&mut Vec<String>>) {
    let Ok(input) = Input::arbitrary_take_rest(Unstructured::new(data)) else {
        return;
    };
    let scene = SCENES.get(input.scene as usize % (SCENES.len() + 1));
    KEPT.with(|kept| {
        let (mut core, fixtures) = kept.core(conformance::fixtures);
        core.set_diagnostics(true);
        core.set_inspect(true);
        if let Some(s) = scene {
            core.env.window = s.env;
        }
        let mut d = Driver {
            core,
            scene,
            fixtures,
            editor: input.editor,
            edit: None,
            size: conformance::VIEWPORT,
            scale: conformance::SCALE,
            phase: 0,
            now: 0.0,
            finite: true,
            dirty: false,
            trace,
        };
        d.frame();
        for action in &input.actions {
            d.note(|| format!("{action:?}"));
            d.apply(action);
            d.frame();
        }
        kept.done(d.core, fixtures, d.dirty);
    });
}

struct Driver<'a> {
    core: Core,
    scene: Option<&'static Scene>,
    fixtures: Fixtures,
    editor: Editor,
    /// The editor's key, once a frame has declared it.
    edit: Option<Key>,
    size: Size,
    scale: f32,
    phase: u32,
    now: f64,
    /// Whether every number handed to the core so far was finite.
    finite: bool,
    /// A window opened or a sound was queued: the session is not handed
    /// back (see [`crate::reuse`]).
    dirty: bool,
    trace: Option<&'a mut Vec<String>>,
}

impl Driver<'_> {
    fn note(&mut self, line: impl FnOnce() -> String) {
        if let Some(t) = self.trace.as_deref_mut() {
            t.push(line());
        }
    }

    fn frame(&mut self) {
        let mut ui = self.core.frame(self.size, self.scale);
        match self.scene {
            Some(s) => (s.build)(&mut ui, &self.fixtures, self.phase),
            None => self.edit = Some(editor(&mut ui, &self.editor, self.phase)),
        }
        ui.finish();
        // What a driver takes off the core after a frame.
        let events = self.core.take_pending_events();
        let windows = self.core.take_window_commands();
        let audio = self.core.take_audio_commands();
        let announced = self.core.take_announcements();
        let warnings = self.core.take_warnings();
        self.dirty |= reuse::opens(&windows) || !audio.is_empty();
        if self.trace.is_some() {
            let tree = format!("{:?}", self.core.access_tree());
            let focus = self.core.focus();
            let text = self.edit.and_then(|k| self.core.edit_text(k));
            self.note(|| {
                format!(
                    "frame: {events:?}\n{windows:?}\n{audio:?}\n{announced:?}\n\
                     {warnings:?}\nfocus {focus:?} {text:?}\n{tree}"
                )
            });
        } else {
            let _ = self.core.access_tree();
        }
        let finite = self.finite;
        let groups = self.core.opacity_groups();
        let (list, _) = self.core.output();
        if let Some(t) = self.trace.as_deref_mut() {
            // Not the handles: a kept session's fixtures are not a new
            // one's, and the textures and fragments name them.
            t.push(format!(
                "{:?}\n{:?}\n{} fragments, {} textures, {} uploads, {} + {} dropped",
                list.quads,
                list.clips,
                list.fragments.len(),
                list.textures.len(),
                list.texture_pixels.len(),
                list.dropped_textures.len(),
                list.dropped_fragments.len()
            ));
        }
        if finite {
            for q in &list.quads {
                let r = q.rect;
                assert!(
                    [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()),
                    "a quad off the map from finite input: {q:?}"
                );
            }
        }
        // What the renderer draws a group by: every one closed and nested,
        // its markers naming each other over one picture of some size,
        // faded by a fade; none with the switch off.
        let mut open = Vec::new();
        for (i, q) in list.quads.iter().enumerate() {
            match q.kind {
                QuadKind::GroupBegin | QuadKind::GroupEnd if !groups => {
                    panic!("a group marker with groups off: {q:?}")
                }
                QuadKind::GroupBegin => open.push(i),
                QuadKind::GroupEnd => {
                    let b = open.pop().expect("a group ends that never began");
                    let begin = &list.quads[b];
                    assert_eq!(
                        (begin.uv[0] as usize, q.uv[0] as usize),
                        (i, b),
                        "the markers at {b} and {i} name other ends"
                    );
                    assert!(
                        begin.rect == q.rect && q.rect.w > 0.0 && q.rect.h > 0.0,
                        "a group's picture is {:?} / {:?}",
                        begin.rect,
                        q.rect
                    );
                    assert!(
                        q.color.a > 0.0 && q.color.a <= 1.0,
                        "a group faded by {}",
                        q.color.a
                    );
                }
                _ => {}
            }
        }
        assert!(open.is_empty(), "{} groups never end", open.len());
        if let Some(k) = self.edit {
            assert!(self.core.edit_text(k).is_some(), "the editor lost its text");
        }
    }

    fn node(&self, i: u16) -> Option<kui_core::NodeInfo> {
        let nodes = self.core.nodes();
        let n = nodes.len();
        nodes.into_iter().nth(i as usize % n.max(1))
    }

    fn input(&mut self, ev: InputEvent) {
        let _ = self.core.handle_input(ev);
    }

    fn point(&mut self, x: Coord, y: Coord) -> Vec2 {
        self.finite &= x.0.is_finite() && y.0.is_finite();
        Vec2::new(x.0, y.0)
    }

    fn apply(&mut self, action: &Action) {
        match action {
            Action::Aim(i, dx, dy) => {
                if let Some(n) = self.node(*i) {
                    let c = n.rect.center();
                    let at = Vec2::new(c.x + f32::from(*dx) / 4.0, c.y + f32::from(*dy) / 4.0);
                    self.finite &= at.x.is_finite() && at.y.is_finite();
                    self.input(InputEvent::CursorMoved(at));
                }
            }
            Action::Cursor(x, y) => {
                let p = self.point(*x, *y);
                self.input(InputEvent::CursorMoved(p));
            }
            Action::CursorLeft => self.input(InputEvent::CursorLeft),
            Action::Down(b, clicks) => self.input(InputEvent::MouseDown {
                button: button(*b),
                clicks: clicks % 4,
            }),
            Action::Up(b) => self.input(InputEvent::MouseUp { button: button(*b) }),
            Action::Scroll(x, y) => {
                let p = self.point(*x, *y);
                self.input(InputEvent::Scroll(p));
            }
            Action::Gesture(x, y, Gesture(bits)) => {
                let delta = self.point(*x, *y);
                let begins = bits & 1 != 0;
                if bits & 0x80 != 0 {
                    self.input(InputEvent::Wheel {
                        lines: Vec2::new(delta.x / 40.0, delta.y / 40.0),
                        begins,
                    });
                } else {
                    self.input(InputEvent::ScrollGesture { delta, begins });
                }
            }
            Action::Text(s) => self.input(InputEvent::Text(s.clone())),
            Action::Commit(s) => self.input(InputEvent::Commit(s.clone())),
            Action::Paste(s, marks) => self.input(InputEvent::Paste {
                text: s.clone(),
                marks: ClipboardMarks::from_bits(u32::from(*marks)),
            }),
            Action::Preedit(s, range) => self.input(InputEvent::Preedit(
                s.clone(),
                range.map(|(a, b)| (usize::from(a), usize::from(b))),
            )),
            Action::Key(k, m) => self.input(InputEvent::Key(
                EditKey::ALL[*k as usize % EditKey::ALL.len()],
                Mods {
                    shift: m & 1 != 0,
                    word: m & 2 != 0,
                    doc: m & 4 != 0,
                },
            )),
            Action::KeyDown(p) => self.input(InputEvent::KeyDown(press(p))),
            Action::KeyUp(p) => self.input(InputEvent::KeyUp(press(p))),
            Action::Modifiers(m) => {
                self.input(InputEvent::Modifiers(KeyMods::from_bits(u32::from(*m))))
            }
            Action::ForceClick(x, y) => {
                let p = self.point(*x, *y);
                self.input(InputEvent::ForceClick(p));
            }
            Action::DragFiles(n, x, y) => {
                let at = self.point(*x, *y);
                self.input(InputEvent::DragFiles {
                    paths: conformance::drop_paths(u32::from(n % 8)),
                    at,
                });
            }
            Action::DropFiles(n, x, y) => {
                let at = self.point(*x, *y);
                self.input(InputEvent::DropFiles {
                    paths: conformance::drop_paths(u32::from(n % 8)),
                    at,
                });
            }
            Action::DragCancel => self.input(InputEvent::DragCancel),
            Action::Files(n) => {
                self.input(InputEvent::Files(conformance::drop_paths(u32::from(n % 8))))
            }
            Action::Open(n) => {
                self.input(InputEvent::Open(conformance::drop_paths(u32::from(n % 8))))
            }
            Action::Access {
                node,
                action,
                value,
                selection,
            } => {
                let Some(n) = self.node(*node) else {
                    return;
                };
                let mut req = AccessRequest::new(
                    n.key,
                    AccessAction::ALL[*action as usize % AccessAction::ALL.len()],
                );
                req.value = value.clone();
                if let Some((a, f)) = selection {
                    // The run is the node itself: what a screen reader
                    // names for a one-run editor.
                    req.anchor = Some(TextPos {
                        run: n.key,
                        character: usize::from(*a),
                    });
                    req.focus = Some(TextPos {
                        run: n.key,
                        character: usize::from(*f),
                    });
                }
                self.input(InputEvent::Access(req));
            }
            Action::Resize(w, h, s) => {
                self.size = Size::new(f32::from(*w) / 16.0, f32::from(*h) / 16.0);
                self.scale = SCALES[*s as usize % SCALES.len()];
            }
            Action::Time(ms) => {
                self.now += f64::from(*ms) / 1000.0;
                self.core.set_time(self.now);
            }
            Action::Phase(p) => {
                self.phase = u32::from(p % 4);
                self.core.set_opacity_groups(p & 4 != 0);
            }
            Action::Appearance(a) => {
                let all = kui_core::Appearance::ALL;
                let system = kui_core::SystemEnv {
                    appearance: all[*a as usize % all.len()],
                    ..self.core.env.system
                };
                self.core.set_system(system);
            }
            Action::WindowClosed(id) => self.core.window_closed(WindowId(u32::from(id % 4))),
            Action::Dismissed(id, escape) => self.core.dismiss_window(
                WindowId(u32::from(id % 4)),
                if *escape {
                    DismissReason::Escape
                } else {
                    DismissReason::Outside
                },
            ),
            Action::SetText(s) => {
                if let Some(k) = self.edit {
                    self.core.set_edit_text(k, s);
                }
            }
            Action::Frame => {}
        }
    }
}

/// The editor scene: one field, its options the fuzzer's, under a label so
/// a second field (phase 1 and up) and a button around it give focus
/// somewhere to go.
fn editor(ui: &mut Ui<'_>, e: &Editor, phase: u32) -> Key {
    ui.configure_root(NodeSpec::column().fill().pad(8.0).gap(6.0));
    let size = SIZES[e.size as usize % SIZES.len()];
    // A long line from the size byte's top bits, above the password's:
    // the text repeated past `LONG_LINE_BYTES`, so both views shape it in
    // chunks and cut it from the back (backlog C42, C57). A byte under 192
    // reads as it always did.
    let long;
    let initial = if e.size >= 192 && !e.initial.is_empty() {
        long = e.initial.repeat(LONG_LINE_BYTES / e.initial.len() + 2);
        &long
    } else {
        &e.initial
    };
    // The same string as a paragraph: shaped, wrapped and selectable
    // without an editor's caret in the way, its first letter dropped
    // through up to five lines by the size byte's bits [`SIZES`] leaves
    // (backlog FZ8) — none for every input kept before they were read.
    let drop = u32::from(e.size / SIZES.len() as u8 % 6);
    ui.text(initial, TextStyle::new(size).initial_letter(drop));
    // A password field, drawn as bullets, from the bits above those
    // (backlog F167): a size byte under 48 reads as it always did.
    let secure = e.size / (SIZES.len() as u8 * 6) % 2 == 1;
    let opts = EditOptions {
        style: TextStyle::new(size),
        multiline: e.multiline,
        wrap: e.wrap,
        secure,
        autofocus: true,
        ..Default::default()
    };
    let spec = if e.width == 0 {
        NodeSpec::row().grow_width().pad(4.0)
    } else {
        NodeSpec::row().width(f32::from(e.width)).pad(4.0)
    };
    let key = ui.text_edit("field", initial, &opts, spec);
    if phase > 0 {
        ui.text_edit("second", "", &opts, NodeSpec::row().grow_width());
    }
    kui_core::widgets::button(ui, "Done", "done");
    key
}

fn button(b: Button) -> MouseButton {
    match b {
        Button::Primary => MouseButton::Primary,
        Button::Secondary => MouseButton::Secondary,
        Button::Middle => MouseButton::Middle,
        Button::Other(n) => MouseButton::Other(n),
    }
}

fn press(p: &Press) -> KeyPress {
    let code = match p.code {
        Code::Char(c) => KeyCode::Char(c),
        // `F(1)` .. `F(35)`, the range the type names.
        Code::F(n) => KeyCode::F(1 + n % 35),
        Code::Named(i) => {
            let named = KeyCode::named();
            named[i as usize % named.len()].0
        }
    };
    let mut k = KeyPress::new(code, KeyMods::from_bits(u32::from(p.mods)));
    k.text = p.text.clone();
    k.repeat = p.repeat;
    k.location = match p.location % 4 {
        0 => KeyLocation::Standard,
        1 => KeyLocation::Left,
        2 => KeyLocation::Right,
        _ => KeyLocation::Numpad,
    };
    k.locks = KeyLocks {
        caps: p.caps,
        num: false,
    };
    k
}
