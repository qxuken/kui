//! `kui.input`: what a window would get from the OS, sent into a headless
//! run (`kui run --headless`) — a key, the pointer, a click, typed text —
//! so a test drives the app as a user would, through the core's routing,
//! the Tab ring and hit testing, rather than dispatching the message a
//! handler would have posted. Each call is handled before the run's next
//! frame, in the order made. A windowed run has the user for this, and
//! the calls say so.

use std::sync::OnceLock;

use kui_native::{HeadlessInput, InputEvent, KeyCode, KeyMods, KeyPress, Vec2};
use mlua::{Lua, Table};

static INPUT: OnceLock<HeadlessInput> = OnceLock::new();

/// The headless runner's input, set once it is open.
pub fn set(input: HeadlessInput) {
    let _ = INPUT.set(input);
}

fn input() -> mlua::Result<&'static HeadlessInput> {
    INPUT.get().ok_or_else(|| {
        mlua::Error::runtime("kui.input drives a headless run only (kui run --headless)")
    })
}

fn mods(t: Option<Table>) -> mlua::Result<KeyMods> {
    let Some(t) = t else {
        return Ok(KeyMods::NONE);
    };
    let on = |k: &str| t.get::<Option<bool>>(k).map(|v| v.unwrap_or(false));
    Ok(KeyMods {
        shift: on("shift")?,
        ctrl: on("ctrl")?,
        alt: on("alt")?,
        super_key: on("super")?,
    })
}

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let t = lua.create_table()?;
    // `key(name, {shift=, ctrl=, alt=, super=})`: pressed and released, by
    // the name `on_key` reports it under ("tab", "down", "a", "f2"), as a
    // keyboard sends it — the raw press (with its character when no chord
    // modifier is held), the editor event it means, the release.
    t.set(
        "key",
        lua.create_function(|_, (name, m): (String, Option<Table>)| {
            let input = input()?;
            let code = KeyCode::from_name(&name).ok_or_else(|| {
                mlua::Error::runtime(format!("kui.input.key: no key named {name:?}"))
            })?;
            let m = mods(m)?;
            let mut press = KeyPress::new(code, m);
            let chord = m.ctrl || m.alt || m.super_key;
            match code {
                KeyCode::Char(c) if !chord => press = press.with_text(c.to_string()),
                KeyCode::Space if !chord => press = press.with_text(" "),
                _ => {}
            }
            input.send(InputEvent::KeyDown(press.clone()));
            if let Some(ev) = press.edit_event() {
                input.send(ev);
            }
            input.send(InputEvent::KeyUp(press.released()));
            Ok(())
        })?,
    )?;
    // `move(x, y)`: the pointer to a point of the window, nothing pressed.
    t.set(
        "move",
        lua.create_function(|_, (x, y): (f32, f32)| {
            input()?.send(InputEvent::CursorMoved(Vec2::new(x, y)));
            Ok(())
        })?,
    )?;
    // `click(x, y)`: the pointer there, the primary button pressed and
    // released.
    t.set(
        "click",
        lua.create_function(|_, (x, y): (f32, f32)| {
            let input = input()?;
            input.send(InputEvent::CursorMoved(Vec2::new(x, y)));
            input.send(InputEvent::mouse_down(1));
            input.send(InputEvent::mouse_up());
            Ok(())
        })?,
    )?;
    // `text(s)`: typed text, as the OS delivers it to the focused editor.
    t.set(
        "text",
        lua.create_function(|_, s: String| {
            input()?.send(InputEvent::Text(s));
            Ok(())
        })?,
    )?;
    kui.set("input", t)
}
