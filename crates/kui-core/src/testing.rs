//! The headless driver the crate's own tests drive a [`Core`] with. Test
//! infrastructure, behind the `conformance` feature like the corpus, so no
//! shipped binary carries it; the self dev-dependency turns it on for
//! `cargo test`.
//!
//! Every integration test used to re-derive these at the shallowest
//! interface there is — three `handle_input` calls per click, in nine
//! named copies and seventy-odd inline triples — so a change to the click
//! protocol was a hunt through the test corpus. These are the verbs once:
//! input as a driver sends it, and the two readbacks every test wants (the
//! `kind` of each event, the quads of one kind). They are plain functions
//! over a borrowed `Core` rather than a struct, since a test already holds
//! the core and builds its own frames.

use crate::display::{Quad, QuadKind};
use crate::geom::Vec2;
use crate::input::{EditKey, InputEvent, Mods, UiEvent};
use crate::runtime::Core;
use crate::value::Value;

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

/// The `kind` of each event, in order: what a payload map's `kind` says,
/// a bare string payload as itself, and `"?"` for anything else.
pub fn kinds(evs: &[UiEvent]) -> Vec<&str> {
    evs.iter()
        .map(|e| match e.payload.get("kind").and_then(Value::as_str) {
            Some(k) => k,
            None => e.payload.as_str().unwrap_or("?"),
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
