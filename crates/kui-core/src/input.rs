//! Input handling and event production. Hit regions come from the previous
//! frame's layout (the standard immediate-mode trade); events leave as plain
//! data tagged with the origin that declared them, so the runner can route to
//! the host app or an extension without knowing what either looks like.

use crate::cursor::CursorShape;
use crate::geom::{Rect, Vec2};
use crate::key::Key;
use crate::tree::OriginId;
use crate::value::Value;
use crate::window::{WindowCommand, WindowId, WindowRole};

#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    /// Logical coordinates.
    CursorMoved(Vec2),
    CursorLeft,
    /// A button press. `clicks` is driver-measured multi-click state
    /// (1 = single, 2 = double, 3+ = triple) — the core is clock-free, so
    /// click timing lives with whoever owns the event loop. Only the
    /// primary button presses, drags and clicks; see [`MouseButton`].
    MouseDown {
        button: MouseButton,
        clicks: u8,
    },
    /// The release of `button`. A non-primary release resolves nothing:
    /// the primary button is the one that can be holding a press.
    MouseUp {
        button: MouseButton,
    },
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
    /// The release of a key, routed the way [`InputEvent::KeyDown`] is —
    /// so a held-key interaction (WASD, press-and-hold to preview, a key
    /// that arms a mode) is a pair of events, not a guess about timing.
    /// Only a key whose press was delivered produces one: a release the
    /// focused sink never saw the press of is dropped, and focus moving
    /// away while a key is held synthesizes the release first (see
    /// `Core::release_held_keys`). The core clears `text` and `repeat` on
    /// the way out — a release inserts nothing and never repeats.
    KeyUp(KeyPress),
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

/// Which button a press came from — driver-facing rather than shaped after
/// any one windowing library, so every driver maps its own vocabulary onto
/// this one.
///
/// Only [`MouseButton::Primary`] drives the pointer model: it presses,
/// drags, places the caret and produces `on_click`. A
/// [`MouseButton::Secondary`] press asks the node under it for a context
/// menu (`NodeSpec::on_context_menu`) and touches nothing else — not
/// focus, not the caret, not a scrollbar thumb — because a right-click on
/// a selection has to leave that selection alone. Nothing routes the
/// remaining buttons yet; they arrive so a driver need not drop them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseButton {
    /// The button that clicks and drags. The OS has already applied a
    /// left-handed swap, so this is not necessarily the left one.
    #[default]
    Primary,
    /// The context-menu button.
    Secondary,
    Middle,
    /// A button this vocabulary does not name (back, forward, thumb
    /// buttons), by driver index.
    Other(u8),
}

impl MouseButton {
    /// The number bindings pass buttons as: 0 primary, 1 secondary,
    /// 2 middle, `3 + n` for `Other(n)`.
    pub fn code(self) -> u32 {
        match self {
            MouseButton::Primary => 0,
            MouseButton::Secondary => 1,
            MouseButton::Middle => 2,
            MouseButton::Other(n) => 3 + n as u32,
        }
    }

    /// Inverse of [`MouseButton::code`]; anything past the named three is
    /// an `Other`, saturating rather than wrapping.
    pub fn from_code(code: u32) -> Self {
        match code {
            0 => MouseButton::Primary,
            1 => MouseButton::Secondary,
            2 => MouseButton::Middle,
            n => MouseButton::Other((n - 3).min(u8::MAX as u32) as u8),
        }
    }

    /// The three named buttons by name, for bindings that spell them as
    /// strings (`"primary"`, `"secondary"`, `"middle"`). `Other` has no
    /// name: a binding that needs one takes a [`MouseButton::code`].
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "primary" => Some(MouseButton::Primary),
            "secondary" => Some(MouseButton::Secondary),
            "middle" => Some(MouseButton::Middle),
            _ => None,
        }
    }
}

impl InputEvent {
    /// A primary-button press — the spelling drivers and tests want when
    /// they only ever send one button.
    pub fn mouse_down(clicks: u8) -> Self {
        InputEvent::MouseDown {
            button: MouseButton::Primary,
            clicks,
        }
    }

