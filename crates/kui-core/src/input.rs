//! Input handling and event production. Hit regions come from the previous
//! frame's layout (the standard immediate-mode trade); events leave as plain
//! data tagged with the origin that declared them, so the runner can route to
//! the host app or an extension without knowing what either looks like.

use crate::geom::{Rect, Vec2};
use crate::key::Key;
use crate::tree::OriginId;
use crate::value::Value;
use crate::window::{WindowCommand, WindowRole};

#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    /// Logical coordinates.
    CursorMoved(Vec2),
    CursorLeft,
    /// Primary button press. The count is driver-measured multi-click state
    /// (1 = single, 2 = double, 3+ = triple) — the core is clock-free, so
    /// click timing lives with whoever owns the event loop.
    MouseDown(u8),
    MouseUp,
    /// Wheel/trackpad delta in logical px (positive y = scroll up).
    Scroll(Vec2),
    /// Committed text (typing, IME commit, paste). Routed to the focused editor.
    Text(String),
    /// In-progress IME composition (text and the caret byte range inside
    /// it), inserted inline at the focused editor's caret as an uncommitted
    /// marked range: following text shifts and the paragraph rewraps.
    /// Empty text cancels it; a commit arrives separately as `Text`.
    Preedit(String, Option<(usize, usize)>),
    /// Navigation/editing key. Routed to the focused editor.
    Key(EditKey, Mods),
    /// A full key press, routed to whatever holds key focus (see
    /// `Core::set_key_focus`). Apps that own their own text model take
    /// keys through this instead of the editor path.
    KeyDown(KeyPress),
    /// A request from assistive technology (see [`crate::access`]):
    /// activate, focus, set an editor's text, scroll. Resolved in the core
    /// the way the pointer or keyboard equivalent would be, so the app
    /// sees the same events either way.
    Access(crate::access::AccessRequest),
    /// The physical modifier state changed. Reaches the host as a
    /// `{kind="modifiers", shift, ctrl, alt, super}` event on the root (an
    /// Elm-style app keeps it in its model and lets the view react — a
    /// Cmd-held drag overlay, a hint bar) and is queryable while building
    /// a frame (`Ui::modifiers`).
    Modifiers(KeyMods),
}

/// Editing keys, decoupled from any windowing library's key codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKey {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Backspace,
    Delete,
    Enter,
    Tab,
    SelectAll,
    /// Undo/redo of the edit widget's own history (drivers map the platform
    /// chords; hosts with their own text model never see these — they take
    /// the raw chord through `KeyDown`).
    Undo,
    Redo,
    Escape,
}

/// Modifier state for editing keys. `word` is Alt/Option (word-wise motion),
/// `doc` is the platform primary modifier (line/document-wise motion).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub word: bool,
    pub doc: bool,
}

/// A physical key press: the full keyboard, decoupled from any windowing
/// library. [`EditKey`] is the input widget's closed navigation vocabulary;
/// this is what apps that own their own text model bind against — an editor
/// with modal keymaps, a game, a scripted panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCode {
    /// A character-producing key, as the active layout produced it — `W`
    /// and `$` arrive as themselves (shift already applied), which is what
    /// keymaps bind against.
    Char(char),
    /// Function key: `F(1)` .. `F(24)`.
    F(u8),
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Backspace,
    Delete,
    Enter,
    Tab,
    Escape,
    Space,
    Insert,
    /// A key this vocabulary doesn't name; `KeyPress::text` may still carry
    /// what it would insert.
    Unknown,
}

