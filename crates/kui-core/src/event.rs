//! Typed readings of the core's own event data: drags, buttons, scrolls,
//! hovers, layouts, key presses and text input.
//!
//! An event's payload is the message its handler declared, with the
//! event's data in one field named for the event (ADR 0047). The payload
//! is a [`Value`] because it crosses into Lua, Node and C in one shape.
//! Rather than reading that field back by name (`get("drag")`, then
//! `get_f32("x")`), a Rust handler asks for the view it wants; each view
//! is `None` for an event of another kind.
//!
//! ```rust
//! use kui_core::{EventKind, Key, OriginId, UiEvent, Value};
//! use kui_core::event::DragPhase;
//!
//! // What an `on_drag("splitter")` node emits as the pointer moves.
//! let ev = UiEvent::declared(
//!     EventKind::Drag,
//!     OriginId::HOST,
//!     Key::ROOT,
//!     Some(&Value::map([("kind", Value::str("splitter"))])),
//!     Value::map([
//!         ("phase", Value::str("move")),
//!         ("x", Value::float(40.0)), ("y", Value::float(12.0)),
//!         ("dx", Value::float(8.0)), ("dy", Value::float(0.0)),
//!     ]),
//! );
//!
//! let d = ev.drag().expect("a drag");
//! assert_eq!(d.phase, DragPhase::Move);
//! assert_eq!(d.delta.x, 8.0);
//! assert_eq!(ev.payload.get_str("kind"), Some("splitter"));
//! assert!(ev.scroll().is_none()); // another kind reads as None
//! ```
//!
//! The app's own message is [`UiEvent::message`]; these are the fields the
//! core writes into it. Nothing here changes what an event carries.
//!
//! Each type a node event's data reads as is also a
//! [`MessageField`] and an [`EventField`] naming the field it rides under
//! ([`Drag`] under `drag`, [`EditChange`] and [`SliderChange`] both under
//! `change`), so a message type can hold the data as a field of its own:
//! that is what `#[derive(Message)]`'s `#[message(on = drag)]` builds a
//! message's arriving twin from (ADR 0047, decision 13).

use crate::geom::{Rect, Vec2};
pub use crate::input::KeyMods;
use crate::input::{EventKind, KeyCode, KeyPhase, KeyPress, MouseButton, UiEvent};
use crate::message::{EventField, MessageField};
use crate::value::Value;

// The data types of a window's `modifiers` and `system` (ADR 0048), here
// with the rest so a derived twin names every event's from this module.
pub use crate::env::SystemEnv;

/// Which part of a drag an event reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragPhase {
    Start,
    Move,
    End,
}

/// A `drag`: an `onDrag` node's pointer capture.
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

/// Which part of a held non-primary button an event reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonPhase {
    Press,
    Move,
    Release,
}

/// A `button`: a non-primary button an `onButton` node
/// claimed, captured by it from press to release.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonEvent {
    pub phase: ButtonPhase,
    pub button: MouseButton,
    /// Where the pointer is.
    pub pos: Vec2,
    /// The driver's click count, on the press only.
    pub clicks: Option<u32>,
    /// On a `cells` grid: the `(row, col)` under the pointer, clamped to
    /// the grid.
    pub cell: Option<(u32, u32)>,
    /// Inside an `onKey` sink that draws `role="line"` rows: the line and
    /// the byte in its text, as a drag carries them.
    pub line: Option<u32>,
    pub byte: Option<usize>,
}

/// A `scroll`: the wheel over an `onScroll` node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroll {
    pub pos: Vec2,
    pub delta: Vec2,
    /// On a `cells` grid: whole rows the wheel moved, the remainder
    /// carried to the next event; later history is negative.
    pub lines: Option<i64>,
    /// On a node that names `scroll_mods`: the modifiers held when the
    /// gesture began.
    pub mods: Option<KeyMods>,
}

/// Which half of a hover an event reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverPhase {
    Enter,
    Leave,
}

/// What moved to change the hover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverBy {
    /// The pointer moved, or left the window.
    Pointer,
    /// The pointer stayed and what is under it changed: a list scrolled
    /// by the wheel or the keys, a row that grew, a float that opened. A
    /// picker that follows the pointer ignores these.
    Content,
}

/// A `hover`: an `onHover` node entered or left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hover {
    pub phase: HoverPhase,
    pub by: HoverBy,
}

/// A `layout`: an `onLayout` node's placed rect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub rect: Rect,
    pub parent: Rect,
    pub scale: f32,
}

/// A `range`: the least and greatest data index in a node's view (ADR 0065).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowRange {
    pub first: u64,
    pub last: u64,
}

/// A `query`: which of a node's `when` entries match, as a bit set over
/// the list, and the node's size (ADR 0069).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryChange {
    pub matched: u64,
    pub width: f32,
    pub height: f32,
}

fn read_query(p: &Value) -> Option<QueryChange> {
    let mut matched = 0u64;
    for i in p.get("matched")?.as_list()? {
        let i = i.as_int()?;
        if (0..64).contains(&i) {
            matched |= 1 << i;
        }
    }
    Some(QueryChange {
        matched,
        width: p.get_f32("width")?,
        height: p.get_f32("height")?,
    })
}

fn read_range(p: &Value) -> Option<RowRange> {
    Some(RowRange {
        first: p.get("first")?.as_int()? as u64,
        last: p.get("last")?.as_int()? as u64,
    })
}

