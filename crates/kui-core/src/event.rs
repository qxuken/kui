//! Typed readings of the core's own event payloads (backlog DX7).
//!
//! A payload is a [`Value`] because it crosses into Lua, Node and C in
//! one shape (`schema::EVENTS` is its spelling). A Rust handler read it
//! back field by field — `get_f32("x")`, `get_str("phase")`, a `KeyPress`
//! rebuilt from nine keys — which is the reading done once here instead.
//! Each view is `None` for an event of another kind, so a handler
//! matches on the view it wants:
//!
//! ```ignore
//! if let Some(d) = ev.drag() {
//!     self.split = d.ratio().x;
//! } else if let Some((KeyPhase::Down, k)) = ev.key_press() {
//!     self.bind(k);
//! }
//! ```
//!
//! The app's own tag is still [`UiEvent::message`]; these are the fields
//! the core adds around it. The payload stays the wire, and nothing here
//! changes what an event carries.

use crate::geom::{Rect, Vec2};
use crate::input::{KeyCode, KeyMods, KeyPhase, KeyPress, UiEvent};
use crate::value::Value;

/// Which part of a drag an event reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragPhase {
    Start,
    Move,
    End,
}

/// A `{kind:"drag"}` event: an `onDrag` node's pointer capture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub phase: DragPhase,
    /// Where the pointer is.
    pub pos: Vec2,
    /// How far it is from where it pressed — in every phase, so a value
    /// is `start + delta`, never a sum.
    pub delta: Vec2,
    /// The dragged node's parent's rect, for fractions without a query.
    pub parent: Rect,
    /// On a `cells` grid: the `(row, col)` under the pointer.
    pub cell: Option<(u32, u32)>,
    /// Inside an `onKey` sink that draws `role="line"` rows: the line, the
    /// byte in its text, and the press's click count.
    pub line: Option<u32>,
    pub byte: Option<usize>,
    pub clicks: Option<u32>,
}

impl Drag {
    /// The pointer's place across the parent, 0 at its left or top edge
    /// and 1 at its right or bottom, clamped: a divider's split, a
    /// slider's value. 0 on an axis the parent has no size on.
    pub fn ratio(&self) -> Vec2 {
        let along = |p: f32, at: f32, len: f32| {
            if len > 0.0 {
                ((p - at) / len).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        Vec2::new(
            along(self.pos.x, self.parent.x, self.parent.w),
            along(self.pos.y, self.parent.y, self.parent.h),
        )
    }
}

/// A `{kind:"scroll"}` event: the wheel over an `onScroll` node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroll {
    pub pos: Vec2,
    pub delta: Vec2,
    /// On a `cells` grid: whole rows the wheel moved, the remainder
    /// carried to the next event; later history is negative.
    pub lines: Option<i64>,
}

/// Which half of a hover an event reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverPhase {
    Enter,
    Leave,
}

/// A `{kind:"layout"}` event: an `onLayout` node's placed rect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub rect: Rect,
    pub parent: Rect,
    pub scale: f32,
}

/// A `{kind:"text"}` event: what a focused key sink or editor was given
/// to insert.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextInput<'a> {
    pub text: &'a str,
    /// The clipboard's answer to a paste the app asked for, not typing.
    pub pasted: bool,
    /// Typed under secure keyboard entry: keep it out of logs and history.
    pub concealed: bool,
    /// A clipboard manager or a password tool marked it as not for keeping.
    pub transient: bool,
}

fn point(p: &Value) -> Option<Vec2> {
    Some(Vec2::new(p.get_f32("x")?, p.get_f32("y")?))
}

fn rect(p: &Value) -> Option<Rect> {
    Some(Rect::new(
        p.get_f32("x")?,
        p.get_f32("y")?,
        p.get_f32("w")?,
        p.get_f32("h")?,
    ))
}

fn small(p: &Value, key: &str) -> Option<u32> {
    p.get_int(key).and_then(|v| u32::try_from(v).ok())
}

impl UiEvent {
    /// The app's tag inside a core event's payload — `onDrag`'s, `onKey`'s
    /// — as sent; [`UiEvent::message`] reads it typed.
    pub fn tag(&self) -> Option<&Value> {
        self.payload.get("tag")
    }

