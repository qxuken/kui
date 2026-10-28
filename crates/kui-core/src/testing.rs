//! Headless input helpers for driving a [`Core`] in tests: clicks, key
//! presses and the common readbacks. Test infrastructure, behind the
//! `conformance` feature (off by default); the crate's own tests and
//! `kui-lua`'s use it, nothing in a shipped app does.
//!
//! They are plain functions over a borrowed `Core`, since a test already
//! holds the core and builds its own frames: [`click`](crate::testing::click)
//! is the move-press-release triple a driver sends,
//! [`tab`](crate::testing::tab) a Tab press, [`kinds`](crate::testing::kinds)
//! the `kind` of each event a drive produced,
//! [`solids`](crate::testing::solids) the solid quads of the last frame, and
//! the `*_face` functions synthesize font fixtures.

use crate::display::{Quad, QuadKind};
use crate::geom::Vec2;
use crate::input::{EditKey, InputEvent, KeyCode, KeyMods, KeyPress, Mods, UiEvent};
use crate::runtime::Core;

/// A primary click at `at`: the pointer moves there, presses once and
/// releases. Every event the three inputs produced, in order — a test
/// that wants only the release's reads the tail.
pub fn click(core: &mut Core, at: Vec2) -> Vec<UiEvent> {
    let mut out = core.handle_input(InputEvent::CursorMoved(at));
    out.extend(core.handle_input(InputEvent::mouse_down(1)));
    out.extend(core.handle_input(InputEvent::mouse_up()));
    out
}

/// [`click`] by coordinates.
pub fn click_at(core: &mut Core, x: f32, y: f32) -> Vec<UiEvent> {
    click(core, Vec2::new(x, y))
}

/// The pointer moves to `at` and presses, and stays down: the start of a
/// drag. [`release`] ends it.
pub fn press(core: &mut Core, at: Vec2) -> Vec<UiEvent> {
    let mut out = core.handle_input(InputEvent::CursorMoved(at));
    out.extend(core.handle_input(InputEvent::mouse_down(1)));
    out
}

/// The primary button released where the pointer is.
pub fn release(core: &mut Core) -> Vec<UiEvent> {
    core.handle_input(InputEvent::mouse_up())
}

/// The pointer moves to `at`, nothing pressed.
pub fn hover(core: &mut Core, at: Vec2) -> Vec<UiEvent> {
    core.handle_input(InputEvent::CursorMoved(at))
}

/// A Tab (or Shift-Tab) on the editor channel — the one every driver
/// sends from `KeyPress::edit_event`, and the one the Tab ring reads.
pub fn tab(core: &mut Core, shift: bool) -> Vec<UiEvent> {
    edit_key(
        core,
        EditKey::Tab,
        Mods {
            shift,
            ..Mods::default()
        },
    )
}

/// An editing key on the editor channel.
pub fn edit_key(core: &mut Core, key: EditKey, mods: Mods) -> Vec<UiEvent> {
    core.handle_input(InputEvent::Key(key, mods))
}

/// Feeds every input in turn; every event they produced, in order.
pub fn drive(core: &mut Core, events: &[InputEvent]) -> Vec<UiEvent> {
    let mut out = Vec::new();
    for ev in events {
        out.extend(core.handle_input(ev.clone()));
    }
    out
}

/// Which event each was, in order: the envelope's [`UiEvent::event`] by
/// its wire name (`"drag"`, `"click"`, `"change"`).
pub fn kinds(evs: &[UiEvent]) -> Vec<&'static str> {
    evs.iter().map(|e| e.event.name()).collect()
}

/// The app's message of each event, in order, by what it names: a bare
/// string message as itself, a map's `kind`, the string it was kept
/// under `tag` as, else `"?"` — the reading for "which message came
/// back", where [`kinds`] is "what happened".
pub fn messages(evs: &[UiEvent]) -> Vec<&str> {
    evs.iter()
        .map(|e| {
            e.payload
                .as_str()
                .or_else(|| e.payload.get_str("kind"))
                .or_else(|| e.payload.get_str("tag"))
                .unwrap_or("?")
        })
        .collect()
}

/// The code of each warning, in order.
pub fn codes(ws: &[crate::diag::Warning]) -> Vec<&'static str> {
    ws.iter().map(|w| w.code).collect()
}

/// The last frame's quads of one kind, in emission order.
pub fn quads_of(core: &mut Core, kind: QuadKind) -> Vec<Quad> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == kind)
        .cloned()
        .collect()
}

/// The last frame's solid quads (fills and borders), in emission order.
pub fn solids(core: &mut Core) -> Vec<Quad> {
    quads_of(core, QuadKind::Solid)
}

/// A raw key going down, unmodified — `InputEvent::KeyDown`, the keymap
/// channel, not the editing one [`edit_key`] drives — as the event, for a
/// [`drive`] batch.
pub fn key_press(code: KeyCode) -> InputEvent {
    InputEvent::KeyDown(KeyPress::new(code, KeyMods::default()))
}

/// The same key coming back up, as the event.
pub fn key_release(code: KeyCode) -> InputEvent {
    InputEvent::KeyUp(KeyPress::new(code, KeyMods::default()))
}

/// [`key_press`] with modifiers, sent: what the focused sink hears.
pub fn key_down(core: &mut Core, code: KeyCode, mods: KeyMods) -> Vec<UiEvent> {
    core.handle_input(InputEvent::KeyDown(KeyPress::new(code, mods)))
}