impl KeyCode {
    /// Stable lowercase name for the data payload: `"a"`, `"f5"`, `"pageup"`.
    /// Bindings in C and Lua match on these.
    pub fn name(self) -> String {
        match self {
            KeyCode::Char(c) => c.to_string(),
            KeyCode::F(n) => format!("f{n}"),
            KeyCode::Left => "left".into(),
            KeyCode::Right => "right".into(),
            KeyCode::Up => "up".into(),
            KeyCode::Down => "down".into(),
            KeyCode::Home => "home".into(),
            KeyCode::End => "end".into(),
            KeyCode::PageUp => "pageup".into(),
            KeyCode::PageDown => "pagedown".into(),
            KeyCode::Backspace => "backspace".into(),
            KeyCode::Delete => "delete".into(),
            KeyCode::Enter => "enter".into(),
            KeyCode::Tab => "tab".into(),
            KeyCode::Escape => "escape".into(),
            KeyCode::Space => "space".into(),
            KeyCode::Insert => "insert".into(),
            KeyCode::Unknown => "unknown".into(),
        }
    }
}

/// Physical modifier state. Unlike [`Mods`] — which abstracts platform
/// conventions for the input widget (`word`, `doc`) — nothing here is
/// normalized: an app binding `Ctrl-w` needs to know it was Control and not
/// Command.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyMods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// Windows key / Command / Super.
    pub super_key: bool,
}

impl KeyMods {
    pub fn any(self) -> bool {
        self.shift || self.ctrl || self.alt || self.super_key
    }

    /// The platform primary shortcut modifier: Command on macOS, Control
    /// elsewhere.
    pub fn primary(self) -> bool {
        if cfg!(target_os = "macos") {
            self.super_key
        } else {
            self.ctrl
        }
    }

    /// The payload form of a modifier change:
    /// `{kind="modifiers", shift=, ctrl=, alt=, super=}`.
    pub fn to_value(self) -> Value {
        Value::map([
            ("kind", Value::str("modifiers")),
            ("shift", Value::Bool(self.shift)),
            ("ctrl", Value::Bool(self.ctrl)),
            ("alt", Value::Bool(self.alt)),
            ("super", Value::Bool(self.super_key)),
        ])
    }
}

/// One key press, delivered to whatever holds key focus. Carries both the
/// binding view (`code` + `mods`) and the typing view (`text`), so an app can
/// serve a modal keymap and an insert mode from the same event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyPress {
    pub code: KeyCode,
    pub mods: KeyMods,
    /// What this press would insert, if anything — already resolved through
    /// the keyboard layout. `None` for pure navigation and chords.
    pub text: Option<String>,
    /// Set when the press came from OS key repeat.
    pub repeat: bool,
}