/// A `text`: what a focused key sink was given to insert.
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
    /// This event as a drag, if it is one.
    pub fn drag(&self) -> Option<Drag> {
        if !self.is(EventKind::Drag) {
            return None;
        }
        read_drag(self.data()?)
    }

    /// This event as a held non-primary button's press, move or release,
    /// if it is one.
    pub fn button(&self) -> Option<ButtonEvent> {
        if !self.is(EventKind::Button) {
            return None;
        }
        read_button(self.data()?)
    }

    /// This event as a key press or release on a key sink, if it is one
    /// (`key_press`, since `key` is the node the event is about):
    /// the [`KeyPress`] the core built it from, so a keymap binds the same
    /// value a headless `press` sends.
    pub fn key_press(&self) -> Option<(KeyPhase, KeyPress)> {
        if !self.is(EventKind::Key) {
            return None;
        }
        read_key(self.data()?).map(|k| (k.phase, k.press))
    }

    /// This event as inserted text, if it is some.
    pub fn text(&self) -> Option<TextInput<'_>> {
        if !self.is(EventKind::Text) {
            return None;
        }
        let p = self.data()?;
        Some(TextInput {
            text: p.get_str("text")?,
            pasted: p.get_bool("pasted").unwrap_or(false),
            concealed: p.get_bool("concealed").unwrap_or(false),
            transient: p.get_bool("transient").unwrap_or(false),
        })
    }

    /// This event as a wheel scroll, if it is one.
    pub fn scroll(&self) -> Option<Scroll> {
        if !self.is(EventKind::Scroll) {
            return None;
        }
        read_scroll(self.data()?)
    }

    /// This event as a hover edge, if it is one.
    pub fn hover(&self) -> Option<Hover> {
        if !self.is(EventKind::Hover) {
            return None;
        }
        read_hover(self.data()?)
    }

    /// This event as a modifier change, if it is one: the keys now held.
    pub fn modifiers(&self) -> Option<KeyMods> {
        if !self.is(EventKind::Modifiers) {
            return None;
        }
        let p = self.data()?;
        Some(mods(p))
    }

    /// This event as an `onLayout` report, if it is one.
    pub fn layout(&self) -> Option<Layout> {
        if !self.is(EventKind::Layout) {
            return None;
        }
        read_layout(self.data()?)
    }
}

// What each event's data reads as, from the data map alone: the views
// above, `UiEvent::read` and each type's `MessageField` (the arriving
// twin's field) share them, so the three cannot disagree.

fn cell_of(p: &Value) -> Option<(u32, u32)> {
    p.get("cell")
        .and_then(|c| Some((small(c, "row")?, small(c, "col")?)))
}

fn byte_of(p: &Value) -> Option<usize> {
    p.get_int("byte").and_then(|v| usize::try_from(v).ok())
}

fn read_drag(p: &Value) -> Option<Drag> {
    let phase = match p.get_str("phase")? {
        "start" => DragPhase::Start,
        "move" => DragPhase::Move,
        "end" => DragPhase::End,
        _ => return None,
    };
    Some(Drag {
        phase,
        pos: point(p)?,
        delta: Vec2::new(p.get_f32("dx")?, p.get_f32("dy")?),
        parent: p.get("parent").and_then(rect).unwrap_or_default(),
        cell: cell_of(p),
        line: small(p, "line"),
        byte: byte_of(p),
        clicks: small(p, "clicks"),
    })
}

fn read_button(p: &Value) -> Option<ButtonEvent> {
    let phase = match p.get_str("phase")? {
        "press" => ButtonPhase::Press,
        "move" => ButtonPhase::Move,
        "release" => ButtonPhase::Release,
        _ => return None,
    };
    let button = match p.get("button")? {
        Value::Str(name) => MouseButton::from_name(name)?,
        Value::Int(code) => MouseButton::from_code(u32::try_from(*code).ok()?),
        _ => return None,
    };
    Some(ButtonEvent {
        phase,
        button,
        pos: point(p)?,
        clicks: small(p, "clicks"),
        cell: cell_of(p),
        line: small(p, "line"),
        byte: byte_of(p),
    })
}

fn read_key(p: &Value) -> Option<KeyEvent> {
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
        location: p
            .get_str("location")
            .and_then(crate::input::KeyLocation::from_name)
            .unwrap_or_default(),
        locks: crate::input::KeyLocks {
            caps: p.get_bool("caps_lock").unwrap_or(false),
            num: p.get_bool("num_lock").unwrap_or(false),
        },
    };
    Some(KeyEvent { phase, press })
}

fn read_scroll(p: &Value) -> Option<Scroll> {
    Some(Scroll {
        pos: point(p)?,
        delta: Vec2::new(p.get_f32("dx")?, p.get_f32("dy")?),
        lines: p.get_int("lines"),
        mods: p.get("mods").map(mods),
    })
}

fn read_hover(p: &Value) -> Option<Hover> {
    let phase = match p.get_str("phase")? {
        "enter" => HoverPhase::Enter,
        "leave" => HoverPhase::Leave,
        _ => return None,
    };
    let by = match p.get_str("by") {
        Some("content") => HoverBy::Content,
        _ => HoverBy::Pointer,
    };
    Some(Hover { phase, by })
}

fn read_layout(p: &Value) -> Option<Layout> {
    Some(Layout {
        rect: rect(p)?,
        parent: p.get("parent").and_then(rect).unwrap_or_default(),
        scale: p.get_f32("scale").unwrap_or(1.0),
    })
}

/// Lenient: a plain click carries no data, and reads as all `None`.
fn read_click(p: &Value) -> Click {
    Click {
        cell: cell_of(p),
        line: small(p, "line"),
        byte: byte_of(p),
        inside: p.get_bool("inside"),
        clicks: small(p, "clicks"),
    }
}

fn read_text(p: &Value) -> Option<Text> {
    Some(Text {
        text: p.get_str("text")?.to_string(),
        pasted: p.get_bool("pasted").unwrap_or(false),
        concealed: p.get_bool("concealed").unwrap_or(false),
        transient: p.get_bool("transient").unwrap_or(false),
    })
}

fn read_preedit(p: &Value) -> Option<Preedit> {
    Some(Preedit {
        text: p.get_str("text")?.to_string(),
        cursor: p.get("cursor").and_then(Value::as_list).and_then(|c| {
            let at = |i: usize| c.get(i)?.as_int().and_then(|v| usize::try_from(v).ok());
            Some((at(0)?, at(1)?))
        }),
    })
}

fn read_select(p: &Value) -> Option<Select> {
    Some(Select {
        anchor: line_pos(p.get("anchor")?)?,
        focus: line_pos(p.get("focus")?)?,
    })
}

fn read_selection_range(p: &Value) -> Option<SelectionRange> {
    Some(SelectionRange {
        from: range_end(p.get("from")?)?,
        to: range_end(p.get("to")?)?,
    })
}

fn read_menu(p: &Value) -> Option<MenuChoice> {
    Some(MenuChoice {
        role: crate::menu::MenuRole::from_name(p.get_str("role")?)?,
        item: p.get("item").cloned().unwrap_or_default(),
    })
}