/// [`key_release`] with modifiers, sent.
pub fn key_up(core: &mut Core, code: KeyCode, mods: KeyMods) -> Vec<UiEvent> {
    core.handle_input(InputEvent::KeyUp(KeyPress::new(code, mods)))
}

/// What each event names, in order: a bare string payload as itself, else
/// its map's `tag` when that is a string. Events with neither are skipped
/// — the reading a test of "which node heard it" wants, where [`kinds`]
/// is the reading for "what happened".
pub fn tags(evs: &[UiEvent]) -> Vec<&str> {
    evs.iter()
        .filter_map(|e| {
            e.payload
                .as_str()
                .or_else(|| e.payload.get("tag")?.as_str())
        })
        .collect()
}

/// A font file the tests can register without one being installed: a
/// TrueType face, family **"Kui Liga"**, that maps every
/// printable ASCII character to a filled square and carries one `liga`
/// ligature — `f` followed by `i` shapes as one wider glyph unless the
/// feature is off. Built here rather than checked in as bytes, so the
/// tables are readable and the fixture cannot rot into a blob nobody can
/// regenerate; ~1.5 KB, built in microseconds.
///
/// Five glyphs: `.notdef` (empty), the square, `f`, `i` and `fi`; 1000
/// units per em, ascender 800, descender −200, every advance 500 but the
/// ligature's 700. The GSUB has one lookup, `liga` under `DFLT` and
/// `latn`, so the shaper applies it by default the way a real font's is.
pub fn liga_font() -> Vec<u8> {
    crate::stock_face::build(&crate::stock_face::Face::LIGA)
}

/// The [`liga_font`] fixture as a face of another family — `family`, at
/// `weight` on the CSS scale (the `OS/2` table's `usWeightClass`), italic
/// or upright, and saying it is fixed-pitch or not (the `post` table's
/// `isFixedPitch`) — so a test can stock a font database with families
/// whose faces differ in what the database reads off them.
/// The glyphs are the fixture's whatever the flag says.
pub fn font_face(family: &str, weight: u16, italic: bool, fixed_pitch: bool) -> Vec<u8> {
    crate::stock_face::build(&crate::stock_face::Face {
        family,
        weight,
        italic,
        fixed_pitch,
        ..crate::stock_face::Face::LIGA
    })
}

/// The [`liga_font`] fixture as a proportional face of `family` that maps
/// 字 (U+5B57) to its square besides printable ASCII: a face Han text can
/// fall back to, measured like any other.
pub fn han_face(family: &str) -> Vec<u8> {
    crate::stock_face::build(&crate::stock_face::Face {
        family,
        han: true,
        ..crate::stock_face::Face::LIGA
    })
}

/// The [`han_face`] fixture as a fixed-pitch face whose 字 is 700 units
/// wide, not 500: a monospaced face with 字 that a test tells apart from a
/// [`han_face`] by the advance alone.
pub fn mono_han_face(family: &str) -> Vec<u8> {
    crate::stock_face::build(&crate::stock_face::Face {
        family,
        fixed_pitch: true,
        han: true,
        wide_han: true,
        ..crate::stock_face::Face::LIGA
    })
}

/// The [`han_face`] fixture with 字 its one character: a proportional
/// face of `family` with no `M`, as a CJK-only or symbols-only family has
/// none.
pub fn han_only_face(family: &str) -> Vec<u8> {
    crate::stock_face::build(&crate::stock_face::Face {
        family,
        han: true,
        ascii: false,
        ..crate::stock_face::Face::LIGA
    })
}

/// A face whose glyph advances cannot be measured: the
/// [`han_face`] fixture, fixed-pitch, without its `head`, `hhea` and
/// `hmtx` tables — no units per em, no horizontal metrics — and without
/// its outlines, the way macOS's GB18030 Bitmap has none of them (it
/// carries Apple's `bhed` and bitmap tables instead, and this carries no
/// bitmaps either). The font database takes it, since its records come
/// from `name`, `OS/2` and `post`; the shaper reads its units per em as 0
/// and so its every advance as infinite.
pub fn unmeasurable_face(family: &str) -> Vec<u8> {
    crate::stock_face::build(&crate::stock_face::Face {
        family,
        fixed_pitch: true,
        han: true,
        metrics: false,
        ..crate::stock_face::Face::LIGA
    })
}

/// A variable face of `family`: the [`liga_font`] fixture,
/// fixed-pitch, whose `OS/2` says weight 400 and which carries a `wght`
/// axis from `wght[0]` to `wght[2]`, its default at `wght[1]`, with the
/// named instances `instances` (subfamily name, coordinate). At the axis's
/// maximum every square's right edge is 100 units further right (`gvar`),
/// so a glyph drawn at the heavy end is 500 units wide where the default's
/// is 400; the advances stay 500. `[100, 100, 150]` with "Regular" at 100
/// and "Bold" at 150 is Berkeley Mono Variable's axis, which is not on the
/// CSS scale its `OS/2` weight is.
pub fn variable_face(
    family: &str,
    italic: bool,
    wght: [u16; 3],
    instances: &[(&str, u16)],
) -> Vec<u8> {
    crate::stock_face::build(&crate::stock_face::Face {
        family,
        italic,
        fixed_pitch: true,
        wght: Some(crate::stock_face::Wght {
            axis: wght,
            instances,
        }),
        ..crate::stock_face::Face::LIGA
    })
}
