//! Input handling and event production. Hit regions come from the previous
//! frame's layout (the standard immediate-mode trade); events leave as plain
//! data tagged with the origin that declared them, so the runner can route to
//! the host app or an extension without knowing what either looks like.

use crate::geom::{Rect, Vec2};
use crate::key::Key;
use crate::tree::OriginId;
use crate::value::Value;

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
            }
            // Routed by the core (they need the retained stores).
            InputEvent::Scroll(_) | InputEvent::Text(_) | InputEvent::Key(..) => {}
            InputEvent::MouseUp => {
                if let (Some(pressed), Some(hovered)) = (self.pressed, self.hovered)
                    && pressed == hovered
                    && let Some(region) = self.hits.iter().rev().find(|h| h.key == pressed)
                {
                    out.push(UiEvent {
                        origin: region.origin,
                        key: region.key,
                        payload: region.payload.clone(),
                    });
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