    /// A primary-button release.
    pub fn mouse_up() -> Self {
        InputEvent::MouseUp {
            button: MouseButton::Primary,
        }
    }
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
    ///
    /// A layout that produces something outside ASCII does not reach here:
    /// the driver substitutes the US-QWERTY letter at that position, so a
    /// keymap written in Latin keeps working on a Cyrillic, Greek, Hebrew
    /// or Arabic layout instead of matching nothing at all. See
    /// [`KeyPress::physical`], and `docs/adr/0002` decision 11 for why the
    /// layout still wins whenever it speaks ASCII.
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

    /// The inverse of [`KeyCode::name`]: the name a binding spells a key
    /// with. A single character is that character (already
    /// layout-resolved, so `"W"` and `"$"` arrive as themselves), `"f1"`
    /// .. `"f24"` a function key, and the rest are the names above.
    /// `None` for a name this vocabulary does not know — every binding
    /// that takes keys as strings parses them here, so they cannot drift
    /// apart.
    pub fn from_name(s: &str) -> Option<KeyCode> {
        let mut chars = s.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return Some(KeyCode::Char(c));
        }
        if let Some(n) = s.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
            && (1..=24).contains(&n)
        {
            return Some(KeyCode::F(n));
        }
        Some(match s {
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "backspace" => KeyCode::Backspace,
            "delete" => KeyCode::Delete,
            "enter" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "escape" => KeyCode::Escape,
            "space" => KeyCode::Space,
            "insert" => KeyCode::Insert,
            "unknown" => KeyCode::Unknown,
            _ => return None,
        })
    }
}

/// Which half of a key's life an event reports. Both halves arrive as one
/// `{kind="key"}` payload — the way a drag's three phases and a hover's
/// two do — so an app binds one handler and matches `phase`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyPhase {
    #[default]
    Down,
    Up,
}

impl KeyPhase {
    /// The payload spelling: `"down"` / `"up"`.
    pub fn name(self) -> &'static str {
        match self {
            KeyPhase::Down => "down",
            KeyPhase::Up => "up",
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
    /// Where the key *is*, independent of the layout: the US-QWERTY key at
    /// that position, in the same vocabulary as `code`. The key left of B
    /// is `Char('v')` on every layout on earth, so a chord map written
    /// against this one binds a shape rather than a character — what a
    /// game's WASD wants, and what a keymap wants when it would rather be
    /// wrong about the label than wrong about the finger.
    ///
    /// `code` is usually the better default; see its note. `Unknown` when
    /// the platform reports a position this vocabulary cannot name.
    pub physical: KeyCode,
    pub mods: KeyMods,
    /// What this press would insert, if anything — already resolved through
    /// the keyboard layout. `None` for pure navigation and chords.
    pub text: Option<String>,
    /// Set when the press came from OS key repeat.
    pub repeat: bool,
}

impl KeyPress {
    /// A press whose position is its own code — what a layout that agrees
    /// with US-QWERTY produces, and the sane reading of an injected press:
    /// naming a key is saying which key was pressed.
    pub fn new(code: KeyCode, mods: KeyMods) -> Self {
        Self {
            code,
            physical: code,
            mods,
            text: None,
            repeat: false,
        }
    }

    /// Says which physical key produced this press, when the layout put a
    /// different code on it (`⌥v` on Dvorak: code `v`, physical `.`).
    pub fn with_physical(mut self, physical: KeyCode) -> Self {
        self.physical = physical;
        self
    }

