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
    /// Committed text (typing, paste). Routed to the focused editor; with
    /// none, a printable character presses or searches the focused
    /// control. Never delivered to an `onKey` sink: the raw press already
    /// reached it as a `key` event carrying `text`, and a sink hearing
    /// both would type every character twice.
    Text(String),
    /// Text an IME committed at the end of a composition (backlog C17).
    /// Routed like `Text` to a focused editor; otherwise delivered to the
    /// focused sink as `{kind:"text", text, tag}` — the one committed text
    /// the platform never reports as a key press with `text`, so it is the
    /// one a sink has to be told about. Drivers send `Ime::Commit` here and
    /// keep typing on `Text`.
    Commit(String),
    /// The clipboard's answer to a paste the app asked for
    /// (`Core::request_paste`, a menu's Paste), with what the pasteboard
    /// said about it (backlog F84). Routed exactly as [`InputEvent::Commit`]
    /// is — a focused editor takes it as typing, a focused sink hears
    /// `{kind:"text", text, tag}` — and the sink's event gains
    /// `concealed: true` and `transient: true` for the markers that are
    /// set, and nothing for those that are not.
    ///
    /// A variant of its own rather than two fields on `Commit`, so every
    /// match on a commit still compiles and a driver that answers with a
    /// bare `Commit` (an older C or Node host) is still an answer: both
    /// clear the one-ask gate (backlog AR34), and a `Commit` is a paste
    /// whose pasteboard marked nothing.
    Paste {
        text: String,
        marks: ClipboardMarks,
    },
    /// In-progress IME composition (text and the caret byte range inside
    /// it), inserted inline at the focused editor's caret as an uncommitted
    /// marked range: following text shifts and the paragraph rewraps.
    /// Empty text cancels it; the commit arrives separately as `Commit`.
    /// With no editor focused it goes to the focused sink as
    /// `{kind:"preedit", text, cursor: [start, end] | null, tag}`, an
    /// empty `text` meaning the composition ended without a commit.
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
    /// A force click at a point in logical viewport coordinates: the
    /// press deepened past the second stage of a Force Touch trackpad
    /// (`docs/adr/0017-selection-as-a-scope.md`, decision 6).
    ///
    /// Routed like the secondary press — the topmost node under the point,
    /// no focus moved, no caret placed, no click — because it arrives
    /// *during* an ordinary press that is still running, and the click
    /// that press produces still happens afterwards. Over text it selects
    /// the word and asks the host to look it up; anywhere else it reaches
    /// a node declaring `on_force_click`.
    ///
    /// macOS-only in practice: no other platform winit supports reports
    /// pressure at all, and there a user can switch it off.
    ForceClick(Vec2),
    /// Files dragged in from the OS are over the window at `at` (logical
    /// viewport coordinates) — entering and moving alike: the core tells
    /// the two apart by whether the zone under the point changed, and a
    /// change is the old zone's `leave` then the new one's `enter`
    /// (`docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md`,
    /// decision 4). `paths` are the OS paths as the driver reported them.
    /// A repeat at the same point emits nothing.
    DragFiles {
        paths: Vec<String>,
        at: Vec2,
    },
    /// The dragged files were released at `at`: the zone there hears
    /// `{kind="drop", phase="drop"}` and nothing hears a `leave`; with no
    /// zone there, nothing is emitted and whatever was lit hears its
    /// `leave`.
    DropFiles {
        paths: Vec<String>,
        at: Vec2,
    },
    /// The dragged files left the window, or the OS ended the drag
    /// elsewhere: the lit zone hears its `leave`.
    DragCancel,
}

/// What the pasteboard said about the text a paste brought back (backlog
/// F84): the markers password managers set on a copied secret, after the
/// convention at nspasteboard.org that 1Password, Bitwarden, KeePassXC and
/// the macOS clipboard managers follow. Read by the driver, which owns the
/// clipboard, and handed over with the text as [`InputEvent::Paste`].
///
/// Where the runner reads each:
///
/// - `concealed`: the macOS pasteboard type `org.nspasteboard.ConcealedType`;
///   on Windows the registered format
///   `ExcludeClipboardContentFromMonitorProcessing` being present.
/// - `transient`: `org.nspasteboard.TransientType`; on Windows the format
///   `CanIncludeInClipboardHistory` holding 0.
///
/// The runner does not read them on Linux yet — KDE's
/// `x-kde-passwordManagerHint: secret` is a MIME type arboard writes but
/// cannot list — so there both stay false.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ClipboardMarks {
    /// The text is a secret: do not show it, log it or keep it anywhere.
    pub concealed: bool,
    /// The text is on the clipboard for a moment: do not keep it in a
    /// history.
    pub transient: bool,
}

impl ClipboardMarks {
    /// Both markers: what a password manager puts on a secret it copies,
    /// and what `Core::set_clipboard_secret` writes.
    pub const SECRET: Self = Self {
        concealed: true,
        transient: true,
    };

    /// Neither marker set.
    pub fn is_empty(self) -> bool {
        !self.concealed && !self.transient
    }

    /// As bits, the C ABI's spelling: `KUI_PASTE_CONCEALED` 1,
    /// `KUI_PASTE_TRANSIENT` 2.
    pub fn bits(self) -> u32 {
        self.concealed as u32 | (self.transient as u32) << 1
    }

    /// From [`ClipboardMarks::bits`]; unknown bits are ignored.
    pub fn from_bits(bits: u32) -> Self {
        Self {
            concealed: bits & 1 != 0,
            transient: bits & 2 != 0,
        }
    }
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

    /// The three buttons that have a name, in code order. `Other` has no
    /// name: a binding that needs one takes a [`MouseButton::code`].
    pub const NAMED: [MouseButton; 3] = [
        MouseButton::Primary,
        MouseButton::Secondary,
        MouseButton::Middle,
    ];