fn read_focus(p: &Value) -> Option<Focus> {
    Some(Focus {
        phase: match p.get_str("phase")? {
            "in" => FocusPhase::In,
            "out" => FocusPhase::Out,
            _ => return None,
        },
        by: match p.get_str("by")? {
            "pointer" => FocusBy::Pointer,
            "keyboard" => FocusBy::Keyboard,
            "assistive" => FocusBy::Assistive,
            "program" => FocusBy::Program,
            _ => return None,
        },
    })
}

fn read_drop(p: &Value) -> Option<FileDrop> {
    Some(FileDrop {
        phase: match p.get_str("phase")? {
            "enter" => DropPhase::Enter,
            "move" => DropPhase::Move,
            "leave" => DropPhase::Leave,
            "drop" => DropPhase::Drop,
            _ => return None,
        },
        paths: paths(p)?,
        pos: point(p),
    })
}

fn read_slider_change(p: &Value) -> Option<SliderChange> {
    Some(SliderChange {
        value: p.get_float("value")?,
        phase: match p.get_str("phase")? {
            "move" => ChangePhase::Move,
            "end" => ChangePhase::End,
            _ => return None,
        },
    })
}

fn read_edit_change(p: &Value) -> Option<EditChange> {
    Some(EditChange {
        text: p.get_str("text")?.to_string(),
    })
}

fn read_boundary(p: &Value) -> Option<Boundary> {
    Some(Boundary {
        key: crate::input::EditKey::from_name(p.get_str("key")?)?,
        edge: match p.get_str("edge")? {
            "start" => TextEdge::Start,
            "end" => TextEdge::End,
            _ => return None,
        },
        word: p.get_bool("word").unwrap_or(false),
        doc: p.get_bool("doc").unwrap_or(false),
        ctrl: p.get_bool("ctrl").unwrap_or(false),
    })
}

fn read_sound(p: &Value) -> Option<Sound> {
    Some(Sound {
        phase: match p.get_str("phase")? {
            "ended" => SoundPhase::Ended,
            "refused" => SoundPhase::Refused,
            _ => return None,
        },
        playback: crate::audio::PlaybackId(u64::try_from(p.get_int("playback")?).ok()?),
    })
}

fn read_captured(p: &Value) -> Option<Captured> {
    let count = |k| p.get_int(k).and_then(|n| u32::try_from(n).ok());
    Some(Captured {
        phase: match p.get_str("phase")? {
            "saved" => CapturePhase::Saved,
            "failed" => CapturePhase::Failed,
            _ => return None,
        },
        path: p.get_str("path")?.to_string(),
        width: count("width")?,
        height: count("height")?,
        frames: count("frames")?,
        seconds: p.get_float("seconds")?,
        error: p.get_str("error").map(str::to_string),
    })
}

fn read_dismiss(p: &Value) -> Option<Dismiss> {
    Some(Dismiss {
        reason: match p.get_str("reason")? {
            "escape" => crate::window::DismissReason::Escape,
            "outside" => crate::window::DismissReason::Outside,
            _ => return None,
        },
        window: p
            .get_int("id")
            .and_then(|id| u32::try_from(id).ok())
            .map(crate::window::WindowId),
    })
}

/// A `click`'s data, where the node can say more than that it was
/// clicked: on a `cells` grid the cell, inside an `onKey` sink that draws
/// `role="line"` rows the line, the byte and the press's click count. A
/// plain click carries none of it, and reads as all `None`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Click {
    pub cell: Option<(u32, u32)>,
    pub line: Option<u32>,
    pub byte: Option<usize>,
    pub inside: Option<bool>,
    pub clicks: Option<u32>,
}

/// A `text`, owned: what [`Event::Text`] carries ([`TextInput`] is the
/// same reading borrowed from the event).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub text: String,
    pub pasted: bool,
    pub concealed: bool,
    pub transient: bool,
}

/// A `preedit`: a composition in progress on a key sink.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preedit {
    /// The uncommitted text to show at the caret; empty when the
    /// composition ended without a commit.
    pub text: String,
    /// The byte range in `text` the input method's own caret covers.
    pub cursor: Option<(usize, usize)>,
}

/// A place in a custom editor's text: a line ordinal and a byte in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinePos {
    pub line: u32,
    pub offset: usize,
}

/// A `select`: assistive technology asked a custom editor to set its
/// selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Select {
    pub anchor: LinePos,
    pub focus: LinePos,
}

/// A `selectionrange`: a copy reached rows no frame built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionRange {
    pub from: crate::select::RangeEnd,
    pub to: crate::select::RangeEnd,
}

/// A `menu`: a row of one of the core's menus was chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuChoice {
    pub role: crate::menu::MenuRole,
    /// The row's `id`, or its label when it declared none.
    pub item: Value,
}

/// Which way focus went for a `focus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusPhase {
    In,
    Out,
}

/// What moved focus, for a `focus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusBy {
    Pointer,
    Keyboard,
    Assistive,
    Program,
}

/// A `focus`: keyboard focus entered or left an `onFocus` node's subtree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Focus {
    pub phase: FocusPhase,
    pub by: FocusBy,
}

/// Which part of a file drag a `drop` reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropPhase {
    Enter,
    Move,
    Leave,
    Drop,
}

/// A `drop`: files dragged over an `onDrop` zone, or landing on it.
#[derive(Clone, Debug, PartialEq)]
pub struct FileDrop {
    pub phase: DropPhase,
    pub paths: Vec<String>,
    /// Where the pointer is; `None` on `leave`.
    pub pos: Option<Vec2>,
}

/// A `resize`: the viewport changed size or DPI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resize {
    pub size: crate::geom::Size,
    pub scale: f32,
}

/// What happened to a window, for a `window`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowPhase {
    Opened,
    Closed,
    Focused,
    Blurred,
}

/// A `window`: a declared window opened or closed, as its `windows`
/// entry's `onWindow` message, or this window gained or lost the keyboard,
/// as its root's `windowEvents.onFocus` (ADR 0048). The window's name is
/// the message's to say; `id` is the opened or closed window's, and `None`
/// for a focus change, which is the receiving window's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowEvent {
    pub phase: WindowPhase,
    pub id: Option<crate::window::WindowId>,
}