    /// This event as a drag, if it is one.
    pub fn drag(&self) -> Option<Drag> {
        let p = &self.payload;
        if self.kind()? != "drag" {
            return None;
        }
        let phase = match p.get_str("phase")? {
            "start" => DragPhase::Start,
            "move" => DragPhase::Move,
            "end" => DragPhase::End,
            _ => return None,
        };
        let cell = p
            .get("cell")
            .and_then(|c| Some((small(c, "row")?, small(c, "col")?)));
        Some(Drag {
            phase,
            pos: point(p)?,
            delta: Vec2::new(p.get_f32("dx")?, p.get_f32("dy")?),
            parent: p.get("parent").and_then(rect).unwrap_or_default(),
            cell,
            line: small(p, "line"),
            byte: p.get_int("byte").and_then(|v| usize::try_from(v).ok()),
            clicks: small(p, "clicks"),
        })
    }

    /// This event as a key press or release on a key sink, if it is one
    /// (`key_press`, since `key` is the node the event is about):
    /// the [`KeyPress`] the core built it from, so a keymap binds the same
    /// value a headless `press` sends.
    pub fn key_press(&self) -> Option<(KeyPhase, KeyPress)> {
        let p = &self.payload;
        if self.kind()? != "key" {
            return None;
        }
        let phase = match p.get_str("phase")? {
            "down" => KeyPhase::Down,
            "up" => KeyPhase::Up,
            _ => return None,
        };
        let code = KeyCode::from_name(p.get_str("code")?)?;
        let physical = p
            .get_str("physical")
            .and_then(KeyCode::from_name)
            .unwrap_or(KeyCode::Unknown);
        let press = KeyPress {
            code,
            physical,
            mods: mods(p),
            text: p.get_str("text").map(str::to_string),
            repeat: p.get_bool("repeat").unwrap_or(false),
        };
        Some((phase, press))
    }

    /// This event as inserted text, if it is some.
    pub fn text(&self) -> Option<TextInput<'_>> {
        let p = &self.payload;
        if self.kind()? != "text" {
            return None;
        }
        Some(TextInput {
            text: p.get_str("text")?,
            pasted: p.get_bool("pasted").unwrap_or(false),
            concealed: p.get_bool("concealed").unwrap_or(false),
            transient: p.get_bool("transient").unwrap_or(false),
        })
    }

    /// This event as a wheel scroll, if it is one.
    pub fn scroll(&self) -> Option<Scroll> {
        let p = &self.payload;
        if self.kind()? != "scroll" {
            return None;
        }
        Some(Scroll {
            pos: point(p)?,
            delta: Vec2::new(p.get_f32("dx")?, p.get_f32("dy")?),
            lines: p.get_int("lines"),
        })
    }

    /// This event as a hover edge, if it is one.
    pub fn hover(&self) -> Option<HoverPhase> {
        if self.kind()? != "hover" {
            return None;
        }
        match self.payload.get_str("phase")? {
            "enter" => Some(HoverPhase::Enter),
            "leave" => Some(HoverPhase::Leave),
            _ => None,
        }
    }

    /// This event as a modifier change, if it is one: the keys now held.
    pub fn modifiers(&self) -> Option<KeyMods> {
        if self.kind()? != "modifiers" {
            return None;
        }
        Some(mods(&self.payload))
    }

    /// This event as an `onLayout` report, if it is one.
    pub fn layout(&self) -> Option<Layout> {
        let p = &self.payload;
        if self.kind()? != "layout" {
            return None;
        }
        Some(Layout {
            rect: rect(p)?,
            parent: p.get("parent").and_then(rect).unwrap_or_default(),
            scale: p.get_f32("scale").unwrap_or(1.0),
        })
    }
}

/// The four modifier flags a `key` and a `modifiers` payload both carry.
fn mods(p: &Value) -> KeyMods {
    let on = |k| p.get_bool(k).unwrap_or(false);
    KeyMods {
        shift: on("shift"),
        ctrl: on("ctrl"),
        alt: on("alt"),
        super_key: on("super"),
    }
}