impl KeyPress {
    pub fn new(code: KeyCode, mods: KeyMods) -> Self {
        Self {
            code,
            mods,
            text: None,
            repeat: false,
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// The payload form crossing into events, C, and Lua:
    /// `{kind="key", code="w", shift=, ctrl=, alt=, super=, text=, repeat=}`.
    pub fn to_value(&self) -> Value {
        Value::map([
            ("kind", Value::str("key")),
            ("code", Value::Str(self.code.name())),
            ("shift", Value::Bool(self.mods.shift)),
            ("ctrl", Value::Bool(self.mods.ctrl)),
            ("alt", Value::Bool(self.mods.alt)),
            ("super", Value::Bool(self.mods.super_key)),
            (
                "text",
                match &self.text {
                    Some(t) => Value::Str(t.clone()),
                    None => Value::Null,
                },
            ),
            ("repeat", Value::Bool(self.repeat)),
        ])
    }
}

/// An event produced by the UI, ready for routing.
#[derive(Clone, Debug, PartialEq)]
pub struct UiEvent {
    pub origin: OriginId,
    pub key: Key,
    pub payload: Value,
}

#[derive(Clone, Debug)]
pub struct HitRegion {
    pub key: Key,
    pub origin: OriginId,
    /// Logical coordinates.
    pub rect: Rect,
    /// Ancestor clip; a point must be inside both to hit.
    pub clip: Rect,
    /// Click payload; None for hover-only regions (hoverable, edits) — a
    /// click on those emits no `UiEvent`.
    pub payload: Option<Value>,
    /// Drag tag when the node declared `on_drag`: pressing it starts a
    /// pointer-captured drag, and cursor motion until release emits
    /// `{kind="drag", phase, x, y, dx, dy, parent, tag}` events on this node.
    pub drag: Option<Value>,
    /// The node's parent rect (logical) — carried into drag payloads so
    /// handlers can turn absolute positions into fractions of the container
    /// (a splitter's ratio) without any geometry query API.
    pub parent_rect: Rect,
    /// Content-box origin of an editable text node; None for plain hits.
    pub edit_origin: Option<Vec2>,
    /// Key-sink tag when the node declared `on_key`: clicking it takes
    /// key focus, and key presses then arrive on it carrying this tag.
    pub key_sink: Option<Value>,
    /// A press on this node moves keyboard focus to it (an editor, a
    /// sink, a control, a `focusable` node — never a disabled one).
    pub focusable: bool,
    /// Window-chrome role: interactions become `WindowCommand`s, not events.
    pub window: Option<WindowRole>,
    /// Hover tag when the node declared `on_hover`: the pointer entering or
    /// leaving emits `{kind="hover", phase="enter"|"leave", tag}` on it.
    pub hover: Option<Value>,
    /// Hover group id (`NodeSpec::hover_group`): hovering or pressing any
    /// member lights up every member.
    pub group: Option<u64>,
    /// Sounds the node declared (`NodeSpec::click_sound` / `hover_sound`):
    /// a click / the pointer entering queues them as sound requests the
    /// core turns into audio commands.
    pub click_sound: Option<crate::resources::SoundId>,
    pub hover_sound: Option<crate::resources::SoundId>,
}

/// A scroll container's on-screen area, for wheel routing.
#[derive(Clone, Copy, Debug)]
pub struct ScrollRegion {
    pub key: Key,
    pub rect: Rect,
    pub clip: Rect,
    /// Outside the frame's modal scope: the bar still draws, the wheel
    /// and the thumb do nothing (see `docs/adr/0003-modal-surfaces.md`).
    pub inert: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollAxis {
    X,
    Y,
}

/// One scrollbar drawn this frame (logical coordinates), for thumb dragging
/// and track jumps. Rebuilt by `finish_frame` alongside the indicator quads.
#[derive(Clone, Copy, Debug)]
pub struct ScrollbarRegion {
    pub key: Key,
    pub axis: ScrollAxis,
    /// The thumb as drawn.
    pub thumb: Rect,
    /// The full track strip (the grabbable gutter).
    pub track: Rect,
    /// Thumb length along the axis.
    pub bar_len: f32,
    /// The container's max scroll offset on this axis.
    pub max: f32,
    /// Behind a modal: drawn, but not grabbable.
    pub inert: bool,
}

impl ScrollbarRegion {
    /// Offset for a cursor position, given where inside the thumb it grabbed.
    pub(crate) fn offset_for(&self, p: Vec2, grab: f32) -> f32 {
        let (pos, track_start, track_len) = match self.axis {
            ScrollAxis::X => (p.x, self.track.x, self.track.w),
            ScrollAxis::Y => (p.y, self.track.y, self.track.h),
        };
        let range = (track_len - self.bar_len).max(1.0);
        ((pos - track_start - grab) / range).clamp(0.0, 1.0) * self.max
    }
}

/// An in-flight pointer-captured drag on an `on_drag` node.
#[derive(Clone, Debug)]
struct DragState {
    key: Key,
    origin: OriginId,
    tag: Value,
    parent_rect: Rect,
    last: Vec2,
    /// Whether motion exceeded the click slop; suppresses the click on
    /// release so a node can carry both `on_click` and `on_drag`.
    moved: bool,
}

/// How far a press may wander before it stops counting as a click.
const DRAG_SLOP: f32 = 3.0;

#[derive(Default)]
pub struct Interaction {
    /// In paint order: later entries are on top.
    pub(crate) hits: Vec<HitRegion>,
    /// In paint order: later entries are on top (innermost last).
    pub(crate) scroll_regions: Vec<ScrollRegion>,
    /// This frame's scrollbars, topmost last (they draw over content).
    pub(crate) scrollbars: Vec<ScrollbarRegion>,
    /// Scrollbar thumb being dragged: which bar, and the grab point inside
    /// the thumb (axis-local). Offset math happens in `Core::handle_input`
    /// (it needs the `ScrollStore`).
    pub(crate) scrollbar_drag: Option<(Key, ScrollAxis, f32)>,
    /// Window intents produced by chrome nodes; drained by the driver via
    /// `Core::take_window_commands`.
    pub(crate) window_commands: Vec<WindowCommand>,
    /// Sounds nodes asked for (`click_sound` on click, `hover_sound` on
    /// enter); the core turns them into play commands (`take_sound_requests`).
    pub(crate) sound_requests: Vec<crate::resources::SoundId>,
    /// Pointer-captured drag on an `on_drag` node.
    drag: Option<DragState>,
    /// Last reported physical modifier state.
    modifiers: KeyMods,
    cursor: Option<Vec2>,
    hovered: Option<Key>,
    pressed: Option<Key>,
    /// Hover group of the hovered / pressed region, for group styling.
    hovered_group: Option<u64>,
    pressed_group: Option<u64>,
    /// The `on_hover` leave event for the hovered node, prepared on enter.
    hovered_leave: Option<UiEvent>,
    /// Hover enter/leave events raised outside `handle` — a new frame's hit
    /// regions changing what sits under a still cursor. Drained by the next
    /// `handle` or by `take_pending`.
    pending: Vec<UiEvent>,
}

impl Interaction {
    pub fn set_hits(&mut self, hits: Vec<HitRegion>) {
        self.hits = hits;
        let mut out = std::mem::take(&mut self.pending);
        self.refresh_hover(&mut out);
        self.pending = out;
    }

    /// Events produced outside `handle` (see `pending`); drivers take them
    /// after finishing a frame so a hover change under a still cursor is
    /// not delayed until the next input.
    pub fn take_pending(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.pending)
    }

    /// Drains the sounds nodes asked for since the last drain.
    pub(crate) fn take_sound_requests(&mut self) -> Vec<crate::resources::SoundId> {
        std::mem::take(&mut self.sound_requests)
    }

    /// Hands back the previous frame's hit buffer (cleared) so emission can
    /// refill it without reallocating.
    pub fn take_hit_buffer(&mut self) -> Vec<HitRegion> {
        let mut hits = std::mem::take(&mut self.hits);
        hits.clear();
        hits
    }

    pub fn cursor(&self) -> Option<Vec2> {
        self.cursor
    }

    /// Physical modifier state as of the last `InputEvent::Modifiers`.
    pub fn modifiers(&self) -> KeyMods {
        self.modifiers
    }

    /// This frame's hit regions in paint order (topmost last) — for hosts
    /// that mirror chrome regions into OS-level hit testing (e.g. answering
    /// Windows' WM_NCHITTEST so snap layouts and native caption behavior
    /// work over custom-drawn controls).
    pub fn hits(&self) -> &[HitRegion] {
        &self.hits
    }

    pub(crate) fn hit_at(&self, p: Vec2) -> Option<&HitRegion> {
        self.hits
            .iter()
            .rev()
            .find(|h| h.rect.contains(p) && h.clip.contains(p))
    }

    /// Topmost scroll container under the cursor, if any.
    pub fn scroll_target(&self) -> Option<Key> {
        let p = self.cursor?;
        self.scroll_regions
            .iter()
            .rev()
            .find(|r| !r.inert && r.rect.contains(p) && r.clip.contains(p))
            .map(|r| r.key)
    }

    /// Re-resolves the hovered region under the cursor, emitting `on_hover`
    /// leave/enter events when the hovered node changes.
    fn refresh_hover(&mut self, out: &mut Vec<UiEvent>) {
        let before = self.hovered;
        let idx = self.cursor.and_then(|p| {
            self.hits
                .iter()
                .rposition(|h| h.rect.contains(p) && h.clip.contains(p))
        });
        let (hovered, group) = match idx {
            Some(i) => (Some(self.hits[i].key), self.hits[i].group),
            None => (None, None),
        };
        self.hovered = hovered;
        self.hovered_group = group;
        if before == hovered {
            return;
        }
        // The old region may be gone from a new frame's hits, so the leave
        // event was prepared when the node was entered.
        out.extend(self.hovered_leave.take());
        if let Some(i) = idx {
            out.extend(Self::hover_event(&self.hits[i], "enter"));
            self.hovered_leave = Self::hover_event(&self.hits[i], "leave");
            if let Some(sound) = self.hits[i].hover_sound {
                self.sound_requests.push(sound);
            }
        }
    }

    fn hover_event(region: &HitRegion, phase: &str) -> Option<UiEvent> {
        let tag = region.hover.as_ref()?;
        let mut payload = Value::map([("kind", Value::str("hover")), ("phase", Value::str(phase))]);
        if *tag != Value::Null
            && let Value::Map(entries) = &mut payload
        {
            entries.push(("tag".to_string(), tag.clone()));
        }
        Some(UiEvent {
            origin: region.origin,
            key: region.key,
            payload,
        })
    }

    /// Topmost scrollbar whose track contains `p` (scrollbars draw over
    /// content, so they win hit-testing over it too).
    pub(crate) fn scrollbar_at(&self, p: Vec2) -> Option<ScrollbarRegion> {
        self.scrollbars
            .iter()
            .rev()
            .find(|b| !b.inert && b.track.contains(p))
            .copied()
    }

    /// Whether this bar is being thumb-dragged (for active styling).
    pub fn is_scrollbar_dragging(&self, key: Key, axis: ScrollAxis) -> bool {
        matches!(self.scrollbar_drag, Some((k, a, _)) if k == key && a == axis)
    }

    fn drag_event(state: &DragState, phase: &str, p: Vec2, d: Vec2) -> UiEvent {
        let pr = state.parent_rect;
        let mut payload = Value::map([
            ("kind", Value::str("drag")),
            ("phase", Value::str(phase)),
            ("x", Value::Float(p.x as f64)),
            ("y", Value::Float(p.y as f64)),
            ("dx", Value::Float(d.x as f64)),
            ("dy", Value::Float(d.y as f64)),
            (
                "parent",
                Value::map([
                    ("x", Value::Float(pr.x as f64)),
                    ("y", Value::Float(pr.y as f64)),
                    ("w", Value::Float(pr.w as f64)),
                    ("h", Value::Float(pr.h as f64)),
                ]),
            ),
        ]);
        if state.tag != Value::Null
            && let Value::Map(entries) = &mut payload
        {
            entries.push(("tag".to_string(), state.tag.clone()));
        }
        UiEvent {
            origin: state.origin,
            key: state.key,
            payload,
        }
    }

    pub fn handle(&mut self, ev: InputEvent, out: &mut Vec<UiEvent>) {
        out.append(&mut self.pending);
        match ev {
            InputEvent::CursorMoved(p) => {
                self.cursor = Some(p);
                self.refresh_hover(out);
                if let Some(drag) = &mut self.drag {
                    let d = Vec2::new(p.x - drag.last.x, p.y - drag.last.y);
                    if d.x != 0.0 || d.y != 0.0 {
                        drag.last = p;
                        if d.x.abs() + d.y.abs() > DRAG_SLOP {
                            drag.moved = true;
                        }
                        if drag.moved {
                            out.push(Self::drag_event(drag, "move", p, d));
                        }
                    }
                }
            }
            InputEvent::CursorLeft => {
                self.cursor = None;
                self.refresh_hover(out);
            }
            InputEvent::MouseDown(_) => {
                self.pressed = self.hovered;
                self.pressed_group = self.hovered_group;
                if let Some(h) = self.cursor.and_then(|p| self.hit_at(p)) {
                    if h.window == Some(WindowRole::Drag) {
                        // The OS drag steals subsequent mouse events, so don't
                        // leave a press pending.
                        self.pressed = None;
                        self.window_commands.push(WindowCommand::StartDrag);
                    } else if let Some(tag) = &h.drag {
                        let p = self.cursor.unwrap();
                        let state = DragState {
                            key: h.key,
                            origin: h.origin,
                            tag: tag.clone(),
                            parent_rect: h.parent_rect,
                            last: p,
                            moved: false,
                        };
                        out.push(Self::drag_event(&state, "start", p, Vec2::ZERO));
                        self.drag = Some(state);
                    }
                }
            }
            InputEvent::Modifiers(m) => {
                if m != self.modifiers {
                    self.modifiers = m;
                    out.push(UiEvent {
                        origin: OriginId::HOST,
                        key: Key::ROOT,
                        payload: m.to_value(),
                    });
                }
            }
            // Routed by the core (they need the retained stores).
            InputEvent::Scroll(_)
            | InputEvent::Text(_)
            | InputEvent::Preedit(..)
            | InputEvent::Key(..)
            | InputEvent::KeyDown(_)
            | InputEvent::Access(_) => {}
            InputEvent::MouseUp => {
                let dragged = self.drag.take().inspect(|drag| {
                    let p = self.cursor.unwrap_or(drag.last);
                    out.push(Self::drag_event(drag, "end", p, Vec2::ZERO));
                });
                // A press that actually dragged is not a click.
                let click_ok = !dragged.is_some_and(|d| d.moved);
                if click_ok
                    && let (Some(pressed), Some(hovered)) = (self.pressed, self.hovered)
                    && pressed == hovered
                    && let Some(region) = self.hits.iter().rev().find(|h| h.key == pressed)
                {
                    if let Some(sound) = region.click_sound {
                        self.sound_requests.push(sound);
                    }
                    match (region.window, &region.payload) {
                        (Some(WindowRole::Button(b)), _) => self.window_commands.push(b.command()),
                        (Some(WindowRole::Drag), _) | (None, None) => {}
                        (None, Some(payload)) => out.push(UiEvent {
                            origin: region.origin,
                            key: region.key,
                            payload: payload.clone(),
                        }),
                    }
                }
                self.pressed = None;
                self.pressed_group = None;
            }
        }
    }

    pub fn is_hovered(&self, key: Key) -> bool {
        self.hovered == Some(key)
    }

    pub fn is_pressed(&self, key: Key) -> bool {
        self.pressed == Some(key) && (self.hovered == Some(key) || self.drag_captured(key))
    }

    /// Whether a pointer-captured drag is running on `key`. The press is
    /// stuck to that node until release, so it stays pressed even when the
    /// cursor wanders off it (hover itself keeps following the cursor, so
    /// drop targets under the drag still light up).
    fn drag_captured(&self, key: Key) -> bool {
        self.drag.as_ref().is_some_and(|d| d.key == key)
    }

    /// The hovered node, if any (its key from the last finished frame).
    pub fn hovered(&self) -> Option<Key> {
        self.hovered
    }

    /// Whether any member of hover group `group` is hovered.
    pub fn is_group_hovered(&self, group: u64) -> bool {
        self.hovered_group == Some(group)
    }

    /// Whether the press started on a member of `group` and the pointer is
    /// still over one (the group analogue of `is_pressed`).
    pub fn is_group_pressed(&self, group: u64) -> bool {
        self.pressed_group == Some(group)
            && (self.hovered_group == Some(group) || self.drag.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(key: Key, origin: u16, x: f32, y: f32, w: f32, h: f32, tag: &str) -> HitRegion {
        HitRegion {
            key,
            origin: OriginId(origin),
            rect: Rect::new(x, y, w, h),
            clip: Rect::new(-1e9, -1e9, 2e9, 2e9),
            payload: Some(Value::str(tag)),
            drag: None,
            parent_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            edit_origin: None,
            key_sink: None,
            focusable: true,
            window: None,
            hover: None,
            group: None,
            click_sound: None,
            hover_sound: None,
        }
    }

    fn drive(interaction: &mut Interaction, events: &[InputEvent]) -> Vec<UiEvent> {
        let mut out = Vec::new();
        for ev in events {
            interaction.handle(ev.clone(), &mut out);
        }
        out
    }

    #[test]
    fn click_inside_produces_event() {
        let mut it = Interaction::default();
        let k = Key::ROOT.str("btn");
        it.set_hits(vec![region(k, 0, 10.0, 10.0, 100.0, 30.0, "go")]);
        let evs = drive(
            &mut it,
            &[
                InputEvent::CursorMoved(Vec2::new(50.0, 20.0)),
                InputEvent::MouseDown(1),
                InputEvent::MouseUp,
            ],
        );
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].key, k);
        assert_eq!(evs[0].payload.as_str(), Some("go"));
    }

    #[test]
    fn press_then_drag_away_does_not_click() {
        let mut it = Interaction::default();
        let k = Key::ROOT.str("btn");
        it.set_hits(vec![region(k, 0, 0.0, 0.0, 50.0, 50.0, "go")]);
        let evs = drive(
            &mut it,
            &[
                InputEvent::CursorMoved(Vec2::new(10.0, 10.0)),
                InputEvent::MouseDown(1),
                InputEvent::CursorMoved(Vec2::new(500.0, 500.0)),
                InputEvent::MouseUp,
            ],
        );
        assert!(evs.is_empty());
    }

    #[test]
    fn topmost_region_wins_on_overlap() {
        let mut it = Interaction::default();
        let bottom = Key::ROOT.str("bottom");
        let top = Key::ROOT.str("top");
        it.set_hits(vec![
            region(bottom, 0, 0.0, 0.0, 100.0, 100.0, "bottom"),
            region(top, 0, 25.0, 25.0, 50.0, 50.0, "top"),
        ]);
        let evs = drive(
            &mut it,
            &[
                InputEvent::CursorMoved(Vec2::new(50.0, 50.0)),
                InputEvent::MouseDown(1),
                InputEvent::MouseUp,
            ],
        );
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].key, top);
    }