/// Which part of a slider's move a `change` reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangePhase {
    /// The pointer still holds the slider.
    Move,
    /// It let go, or a key moved it.
    End,
}

/// A slider's `change`: the value the core proposes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderChange {
    pub value: f64,
    pub phase: ChangePhase,
}

/// An editor's `change`: its whole text after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditChange {
    pub text: String,
}

/// Which end of an editor's text a `boundary` key met.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEdge {
    Start,
    End,
}

/// A `boundary`: an editing key met the edge of an editor's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Boundary {
    pub key: crate::input::EditKey,
    pub edge: TextEdge,
    pub word: bool,
    pub doc: bool,
    /// Control was held (backlog C94): on macOS not the same key as `doc`.
    pub ctrl: bool,
}

/// How a tagged playback came to its `sound`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundPhase {
    Ended,
    Refused,
}

/// A `sound`: a playback that asked to be told finished, or was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sound {
    pub phase: SoundPhase,
    pub playback: crate::audio::PlaybackId,
}

/// Whether a [`Captured`]'s file was written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapturePhase {
    Saved,
    Failed,
}

/// A `capture` (ADR 0055): a still or a recording the app asked for with
/// `capture_to` / `record_to`, written to `path` or not, in the message
/// the verb named — or a frame `capture_to_clipboard` put on the
/// clipboard, whose `path` is empty. `width` and `height` are physical px, `frames` 1 for a
/// still and the frames drawn while recording for a recording — the file
/// can hold fewer (a frame that changed nothing joins the one before) or
/// more (a frame held past what a delay holds is split) — `seconds` a
/// recording's length (0 for a still), and `error` the reason a `Failed`
/// one gives.
#[derive(Clone, Debug, PartialEq)]
pub struct Captured {
    pub phase: CapturePhase,
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub seconds: f64,
    pub error: Option<String>,
}

/// A `dismiss`: the user asked a surface to go away. A popup window's
/// carries its id, and its name is its `onDismiss` message's to say; a
/// `modal` node's carries neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dismiss {
    pub reason: crate::window::DismissReason,
    pub window: Option<crate::window::WindowId>,
}

/// What happened, read whole: one variant per [`EventKind`], each with
/// its data (ADR 0047, decision 13). [`UiEvent::split`] hands it back
/// beside the app's message, so a Rust handler matches the pair in one
/// `match`:
///
/// ```rust
/// use kui_core::{EventKind, Key, OriginId, UiEvent, Value};
/// use kui_core::event::Event;
///
/// let ev = UiEvent::declared(
///     EventKind::Drag,
///     OriginId::HOST,
///     Key::ROOT,
///     Some(&Value::map([("kind", Value::str("pan"))])),
///     Value::map([
///         ("phase", Value::str("move")),
///         ("x", Value::float(4.0)), ("y", Value::float(0.0)),
///         ("dx", Value::float(4.0)), ("dy", Value::float(0.0)),
///     ]),
/// );
/// let (msg, event) = ev.split::<Value>();
/// match (msg.as_ref().and_then(|m| m.get_str("kind")), event) {
///     (Some("pan"), Event::Drag(d)) => assert_eq!(d.delta.x, 4.0),
///     _ => unreachable!(),
/// }
/// ```
///
/// A payload this core could not read — a field missing, a phase it
/// does not name — is [`Event::Unread`], with the kind it claimed.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Click(Click),
    Drag(Drag),
    Key(KeyPhase, KeyPress),
    Text(Text),
    Preedit(Preedit),
    Select(Select),
    SelectionRange(SelectionRange),
    ContextMenu(Vec2),
    Menu(MenuChoice),
    ForceClick(Vec2),
    Button(ButtonEvent),
    Scroll(Scroll),
    Focus(Focus),
    Hover(Hover),
    Drop(FileDrop),
    Files(Vec<String>),
    Open(Vec<String>),
    Layout(Layout),
    Resize(Resize),
    Window(WindowEvent),
    /// `env.system` as it now reads.
    System(SystemEnv),
    Fonts,
    Modifiers(KeyMods),
    SliderChange(SliderChange),
    EditChange(EditChange),
    Submit,
    Cancel,
    Boundary(Boundary),
    Sound(Sound),
    Capture(Captured),
    Dismiss(Dismiss),
    Range(RowRange),
    Query(QueryChange),
    /// A component instance's event (ADR 0060): the host's own message
    /// for the event prop is [`UiEvent::message`]'s to decode.
    Component,
    Unread(EventKind),
}

fn paths(p: &Value) -> Option<Vec<String>> {
    p.get("paths")?
        .as_list()?
        .iter()
        .map(|v| v.as_str().map(str::to_string))
        .collect()
}

fn range_end(v: &Value) -> Option<crate::select::RangeEnd> {
    Some(crate::select::RangeEnd {
        row: v.get_int("index").and_then(|i| u64::try_from(i).ok()),
        byte: usize::try_from(v.get_int("byte")?).ok()?,
    })
}

fn line_pos(v: &Value) -> Option<LinePos> {
    Some(LinePos {
        line: small(v, "line")?,
        offset: usize::try_from(v.get_int("offset")?).ok()?,
    })
}

impl UiEvent {
    /// The app's message and the event, read whole: [`Self::message`]
    /// beside [`Self::read`], for one `match` over the pair.
    pub fn split<M>(&self) -> (Option<M>, Event)
    where
        M: for<'a> TryFrom<&'a Value>,
    {
        (self.message(), self.read())
    }

    /// The event, read whole from its kind and its data.
    pub fn read(&self) -> Event {
        self.try_read().unwrap_or(Event::Unread(self.event))
    }