    /// The press a driver builds from the two things the OS tells it: what
    /// the active layout put on the key, and which key it was. Every driver
    /// resolves `code` the same way because they all come through here.
    ///
    /// The layout wins while it speaks ASCII, so a chord lands on the key
    /// the user can *see* — Dvorak's `⌥v` on the key printed V, AZERTY's
    /// `⌘a` on the one printed A, QWERTZ's `⌘z` on the one printed Z. A
    /// layout that produces anything else (Cyrillic, Greek, Hebrew, Arabic)
    /// would make every Latin keymap in every app match nothing at all, so
    /// the US-QWERTY letter at that position stands in; this is the rule
    /// browsers use to keep `⌘C` copying on a Russian layout. A layout key
    /// this vocabulary cannot name falls back the same way.
    ///
    /// `physical` is reported either way, for a keymap that would rather
    /// bind the finger than the label. See `docs/adr/0002` decision 11.
    pub fn from_layout(layout: KeyCode, physical: KeyCode, mods: KeyMods) -> Self {
        let code = match layout {
            KeyCode::Char(c) if !c.is_ascii() => physical,
            KeyCode::Unknown => physical,
            named_or_ascii => named_or_ascii,
        };
        Self {
            code,
            physical,
            mods,
            text: None,
            repeat: false,
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Strips a press down to what a release reports: nothing is inserted
    /// on the way up, and a release never comes from key repeat.
    pub fn released(mut self) -> Self {
        self.text = None;
        self.repeat = false;
        self
    }

    /// The payload form crossing into events, C, and Lua:
    /// `{kind="key", phase="down"|"up", code="w", physical="w", shift=,
    /// ctrl=, alt=, super=, text=, repeat=}`.
    pub fn to_value(&self, phase: KeyPhase) -> Value {
        Value::map([
            ("kind", Value::str("key")),
            ("phase", Value::str(phase.name())),
            ("code", Value::Str(self.code.name())),
            ("physical", Value::Str(self.physical.name())),
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
    /// Which window the event came from — a *new* field and not a second
    /// reading of `origin`, which says which frontend drew the node and
    /// answers `HOST` for a window an extension also draws into.
    ///
    /// Producers cannot fill it in: a hit test, the edit buffer and the
    /// audio queue know nothing about windows. They leave it
    /// [`WindowId::MAIN`] and the core stamps its own `env.window.id` over
    /// it as the event leaves (`Core::handle_input`,
    /// `Core::take_pending_events`) — one core is one window, so that is the
    /// whole answer. A driver that builds an event itself stamps it itself.
    pub window: WindowId,
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
    /// Context-menu tag when the node declared `on_context_menu`: a
    /// secondary-button press emits `{kind="contextmenu", x, y, tag}` on
    /// it. Like a click, the topmost region under the pointer is the one
    /// asked — a region that declared none swallows the press.
    pub context_menu: Option<Value>,
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
    /// Pointer shape declared by the node (`NodeSpec::cursor`), overriding
    /// what the rest of this region would derive. None = derive.
    pub cursor: Option<CursorShape>,
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
    /// Where the press landed. Every `dx`/`dy` the drag reports is the
    /// displacement from here — `start` is zero, a `move` is where the
    /// pointer is now, `end` is the whole distance — so a handler commits
    /// from any phase without summing anything, and the slop below drops
    /// nothing from the total (backlog F2).
    press: Vec2,
    /// Where the pointer was last seen: the `end` position of a drag
    /// released while the cursor was outside the window.
    last: Vec2,
    /// Whether motion left the click slop; suppresses the click on
    /// release so a node can carry both `on_click` and `on_drag`. Once
    /// set it stays set — a drag that wanders back is still a drag.
    moved: bool,
}

/// How far from the press point a pointer may wander before the press
/// stops counting as a click and the drag starts reporting `move`s.
/// Measured from the press, not per event, so a slow pointer that never
/// covers 3 px between two events still gets there.
const DRAG_SLOP: f32 = 3.0;

impl DragState {
    /// The displacement `p` is from the press point.
    fn displacement(&self, p: Vec2) -> Vec2 {
        Vec2::new(p.x - self.press.x, p.y - self.press.y)
    }
}

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
    /// The window this core draws, for the commands chrome nodes issue —
    /// a hit region has no window, so the core writes it here from
    /// `env.window.id` before routing each input.
    pub(crate) window: WindowId,
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
            window: WindowId::MAIN,
            key: region.key,
            payload,
        })
    }

    fn context_menu_event(region: &HitRegion, p: Vec2) -> Option<UiEvent> {
        let tag = region.context_menu.as_ref()?;
        let mut payload = Value::map([
            ("kind", Value::str("contextmenu")),
            ("x", Value::Float(p.x as f64)),
            ("y", Value::Float(p.y as f64)),
        ]);
        if *tag != Value::Null
            && let Value::Map(entries) = &mut payload
        {
            entries.push(("tag".to_string(), tag.clone()));
        }
        Some(UiEvent {
            origin: region.origin,
            window: WindowId::MAIN,
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
            window: WindowId::MAIN,
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
                if let Some(drag) = &mut self.drag
                    && p != drag.last
                {
                    drag.last = p;
                    let d = drag.displacement(p);
                    if d.x.abs() + d.y.abs() > DRAG_SLOP {
                        drag.moved = true;
                    }
                    if drag.moved {
                        out.push(Self::drag_event(drag, "move", p, d));
                    }
                }
            }
            InputEvent::CursorLeft => {
                self.cursor = None;
                self.refresh_hover(out);
            }
            InputEvent::MouseDown { button, .. } if button != MouseButton::Primary => {
                // Nothing but the primary button presses: no pressed
                // state, so a release cannot become a click, and a drag
                // already in flight keeps its capture. A secondary press
                // asks whatever is under the pointer for a context menu.
                if button == MouseButton::Secondary
                    && let Some(p) = self.cursor
                    && let Some(ev) = self.hit_at(p).and_then(|h| Self::context_menu_event(h, p))
                {
                    out.push(ev);
                }
            }
            InputEvent::MouseDown { .. } => {
                self.pressed = self.hovered;
                self.pressed_group = self.hovered_group;
                if let Some(h) = self.cursor.and_then(|p| self.hit_at(p)) {
                    if h.window == Some(WindowRole::Drag) {
                        // The OS drag steals subsequent mouse events, so don't
                        // leave a press pending.
                        self.pressed = None;
                        self.window_commands
                            .push(WindowCommand::StartDrag(self.window));
                    } else if let Some(tag) = &h.drag {
                        let p = self.cursor.unwrap();
                        let state = DragState {
                            key: h.key,
                            origin: h.origin,
                            tag: tag.clone(),
                            parent_rect: h.parent_rect,
                            press: p,
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
                        window: WindowId::MAIN,
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
            | InputEvent::KeyUp(_)
            | InputEvent::Access(_) => {}
            InputEvent::MouseUp { button } if button != MouseButton::Primary => {}
            InputEvent::MouseUp { .. } => {
                let dragged = self.drag.take().inspect(|drag| {
                    let p = self.cursor.unwrap_or(drag.last);
                    out.push(Self::drag_event(drag, "end", p, drag.displacement(p)));
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
                        (Some(WindowRole::Button(b)), _) => {
                            self.window_commands.push(b.command(self.window))
                        }
                        (Some(WindowRole::Drag), _) | (None, None) => {}
                        (None, Some(payload)) => out.push(UiEvent {
                            origin: region.origin,
                            window: WindowId::MAIN,
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

    /// The pointer shape for where the pointer is now (see
    /// [`crate::cursor`]). Derived from the topmost region under it — the
    /// same region a click would go to — so nothing declares a cursor for
    /// the ordinary cases; a region's own `cursor` overrides the
    /// derivation.
    pub fn cursor_shape(&self) -> CursorShape {
        // A captured drag owns the pointer: the shape stays the dragged
        // node's however far the cursor wanders off it.
        if let Some(drag) = &self.drag {
            return self
                .hits
                .iter()
                .rev()
                .find(|h| h.key == drag.key)
                .and_then(|h| h.cursor)
                .unwrap_or(CursorShape::Grabbing);
        }
        // Scrollbars draw over content and win the press, so they win the
        // shape too — an overlay bar across an editor is not an I-beam.
        if self.scrollbar_drag.is_some() {
            return CursorShape::Default;
        }
        let Some(p) = self.cursor else {
            return CursorShape::Default;
        };
        if self.scrollbar_at(p).is_some() {
            return CursorShape::Default;
        }
        let Some(region) = self.hit_at(p) else {
            return CursorShape::Default;
        };
        region.cursor.unwrap_or_else(|| Self::derived_shape(region))
    }

    /// The shape a region implies when it declares none.
    fn derived_shape(region: &HitRegion) -> CursorShape {
        match region {
            // Window chrome is the platform's: every desktop points at a
            // titlebar and its buttons with the plain arrow.
            _ if region.window.is_some() => CursorShape::Default,
            _ if region.edit_origin.is_some() => CursorShape::Text,
            // Draggable before clickable: a node can be both, and the
            // grab is the gesture that starts on the press.
            _ if region.drag.is_some() => CursorShape::Grab,
            _ if region.payload.is_some() || region.focusable => CursorShape::Pointer,
            // Hover-only regions (a tooltip badge, a modal's backdrop) and
            // disabled nodes, whose payloads the frame already stripped.
            _ => CursorShape::Default,
        }
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
            context_menu: None,
            focusable: true,
            window: None,
            hover: None,
            group: None,
            click_sound: None,
            hover_sound: None,
            cursor: None,
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
                InputEvent::mouse_down(1),
                InputEvent::mouse_up(),
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
                InputEvent::mouse_down(1),
                InputEvent::CursorMoved(Vec2::new(500.0, 500.0)),
                InputEvent::mouse_up(),
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
                InputEvent::mouse_down(1),
                InputEvent::mouse_up(),
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
                InputEvent::mouse_down(1),
                InputEvent::mouse_up(),
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
        let evs = drive(
            &mut it,
            &[InputEvent::mouse_down(1), InputEvent::mouse_up()],
        );
        assert!(evs.is_empty());
    }

    #[test]
    fn secondary_press_asks_the_node_under_it_for_a_menu() {
        let mut it = Interaction::default();
        let k = Key::ROOT.str("panel");
        let mut r = region(k, 0, 0.0, 0.0, 100.0, 100.0, "click-me");
        r.context_menu = Some(Value::str("panel-menu"));
        it.set_hits(vec![r]);
        let evs = drive(
            &mut it,
            &[
                InputEvent::CursorMoved(Vec2::new(40.0, 30.0)),
                InputEvent::MouseDown {
                    button: MouseButton::Secondary,
                    clicks: 1,
                },
                InputEvent::MouseUp {
                    button: MouseButton::Secondary,
                },
            ],
        );
        // The menu arrives on the press, with the point to open it at, and
        // the release adds nothing — no click, though the node has one.
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].key, k);
        assert_eq!(
            evs[0].payload.get("kind").unwrap().as_str(),
            Some("contextmenu")
        );
        assert_eq!(evs[0].payload.get("x").unwrap().as_float(), Some(40.0));
        assert_eq!(evs[0].payload.get("y").unwrap().as_float(), Some(30.0));
        assert_eq!(
            evs[0].payload.get("tag").unwrap().as_str(),
            Some("panel-menu")
        );
        assert!(!it.is_pressed(k));
    }

    #[test]
    fn secondary_press_on_a_node_without_a_menu_emits_nothing() {
        let mut it = Interaction::default();
        let k = Key::ROOT.str("btn");
        it.set_hits(vec![region(k, 0, 0.0, 0.0, 100.0, 100.0, "go")]);
        let evs = drive(
            &mut it,
            &[
                InputEvent::CursorMoved(Vec2::new(40.0, 30.0)),
                InputEvent::MouseDown {
                    button: MouseButton::Secondary,
                    clicks: 1,
                },
                InputEvent::MouseUp {
                    button: MouseButton::Secondary,
                },
            ],
        );
        assert!(evs.is_empty());
    }

    /// A secondary press in the middle of a primary one leaves the press
    /// alone: the primary release still clicks.
    #[test]
    fn secondary_press_does_not_interrupt_a_held_primary() {
        let mut it = Interaction::default();
        let k = Key::ROOT.str("btn");
        it.set_hits(vec![region(k, 0, 0.0, 0.0, 100.0, 100.0, "go")]);
        let evs = drive(
            &mut it,
            &[
                InputEvent::CursorMoved(Vec2::new(40.0, 30.0)),
                InputEvent::mouse_down(1),
                InputEvent::MouseDown {
                    button: MouseButton::Secondary,
                    clicks: 1,
                },
                InputEvent::MouseUp {
                    button: MouseButton::Secondary,
                },
            ],
        );
        assert!(evs.is_empty());
        assert!(it.is_pressed(k));
        let evs = drive(&mut it, &[InputEvent::mouse_up()]);
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].payload.as_str(), Some("go"));
    }

    #[test]
    fn middle_press_routes_nowhere() {
        let mut it = Interaction::default();
        let k = Key::ROOT.str("panel");
        let mut r = region(k, 0, 0.0, 0.0, 100.0, 100.0, "go");
        r.context_menu = Some(Value::str("panel-menu"));
        it.set_hits(vec![r]);
        let evs = drive(
            &mut it,
            &[
                InputEvent::CursorMoved(Vec2::new(40.0, 30.0)),
                InputEvent::MouseDown {
                    button: MouseButton::Middle,
                    clicks: 1,
                },
                InputEvent::MouseUp {
                    button: MouseButton::Middle,
                },
            ],
        );
        assert!(evs.is_empty());
    }

    #[test]
    fn button_codes_round_trip() {
        for b in [
            MouseButton::Primary,
            MouseButton::Secondary,
            MouseButton::Middle,
            MouseButton::Other(0),
            MouseButton::Other(9),
        ] {
            assert_eq!(MouseButton::from_code(b.code()), b);
        }
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
