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
    MouseDown,
    MouseUp,
    /// Wheel/trackpad delta in logical px (positive y = scroll up).
    Scroll(Vec2),
    /// Committed text (typing, IME commit, paste). Routed to the focused editor.
    Text(String),
    /// Navigation/editing key. Routed to the focused editor.
    Key(EditKey, Mods),
    /// A full key press, routed to whatever holds key focus (see
    /// `Core::set_key_focus`). Apps that own their own text model take
    /// keys through this instead of the editor path.
    KeyDown(KeyPress),
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
        Self { code, mods, text: None, repeat: false }
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
    pub payload: Value,
    /// Content-box origin of an editable text node; None for plain hits.
    pub edit_origin: Option<Vec2>,
    /// Key-sink tag when the node declared `on_key`: clicking it takes
    /// key focus, and key presses then arrive on it carrying this tag.
    pub key_sink: Option<Value>,
    /// Window-chrome role: interactions become `WindowCommand`s, not events.
    pub window: Option<WindowRole>,
}

/// A scroll container's on-screen area, for wheel routing.
#[derive(Clone, Copy, Debug)]
pub struct ScrollRegion {
    pub key: Key,
    pub rect: Rect,
    pub clip: Rect,
}

#[derive(Default)]
pub struct Interaction {
    /// In paint order: later entries are on top.
    pub(crate) hits: Vec<HitRegion>,
    /// In paint order: later entries are on top (innermost last).
    pub(crate) scroll_regions: Vec<ScrollRegion>,
    /// Window intents produced by chrome nodes; drained by the driver via
    /// `Core::take_window_commands`.
    pub(crate) window_commands: Vec<WindowCommand>,
    cursor: Option<Vec2>,
    hovered: Option<Key>,
    pressed: Option<Key>,
}

impl Interaction {
    pub fn set_hits(&mut self, hits: Vec<HitRegion>) {
        self.hits = hits;
        self.refresh_hover();
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

    /// This frame's hit regions in paint order (topmost last) — for hosts
    /// that mirror chrome regions into OS-level hit testing (e.g. answering
    /// Windows' WM_NCHITTEST so snap layouts and native caption behavior
    /// work over custom-drawn controls).
    pub fn hits(&self) -> &[HitRegion] {
        &self.hits
    }

    pub(crate) fn hit_at(&self, p: Vec2) -> Option<&HitRegion> {
        self.hits.iter().rev().find(|h| h.rect.contains(p) && h.clip.contains(p))
    }

    /// Topmost scroll container under the cursor, if any.
    pub fn scroll_target(&self) -> Option<Key> {
        let p = self.cursor?;
        self.scroll_regions
            .iter()
            .rev()
            .find(|r| r.rect.contains(p) && r.clip.contains(p))
            .map(|r| r.key)
    }

    fn refresh_hover(&mut self) {
        self.hovered = self.cursor.and_then(|p| self.hit_at(p)).map(|h| h.key);
    }

    pub fn handle(&mut self, ev: InputEvent, out: &mut Vec<UiEvent>) {
        match ev {
            InputEvent::CursorMoved(p) => {
                self.cursor = Some(p);
                self.refresh_hover();
            }
            InputEvent::CursorLeft => {
                self.cursor = None;
                self.hovered = None;
            }
            InputEvent::MouseDown => {
                self.pressed = self.hovered;
                if let Some(h) = self.cursor.and_then(|p| self.hit_at(p))
                    && h.window == Some(WindowRole::Drag)
                {
                    // The OS drag steals subsequent mouse events, so don't
                    // leave a press pending.
                    self.pressed = None;
                    self.window_commands.push(WindowCommand::StartDrag);
                }
            }
            // Routed by the core (they need the retained stores).
            InputEvent::Scroll(_)
            | InputEvent::Text(_)
            | InputEvent::Key(..)
            | InputEvent::KeyDown(_) => {}
            InputEvent::MouseUp => {
                if let (Some(pressed), Some(hovered)) = (self.pressed, self.hovered)
                    && pressed == hovered
                    && let Some(region) = self.hits.iter().rev().find(|h| h.key == pressed)
                {
                    match region.window {
                        Some(WindowRole::Button(b)) => self.window_commands.push(b.command()),
                        Some(WindowRole::Drag) => {}
                        None => out.push(UiEvent {
                            origin: region.origin,
                            key: region.key,
                            payload: region.payload.clone(),
                        }),
                    }
                }
                self.pressed = None;
            }
        }
    }

    pub fn is_hovered(&self, key: Key) -> bool {
        self.hovered == Some(key)
    }

    pub fn is_pressed(&self, key: Key) -> bool {
        self.pressed == Some(key) && self.hovered == Some(key)
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
            payload: Value::str(tag),
            edit_origin: None,
            key_sink: None,
            window: None,
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
            &[InputEvent::CursorMoved(Vec2::new(50.0, 20.0)), InputEvent::MouseDown, InputEvent::MouseUp],
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
                InputEvent::MouseDown,
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
            &[InputEvent::CursorMoved(Vec2::new(50.0, 50.0)), InputEvent::MouseDown, InputEvent::MouseUp],
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
            &[InputEvent::CursorMoved(Vec2::new(5.0, 5.0)), InputEvent::MouseDown, InputEvent::MouseUp],
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
        let evs = drive(&mut it, &[InputEvent::MouseDown, InputEvent::MouseUp]);
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