    fn try_read(&self) -> Option<Event> {
        let empty = Value::Null;
        let p = self.data().unwrap_or(&empty);
        Some(match self.event {
            EventKind::Click => Event::Click(read_click(p)),
            EventKind::Drag => Event::Drag(self.drag()?),
            EventKind::Key => {
                let (phase, press) = self.key_press()?;
                Event::Key(phase, press)
            }
            EventKind::Text => {
                let t = self.text()?;
                Event::Text(Text {
                    text: t.text.to_string(),
                    pasted: t.pasted,
                    concealed: t.concealed,
                    transient: t.transient,
                })
            }
            EventKind::Preedit => Event::Preedit(read_preedit(p)?),
            EventKind::Select => Event::Select(read_select(p)?),
            EventKind::SelectionRange => Event::SelectionRange(read_selection_range(p)?),
            EventKind::ContextMenu => Event::ContextMenu(point(p)?),
            EventKind::Menu => Event::Menu(read_menu(p)?),
            EventKind::ForceClick => Event::ForceClick(point(p)?),
            EventKind::Button => Event::Button(self.button()?),
            EventKind::Scroll => Event::Scroll(self.scroll()?),
            EventKind::Focus => Event::Focus(read_focus(p)?),
            EventKind::Hover => Event::Hover(self.hover()?),
            EventKind::Drop => Event::Drop(read_drop(p)?),
            EventKind::Files => Event::Files(paths(p)?),
            EventKind::Open => Event::Open(paths(p)?),
            EventKind::Layout => Event::Layout(self.layout()?),
            EventKind::Resize => Event::Resize(read_resize(p)?),
            EventKind::Window => Event::Window(read_window(p)?),
            EventKind::System => Event::System(SystemEnv::from_value(p)?),
            EventKind::Fonts => Event::Fonts,
            EventKind::Modifiers => Event::Modifiers(self.modifiers()?),
            EventKind::Change => match p.get("text") {
                Some(text) => Event::EditChange(EditChange {
                    text: text.as_str()?.to_string(),
                }),
                None => Event::SliderChange(read_slider_change(p)?),
            },
            EventKind::Submit => Event::Submit,
            EventKind::Cancel => Event::Cancel,
            EventKind::Boundary => Event::Boundary(read_boundary(p)?),
            EventKind::Sound => Event::Sound(read_sound(p)?),
            EventKind::Capture => Event::Capture(read_captured(p)?),
            EventKind::Dismiss => Event::Dismiss(read_dismiss(p)?),
            EventKind::Component => Event::Component,
            EventKind::Range => Event::Range(read_range(p)?),
            EventKind::Query => Event::Query(read_query(p)?),
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

// The events whose data `Event` holds as a tuple or a bare value, given
// a struct of their own so each is one message field's type.

/// A `key`, owned whole: the phase and the [`KeyPress`] a key sink heard,
/// what a message's `key` field reads as ([`UiEvent::key_press`] is the
/// same pair).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub phase: KeyPhase,
    pub press: KeyPress,
}

/// A `contextmenu`: a secondary click on a node that declared a menu,
/// where the pointer was.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextMenu {
    pub pos: Vec2,
}

/// A `forceclick`: a force-touch press on a node that asked for one,
/// where the pointer was.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForceClick {
    pub pos: Vec2,
}

/// An `open`'s data as a message field: the documents the OS asked the
/// app to open, as the root's `windowEvents.onOpen` message carries them
/// (ADR 0048). [`Event::Open`] holds the same paths bare.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Open {
    pub paths: Vec<String>,
}

/// A `files`: the paths a file dialog answered with, empty when it was
/// cancelled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Files {
    pub paths: Vec<String>,
}

// Each event's data back to the map the core sends, for a message that
// carries it as a field (`MessageField::to_value`): the same keys and
// number types the producers write, optional ones left out when unset.