    /// The wire name of a named button (`"primary"`, `"secondary"`,
    /// `"middle"`); `None` for an `Other`.
    pub fn name(self) -> Option<&'static str> {
        match self {
            MouseButton::Primary => Some("primary"),
            MouseButton::Secondary => Some("secondary"),
            MouseButton::Middle => Some("middle"),
            MouseButton::Other(_) => None,
        }
    }

    /// The three named buttons by name, for bindings that spell them as
    /// strings: the inverse of [`Self::name`].
    pub fn from_name(name: &str) -> Option<Self> {
        Self::NAMED.into_iter().find(|b| b.name() == Some(name))
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

impl EditKey {
    /// Every editing key, in declaration order — the list a binding's
    /// name table and a generated type union are checked against, so a
    /// key added here reaches C, Node and TypeScript or fails a build
    /// (ADR 0020, decision 9, the same way `Role::ALL` did).
    pub const ALL: [EditKey; 16] = [
        EditKey::Left,
        EditKey::Right,
        EditKey::Up,
        EditKey::Down,
        EditKey::Home,
        EditKey::End,
        EditKey::PageUp,
        EditKey::PageDown,
        EditKey::Backspace,
        EditKey::Delete,
        EditKey::Enter,
        EditKey::Tab,
        EditKey::SelectAll,
        EditKey::Undo,
        EditKey::Redo,
        EditKey::Escape,
    ];

    /// The wire name a binding spells the key as (`"pageup"`,
    /// `"selectall"`: lower case, no separator).
    pub fn name(self) -> &'static str {
        match self {
            EditKey::Left => "left",
            EditKey::Right => "right",
            EditKey::Up => "up",
            EditKey::Down => "down",
            EditKey::Home => "home",
            EditKey::End => "end",
            EditKey::PageUp => "pageup",
            EditKey::PageDown => "pagedown",
            EditKey::Backspace => "backspace",
            EditKey::Delete => "delete",
            EditKey::Enter => "enter",
            EditKey::Tab => "tab",
            EditKey::SelectAll => "selectall",
            EditKey::Undo => "undo",
            EditKey::Redo => "redo",
            EditKey::Escape => "escape",
        }
    }

    /// The key a wire name spells: the inverse of [`Self::name`].
    pub fn from_name(name: &str) -> Option<EditKey> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }
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
    /// Bit 0 Shift, 1 Ctrl, 2 Alt, 3 Super — the order the fields are
    /// declared in, and the C header's `KUI_KMOD_*` (pinned there). What
    /// a wire that carries the state as one integer spells it as: the
    /// conformance corpus's `modifiers` step, the C input door.
    pub const SHIFT: u32 = 1 << 0;
    pub const CTRL: u32 = 1 << 1;
    pub const ALT: u32 = 1 << 2;
    pub const SUPER: u32 = 1 << 3;

    pub fn from_bits(bits: u32) -> Self {
        Self {
            shift: bits & Self::SHIFT != 0,
            ctrl: bits & Self::CTRL != 0,
            alt: bits & Self::ALT != 0,
            super_key: bits & Self::SUPER != 0,
        }
    }

    pub fn bits(self) -> u32 {
        let bit = |on: bool, b: u32| if on { b } else { 0 };
        bit(self.shift, Self::SHIFT)
            | bit(self.ctrl, Self::CTRL)
            | bit(self.alt, Self::ALT)
            | bit(self.super_key, Self::SUPER)
    }

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