    #[test]
    fn event_carries_declaring_origin() {
        let mut it = Interaction::default();
        let k = Key::ROOT.str("ext-btn");
        it.set_hits(vec![region(k, 3, 0.0, 0.0, 10.0, 10.0, "x")]);
        let evs = drive(
            &mut it,
            &[
                InputEvent::CursorMoved(Vec2::new(5.0, 5.0)),
                InputEvent::MouseDown(1),
                InputEvent::MouseUp,
            ],
        );
        assert_eq!(evs[0].origin, OriginId(3));
    }

    #[test]
    fn cursor_leave_clears_hover() {
        let mut it = Interaction::default();
        let k = Key::ROOT.str("btn");
        it.set_hits(vec![region(k, 0, 0.0, 0.0, 50.0, 50.0, "x")]);
        drive(&mut it, &[InputEvent::CursorMoved(Vec2::new(10.0, 10.0))]);
        assert!(it.is_hovered(k));
        drive(&mut it, &[InputEvent::CursorLeft]);
        assert!(!it.is_hovered(k));
        // Click after leaving produces nothing.
        let evs = drive(&mut it, &[InputEvent::MouseDown(1), InputEvent::MouseUp]);
        assert!(evs.is_empty());
    }

    #[test]
    fn new_frame_hits_preserve_hover_state() {
        let mut it = Interaction::default();
        let k = Key::ROOT.str("btn");
        it.set_hits(vec![region(k, 0, 0.0, 0.0, 50.0, 50.0, "x")]);
        drive(&mut it, &[InputEvent::CursorMoved(Vec2::new(10.0, 10.0))]);
        // Same widget moved: hover follows the rect under the cursor.
        it.set_hits(vec![region(k, 0, 100.0, 100.0, 50.0, 50.0, "x")]);
        assert!(!it.is_hovered(k));
        it.set_hits(vec![region(k, 0, 0.0, 0.0, 50.0, 50.0, "x")]);
        assert!(it.is_hovered(k));
    }
}