type Fields = Vec<(&'static str, Value)>;

fn num(v: f32) -> Value {
    Value::Float(v as f64)
}

fn int(v: impl TryInto<i64>) -> Value {
    Value::Int(v.try_into().unwrap_or(i64::MAX))
}

fn rect_value(r: Rect) -> Value {
    Value::map([
        ("x", num(r.x)),
        ("y", num(r.y)),
        ("w", num(r.w)),
        ("h", num(r.h)),
    ])
}

fn str_list(items: &[String]) -> Value {
    Value::list(items.iter().map(|s| Value::str(s.as_str())))
}

/// The cell, line and byte a pointer event carries on a grid or a sink's
/// line rows.
fn push_place(f: &mut Fields, cell: Option<(u32, u32)>, line: Option<u32>, byte: Option<usize>) {
    if let Some((row, col)) = cell {
        f.push(("cell", Value::map([("row", int(row)), ("col", int(col))])));
    }
    if let Some(line) = line {
        f.push(("line", int(line)));
    }
    if let Some(byte) = byte {
        f.push(("byte", int(byte)));
    }
}

fn write_click(c: Click) -> Value {
    let mut f = Fields::new();
    push_place(&mut f, c.cell, c.line, c.byte);
    if let Some(inside) = c.inside {
        f.push(("inside", Value::Bool(inside)));
    }
    if let Some(clicks) = c.clicks {
        f.push(("clicks", int(clicks)));
    }
    Value::map(f)
}

fn write_drag(d: Drag) -> Value {
    let phase = match d.phase {
        DragPhase::Start => "start",
        DragPhase::Move => "move",
        DragPhase::End => "end",
    };
    let mut f = vec![
        ("phase", Value::str(phase)),
        ("x", num(d.pos.x)),
        ("y", num(d.pos.y)),
        ("dx", num(d.delta.x)),
        ("dy", num(d.delta.y)),
        ("parent", rect_value(d.parent)),
    ];
    push_place(&mut f, d.cell, d.line, d.byte);
    if let Some(clicks) = d.clicks {
        f.push(("clicks", int(clicks)));
    }
    Value::map(f)
}

fn write_key(k: KeyEvent) -> Value {
    k.press.to_value(k.phase)
}

fn write_text(t: Text) -> Value {
    let mut f = vec![("text", Value::Str(t.text))];
    for (name, on) in [
        ("pasted", t.pasted),
        ("concealed", t.concealed),
        ("transient", t.transient),
    ] {
        if on {
            f.push((name, Value::Bool(true)));
        }
    }
    Value::map(f)
}

fn write_preedit(p: Preedit) -> Value {
    let cursor = match p.cursor {
        Some((a, b)) => Value::List(vec![int(a), int(b)]),
        None => Value::Null,
    };
    Value::map([("text", Value::Str(p.text)), ("cursor", cursor)])
}

fn write_select(s: Select) -> Value {
    let pos = |p: LinePos| Value::map([("line", int(p.line)), ("offset", int(p.offset))]);
    Value::map([("anchor", pos(s.anchor)), ("focus", pos(s.focus))])
}

fn write_selection_range(r: SelectionRange) -> Value {
    Value::map([("from", r.from.to_value()), ("to", r.to.to_value())])
}

fn write_context_menu(c: ContextMenu) -> Value {
    Value::map([("x", num(c.pos.x)), ("y", num(c.pos.y))])
}

fn write_menu(m: MenuChoice) -> Value {
    Value::map([("role", Value::str(m.role.name())), ("item", m.item)])
}

fn write_force_click(c: ForceClick) -> Value {
    Value::map([("x", num(c.pos.x)), ("y", num(c.pos.y))])
}

fn write_button(b: ButtonEvent) -> Value {
    let phase = match b.phase {
        ButtonPhase::Press => "press",
        ButtonPhase::Move => "move",
        ButtonPhase::Release => "release",
    };
    let mut f = vec![
        ("phase", Value::str(phase)),
        ("button", b.button.to_value()),
        ("x", num(b.pos.x)),
        ("y", num(b.pos.y)),
    ];
    if let Some(clicks) = b.clicks {
        f.push(("clicks", int(clicks)));
    }
    push_place(&mut f, b.cell, b.line, b.byte);
    Value::map(f)
}

fn write_scroll(s: Scroll) -> Value {
    let mut f = vec![
        ("x", num(s.pos.x)),
        ("y", num(s.pos.y)),
        ("dx", num(s.delta.x)),
        ("dy", num(s.delta.y)),
        ("lines", s.lines.map_or(Value::Null, Value::Int)),
    ];
    if let Some(m) = s.mods {
        f.push(("mods", m.to_fields()));
    }
    Value::map(f)
}

fn write_focus(x: Focus) -> Value {
    let phase = match x.phase {
        FocusPhase::In => "in",
        FocusPhase::Out => "out",
    };
    let by = match x.by {
        FocusBy::Pointer => "pointer",
        FocusBy::Keyboard => "keyboard",
        FocusBy::Assistive => "assistive",
        FocusBy::Program => "program",
    };
    Value::map([("phase", Value::str(phase)), ("by", Value::str(by))])
}

fn write_hover(h: Hover) -> Value {
    let phase = match h.phase {
        HoverPhase::Enter => "enter",
        HoverPhase::Leave => "leave",
    };
    let by = match h.by {
        HoverBy::Pointer => "pointer",
        HoverBy::Content => "content",
    };
    Value::map([("phase", Value::str(phase)), ("by", Value::str(by))])
}

fn write_drop(d: FileDrop) -> Value {
    let phase = match d.phase {
        DropPhase::Enter => "enter",
        DropPhase::Move => "move",
        DropPhase::Leave => "leave",
        DropPhase::Drop => "drop",
    };
    let mut f = vec![("phase", Value::str(phase)), ("paths", str_list(&d.paths))];
    if let Some(p) = d.pos {
        f.push(("x", num(p.x)));
        f.push(("y", num(p.y)));
    }
    Value::map(f)
}

fn write_files(x: Files) -> Value {
    Value::map([("paths", str_list(&x.paths))])
}

fn write_layout(l: Layout) -> Value {
    Value::map([
        ("x", num(l.rect.x)),
        ("y", num(l.rect.y)),
        ("w", num(l.rect.w)),
        ("h", num(l.rect.h)),
        ("parent", rect_value(l.parent)),
        ("scale", num(l.scale)),
    ])
}

fn write_slider_change(c: SliderChange) -> Value {
    let phase = match c.phase {
        ChangePhase::Move => "move",
        ChangePhase::End => "end",
    };
    Value::map([
        ("value", Value::Float(c.value)),
        ("phase", Value::str(phase)),
    ])
}

fn write_edit_change(c: EditChange) -> Value {
    Value::map([("text", Value::Str(c.text))])
}

fn write_boundary(b: Boundary) -> Value {
    let edge = match b.edge {
        TextEdge::Start => "start",
        TextEdge::End => "end",
    };
    Value::map([
        ("key", Value::str(b.key.name())),
        ("edge", Value::str(edge)),
        ("word", Value::Bool(b.word)),
        ("doc", Value::Bool(b.doc)),
        ("ctrl", Value::Bool(b.ctrl)),
    ])
}

fn write_sound(s: Sound) -> Value {
    let phase = match s.phase {
        SoundPhase::Ended => "ended",
        SoundPhase::Refused => "refused",
    };
    Value::map([
        ("phase", Value::str(phase)),
        ("playback", int(s.playback.0)),
    ])
}

fn write_captured(c: Captured) -> Value {
    let phase = match c.phase {
        CapturePhase::Saved => "saved",
        CapturePhase::Failed => "failed",
    };
    let mut f = vec![
        ("phase", Value::str(phase)),
        ("path", Value::Str(c.path)),
        ("width", int(c.width)),
        ("height", int(c.height)),
        ("frames", int(c.frames)),
        ("seconds", Value::Float(c.seconds)),
    ];
    if let Some(e) = c.error {
        f.push(("error", Value::Str(e)));
    }
    Value::map(f)
}

fn write_dismiss(d: Dismiss) -> Value {
    let mut f = vec![("reason", Value::str(d.reason.as_str()))];
    if let Some(id) = d.window {
        f.push(("id", int(id.0)));
    }
    Value::map(f)
}

/// A strict click: a field that is there, as a map. A plain click carries
/// no `click` field, so a message that asks for one does not read off it.
fn read_click_field(p: &Value) -> Option<Click> {
    matches!(p, Value::Map(_)).then(|| read_click(p))
}

/// Each event's data as a message field ([`MessageField`], in the shape
/// the core sends) and as the field it rides under ([`EventField`]).
macro_rules! event_fields {
    ($($t:ty => $kind:ident $field:literal, $read:ident, $write:ident;)*) => {$(
        impl MessageField for $t {
            fn to_value(self) -> Value {
                $write(self)
            }
            fn from_value(v: &Value) -> Option<Self> {
                $read(v)
            }
        }

        impl EventField for $t {
            const EVENT: EventKind = EventKind::$kind;
            const FIELD: &'static str = $field;
        }
    )*};
}

fn read_context_menu(p: &Value) -> Option<ContextMenu> {
    Some(ContextMenu { pos: point(p)? })
}

fn read_force_click(p: &Value) -> Option<ForceClick> {
    Some(ForceClick { pos: point(p)? })
}

fn read_files(p: &Value) -> Option<Files> {
    Some(Files { paths: paths(p)? })
}

fn read_open(p: &Value) -> Option<Open> {
    Some(Open { paths: paths(p)? })
}

fn write_open(o: Open) -> Value {
    Value::map([(
        "paths",
        Value::List(o.paths.into_iter().map(Value::Str).collect()),
    )])
}

fn read_resize(p: &Value) -> Option<Resize> {
    Some(Resize {
        size: crate::geom::Size::new(p.get_f32("width")?, p.get_f32("height")?),
        scale: p.get_f32("scale")?,
    })
}

fn write_resize(r: Resize) -> Value {
    Value::map([
        ("width", Value::float(r.size.w)),
        ("height", Value::float(r.size.h)),
        ("scale", Value::float(r.scale)),
    ])
}

fn read_window(p: &Value) -> Option<WindowEvent> {
    Some(WindowEvent {
        phase: match p.get_str("phase")? {
            "opened" => WindowPhase::Opened,
            "closed" => WindowPhase::Closed,
            "focused" => WindowPhase::Focused,
            "blurred" => WindowPhase::Blurred,
            _ => return None,
        },
        id: match p.get("id") {
            Some(id) => Some(crate::window::WindowId(u32::try_from(id.as_int()?).ok()?)),
            None => None,
        },
    })
}

fn write_window(w: WindowEvent) -> Value {
    let phase = match w.phase {
        WindowPhase::Opened => "opened",
        WindowPhase::Closed => "closed",
        WindowPhase::Focused => "focused",
        WindowPhase::Blurred => "blurred",
    };
    let mut f = vec![("phase", Value::str(phase))];
    if let Some(id) = w.id {
        f.push(("id", int(id.0)));
    }
    Value::map(f)
}

fn read_system(p: &Value) -> Option<SystemEnv> {
    SystemEnv::from_value(p)
}

fn write_system(s: SystemEnv) -> Value {
    // By path: `s.to_value()` would pick the by-value `MessageField` one,
    // which is this function.
    SystemEnv::to_value(&s)
}

fn read_modifiers(p: &Value) -> Option<KeyMods> {
    matches!(p, Value::Map(_)).then(|| mods(p))
}

fn write_modifiers(m: KeyMods) -> Value {
    KeyMods::to_value(m)
}

event_fields! {
    Click => Click "click", read_click_field, write_click;
    Drag => Drag "drag", read_drag, write_drag;
    KeyEvent => Key "key", read_key, write_key;
    Text => Text "text", read_text, write_text;
    Preedit => Preedit "preedit", read_preedit, write_preedit;
    Select => Select "select", read_select, write_select;
    SelectionRange => SelectionRange "selectionrange", read_selection_range, write_selection_range;
    ContextMenu => ContextMenu "contextmenu", read_context_menu, write_context_menu;
    MenuChoice => Menu "menu", read_menu, write_menu;
    ForceClick => ForceClick "forceclick", read_force_click, write_force_click;
    ButtonEvent => Button "button", read_button, write_button;
    Scroll => Scroll "scroll", read_scroll, write_scroll;
    Focus => Focus "focus", read_focus, write_focus;
    Hover => Hover "hover", read_hover, write_hover;
    FileDrop => Drop "drop", read_drop, write_drop;
    Files => Files "files", read_files, write_files;
    Layout => Layout "layout", read_layout, write_layout;
    EditChange => Change "change", read_edit_change, write_edit_change;
    SliderChange => Change "change", read_slider_change, write_slider_change;
    Boundary => Boundary "boundary", read_boundary, write_boundary;
    Sound => Sound "sound", read_sound, write_sound;
    Captured => Capture "capture", read_captured, write_captured;
    Dismiss => Dismiss "dismiss", read_dismiss, write_dismiss;
    Open => Open "open", read_open, write_open;
    Resize => Resize "resize", read_resize, write_resize;
    WindowEvent => Window "window", read_window, write_window;
    SystemEnv => System "system", read_system, write_system;
    KeyMods => Modifiers "modifiers", read_modifiers, write_modifiers;
}

#[cfg(test)]
mod field_tests {
    use super::*;
    use crate::key::Key;
    use crate::tree::OriginId;

    /// The field an event's data names is the event's wire name, and its
    /// data written out reads back as itself, both as a message field and
    /// through the event the core would send with it.
    fn round_trip<T: EventField + Clone + PartialEq + std::fmt::Debug>(x: T) -> Event {
        assert_eq!(T::FIELD, T::EVENT.name());
        let data = x.clone().to_value();
        assert_eq!(T::from_value(&data).as_ref(), Some(&x), "{data:?}");
        let msg = Value::map([("kind", Value::str("m"))]);
        let ev = UiEvent::declared(T::EVENT, OriginId::HOST, Key::ROOT, Some(&msg), data);
        assert_eq!(ev.payload.get_str("kind"), Some("m"));
        ev.read()
    }

    #[test]
    fn every_event_field_round_trips_and_names_its_event() {
        let pos = Vec2::new(3.0, 4.5);
        let parent = Rect::new(1.0, 2.0, 30.0, 40.0);
        let click = Click {
            cell: Some((2, 3)),
            line: Some(1),
            byte: Some(4),
            inside: Some(true),
            clicks: Some(2),
        };
        assert_eq!(round_trip(click), Event::Click(click));
        let drag = Drag {
            phase: DragPhase::Move,
            pos,
            delta: Vec2::new(-1.0, 2.0),
            parent,
            cell: None,
            line: Some(3),
            byte: Some(7),
            clicks: Some(1),
        };
        assert_eq!(round_trip(drag), Event::Drag(drag));
        let press = KeyPress {
            code: KeyCode::from_name("a").unwrap(),
            physical: KeyCode::from_name("a").unwrap(),
            mods: KeyMods {
                shift: true,
                ..KeyMods::default()
            },
            text: Some("A".into()),
            repeat: false,
            location: Default::default(),
            locks: Default::default(),
        };
        let key = KeyEvent {
            phase: KeyPhase::Up,
            press: press.clone(),
        };
        assert_eq!(round_trip(key), Event::Key(KeyPhase::Up, press));
        let text = Text {
            text: "hi".into(),
            pasted: true,
            concealed: false,
            transient: true,
        };
        assert_eq!(round_trip(text.clone()), Event::Text(text));
        for cursor in [None, Some((0, 2))] {
            let p = Preedit {
                text: "か".into(),
                cursor,
            };
            assert_eq!(round_trip(p.clone()), Event::Preedit(p));
        }
        let select = Select {
            anchor: LinePos { line: 0, offset: 1 },
            focus: LinePos { line: 2, offset: 3 },
        };
        assert_eq!(round_trip(select), Event::Select(select));
        let range = SelectionRange {
            from: crate::select::RangeEnd {
                row: Some(4),
                byte: 0,
            },
            to: crate::select::RangeEnd { row: None, byte: 9 },
        };
        assert_eq!(round_trip(range), Event::SelectionRange(range));
        assert_eq!(round_trip(ContextMenu { pos }), Event::ContextMenu(pos));
        let menu = MenuChoice {
            role: crate::menu::MenuRole::from_name("copy").unwrap(),
            item: Value::str("c"),
        };
        assert_eq!(round_trip(menu.clone()), Event::Menu(menu));
        assert_eq!(round_trip(ForceClick { pos }), Event::ForceClick(pos));
        for button in [MouseButton::Secondary, MouseButton::Other(7)] {
            let b = ButtonEvent {
                phase: ButtonPhase::Press,
                button,
                pos,
                clicks: Some(1),
                cell: Some((0, 1)),
                line: None,
                byte: None,
            };
            assert_eq!(round_trip(b), Event::Button(b));
        }
        let scroll = Scroll {
            pos,
            delta: Vec2::new(0.0, -12.0),
            lines: Some(-1),
            mods: Some(KeyMods {
                ctrl: true,
                ..KeyMods::default()
            }),
        };
        assert_eq!(round_trip(scroll), Event::Scroll(scroll));
        let bare = Scroll {
            lines: None,
            mods: None,
            ..scroll
        };
        assert_eq!(round_trip(bare), Event::Scroll(bare));
        let focus = Focus {
            phase: FocusPhase::Out,
            by: FocusBy::Assistive,
        };
        assert_eq!(round_trip(focus), Event::Focus(focus));
        let hover = Hover {
            phase: HoverPhase::Enter,
            by: HoverBy::Content,
        };
        assert_eq!(round_trip(hover), Event::Hover(hover));
        for at in [Some(pos), None] {
            let d = FileDrop {
                phase: DropPhase::Drop,
                paths: vec!["/a".into(), "/b c".into()],
                pos: at,
            };
            assert_eq!(round_trip(d.clone()), Event::Drop(d));
        }
        let files = Files {
            paths: vec!["/x".into()],
        };
        assert_eq!(round_trip(files), Event::Files(vec!["/x".into()]));
        let layout = Layout {
            rect: parent,
            parent: Rect::new(0.0, 0.0, 100.0, 50.0),
            scale: 2.0,
        };
        assert_eq!(round_trip(layout), Event::Layout(layout));
        let edit = EditChange { text: "abc".into() };
        assert_eq!(round_trip(edit.clone()), Event::EditChange(edit));
        let slider = SliderChange {
            value: 0.25,
            phase: ChangePhase::End,
        };
        assert_eq!(round_trip(slider), Event::SliderChange(slider));
        let boundary = Boundary {
            key: crate::input::EditKey::Backspace,
            edge: TextEdge::Start,
            word: true,
            doc: false,
            ctrl: true,
        };
        assert_eq!(round_trip(boundary), Event::Boundary(boundary));
        let sound = Sound {
            phase: SoundPhase::Refused,
            playback: crate::audio::PlaybackId(9),
        };
        assert_eq!(round_trip(sound), Event::Sound(sound));
        for window in [None, Some(crate::window::WindowId(3))] {
            let d = Dismiss {
                reason: crate::window::DismissReason::Outside,
                window,
            };
            assert_eq!(round_trip(d), Event::Dismiss(d));
        }
        // The window's own (ADR 0048).
        let open = Open {
            paths: vec!["/a.txt".into(), "/b c.md".into()],
        };
        assert_eq!(round_trip(open.clone()), Event::Open(open.paths));
        let resize = Resize {
            size: crate::geom::Size::new(300.0, 200.0),
            scale: 2.0,
        };
        assert_eq!(round_trip(resize), Event::Resize(resize));
        for w in [
            WindowEvent {
                phase: WindowPhase::Opened,
                id: Some(crate::window::WindowId(4)),
            },
            WindowEvent {
                phase: WindowPhase::Blurred,
                id: None,
            },
        ] {
            assert_eq!(round_trip(w), Event::Window(w));
        }
        let system = SystemEnv {
            appearance: crate::env::Appearance::Dark,
            accent: Some(crate::color::Color::hex(0x3b5bd4ff)),
            locale: crate::env::Locale::new("de-CH"),
            ..Default::default()
        };
        assert_eq!(round_trip(system), Event::System(system));
        let shift = KeyMods {
            shift: true,
            ..Default::default()
        };
        assert_eq!(round_trip(shift), Event::Modifiers(shift));
    }

    /// A field the message lacks does not read as the event's data: what
    /// makes a message declared on the wrong handler fail to decode as
    /// its arriving twin. A plain click's absent field is not an empty
    /// click either.
    #[test]
    fn an_absent_field_is_not_the_events_data() {
        assert_eq!(Click::from_value(&Value::Null), None);
        assert_eq!(Click::from_value(&Value::map([])), Some(Click::default()));
        assert_eq!(Drag::from_value(&Value::Null), None);
        assert_eq!(EditChange::from_value(&Value::Null), None);
        // A slider's change is not an editor's, nor the other way round.
        let slider = Value::map([("value", Value::Float(1.0)), ("phase", Value::str("move"))]);
        assert_eq!(EditChange::from_value(&slider), None);
        assert!(SliderChange::from_value(&slider).is_some());
        assert_eq!(
            SliderChange::from_value(&Value::map([("text", Value::str("x"))])),
            None
        );
    }
}