/// What US-QWERTY prints on a key under Shift: the upper-case letter, the
/// symbol above a digit, the pair on a punctuation key. Anything else —
/// already shifted, or not a US key at all — is itself.
fn us_shifted(c: char) -> char {
    match c {
        'a'..='z' => c.to_ascii_uppercase(),
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '`' => '~',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        other => other,
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
    /// naming a key is saying which key was pressed. The one fold: an
    /// ASCII letter's position is its lower-case letter, since a window
    /// reports `physical` from a table that never sees Shift (`Z` beside
    /// `code: "Z"` for ⇧Z would be a pair no window ever sends; backlog
    /// F65). A `physical` a caller spells is delivered as spelled — this
    /// is only the default, which was already a guess.
    pub fn new(code: KeyCode, mods: KeyMods) -> Self {
        let physical = match code {
            KeyCode::Char(c) if c.is_ascii_uppercase() => KeyCode::Char(c.to_ascii_lowercase()),
            other => other,
        };
        Self {
            code,
            physical,
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
    /// The stand-in is what US-QWERTY would have produced for the *same
    /// press*, Shift included: a window reports `physical` from a table
    /// that never sees Shift (backlog F65), so ⇧ on the key printed J is
    /// `J`, not `j`, and ⇧ on the key printed `;` is `:` — the key a vim
    /// hand on a Russian layout reaches for, and gets `;` from otherwise.
    ///
    /// `physical` is reported either way, for a keymap that would rather
    /// bind the finger than the label. See `docs/adr/0002` decision 11.
    pub fn from_layout(layout: KeyCode, physical: KeyCode, mods: KeyMods) -> Self {
        let stand_in = || match (mods.shift, physical) {
            (true, KeyCode::Char(c)) => KeyCode::Char(us_shifted(c)),
            _ => physical,
        };
        let code = match layout {
            KeyCode::Char(c) if !c.is_ascii() => stand_in(),
            KeyCode::Unknown => stand_in(),
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

    /// Whether `other` is a press or release of the same key as this one
    /// — how a release is matched to the press it lets go of, and a repeat
    /// to the press it repeats. By position when the platform reported
    /// one, because `code` moves under a held key: hold `w`, press Shift,
    /// and the OS repeat arrives as `W`, which by `code` would be a second
    /// key held, with the first stuck down until focus moved (AR9). A
    /// press whose position the vocabulary could not name is matched on
    /// `code`, which is all it has.
    pub fn same_key(&self, other: &KeyPress) -> bool {
        if self.physical != KeyCode::Unknown && other.physical != KeyCode::Unknown {
            self.physical == other.physical
        } else {
            self.code == other.code
        }
    }

    /// The **second** event a real key press produces, after its
    /// [`InputEvent::KeyDown`] — the other half of what a window does with
    /// one key going down, and the one table that says which key is which.
    ///
    /// A press is two channels, and every driver drives both, in this
    /// order. `KeyDown` goes to whatever holds key focus, so an app that
    /// owns its keyboard hears the raw key; this is what the *core* is
    /// asked to do with the same key — Escape dismisses a modal, Tab walks
    /// the focus ring, the arrows nudge a focused slider, Space presses a
    /// focused control, a printable character reaches the focused editor.
    /// A test that sent only `KeyDown` got the first channel and none of
    /// the second, which is why `key_down("escape")` left a modal open
    /// (backlog F6); [`crate::Core::press`] is the pair.
    ///
    /// `None` for a key this vocabulary does not name — a function key,
    /// Insert — and for every chord, which carries no `text` because it
    /// inserts nothing. The press still stands on the sink channel.
    pub fn edit_event(&self) -> Option<InputEvent> {
        let key = match self.code {
            KeyCode::Left => EditKey::Left,
            KeyCode::Right => EditKey::Right,
            KeyCode::Up => EditKey::Up,
            KeyCode::Down => EditKey::Down,
            KeyCode::Home => EditKey::Home,
            KeyCode::End => EditKey::End,
            KeyCode::PageUp => EditKey::PageUp,
            KeyCode::PageDown => EditKey::PageDown,
            KeyCode::Backspace => EditKey::Backspace,
            KeyCode::Delete => EditKey::Delete,
            KeyCode::Enter => EditKey::Enter,
            KeyCode::Tab => EditKey::Tab,
            KeyCode::Escape => EditKey::Escape,
            // Space is the text channel rather than an `EditKey`: it
            // inserts into a focused editor and presses a focused control
            // (`docs/adr/0002`). Under Shift it is still a space; under
            // any other modifier it is a chord like every other chord —
            // an IME toggle, an Emacs mark — and inserts nothing (AR10;
            // before that it said " " whatever was held, so Ctrl+Space
            // typed a space into an editor and clicked a control).
            KeyCode::Space => {
                let m = self.mods;
                return (!m.ctrl && !m.alt && !m.super_key)
                    .then(|| InputEvent::Text(" ".to_string()));
            }
            // Anything else inserts whatever it inserts. A driver leaves
            // `text` unset for a chord, so this is where one stops.
            _ => {
                let text = self.text.as_deref()?;
                return text
                    .chars()
                    .any(|c| !c.is_control())
                    .then(|| InputEvent::Text(text.to_string()));
            }
        };
        // `word` is Alt and `doc` the platform primary, which is the whole
        // of what the editing vocabulary normalizes (see [`Mods`]).
        Some(InputEvent::Key(
            key,
            Mods {
                shift: self.mods.shift,
                word: self.mods.alt,
                doc: self.mods.primary(),
            },
        ))
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
    /// Most producers cannot fill it in: a hit test and the edit buffer
    /// know nothing about windows. They leave it [`WindowId::MAIN`] and the
    /// core stamps its own `env.window.id` over it as the event leaves
    /// (`Core::handle_input`, `Core::take_pending_events`) — one core is
    /// one window, so that is the whole answer. The audio store is the
    /// exception: its mounts are per window, so a `sound` event carries
    /// the window that declared the node and the stamp leaves it alone. A
    /// driver that builds an event itself stamps it itself.
    pub window: WindowId,
    pub key: Key,
    pub payload: Value,
    /// The slot whose fill drew the node — its key, the one `begin_slot`
    /// returned and `key_of(full_name)` answers — or `None` for a node the
    /// host drew itself. What `origin` cannot say: one extension fills
    /// many slots (a Lua host with a view per pane), and an event routed
    /// by pane needs the slot, not the extension. Stamped by the core on
    /// the way out like `window`, from the fill ranges the last frame
    /// recorded (`Tree::fills`); a producer leaves it `None` (backlog K2).
    pub slot: Option<Key>,
}

impl UiEvent {
    /// An event as a producer builds it: the window is left [`WindowId::MAIN`]
    /// and the slot `None` for the core to stamp on the way out (see
    /// [`UiEvent::window`], [`UiEvent::slot`]).
    pub fn on(origin: OriginId, key: Key, payload: Value) -> Self {
        Self {
            origin,
            window: WindowId::MAIN,
            key,
            payload,
            slot: None,
        }
    }

    /// Merges the node's tag into a map payload. A `Null` tag declares the
    /// behaviour and names nothing, so it is the one value left out — the
    /// rule every row with a tag reads by, stated once.
    pub fn tagged(mut self, tag: Option<&Value>) -> Self {
        if let Some(tag) = tag
            && *tag != Value::Null
            && let Value::Map(entries) = &mut self.payload
        {
            entries.push(("tag".to_string(), tag.clone()));
        }
        self
    }
}

/// The node whose `on_context_menu` a secondary press on a region opens:
/// the region's own node or an ancestor's (see `HitRegion::context_menu`).
#[derive(Clone, Debug, PartialEq)]
pub struct MenuOwner {
    pub key: Key,
    pub origin: OriginId,
    pub tag: Value,
}

/// The zone files dragged over a region land on: the region's own node
/// or an ancestor's (see `HitRegion::drop`, ADR 0031, decision 2). The
/// same three fields as [`MenuOwner`], resolved by the same walk.
pub type DropOwner = MenuOwner;

/// The shape inside a region's rect that a point has to be in to hit it
/// (`docs/adr/0026-hit-testing-by-shape.md`). The rect is always tested
/// first, so a shape is evaluated only for the few regions under the
/// pointer. Inline on the region rather than behind an index: the ADR
/// priced the twenty bytes at +7% on a 10k-region frame and measured
/// nothing, so the simpler shape won. Points for a stroke or a fill live
/// in the interaction's own list, relative to the region's top-left in
/// logical px, copied at emission because the frame's stores do not
/// outlive the frame and a press does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HitShape {
    /// The whole rect — every box, and what every region was before.
    Rect,
    /// A box with rounded corners: a point in a corner's square but past
    /// its arc misses. Radii clockwise from the top-left, logical px,
    /// as the node's `radius` row.
    Rounded([f32; 4]),
    /// A round-capped stroke through `len` points from `first`, `width`
    /// wide: a point within half the width of any piece hits. A hairline
    /// is hard to hit, so the grab is at least [`MIN_STROKE_GRAB`] wide.
    Segments { first: u32, len: u32, width: f32 },
    /// A filled outline through `len` points from `first`: a point inside
    /// by the even-odd rule hits — the same rule the stock polygon paints
    /// by, so the hit is the fill exactly, a self-intersecting outline's
    /// unfilled overlaps included.
    Polygon { first: u32, len: u32 },
}

/// The narrowest a stroke's hit target gets, logical px, whatever its
/// drawn width: a 1 px connector is a 4 px target, the way a 1 px splitter
/// handle is wider than its line everywhere.
pub const MIN_STROKE_GRAB: f32 = 4.0;

#[derive(Clone, Debug)]
pub struct HitRegion {
    pub key: Key,
    pub origin: OriginId,
    /// Logical coordinates.
    pub rect: Rect,
    /// Ancestor clip; a point must be inside both to hit.
    pub clip: Rect,
    /// The shape inside `rect` a point must also be in, when there is one.
    pub shape: HitShape,
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
    /// The selection scope this node is inside, when it is inside one
    /// (`docs/adr/0017-selection-as-a-scope.md`): a press here starts a
    /// drag-select over the scope's text. A region that also carries a
    /// click payload is a control first — a press on a button inside a
    /// selectable card clicks it — so this is read only where nothing
    /// else claims the press.
    pub select_scope: Option<Key>,
    /// Key-sink tag when the node declared `on_key`: clicking it takes
    /// key focus, and key presses then arrive on it carrying this tag.
    pub key_sink: Option<Value>,
    /// The sink declared `key_up`: releases reach it too. Without it a
    /// release is dropped at routing, and the sink hears presses only.
    pub key_up: bool,
    /// The context menu a secondary press here opens: the node's own
    /// `on_context_menu`, or the nearest enclosing one — a container
    /// offering a menu for everything inside it is the common case, and
    /// a press on a child that declared none is unclaimed in the sense
    /// ADR 0011 gave keys, so it reaches the enclosing menu the way an
    /// unclaimed key reaches the enclosing sink (backlog T1). Resolved at
    /// emission, where the tree is; the walk stops at the modal boundary
    /// and skips a disabled node's own. The press emits
    /// `{kind="contextmenu", x, y, tag}` on the *owner*, not on this node.
    /// None when nothing encloses this region offers one, and the press
    /// is swallowed here.
    pub context_menu: Option<MenuOwner>,
    /// The drop zone this region belongs to — its own `on_drop` or the
    /// nearest enclosing declaration's — resolved at emission (ADR 0031,
    /// decision 2). None where no zone encloses it: files dragged over
    /// such a region look past it to the topmost zone beneath.
    pub drop: Option<DropOwner>,
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
    /// Pointer shape declared by the node (`NodeSpec::cursor`). None = the
    /// I-beam over text, the arrow otherwise (`Interaction::implied_shape`).
    pub cursor: Option<CursorShape>,
}

/// An OS file drag over a zone (ADR 0031): what `dropBg` reads and what
/// the next `DragFiles` compares against.
#[derive(Clone, Debug)]
struct DropHover {
    owner: DropOwner,
    /// Where the last `DragFiles` put the pointer: a repeat at the same
    /// point is not a `move`.
    last: Vec2,
    /// The `leave`, built at `enter` with the paths of that moment.
    leave: UiEvent,
}

/// The points a frame's stroke and fill shapes index, built beside its
/// regions.
#[derive(Clone, Debug, Default)]
pub struct HitShapes {
    pub points: Vec<Vec2>,
}

impl HitShapes {
    /// Adds a stroke's points and returns the shape over them.
    pub fn segments(&mut self, points: &[Vec2], width: f32) -> HitShape {
        let first = self.points.len() as u32;
        self.points.extend_from_slice(points);
        HitShape::Segments {
            first,
            len: points.len() as u32,
            width,
        }
    }

    /// Adds a fill's points and returns the shape over them.
    pub fn polygon(&mut self, points: &[Vec2]) -> HitShape {
        let first = self.points.len() as u32;
        self.points.extend_from_slice(points);
        HitShape::Polygon {
            first,
            len: points.len() as u32,
        }
    }
}

/// Whether `p` (relative to the box's top-left) is inside a `w`×`h` box
/// with the given corner radii: in the box, and not in a corner's square
/// past its arc. Radii are clamped to the half extents as the shader
/// clamps them, so an oversized radius is the pill it draws as.
pub fn in_rounded_rect(p: Vec2, w: f32, h: f32, radii: [f32; 4]) -> bool {
    let cap = (w * 0.5).min(h * 0.5).max(0.0);
    // Corner centres clockwise from the top-left, each with its radius.
    let corners = [
        (radii[0].min(cap), radii[0].min(cap), radii[0].min(cap)),
        (w - radii[1].min(cap), radii[1].min(cap), radii[1].min(cap)),
        (
            w - radii[2].min(cap),
            h - radii[2].min(cap),
            radii[2].min(cap),
        ),
        (radii[3].min(cap), h - radii[3].min(cap), radii[3].min(cap)),
    ];
    for (i, &(cx, cy, r)) in corners.iter().enumerate() {
        if r <= 0.0 {
            continue;
        }
        // Past the centre toward the corner on both axes: in the square.
        let in_square = match i {
            0 => p.x < cx && p.y < cy,
            1 => p.x > cx && p.y < cy,
            2 => p.x > cx && p.y > cy,
            _ => p.x < cx && p.y > cy,
        };
        if in_square && (p.x - cx).powi(2) + (p.y - cy).powi(2) > r * r {
            return false;
        }
    }
    true
}

/// Distance from `p` to the segment `a`–`b`.
pub fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let (ex, ey) = (b.x - a.x, b.y - a.y);
    let (wx, wy) = (p.x - a.x, p.y - a.y);
    let ee = ex * ex + ey * ey;
    let t = if ee > 0.0 {
        ((wx * ex + wy * ey) / ee).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (dx, dy) = (wx - ex * t, wy - ey * t);
    (dx * dx + dy * dy).sqrt()
}

/// Whether `p` is inside the outline through `pts` by the even-odd rule
/// (the crossing test). A point on an edge counts as inside on one side
/// and outside on the other, which is what every hit test of a shared
/// edge between two wedges wants: exactly one of them.
pub fn in_polygon(p: Vec2, pts: &[Vec2]) -> bool {
    let n = pts.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (pts[i], pts[j]);
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if p.x < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// A scroll container's on-screen area, for wheel routing — or an
/// `on_scroll` node's, which takes the wheel the same way and turns it
/// into an event instead of an offset (ADR 0029, decision 4).
#[derive(Clone, Copy, Debug)]
pub struct ScrollRegion {
    pub key: Key,
    /// The scroller's index in the frame's tree: what its bars are
    /// emitted from, at the end of its layer.
    pub(crate) node: u32,
    pub rect: Rect,
    pub clip: Rect,
    /// Outside the frame's modal scope: the bar still draws, the wheel
    /// and the thumb do nothing (see `docs/adr/0003-modal-surfaces.md`).
    pub inert: bool,
    /// The node declared `on_scroll`: the wheel over it is an event on
    /// it, no bars are drawn and no offset is kept. A region that is
    /// both — a scroller that also declared the row — is the handler's:
    /// the app asked to hear the wheel, and hearing it *and* having the
    /// content move under it would be two answers to one notch.
    pub handler: bool,
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
    /// The hit list's length when the bar was painted: every region below
    /// this index is under the bar, every one at or above it is in a layer
    /// over it (ADR 0023, decision 4).
    pub(crate) above: u32,
}

/// What a press at a point lands on, in paint order: the topmost hit
/// region, unless a scrollbar painted over it is there too.
pub(crate) enum Target<'a> {
    Bar(ScrollbarRegion),
    Hit(&'a HitRegion),
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
    /// The points the stroke and fill shapes index, rebuilt with the
    /// hits; empty on a frame of plain boxes.
    shape_points: Vec<Vec2>,
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
    /// The last primary press's driver-measured click count (1 for a
    /// single, 2 for a double, …): what the `clicks` a press or drag
    /// inside a key sink carries reads (backlog C34).
    press_clicks: u8,
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
    /// The zone files dragged in from the OS are over, with the `leave`
    /// prepared at `enter` — the region may be gone from the next
    /// frame's hits (ADR 0031, decision 2).
    drop: Option<DropHover>,
    /// Hover enter/leave events raised outside `handle` — a new frame's hit
    /// regions changing what sits under a still cursor. Drained by the next
    /// `handle` or by `take_pending`.
    pending: Vec<UiEvent>,
}

impl Interaction {
    pub fn set_hits(&mut self, hits: Vec<HitRegion>) {
        self.set_hits_shaped(hits, HitShapes::default());
    }

    /// `set_hits` with the points the regions' shapes index (ADR 0026).
    pub fn set_hits_shaped(&mut self, hits: Vec<HitRegion>, shapes: HitShapes) {
        self.hits = hits;
        self.shape_points = shapes.points;
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

    /// The previous frame's point list (cleared), on the same terms.
    pub fn take_shape_buffer(&mut self) -> HitShapes {
        let mut shapes = HitShapes {
            points: std::mem::take(&mut self.shape_points),
        };
        shapes.points.clear();
        shapes
    }

    /// Whether `p` is in region `h`: inside its rect and its clip, and
    /// inside its shape when it has one. The rect test is what every
    /// region pays; the shape is paid by the few under the pointer.
    #[inline]
    fn contains(&self, h: &HitRegion, p: Vec2) -> bool {
        if !(h.rect.contains(p) && h.clip.contains(p)) {
            return false;
        }
        if h.shape == HitShape::Rect {
            return true;
        }
        let local = Vec2::new(p.x - h.rect.x, p.y - h.rect.y);
        match h.shape {
            HitShape::Rect => true,
            HitShape::Rounded(radii) => in_rounded_rect(local, h.rect.w, h.rect.h, radii),
            // A shape whose points are not here — a region installed
            // through `set_hits` without its shapes, or one from another
            // frame — misses rather than panics in the input path.
            HitShape::Segments { first, len, width } => {
                let Some(pts) = self
                    .shape_points
                    .get(first as usize..(first + len) as usize)
                else {
                    return false;
                };
                let half = (width * 0.5).max(MIN_STROKE_GRAB * 0.5);
                pts.windows(2)
                    .any(|w| segment_distance(local, w[0], w[1]) <= half)
            }
            HitShape::Polygon { first, len } => {
                let Some(pts) = self
                    .shape_points
                    .get(first as usize..(first + len) as usize)
                else {
                    return false;
                };
                in_polygon(local, pts)
            }
        }
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
        self.hits.iter().rev().find(|h| self.contains(h, p))
    }

    /// The scroll region under the cursor: the topmost container or
    /// `on_scroll` handler there, by paint order.
    pub(crate) fn scroll_region_at(&self) -> Option<&ScrollRegion> {
        let p = self.cursor?;
        self.scroll_regions
            .iter()
            .rev()
            .find(|r| !r.inert && r.rect.contains(p) && r.clip.contains(p))
    }

    /// The content origin the editor `key` was drawn at this frame — what
    /// a caret drag places against. Read off the frame rather than kept
    /// from the press: a scroller nudged under a held drag moves the
    /// origin, and a caret placed against the press's origin would land
    /// the nudge off (ADR 0029, decision 2).
    pub(crate) fn edit_origin_of(&self, key: Key) -> Option<Vec2> {
        self.hits
            .iter()
            .rev()
            .find(|h| h.key == key && h.edit_origin.is_some())
            .and_then(|h| h.edit_origin)
    }

    /// Re-resolves the hovered region under the cursor, emitting `on_hover`
    /// leave/enter events when the hovered node changes.
    fn refresh_hover(&mut self, out: &mut Vec<UiEvent>) {
        let before = self.hovered;
        // Through `target_at`, like the press and the cursor shape (ADR
        // 0023, decision 4): over a bar painted above the node, nothing
        // is hovered — the node beneath used to light its `hover_bg` and
        // fire `enter` while the press would have grabbed the thumb
        // (backlog AR31).
        let idx = self.cursor.and_then(|p| match self.target_at(p)? {
            Target::Bar(_) => None,
            Target::Hit(h) => self.hits.iter().rposition(|x| std::ptr::eq(x, h)),
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
        let payload = Value::map([("kind", Value::str("hover")), ("phase", Value::str(phase))]);
        Some(UiEvent::on(region.origin, region.key, payload).tagged(Some(tag)))
    }

    fn context_menu_event(region: &HitRegion, p: Vec2) -> Option<UiEvent> {
        let owner = region.context_menu.as_ref()?;
        let payload = Value::map([
            ("kind", Value::str("contextmenu")),
            ("x", Value::Float(p.x as f64)),
            ("y", Value::Float(p.y as f64)),
        ]);
        Some(UiEvent::on(owner.origin, owner.key, payload).tagged(Some(&owner.tag)))
    }

    /// What is under `p`, by the paint order and nothing else: the topmost
    /// hit region there, or the topmost scrollbar there if it was painted
    /// over that region — a bar wins the content of its own scroller and
    /// loses to a float over it (ADR 0023, decision 4). The press and the
    /// cursor shape both ask this, so they cannot disagree. A bar behind a
    /// modal is drawn and not a target.
    pub(crate) fn target_at(&self, p: Vec2) -> Option<Target<'_>> {
        let hit = self.hits.iter().rposition(|h| self.contains(h, p));
        let bar = self
            .scrollbars
            .iter()
            .rev()
            .find(|b| !b.inert && b.track.contains(p));
        match (bar, hit) {
            (Some(b), Some(h)) if (h as u32) < b.above => Some(Target::Bar(*b)),
            (Some(b), None) => Some(Target::Bar(*b)),
            (_, Some(h)) => Some(Target::Hit(&self.hits[h])),
            (None, None) => None,
        }
    }

    /// Whether this bar is being thumb-dragged (for active styling).
    pub fn is_scrollbar_dragging(&self, key: Key, axis: ScrollAxis) -> bool {
        matches!(self.scrollbar_drag, Some((k, a, _)) if k == key && a == axis)
    }

    fn drag_event(state: &DragState, phase: &str, p: Vec2, d: Vec2) -> UiEvent {
        let pr = state.parent_rect;
        let payload = Value::map([
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
        UiEvent::on(state.origin, state.key, payload).tagged(Some(&state.tag))
    }

    /// Returns how many of the events at the end of `out` a press made —
    /// a drag in any phase, a click on the release — as against the
    /// hover, context-menu and modifier events it also raises. That is
    /// the mark `Core::attach_pointer` reads to give a click or drag its
    /// `cell` and `line` / `byte` / `clicks`: said here, where the event
    /// is built, rather than guessed afterwards from its payload's
    /// `kind` (AR11). Every arm pushes its pointer-made events last.
    pub fn handle(&mut self, ev: InputEvent, out: &mut Vec<UiEvent>) -> usize {
        out.append(&mut self.pending);
        let mut pointer_made = 0;
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
                        pointer_made += 1;
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
            InputEvent::MouseDown { clicks, .. } => {
                self.pressed = self.hovered;
                self.pressed_group = self.hovered_group;
                self.press_clicks = clicks;
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
                        pointer_made += 1;
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
                        slot: None,
                    });
                }
            }
            // Routed by the core (they need the retained stores).
            InputEvent::Scroll(_)
            | InputEvent::Text(_)
            | InputEvent::Commit(_)
            | InputEvent::Paste { .. }
            | InputEvent::Preedit(..)
            | InputEvent::Key(..)
            | InputEvent::KeyDown(_)
            | InputEvent::KeyUp(_)
            | InputEvent::Access(_)
            // A force click needs the text and selection stores, and the
            // node it lands on it finds by hit test the way a secondary
            // press does.
            | InputEvent::ForceClick(_) => {}
            InputEvent::DragFiles { paths, at } => self.drag_files(&paths, at, out),
            InputEvent::DropFiles { paths, at } => self.drop_files(&paths, at, out),
            InputEvent::DragCancel => self.drag_cancel(out),
            InputEvent::MouseUp { button } if button != MouseButton::Primary => {}
            InputEvent::MouseUp { .. } => {
                let dragged = self.drag.take().inspect(|drag| {
                    let p = self.cursor.unwrap_or(drag.last);
                    out.push(Self::drag_event(drag, "end", p, drag.displacement(p)));
                    pointer_made += 1;
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
                        (None, Some(payload)) => {
                            out.push(UiEvent {
                                origin: region.origin,
                                window: WindowId::MAIN,
                                key: region.key,
                                payload: payload.clone(),
                                slot: None,
                            });
                            pointer_made += 1;
                        }
                    }
                }
                self.pressed = None;
                self.pressed_group = None;
            }
        }
        pointer_made
    }

    pub fn is_hovered(&self, key: Key) -> bool {
        self.hovered == Some(key)
    }

    /// Whether files dragged in from the OS are over `key` (ADR 0031):
    /// what `drop_bg` reads when the node opens.
    pub fn is_drop_target(&self, key: Key) -> bool {
        self.drop.as_ref().is_some_and(|d| d.owner.key == key)
    }

    /// The zone the dragged files are over, if any — what a driver
    /// answers the OS with (a copy cursor over a zone, not-allowed
    /// elsewhere) and what a test reads to say a zone was found.
    pub fn drop_target(&self) -> Option<Key> {
        self.drop.as_ref().map(|d| d.owner.key)
    }

    /// The topmost zone under `p` (ADR 0031, decision 2): the topmost
    /// region there whose resolved `drop` is some. A region resolving to
    /// no zone — an overlay the app showed on `enter` — is looked past.
    fn zone_at(&self, p: Vec2) -> Option<&DropOwner> {
        self.hits
            .iter()
            .rev()
            .find(|h| h.drop.is_some() && self.contains(h, p))
            .and_then(|h| h.drop.as_ref())
    }

    fn drop_event(owner: &DropOwner, phase: &str, paths: &[String], at: Option<Vec2>) -> UiEvent {
        let mut fields = vec![
            ("kind", Value::str("drop")),
            ("phase", Value::str(phase)),
            (
                "paths",
                Value::list(
                    paths
                        .iter()
                        .map(|p| Value::str(p.as_str()))
                        .collect::<Vec<_>>(),
                ),
            ),
        ];
        if let Some(p) = at {
            fields.push(("x", Value::Float(p.x as f64)));
            fields.push(("y", Value::Float(p.y as f64)));
        }
        UiEvent::on(owner.origin, owner.key, Value::map(fields)).tagged(Some(&owner.tag))
    }

    fn drag_files(&mut self, paths: &[String], at: Vec2, out: &mut Vec<UiEvent>) {
        let zone = self.zone_at(at).cloned();
        if let (Some(cur), Some(z)) = (&mut self.drop, &zone)
            && cur.owner.key == z.key
            && cur.owner.origin == z.origin
        {
            if cur.last != at {
                cur.last = at;
                out.push(Self::drop_event(z, "move", paths, Some(at)));
            }
            return;
        }
        out.extend(self.drop.take().map(|d| d.leave));
        if let Some(owner) = zone {
            out.push(Self::drop_event(&owner, "enter", paths, Some(at)));
            let leave = Self::drop_event(&owner, "leave", paths, None);
            self.drop = Some(DropHover {
                owner,
                last: at,
                leave,
            });
        }
    }

    fn drop_files(&mut self, paths: &[String], at: Vec2, out: &mut Vec<UiEvent>) {
        let zone = self.zone_at(at).cloned();
        // The lit zone is not the one under the point (a headless drive
        // that never sent `DragFiles`, a frame that moved the zone): it
        // hears its leave first. The zone that takes the drop hears no
        // leave — the drop ends the hover (decision 1).
        if let Some(cur) = self.drop.take()
            && !zone
                .as_ref()
                .is_some_and(|z| z.key == cur.owner.key && z.origin == cur.owner.origin)
        {
            out.push(cur.leave);
        }
        if let Some(owner) = zone {
            out.push(Self::drop_event(&owner, "drop", paths, Some(at)));
        }
    }

    fn drag_cancel(&mut self, out: &mut Vec<UiEvent>) {
        out.extend(self.drop.take().map(|d| d.leave));
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

    /// The node a press is held on, if any.
    /// The click count the last primary press carried; see
    /// `press_clicks`. Zero after a click nothing pressed for — Enter,
    /// Space, an assistive-technology `click` — so a payload attached
    /// from the pointer's position does not describe a press that never
    /// happened.
    pub(crate) fn press_clicks(&self) -> u8 {
        self.press_clicks
    }

    /// A click is being made without a press (`Core::click_node`): the
    /// count the last press carried no longer describes it.
    pub(crate) fn note_synthetic_click(&mut self) {
        self.press_clicks = 0;
    }

    pub fn pressed_key(&self) -> Option<Key> {
        self.pressed
    }

    /// The pointer shape for where the pointer is now (see
    /// [`crate::cursor`]): what the topmost region under it — the same
    /// region a click would go to — declared with `cursor`, the I-beam
    /// over text, and the arrow otherwise. A clickable or draggable node
    /// that declared nothing is the arrow: a hand or a grab is the view's
    /// to say.
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
                .unwrap_or(CursorShape::Default);
        }
        // A bar that would take the press takes the shape too — an
        // overlay bar across an editor is not an I-beam — and a float
        // over the bar keeps its own.
        if self.scrollbar_drag.is_some() {
            return CursorShape::Default;
        }
        let Some(p) = self.cursor else {
            return CursorShape::Default;
        };
        match self.target_at(p) {
            Some(Target::Hit(region)) => {
                region.cursor.unwrap_or_else(|| Self::implied_shape(region))
            }
            Some(Target::Bar(_)) | None => CursorShape::Default,
        }
    }

    /// The shape a region takes when it declares none: the I-beam over
    /// text that can be edited or selected — the one shape every desktop
    /// derives, because the words themselves are what says they can be
    /// taken — and the arrow over everything else. Nothing here reads
    /// `payload`, `drag` or `focusable`: a hand over a button and a grab
    /// over a handle are declared, and the stock button declares its own.
    fn implied_shape(region: &HitRegion) -> CursorShape {
        match region {
            // Window chrome is the platform's: every desktop points at a
            // titlebar and its buttons with the plain arrow.
            _ if region.window.is_some() => CursorShape::Default,
            _ if region.edit_origin.is_some() => CursorShape::Text,
            _ if region.select_scope.is_some() => CursorShape::Text,
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
            shape: HitShape::Rect,
            payload: Some(Value::str(tag)),
            drag: None,
            parent_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            edit_origin: None,
            select_scope: None,
            key_sink: None,
            key_up: false,
            context_menu: None,
            drop: None,
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
        r.context_menu = Some(MenuOwner {
            key: k,
            origin: OriginId::HOST,
            tag: Value::str("panel-menu"),
        });
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

    /// ADR 0031, decisions 1, 2 and 4: a button inside a zone is the
    /// zone, an overlay that is no zone is looked past, a drop ends the
    /// hover without a leave, a cancel leaves.
    #[test]
    fn dragged_files_find_the_topmost_zone_and_look_past_what_is_none() {
        let mut it = Interaction::default();
        let zone = Key::ROOT.str("zone");
        let button = Key::ROOT.str("button");
        let overlay = Key::ROOT.str("overlay");
        let other = Key::ROOT.str("other");
        let owner = |k: Key, tag: &str| {
            Some(DropOwner {
                key: k,
                origin: OriginId::HOST,
                tag: Value::str(tag),
            })
        };
        let mut z = region(zone, 0, 0.0, 0.0, 100.0, 100.0, "z");
        z.drop = owner(zone, "files");
        // The button is inside the zone: its region resolved to the zone.
        let mut b = region(button, 0, 10.0, 10.0, 30.0, 30.0, "press");
        b.drop = owner(zone, "files");
        // The overlay is painted over everything and belongs to no zone.
        let o = region(overlay, 0, 0.0, 0.0, 100.0, 100.0, "overlay");
        let mut second = region(other, 0, 100.0, 0.0, 100.0, 100.0, "o");
        second.drop = owner(other, "other-files");
        it.set_hits(vec![z, b, second, o]);
        let paths = vec!["/drop/1.txt".to_string()];
        let phases = |evs: &[UiEvent]| {
            evs.iter()
                .map(|e| {
                    (
                        e.key,
                        e.payload
                            .get("phase")
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_string(),
                    )
                })
                .collect::<Vec<_>>()
        };
        // Over the button, through the overlay: the zone's enter.
        let evs = drive(
            &mut it,
            &[InputEvent::DragFiles {
                paths: paths.clone(),
                at: Vec2::new(20.0, 20.0),
            }],
        );
        assert_eq!(phases(&evs), vec![(zone, "enter".to_string())]);
        assert_eq!(evs[0].payload.get("tag").unwrap().as_str(), Some("files"));
        assert_eq!(evs[0].payload.get("x").unwrap().as_float(), Some(20.0));
        assert_eq!(it.drop_target(), Some(zone));
        assert!(it.is_drop_target(zone));
        // The same point again is nothing; a new one is a move.
        let evs = drive(
            &mut it,
            &[
                InputEvent::DragFiles {
                    paths: paths.clone(),
                    at: Vec2::new(20.0, 20.0),
                },
                InputEvent::DragFiles {
                    paths: paths.clone(),
                    at: Vec2::new(60.0, 60.0),
                },
            ],
        );
        assert_eq!(phases(&evs), vec![(zone, "move".to_string())]);
        // Into the other zone: leave, then enter, in that order.
        let evs = drive(
            &mut it,
            &[InputEvent::DragFiles {
                paths: paths.clone(),
                at: Vec2::new(150.0, 50.0),
            }],
        );
        assert_eq!(
            phases(&evs),
            vec![(zone, "leave".to_string()), (other, "enter".to_string())]
        );
        assert!(evs[0].payload.get("x").is_none());
        // Dropped there: the drop and nothing after it.
        let evs = drive(
            &mut it,
            &[InputEvent::DropFiles {
                paths: paths.clone(),
                at: Vec2::new(150.0, 50.0),
            }],
        );
        assert_eq!(phases(&evs), vec![(other, "drop".to_string())]);
        assert_eq!(it.drop_target(), None);
        // Over the first zone, then out of the window: its leave.
        let evs = drive(
            &mut it,
            &[
                InputEvent::DragFiles {
                    paths: paths.clone(),
                    at: Vec2::new(50.0, 50.0),
                },
                InputEvent::DragCancel,
            ],
        );
        assert_eq!(
            phases(&evs),
            vec![(zone, "enter".to_string()), (zone, "leave".to_string())]
        );
        // A drop off every zone with one lit: the lit one's leave, no drop.
        let evs = drive(
            &mut it,
            &[
                InputEvent::DragFiles {
                    paths: paths.clone(),
                    at: Vec2::new(50.0, 50.0),
                },
                InputEvent::DropFiles {
                    paths: paths.clone(),
                    at: Vec2::new(250.0, 50.0),
                },
            ],
        );
        assert_eq!(
            phases(&evs),
            vec![(zone, "enter".to_string()), (zone, "leave".to_string())]
        );
        assert_eq!(it.drop_target(), None);
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
        r.context_menu = Some(MenuOwner {
            key: k,
            origin: OriginId::HOST,
            tag: Value::str("panel-menu"),
        });
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
