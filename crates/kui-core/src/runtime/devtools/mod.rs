#![cfg_attr(not(feature = "devtools"), allow(dead_code))]
//! The devtools panel, drawn by the core into any app's frame
//! (`docs/adr/0024-the-devtools-are-the-cores.md`).
//!
//! Turned on by [`Core::set_devtools`] — or by `KUI_DEVTOOLS` in the
//! environment, which the windowed runners read before the first frame
//! ([`Core::devtools_from_env`]) — the panel sits beside the host's
//! tree in the main window (`left`, `right`, `bottom`; a handle on the
//! pane's inner edge resizes it), in a window of its own (`window`), or
//! nowhere with its chords still live (`off`). It shows what
//! the runtime is doing while the app runs: a header with the window's
//! title, the frame counter and an icon strip (the theme base, the accent,
//! native menus, the placement), and three tabs —
//!
//! - **facts**: the latency graph; the status block — what the runtime
//!   believes right now, each row read from the door it comes from; and
//!   the key legend a host declared ([`Core::set_devtools_legend`]);
//! - **events**: every `UiEvent` handed to the host from any window, as a
//!   virtual list whose rows open into the payload as data, with a
//!   filter, a pause and a follow toggle — plus every warning the core
//!   raised and the panel's own notes;
//! - **tree**: the main window's last frame ([`Core::nodes`]), collapsible,
//!   filterable, with a picker that finds the node under the pointer from
//!   the app itself, and an inspector for the selected one.
//!
//! The chords are `Ctrl+Shift+<letter>`, a family no app keymap should
//! use while the panel is on: `T` cycles the base (the app's own → light →
//! dark), `A` the accent, `M` the menus (the platform's own → native →
//! drawn), `D` moves the panel
//! (left → right → bottom → window → off; the header has a button per
//! placement too), `N` the tab, `C` clears the stream, `I`
//! moves the keyboard into the panel and back out, `P` picks a node, and
//! `Escape` leaves the picker. The `I` chord alone is the app's to
//! respell ([`Core::set_devtools_key`]: `f12`, `mod+shift+d`, any
//! [`Accel`] spelling), since it is the one an app names in its own
//! help — the others are the panel's, reached once the keyboard is in.
//!
//! **How it is in the frame** (ADR 0024, decisions 2–4). Docked, the main
//! window's `begin_frame` opens an *app container* under the root, keyed
//! so the host's children are named as if it were not there; the host's
//! `configure_root` is split between the two — layout and paint to the
//! container, everything addressed to the root; `Ui::finish` closes the
//! container and opens the dock as the root's next child, under
//! [`OriginId::DEVTOOLS`]. `handle_input` acts on the chords before
//! routing and on every event of that origin before returning, and logs
//! what it does return. Everything the panel declares in the main window
//! is under `Key::ROOT.str("kui-devtools")`. State is the session's, so a
//! second window can draw the same panel from the same stream.

use std::collections::VecDeque;

use rustc_hash::FxHashSet;

use crate::color::Color;
use crate::env::{Appearance, SystemEnv};
use crate::geom::{Rect, Size, Vec2};
use crate::key::Key;
use crate::menu::Accel;
use crate::runtime::inspect::NodeInfo;
use crate::spec::{NodeSpec, Sizing};
use crate::theme::{Theme, ThemeSource};
use crate::tree::OriginId;
use crate::value::Value;
use crate::widgets::RowHeights;
use crate::window::{WindowCommand, WindowConfig, WindowId};
use crate::{Core, InputEvent, KeyCode, UiEvent};

#[cfg(feature = "devtools")]
mod icons;
#[cfg(feature = "devtools")]
mod panel;
#[cfg(feature = "devtools")]
mod stream;
#[cfg(all(test, feature = "devtools"))]
mod tests;
#[cfg(feature = "devtools")]
mod tree;

/// Width of the side dock, logical px.
pub const DOCK_SIDE_W: f32 = 340.0;
/// Height of the bottom dock, logical px.
pub const DOCK_BOTTOM_H: f32 = 280.0;
/// The size the popped-out window opens at.
pub const WINDOW_SIZE: Size = Size { w: 420.0, h: 720.0 };
/// The name of the panel's own window, when it has one: what
/// `Core::windows` lists and `Ui::window_name` answers there.
pub const DEVTOOLS_WINDOW: &str = "kui-devtools";
/// The label of the node everything the panel declares in the main
/// window is under, so `key_of(DEVTOOLS_KEY)` finds it — and a tree reader
/// that wants the app alone skips its subtree.
pub const DEVTOOLS_KEY: &str = "kui-devtools";
/// The app container's label (decision 2). Never indexed for `key_of`.
const APP_KEY: &str = "kui-devtools/app";
/// How many stream entries are kept.
pub const STREAM_CAP: usize = 512;
/// A closed stream row's height, logical px.
const STREAM_ROW_H: f32 = 18.0;
/// How many are dropped at once when the cap is hit, so a busy stream is
/// not rebuilding its row heights on every event.
const STREAM_EVICT: usize = 64;

/// The accents `Ctrl+Shift+A` walks: kui's own, then four the OS might
/// report. `None` is the app's own.
const ACCENTS: [(&str, u32); 5] = [
    ("kui blue", 0x3b5bd4ff),
    ("macOS blue", 0x007affff),
    ("macOS yellow", 0xffc409ff),
    ("macOS pink", 0xf74f9eff),
    ("forest", 0x2f7d4fff),
];

/// Where the panel sits. The header has a button per placement, and
/// `Ctrl+Shift+D` walks them in [`Dock::ALL`]'s order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Dock {
    /// A column on the left of the main window, [`DOCK_SIDE_W`] wide to
    /// start with; the handle on its edge resizes it.
    Left,
    /// The same column on the right.
    #[default]
    Right,
    /// A strip along the bottom, [`DOCK_BOTTOM_H`] tall to start with.
    Bottom,
    /// A window of its own, [`DEVTOOLS_WINDOW`] — "undock".
    Window,
    /// Hidden: nothing drawn, no window declared, the chords still live
    /// (`Ctrl+Shift+D` brings it back) — "close".
    Off,
}

impl Dock {
    pub const ALL: [Dock; 5] = [
        Dock::Left,
        Dock::Right,
        Dock::Bottom,
        Dock::Window,
        Dock::Off,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Dock::Left => "left",
            Dock::Right => "right",
            Dock::Bottom => "bottom",
            Dock::Window => "window",
            Dock::Off => "off",
        }
    }

    /// A placement by name; `"side"` is the right, the name it had before
    /// there was a left.
    pub fn parse(s: &str) -> Option<Dock> {
        if s == "side" {
            return Some(Dock::Right);
        }
        Self::ALL.into_iter().find(|d| d.name() == s)
    }

    fn next(self) -> Dock {
        let i = Self::ALL.iter().position(|d| *d == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// Whether the panel is drawn inside the main window.
    pub fn docked(self) -> bool {
        matches!(self, Dock::Left | Dock::Right | Dock::Bottom)
    }

    /// Whether the panel is a column beside the app.
    pub fn is_side(self) -> bool {
        matches!(self, Dock::Left | Dock::Right)
    }
}

/// The smallest a docked pane can be — dragged, or squeezed by the
/// window — and the least it leaves the app while the window allows.
pub const SIDE_MIN_W: f32 = 280.0;
pub const BOTTOM_MIN_H: f32 = 160.0;
const APP_MIN: f32 = 160.0;

/// The panel's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    Facts,
    #[default]
    Events,
    Tree,
}

/// One tab an app or an extension declared this frame (ADR 0032,
/// decision 1): its name (the identity), the label the strip shows, and
/// the slot an extension fills it through — `None` for the host form,
/// whose content is the host's own subtree, anchored to the body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TabDecl {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) slot: Option<String>,
}

/// Which tab the panel shows: one of its own three, or a declared one by
/// index into this frame's list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shown {
    Builtin(Tab),
    Custom(usize),
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::Facts, Tab::Events, Tab::Tree];

    fn name(self) -> &'static str {
        match self {
            Tab::Facts => "facts",
            Tab::Events => "events",
            Tab::Tree => "tree",
        }
    }

    /// What the strip shows and the access tree names: the name with
    /// its first letter up, as a declared tab's label is written.
    fn label(self) -> &'static str {
        match self {
            Tab::Facts => "Facts",
            Tab::Events => "Events",
            Tab::Tree => "Tree",
        }
    }

    fn parse(s: &str) -> Option<Tab> {
        Self::ALL.into_iter().find(|t| t.name() == s)
    }
}

/// One entry of the stream.
pub(crate) struct Entry {
    /// Its number, ever increasing: what an expanded row is remembered by
    /// across evictions.
    seq: u64,
    /// The main window's frame count when it was logged.
    frame: u64,
    kind: EntryKind,
    window: WindowId,
    key: Key,
    /// The node's label when it was logged — the tree that named it is
    /// gone by the time the row is read.
    label: Option<String>,
    origin: OriginId,
    payload: Value,
    /// How many lines the payload takes as a tree (`stream::lines`), so a
    /// row's height is arithmetic and never a measurement.
    lines: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EntryKind {
    Event,
    Warning,
    Note,
}

/// What the main window's core believed at the end of its last frame,
/// read by whichever window draws the facts tab. Strings rather than the
/// facts themselves: the panel prints them, and a second window's core
/// cannot ask the first's doors.
/// One declared token as the panel shows it (ADR 0027, decision 7):
/// which origin declared it, its halves, and what it resolved to this
/// frame — the value the inspector matches a node's paint against.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TokenFact {
    pub(crate) origin: OriginId,
    pub(crate) name: String,
    pub(crate) kind: crate::tokens::TokenKind,
    pub(crate) light: Color,
    pub(crate) dark: Color,
    /// This frame's colour, for a colour token.
    pub(crate) resolved: Color,
    /// The px, for a length token.
    pub(crate) length: f32,
    /// A derived colour token's recipe, as the panel prints it beside the
    /// hex (ADR 0028): `peach → lift 0.3`.
    pub(crate) recipe: Option<String>,
}

#[derive(Clone, Default)]
pub(crate) struct Facts {
    title: String,
    /// `(name, value)` rows of the status block, in order.
    rows: Vec<(&'static str, String)>,
    /// Every declared token, the host's first then each extension's,
    /// each origin in its declaration order.
    pub(crate) tokens: Vec<TokenFact>,
    /// The main core's theme source, for the panel's own window to
    /// mirror when no override is in force — and the base it resolves to,
    /// so the base toggle's "the app's own" can say which that is.
    theme_source: ThemeSource,
    app_appearance: Appearance,
    /// The viewport the main window's nodes were laid out in — what the
    /// picker's overlay covers.
    viewport: Size,
    /// The frame's focus, hover and region, for the inspector's state rows.
    focus: Option<Key>,
    focus_visible: bool,
    region: Option<Key>,
    hovered: Option<Key>,
    pressed: Option<Key>,
    /// The main window's pointer, for the picker.
    cursor: Option<Vec2>,
}

/// The session's devtools state (ADR 0024, decision 5).
pub(crate) struct State {
    pub(crate) on: bool,
    pub(crate) dock: Dock,
    /// The docked pane's extent — a side column's width, the bottom
    /// strip's height — as the handle last left it.
    side_w: f32,
    bottom_h: f32,
    tab: Tab,
    // The overrides. `None` leaves the app's own.
    base: Option<Appearance>,
    /// `Some(i)` walks [`ACCENTS`]; with `custom_accent` it is that colour.
    accent: Option<usize>,
    custom_accent: Option<Color>,
    native_menus: Option<bool>,
    // The stream.
    stream: VecDeque<Entry>,
    seq: u64,
    /// The rows the events tab shows (indices into `stream`, after the
    /// filter) and their heights — rebuilt when `stream_dirty`.
    rows: Vec<usize>,
    heights: RowHeights,
    expanded: FxHashSet<u64>,
    stream_dirty: bool,
    /// The stream grew since the list last followed it.
    stream_grew: bool,
    /// The list's travel (`max_offset.y`) the follow last pinned against:
    /// a travel that differs is content that changed under the pin, an
    /// offset short of an unchanged travel is the user's wheel.
    followed_max: f32,
    paused: bool,
    follow: bool,
    stream_filter: String,
    // The tree.
    selected: Option<Key>,
    /// The row under the pointer in whichever window draws the tree, for
    /// the outline the main window paints.
    hovered_row: Option<Key>,
    collapsed: FxHashSet<Key>,
    tree_filter: String,
    /// The picker is up (decision 9).
    pick: bool,
    /// The picker was raised from a declared tab (`Core::set_devtools_pick`
    /// while one was on show, ADR 0032): the pick lands in `selected` for
    /// that tab to read, and the tab stays up rather than the tree tab
    /// taking over.
    pick_keep_tab: bool,
    /// The node the picker last saw under the pointer.
    pick_hover: Option<Key>,
    /// Scroll the tree to this node's row on the next build, with the
    /// rows above it expanded.
    reveal: Option<Key>,
    /// Collapse every row with children on the next build.
    fold_all: bool,
    /// The row the keyboard is on (backlog D1a): the tree's own cursor,
    /// moved by the arrows on the list's sink and painted as a ring while
    /// the list holds focus. None until a key lands on the list.
    tree_cursor: Option<Key>,
    /// A key the list's sink heard, for the next build to apply with the
    /// rows in hand: `up`, `down`, `left`, `right`, `home`, `end`,
    /// `pageup`, `pagedown`, `enter`, `space`.
    tree_key: Option<String>,
    // What the main window wrote for the others.
    facts: Facts,
    nodes: Vec<NodeInfo>,
    /// Frames of the main window so far, and `KUI_SMOKE_FRAMES` when set.
    frames: u64,
    smoke_frames: Option<u64>,
    warnings_seen: usize,
    legend: Vec<(String, String)>,
    /// The chord that moves the keyboard into the panel and back out —
    /// `Ctrl+Shift+I` unless the app respelled it.
    inspect_key: Accel,
    /// The tabs declared in the main window's last finished frame (ADR
    /// 0032): what the strip lists after its own three. Set by the main
    /// window at the end of its frame; a left dock and the panel's own
    /// window read it a frame late, as they read the facts.
    tabs: Vec<TabDecl>,
    /// The declared tab the panel shows, by name — over `tab` while the
    /// name is in `tabs`; a name that is gone falls back to `tab`.
    custom: Option<String>,
    /// The inspect chord waiting for the next main-window build.
    toggle_region: bool,
    /// The picker was left by a raw `Escape` press: the editor channel's
    /// half of the same press, still to come, is the panel's too.
    escape_owed: bool,
    /// The dock wants the keyboard in its own window: `Focus` it once.
    focus_window: bool,
    /// The picker was raised from the panel's own window: `Focus` the
    /// main window once, since that is where the picking happens and
    /// the panel's window is what the pointer and the keyboard are in.
    focus_main: bool,
    /// A build in the panel's own window changed what the main window
    /// paints (the hovered row's outline): ask it to draw once.
    redraw_main: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            on: false,
            dock: Dock::default(),
            side_w: DOCK_SIDE_W,
            bottom_h: DOCK_BOTTOM_H,
            tab: Tab::default(),
            base: None,
            accent: None,
            custom_accent: None,
            native_menus: None,
            stream: VecDeque::new(),
            seq: 0,
            rows: Vec::new(),
            heights: RowHeights::new(0, STREAM_ROW_H),
            expanded: FxHashSet::default(),
            stream_dirty: false,
            stream_grew: false,
            followed_max: 0.0,
            paused: false,
            follow: true,
            stream_filter: String::new(),
            selected: None,
            hovered_row: None,
            collapsed: FxHashSet::default(),
            tree_filter: String::new(),
            pick: false,
            pick_keep_tab: false,
            pick_hover: None,
            reveal: None,
            fold_all: false,
            tree_cursor: None,
            tree_key: None,
            facts: Facts::default(),
            nodes: Vec::new(),
            frames: 0,
            smoke_frames: std::env::var("KUI_SMOKE_FRAMES")
                .ok()
                .and_then(|s| s.parse().ok()),
            warnings_seen: 0,
            legend: Vec::new(),
            inspect_key: DEFAULT_INSPECT_KEY,
            tabs: Vec::new(),
            custom: None,
            toggle_region: false,
            escape_owed: false,
            focus_window: false,
            focus_main: false,
            redraw_main: false,
        }
    }
}

impl State {
    /// What `KUI_DEVTOOLS` asks for: `1`/`true` the panel where it goes by
    /// default (the right), a placement name the panel there, anything
    /// else nothing.
    fn from_var(var: Option<&str>) -> Self {
        let mut s = State::default();
        if !cfg!(feature = "devtools") {
            return s;
        }
        if let Some(v) = var {
            let v = v.trim().to_ascii_lowercase();
            if v == "1" || v == "true" || v == "on" {
                s.on = true;
            } else if let Some(d) = Dock::parse(&v) {
                s.on = true;
                s.dock = d;
            }
        }
        s
    }

    fn base_name(&self) -> &'static str {
        match self.base {
            None => "app",
            Some(Appearance::Light) => "light",
            Some(_) => "dark",
        }
    }

    /// The menu override's name, the select's spelling: `platform` is
    /// the host's own mode, whatever that is.
    fn menus_name(&self) -> &'static str {
        match self.native_menus {
            None => "platform",
            Some(true) => "native",
            Some(false) => "drawn",
        }
    }

    fn accent_name(&self) -> String {
        match (self.accent, self.custom_accent) {
            (Some(i), _) => ACCENTS[i].0.to_string(),
            (None, Some(c)) => format!("#{:06x}", c.to_hex() >> 8),
            (None, None) => "app".to_string(),
        }
    }

    /// The theme the overrides add up to, or `None` for the app's own.
    /// `app` is the app's own source, which the half an override leaves
    /// alone is read from: "the app's base" is the base *the app* chose,
    /// not the OS's, so an app that pinned dark on a light desktop stays
    /// dark under an accent override, and an app with a brand accent
    /// keeps it under a base override. (The accent half used to go out
    /// as `DerivedWithAccent`, which follows `env.system`, so a
    /// pinned-dark app flipped light the moment `Ctrl+Shift+A` was
    /// pressed — the pomodoro's report of 2026-09-12.)
    fn theme_override(&self, app: ThemeSource, sys: &SystemEnv) -> Option<ThemeSource> {
        let accent = self
            .accent
            .map(|i| Color::hex(ACCENTS[i].1))
            .or(self.custom_accent);
        match (self.base, accent) {
            (None, None) => None,
            (None, Some(c)) => Some(match app {
                // The app follows the OS's base: keep following it.
                ThemeSource::Derived | ThemeSource::DerivedWithAccent(_) => {
                    ThemeSource::DerivedWithAccent(c)
                }
                // The app pinned a palette: the same palette, recoloured.
                ThemeSource::Pinned(t) => ThemeSource::Pinned(t.with_accent(c)),
            }),
            (Some(base), c) => {
                // The app's own accent under the chosen base — the OS's
                // where the app follows the OS, so a host that reports
                // none still paints kui's blue byte for byte.
                let own = match app {
                    ThemeSource::Derived => sys.accent,
                    ThemeSource::DerivedWithAccent(a) => Some(a),
                    ThemeSource::Pinned(t) => Some(t.accent),
                };
                Some(ThemeSource::Pinned(Theme::derive(base, c.or(own))))
            }
        }
    }

    fn push(&mut self, mut e: Entry) {
        if self.stream.len() >= STREAM_CAP {
            self.stream.drain(..STREAM_EVICT);
            // The rows that went take their open state with them.
            if let Some(first) = self.stream.front().map(|e| e.seq) {
                self.expanded.retain(|seq| *seq >= first);
            }
        }
        e.seq = self.seq;
        self.seq += 1;
        e.lines = lines(&e.payload);
        self.stream.push_back(e);
        self.stream_dirty = true;
        self.stream_grew = true;
    }

    fn note(&mut self, text: impl Into<String>) {
        let frame = self.frames;
        self.push(Entry {
            seq: 0,
            frame,
            kind: EntryKind::Note,
            window: WindowId::MAIN,
            key: Key::ROOT,
            label: None,
            origin: OriginId::DEVTOOLS,
            payload: Value::str(text.into()),
            lines: 0,
        });
    }

    /// One of the panel's actions, by the name its button and its chord
    /// share. Returns whether `what` was one.
    fn act(&mut self, what: &str) -> bool {
        match what {
            "base" => {
                self.base = match self.base {
                    None => Some(Appearance::Light),
                    Some(Appearance::Light) => Some(Appearance::Dark),
                    Some(_) => None,
                };
                self.note(format!("theme base: {}", self.base_name()));
            }
            "accent" => {
                self.custom_accent = None;
                self.accent = match self.accent {
                    None => Some(0),
                    Some(i) if i + 1 < ACCENTS.len() => Some(i + 1),
                    Some(_) => None,
                };
                self.note(format!("accent: {}", self.accent_name()));
            }
            "menus" => {
                // The select's three, round: the host's own mode (which
                // `begin_frame` puts back from `dt_menus`), native, drawn.
                // Not a toggle from a compile-time guess at the host's
                // mode, which could never return to it (backlog RG12).
                self.native_menus = match self.native_menus {
                    None => Some(true),
                    Some(true) => Some(false),
                    Some(false) => None,
                };
                self.note(format!("menus: {}", self.menus_name()));
            }
            "dock" => {
                self.dock = self.dock.next();
                self.pick = false;
                self.note(format!("dock: {}", self.dock.name()));
            }
            "inspect" => match self.dock {
                // Nothing to enter with the panel hidden: it comes back
                // first, docked.
                Dock::Off => {
                    self.dock = Dock::Right;
                    self.toggle_region = true;
                }
                Dock::Window => self.focus_window = true,
                _ => self.toggle_region = true,
            },
            "clear" => {
                self.stream.clear();
                self.expanded.clear();
                self.stream_dirty = true;
            }
            "pause" => self.paused = !self.paused,
            "follow" => {
                self.follow = !self.follow;
                self.stream_grew |= self.follow;
            }
            "tab" => {
                // The panel's three, then the declared ones, then round.
                let n = Tab::ALL.len() + self.tabs.len();
                let i = match self.shown() {
                    Shown::Builtin(t) => Tab::ALL.iter().position(|x| *x == t).unwrap_or(0),
                    Shown::Custom(i) => Tab::ALL.len() + i,
                };
                self.select_tab((i + 1) % n);
            }
            "pick" => {
                self.pick = !self.pick;
                // The chord's pick shows the tree tab and lands there;
                // a tab's earlier, cancelled pick must not decide otherwise.
                self.pick_keep_tab = false;
                if self.pick {
                    self.show(Tab::Tree);
                    if self.dock == Dock::Off {
                        self.dock = Dock::Right;
                    }
                    // Picking happens in the main window; raised from
                    // the panel's own, the keyboard (for Escape) and the
                    // pointer are both in the wrong one.
                    self.focus_main = self.dock == Dock::Window;
                }
                self.pick_hover = None;
            }
            "fold-all" => self.fold_all = true,
            "unfold-all" => {
                self.collapsed.clear();
                self.fold_all = false;
            }
            other => {
                if let Some(name) = other.strip_prefix("tab:custom:") {
                    if let Some(i) = self.tabs.iter().position(|t| t.name == name) {
                        self.select_tab(Tab::ALL.len() + i);
                    }
                } else if let Some(tab) = other.strip_prefix("tab:").and_then(Tab::parse) {
                    self.tab = tab;
                    self.custom = None;
                } else if let Some(base) = other.strip_prefix("base:") {
                    // The Facts tab's select: one choice, not the next.
                    self.base = match base {
                        "light" => Some(Appearance::Light),
                        "dark" => Some(Appearance::Dark),
                        _ => None,
                    };
                    self.note(format!("theme base: {}", self.base_name()));
                } else if let Some(accent) = other.strip_prefix("accent:") {
                    self.custom_accent = None;
                    self.accent = accent.parse().ok().filter(|i| *i < ACCENTS.len());
                    self.note(format!("accent: {}", self.accent_name()));
                } else if let Some(menus) = other.strip_prefix("menus:") {
                    self.native_menus = match menus {
                        "native" => Some(true),
                        "drawn" => Some(false),
                        _ => None,
                    };
                    self.note(format!("menus: {}", self.menus_name()));
                } else if let Some(dock) = other.strip_prefix("dock:").and_then(Dock::parse) {
                    // One of the header's placement buttons.
                    if dock != self.dock {
                        self.dock = dock;
                        self.pick = false;
                        self.note(format!("dock: {}", dock.name()));
                    }
                } else if let Some(key) = other.strip_prefix("node:").and_then(parse_key) {
                    // A tree row: select it, or unselect the selected one.
                    self.selected = if self.selected == Some(key) {
                        None
                    } else {
                        Some(key)
                    };
                } else if let Some(key) = other.strip_prefix("goto:").and_then(parse_key) {
                    // The inspector's parent row or a breadcrumb: select
                    // and bring its row into view.
                    self.select(key);
                } else if let Some(key) = other.strip_prefix("fold:").and_then(parse_key) {
                    if !self.collapsed.remove(&key) {
                        self.collapsed.insert(key);
                    }
                } else if let Some(seq) = other.strip_prefix("row:").and_then(|s| s.parse().ok()) {
                    if !self.expanded.remove(&seq) {
                        self.expanded.insert(seq);
                    }
                    self.stream_dirty = true;
                } else if let Some(code) = other.strip_prefix("tree-key:") {
                    // A key on the tree's list: the build applies it, since
                    // moving needs the rows and folding needs the children.
                    self.tree_key = Some(code.to_string());
                } else {
                    return false;
                }
            }
        }
        true
    }

    /// Selects `key` and asks the next tree build to expand the rows
    /// above it and scroll to it — what the picker and the inspector's
    /// links do. The build has the nodes; this has only the key.
    fn select(&mut self, key: Key) {
        self.selected = Some(key);
        self.show(Tab::Tree);
        self.reveal = Some(key);
    }

    /// Shows one of the panel's own tabs.
    fn show(&mut self, tab: Tab) {
        self.tab = tab;
        self.custom = None;
    }

    /// The tab on show: `custom` while it names a declared tab, else
    /// `tab` (ADR 0032, decision 1).
    pub(crate) fn shown(&self) -> Shown {
        match self.custom.as_deref() {
            Some(name) => match self.tabs.iter().position(|t| t.name == name) {
                Some(i) => Shown::Custom(i),
                None => Shown::Builtin(self.tab),
            },
            None => Shown::Builtin(self.tab),
        }
    }

    /// Whether `name` is a declared tab the panel lists — what `shown`
    /// falls back on when it is not, and what the laziness rule asks so
    /// a stale `custom` (a tab the app stopped declaring) shows nothing
    /// and builds nothing.
    fn lists(&self, name: &str) -> bool {
        self.tabs.iter().any(|t| t.name == name)
    }

    /// Selects the `i`th tab of the strip: the three, then the declared.
    fn select_tab(&mut self, i: usize) {
        if i < Tab::ALL.len() {
            self.show(Tab::ALL[i]);
        } else if let Some(t) = self.tabs.get(i - Tab::ALL.len()) {
            self.custom = Some(t.name.clone());
        }
    }
}

fn parse_key(hex: &str) -> Option<Key> {
    u64::from_str_radix(hex, 16).ok().map(Key)
}

/// `{ dt: what }`, the payload every control of the panel posts.
fn action(what: impl Into<String>) -> Value {
    Value::map([("dt", Value::str(what.into()))])
}

/// What `Ctrl+Shift+I` parses to: the inspect chord until an app
/// respells it.
const DEFAULT_INSPECT_KEY: Accel = Accel {
    code: KeyCode::Char('i'),
    mods: crate::input::KeyMods {
        shift: true,
        ctrl: true,
        alt: false,
        super_key: false,
    },
};

/// Whether a press is this chord: the modifiers exactly, and the key by
/// the layout's character first and the physical position second (ADR
/// 0002, decision 11) — case-blind for a character, since Shift is part
/// of the chord and the layout has already applied it.
fn hits(accel: Accel, press: &crate::input::KeyPress) -> bool {
    if press.mods != accel.mods {
        return false;
    }
    let same = |code: KeyCode| match (accel.code, code) {
        (KeyCode::Char(a), KeyCode::Char(b)) => a.eq_ignore_ascii_case(&b),
        (a, b) => a == b,
    };
    same(press.code) || same(press.physical)
}

/// The panel's chord for a key press, if it is one: the inspect chord
/// (`Ctrl+Shift+I`, or what the app respelled it to), else `Ctrl+Shift`
/// and a letter, by the layout's character first and the physical
/// position second (ADR 0002, decision 11).
fn chord(press: &crate::input::KeyPress, inspect: Accel) -> Option<&'static str> {
    if hits(inspect, press) {
        return Some("inspect");
    }
    let m = press.mods;
    if !m.ctrl || !m.shift || m.alt {
        return None;
    }
    let letter = |c: KeyCode| match c {
        KeyCode::Char(ch) => Some(ch.to_ascii_lowercase()),
        _ => None,
    };
    match letter(press.code).or_else(|| letter(press.physical))? {
        't' => Some("base"),
        'a' => Some("accent"),
        'm' => Some("menus"),
        'd' => Some("dock"),
        'c' => Some("clear"),
        'n' => Some("tab"),
        // `I` is the inspect chord's letter only while that is what the
        // chord is: respelled to `F12`, `Ctrl+Shift+I` is the app's again.
        'p' => Some("pick"),
        _ => None,
    }
}

/// The key of a declared tab's body (ADR 0032, decision 2): fixed by the
/// tab's name alone, so the content can anchor to it before it exists —
/// the host form is built before a right dock's body, after a left one's.
pub(crate) fn tab_body_key(name: &str) -> Key {
    Key::ROOT.str(DEVTOOLS_KEY).str("tab").str(name)
}

impl Core {
    /// Declares a devtools tab this frame (ADR 0032, decision 1). A name
    /// declared already this frame warns `duplicate-tab` and keeps the
    /// first; returns whether this one stood. The bare door under
    /// `Ui::devtools_tab` / `devtools_tab_with`, for a binding that
    /// opens the content itself.
    pub fn devtools_tab_declare(&mut self, name: &str, label: &str, slot: Option<&str>) -> bool {
        if !cfg!(feature = "devtools") || self.tree.is_empty() {
            return false;
        }
        if self.dt_tabs.iter().any(|t| t.name == name) {
            self.diag.raise(crate::diag::duplicate_tab(name));
            return false;
        }
        self.dt_tabs.push(TabDecl {
            name: name.to_string(),
            label: label.to_string(),
            slot: slot.map(str::to_string),
        });
        true
    }

    /// Whether the host form of tab `name` is shown this frame — the
    /// panel is on, docked in this (the main) window, and `name` is the
    /// tab on show (ADR 0032, decision 3). Read before the content is
    /// built, from the session's state, which is in place while the
    /// host's view runs.
    pub fn devtools_tab_shown(&self, name: &str) -> bool {
        if !cfg!(feature = "devtools") || self.dt_window || self.env.window.id != WindowId::MAIN {
            return false;
        }
        let s = self.session.state();
        let d = &s.devtools;
        d.on && d.dock.docked() && d.custom.as_deref() == Some(name) && d.lists(name)
    }

    /// The tab on show, by name, when it is a declared one — what the
    /// Node and Lua drivers read once a frame to call a tab's function
    /// child (ADR 0032, decision 3). `None` for one of the panel's own,
    /// for the panel off, popped out, or another window's frame.
    pub fn devtools_shown_tab(&self) -> Option<String> {
        if !cfg!(feature = "devtools") || self.dt_window || self.env.window.id != WindowId::MAIN {
            return None;
        }
        let s = self.session.state();
        let d = &s.devtools;
        if !(d.on && d.dock.docked()) {
            return None;
        }
        d.custom.clone().filter(|name| d.lists(name))
    }

    /// Opens the host form's content node: a float anchored to the tab's
    /// body by key, the body's size, clipped, keyed as the host's own
    /// child (ADR 0032, decisions 2 and 7). The caller builds inside and
    /// closes. The bare door under `Ui::devtools_tab_with`; it does not
    /// declare, and it does not ask whether the tab is on show.
    pub fn devtools_tab_open(&mut self, name: &str) {
        let spec = NodeSpec::column()
            .float(crate::spec::FloatConfig {
                anchor: crate::spec::FloatAnchor::Node(tab_body_key(name)),
                ..Default::default()
            })
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .clip();
        self.open_keyed(&format!("devtools-tab:{name}"), spec);
    }

    /// Whether `name` is a slot a declared tab names this frame — what
    /// counts as *declared* for the `unknown-slot` check whether or not
    /// the panel mounted it (ADR 0032, decision 5).
    pub(crate) fn devtools_tab_slot(&self, name: &str) -> bool {
        self.dt_tabs.iter().any(|t| t.slot.as_deref() == Some(name))
    }

    /// Mounts the extension form of the tab on show, if it has one: the
    /// slot is declared at the cursor under the host's origin — so the
    /// fill's replies reach the host, as ADR 0014's rule reads with the
    /// panel for declarer — and filled inside a float anchored to the
    /// tab's body, the panel's facts as params (ADR 0032, decisions 2,
    /// 4 and 5). Called with the filler in hand: from `Ui::finish` in the
    /// main window before the filler's own finish, and in the panel's
    /// window after its build. Nothing to do when the tab on show is the
    /// panel's own or a host form.
    pub(crate) fn devtools_fill_mount(&mut self, filler: &mut dyn crate::slot::Fill) {
        if !cfg!(feature = "devtools") || self.tree.is_empty() {
            return;
        }
        let (name, slot, params) = {
            let s = self.session.state();
            let d = &s.devtools;
            if !d.on || !(d.dock.docked() || self.dt_window) {
                return;
            }
            let tabs = if self.dt_window {
                &d.tabs
            } else {
                &self.dt_tabs
            };
            let Some(name) = d.custom.as_deref() else {
                return;
            };
            let Some(decl) = tabs.iter().find(|t| t.name == name) else {
                return;
            };
            let Some(slot) = decl.slot.clone() else {
                return;
            };
            (decl.name.clone(), slot, d.facts_params())
        };
        let saved = self.origin;
        self.origin = OriginId::HOST;
        if let Some(key) = self.begin_slot(&slot) {
            self.devtools_tab_open(&name);
            filler.fill(&slot, key, &params, &mut crate::ui::Ui::new(self));
            self.close();
        }
        self.origin = saved;
    }
}

impl State {
    /// The panel's facts as a slot's params (ADR 0032, decision 4): the
    /// selected, hovered and picked nodes as hex keys (`null` for none),
    /// the region and the focus by label from the facts rows.
    fn facts_params(&self) -> Value {
        let key = |k: Option<Key>| match k {
            Some(k) => Value::str(format!("{:016x}", k.0)),
            None => Value::Null,
        };
        let row = |name: &str| {
            self.facts
                .rows
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| Value::str(v.clone()))
                .unwrap_or(Value::Null)
        };
        Value::map([
            ("selected", key(self.selected)),
            ("hovered", key(self.hovered_row)),
            ("picked", key(self.pick_hover)),
            ("region", row("region")),
            ("focus", row("focus")),
        ])
    }
}

// ---------------------------------------------------------------------------
// The doors.

impl Core {
    /// Turns the devtools panel on or off for this session (ADR 0024). On,
    /// it is drawn where [`Self::set_devtools_dock`] says — beside the
    /// host's tree in the main window by default — and its chords are
    /// live in every window. `KUI_DEVTOOLS=1` in the environment is the
    /// same call made by nobody; `KUI_DEVTOOLS=bottom` (or `left`,
    /// `right`, `window`, `off`) also says where.
    pub fn set_devtools(&mut self, on: bool) {
        // Built without the panel: the door stays and does nothing.
        if !cfg!(feature = "devtools") {
            return;
        }
        let mut s = self.session.state();
        if s.devtools.on == on {
            return;
        }
        s.devtools.on = on;
        if !on {
            s.devtools.pick = false;
            s.devtools.pick_keep_tab = false;
        }
    }

    /// Opens the panel if `KUI_DEVTOOLS` in the environment asks for it
    /// (`1`, `true`, or a placement name), and says whether it did. What
    /// the windowed runners call once, before the first frame — the door
    /// for a program that was never told about the panel — and what a
    /// headless core never reads, so a variable left exported cannot put
    /// a dock into a test's tree.
    pub fn devtools_from_env(&mut self) -> bool {
        let asked = State::from_var(std::env::var("KUI_DEVTOOLS").ok().as_deref());
        if asked.on {
            self.set_devtools(true);
            self.set_devtools_dock(asked.dock);
        }
        asked.on
    }

    /// Whether the panel is on.
    pub fn devtools(&self) -> bool {
        self.session.state().devtools.on
    }

    /// Where the panel sits; `Ctrl+Shift+D` moves it from there.
    pub fn set_devtools_dock(&mut self, dock: Dock) {
        self.session.state().devtools.dock = dock;
    }

    pub fn devtools_dock(&self) -> Dock {
        self.session.state().devtools.dock
    }

    /// Seeds the panel's theme override — what its `T` and `A` chords
    /// cycle from. `None` for either leaves the app's own.
    pub fn set_devtools_theme(&mut self, base: Option<Appearance>, accent: Option<Color>) {
        let mut s = self.session.state();
        s.devtools.base = base;
        s.devtools.accent = None;
        s.devtools.custom_accent = accent;
    }

    /// Respells the chord that moves the keyboard into the panel and
    /// back out — and brings the panel back when it is `off` — from its
    /// default `Ctrl+Shift+I`: any [`Accel`] spelling (`"f12"`,
    /// `"mod+shift+d"`, `"⌥⌘I"`). The other chords stay `Ctrl+Shift+
    /// <letter>`; this is the one an app puts in its own help, and the
    /// one whose default an app's keymap may want for itself. A chord
    /// the app takes is the app's for good: with `F12` set,
    /// `Ctrl+Shift+I` reaches the app's sinks like any other press.
    pub fn set_devtools_key(&mut self, key: Accel) {
        self.session.state().devtools.inspect_key = key;
    }

    /// The chord that moves the keyboard into the panel, as set or as
    /// it defaults.
    pub fn devtools_key(&self) -> Accel {
        self.session.state().devtools.inspect_key
    }

    /// The node the panel's tree tab has selected (ADR 0032, decision
    /// 4): what an inspector in a declared tab reads to say which node
    /// it is about. Answered from the session, so it is right inside the
    /// host's view and inside an extension's fill alike.
    pub fn devtools_selected(&self) -> Option<Key> {
        self.session.state().devtools.selected
    }

    /// The tree row under the pointer in whichever window draws the
    /// tree — the node the main window outlines.
    pub fn devtools_hovered(&self) -> Option<Key> {
        self.session.state().devtools.hovered_row
    }

    /// The node the picker last saw under the pointer, while picking.
    pub fn devtools_picked(&self) -> Option<Key> {
        self.session.state().devtools.pick_hover
    }

    /// Selects a node in the panel's tree tab from outside it — an
    /// inspector driving the highlight from its side — and reveals it
    /// there, as the picker does; `None` clears. The tab does not move:
    /// the caller is drawing in one.
    pub fn set_devtools_selected(&mut self, key: Option<Key>) {
        let mut s = self.session.state();
        let d = &mut s.devtools;
        d.selected = key;
        d.reveal = key;
        drop(s);
        self.devtools_redraw_others();
    }

    /// Raises the panel's picker from outside it — an inspector in a
    /// declared tab asking "which node?" — or puts it away (ADR 0032,
    /// decision 4). Picking happens in the main window, over the app: the
    /// node under the pointer is `devtools_picked` while it is up, and
    /// the press lands it in `devtools_selected`. Raised while a declared
    /// tab is on show, the pick leaves that tab up; raised otherwise it
    /// is the `Ctrl+Shift+P` pick, which shows the tree tab. A hidden
    /// panel comes back docked, as the chord's does.
    pub fn set_devtools_pick(&mut self, on: bool) {
        if !cfg!(feature = "devtools") {
            return;
        }
        let focus_main = {
            let mut s = self.session.state();
            let d = &mut s.devtools;
            if d.pick == on {
                return;
            }
            d.pick = on;
            d.pick_hover = None;
            d.pick_keep_tab = on && d.custom.is_some();
            if on {
                if d.custom.is_none() {
                    d.show(Tab::Tree);
                }
                if d.dock == Dock::Off {
                    d.dock = Dock::Right;
                }
            }
            on && d.dock == Dock::Window
        };
        if focus_main {
            self.interaction
                .window_commands
                .push(WindowCommand::Focus(WindowId::MAIN));
        }
        self.devtools_redraw_others();
    }

    /// Whether the panel's picker is up.
    pub fn devtools_picking(&self) -> bool {
        self.session.state().devtools.pick
    }

    /// Shows the panel's tab named `name` from outside the panel — what
    /// the strip's click and `Ctrl+Shift+N` do, for an app with a command
    /// that jumps to its own tab (ADR 0032). `name` is one of the panel's
    /// own (`facts`, `events`, `tree`) or a declared tab's. A declared
    /// name the panel does not list yet is kept and shows once a frame
    /// declares it, as a strip click on it would; the return says whether
    /// the panel lists it now (it lists a declared tab from the first
    /// frame it is on). A hidden panel comes back docked, as the
    /// picker's does. The panel's `on` is not touched: that is
    /// [`Self::set_devtools`]'s. Edge-triggered — called once a frame it
    /// would pin the strip against the user's own clicks.
    pub fn set_devtools_tab(&mut self, name: &str) -> bool {
        if !cfg!(feature = "devtools") {
            return false;
        }
        let listed = {
            let mut s = self.session.state();
            let d = &mut s.devtools;
            let listed = match Tab::parse(name) {
                Some(tab) => {
                    d.show(tab);
                    true
                }
                None => {
                    d.custom = Some(name.to_string());
                    d.lists(name)
                }
            };
            if d.dock == Dock::Off {
                d.dock = Dock::Right;
            }
            listed
        };
        self.devtools_redraw_others();
        listed
    }

    /// The tab the panel is on, by name: one of its own (`facts`,
    /// `events`, `tree`) or a declared tab's — what the strip marks,
    /// panel on or off, in any window. A declared name the panel stopped
    /// listing answers the panel's own tab the strip falls back to.
    /// Unlike [`Self::devtools_shown_tab`], which answers only a declared
    /// tab on show in the main window for a data binding's function
    /// child, this is the selection itself.
    pub fn devtools_current_tab(&self) -> String {
        let s = self.session.state();
        let d = &s.devtools;
        match d.shown() {
            Shown::Builtin(t) => t.name().to_string(),
            Shown::Custom(i) => d.tabs[i].name.clone(),
        }
    }

    /// The key legend the facts tab shows: `(keys, what they do)`.
    pub fn set_devtools_legend(&mut self, legend: &[(&str, &str)]) {
        self.session.state().devtools.legend = legend
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
    }

    /// Whether this core draws the panel's own window — a frame the host
    /// builds nothing into (decision 6).
    pub fn devtools_window(&self) -> bool {
        self.dt_window
    }

    /// The app's own theme source: the one in force while no override is,
    /// else the one remembered when the override went on — unless the app
    /// has set another since, which shows as the source in force not being
    /// the override that was applied. That one is the app's now, and it
    /// is what the override is lifted back to.
    fn app_theme_source(&self) -> ThemeSource {
        match self.dt_theme {
            Some((app, applied)) if self.theme_source == applied => app,
            _ => self.theme_source,
        }
    }

    /// The id of the panel's window while it is open.
    fn devtools_window_id(&self) -> Option<WindowId> {
        self.session
            .state()
            .windows
            .live()
            .into_iter()
            .find(|(_, name)| &**name == DEVTOOLS_WINDOW)
            .map(|(id, _)| id)
    }
}

// ---------------------------------------------------------------------------
// The hooks.

impl Core {
    /// What the dock leaves of a window `window` big (ADR 0024): the
    /// viewport a frame begun at that size lays out into, which
    /// `viewport()` reports once the frame has begun and a `resize`
    /// reports when it changes. It takes the window's size and the dock's
    /// state and nothing of the frame, so it answers *before* the first
    /// frame too — what a driver's window-size reading hands a host that
    /// sizes its model at setup (Node's `KuiWindow.size()`; backlog F43,
    /// where that reading was the window's and `env().viewport` was still
    /// 0×0 that early, so no reading said the right number).
    pub fn host_area(&self, window: Size) -> Size {
        let r = self.devtools_area(window);
        Size::new(r.w, r.h)
    }

    /// What a docked pane takes off the main window, in the axis it
    /// takes it: the side column's width as `(w, 0)`, the bottom strip's
    /// height as `(0, h)`, and zero with the panel off, in a window of
    /// its own, or asked of any window but the main one. The pane's
    /// extent *as the handle left it*, not as the window clamps it — what
    /// a driver adds to the app's minimum window size while the panel is
    /// docked, so the floor the app declared is a floor on the app and
    /// not on the app less the dock (the pomodoro's report, 2026-09-12:
    /// a 620×500 minimum with a 340 px dock left the app 280 px, below
    /// the tier it was drawn to fit). Read after a frame, since the
    /// handle's drag and the placement buttons land in one.
    pub fn devtools_inset(&self) -> Size {
        if self.env.window.id != WindowId::MAIN {
            return Size::ZERO;
        }
        let s = self.session.state();
        let d = &s.devtools;
        if !d.on {
            return Size::ZERO;
        }
        match d.dock {
            Dock::Left | Dock::Right => Size::new(d.side_w.max(SIDE_MIN_W), 0.0),
            Dock::Bottom => Size::new(0.0, d.bottom_h.max(BOTTOM_MIN_H)),
            Dock::Window | Dock::Off => Size::ZERO,
        }
    }

    /// The host's viewport for a frame at `viewport` (ADR 0024): the
    /// window, less the dock when the panel is docked in the main window.
    /// The pane keeps its minimum before the app keeps its own, so a
    /// window too small for both squeezes the app.
    pub(crate) fn devtools_area(&self, viewport: Size) -> Rect {
        let window = Rect::new(0.0, 0.0, viewport.w, viewport.h);
        if self.env.window.id != WindowId::MAIN {
            return window;
        }
        let s = self.session.state();
        let d = &s.devtools;
        if !d.on || !d.dock.docked() {
            return window;
        }
        match d.dock {
            Dock::Left => {
                let w = pane_w(d.side_w, viewport.w);
                Rect::new(w, 0.0, (viewport.w - w).max(0.0), viewport.h)
            }
            Dock::Right => {
                let w = pane_w(d.side_w, viewport.w);
                Rect::new(0.0, 0.0, (viewport.w - w).max(0.0), viewport.h)
            }
            Dock::Bottom => {
                let h = pane_h(d.bottom_h, viewport.h);
                Rect::new(0.0, 0.0, viewport.w, (viewport.h - h).max(0.0))
            }
            _ => window,
        }
    }

    /// The origin of the host's viewport in window coordinates: nonzero
    /// only under a left dock.
    #[inline]
    pub(crate) fn dt_shift(&self) -> Vec2 {
        Vec2::new(self.dt_area.x, self.dt_area.y)
    }

    /// The coordinates the host is handed, in its own viewport (ADR
    /// 0024): a drag's point and parent, a layout's rect and parent, a
    /// context menu's and a force click's point. A no-op wherever the
    /// dock's origin is the window's.
    pub(crate) fn devtools_translate(&self, out: &mut [UiEvent]) {
        let shift = self.dt_shift();
        if shift == Vec2::ZERO {
            return;
        }
        fn shift_xy(v: &mut Value, shift: Vec2) {
            if let Value::Map(entries) = v {
                for (k, v) in entries.iter_mut() {
                    match (k.as_str(), &mut *v) {
                        ("x", Value::Float(x)) => *x -= shift.x as f64,
                        ("y", Value::Float(y)) => *y -= shift.y as f64,
                        ("parent", nested @ Value::Map(_)) => shift_xy(nested, shift),
                        _ => {}
                    }
                }
            }
        }
        for ev in out {
            if ev.origin == OriginId::DEVTOOLS {
                continue;
            }
            if matches!(
                ev.payload.get("kind").and_then(Value::as_str),
                Some("drag" | "layout" | "contextmenu" | "forceclick")
            ) {
                shift_xy(&mut ev.payload, shift);
            }
        }
    }

    /// The end of `begin_frame`: the overrides for this window, and the
    /// wrap of the host's tree in the main one (decision 2) or the
    /// deferred root in the panel's own (decision 6).
    /// Puts the host's own menu mode back after an override (see
    /// `dt_menus`); nothing to do when there was none.
    fn restore_menus(&mut self) {
        if let Some((menus, bar)) = self.dt_menus.take() {
            self.set_native_menus(menus);
            self.set_native_menu_bar(bar);
        }
    }

    pub(crate) fn devtools_begin_frame(&mut self) {
        self.dt_app = None;
        self.dt_dock = None;
        self.dt_window = false;
        self.dt_built = false;
        // This frame's declarations start empty whatever the panel's state
        // and whichever window this is: `after_frame` moves the main
        // window's into the session only while the panel is on, and a
        // declaration made every frame must not pile up into
        // `duplicate-tab` on the second one.
        self.dt_tabs.clear();
        let main = self.env.window.id == WindowId::MAIN;
        let this = if main {
            false
        } else {
            &*self.window_name() == DEVTOOLS_WINDOW
        };
        let (on, dock, theme, menus, mirror, want_inspect) = {
            let s = self.session.state();
            let d = &s.devtools;
            // The app's own source, for the override to keep the half it
            // leaves alone — and the main window's in the panel's own
            // window, which has none of its own.
            let app = if this {
                d.facts.theme_source
            } else {
                self.app_theme_source()
            };
            (
                d.on,
                d.dock,
                d.theme_override(app, &self.env.system),
                d.native_menus,
                app,
                // The panel's need for the node snapshot, derived every
                // frame rather than latched (backlog AR38): the tree tab
                // showing, or a pick under way. Off, the O(nodes) copy a
                // frame stops; the host's own `set_inspect` is apart.
                d.on && (d.shown() == Shown::Builtin(Tab::Tree) || d.pick),
            )
        };
        // A menu of the panel's own — a Facts select's — hangs under a
        // field this window draws only while it builds the panel: off,
        // hidden, or popped into its own window while this is the main
        // one, the field is gone and the menu goes with it, or else the
        // rows would stay drawn with nobody to answer them (backlog RG2).
        // An app's menu is not the panel's to close.
        let builds_panel = on && (this || (main && dock.docked()));
        if !builds_panel
            && self
                .menu
                .as_ref()
                .is_some_and(|m| m.origin == OriginId::DEVTOOLS)
        {
            self.close_menu();
        }
        if !on {
            if let Some((app, _)) = self.dt_theme.take() {
                self.set_theme_source(app);
            }
            self.restore_menus();
            self.dt_inspect = false;
            return;
        }
        // The theme: the override while there is one, and the app's own
        // source — remembered from when the override went on — when it
        // is taken away again. The panel's own window mirrors the main
        // one's app source, since it has none of its own.
        match theme {
            Some(src) => {
                self.dt_theme = Some((mirror, src));
                self.set_theme_source(src);
            }
            None => {
                if self.dt_theme.take().is_some() || this {
                    self.set_theme_source(mirror);
                }
            }
        }
        // The menus the same way: the host's own mode — what the driver
        // said at launch — remembered when the override goes on, and put
        // back when it is lifted (`platform` in the select, or the panel
        // off), since nothing else would.
        match menus {
            Some(m) => {
                if self.dt_menus.is_none() {
                    self.dt_menus = Some((self.native_menus, self.native_menu_bar()));
                }
                self.set_native_menus(m);
                self.set_native_menu_bar(m);
            }
            None => self.restore_menus(),
        }
        if this {
            // The panel's window: no root until `finish`, so what the host
            // builds here builds nothing (every builder door is a no-op
            // on an empty tree).
            self.dt_window = true;
            self.tree.clear();
            self.stack.clear();
            self.counters.clear();
            return;
        }
        if !main {
            return;
        }
        self.dt_inspect = want_inspect;
        if dock.docked() {
            self.dt_dock = Some(dock);
            self.tree.specs[0] = match dock {
                Dock::Bottom => NodeSpec::column(),
                _ => NodeSpec::row(),
            }
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0));
            // A left dock precedes the app in the root row, so it is built
            // now, from the last frame's facts, and `finish` skips it.
            if dock == Dock::Left {
                self.build_panel(Place::Main(dock));
                self.dt_built = true;
            }
            // By key and not by label: the container is not the host's
            // to find through `key_of`.
            self.open_with_key(
                Key::ROOT.str(APP_KEY),
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0))
                    .clip(),
            );
            // The host's children are keyed from the root, as if the
            // container were not there (ADR 0014's namespace, reused).
            self.ns_depth = self.stack.len();
            self.ns_key = Key::ROOT;
            self.dt_app = Some(self.tree.len() - 1);
        }
    }

    /// `configure_root` while the host's tree is wrapped (decision 2):
    /// what lays out and paints the children goes to the container, what
    /// is addressed stays on the root.
    pub(crate) fn devtools_configure_root(&mut self, app: usize, spec: NodeSpec) {
        let key = self.tree.keys[app];
        let mut inner = spec.clone();
        inner.layout.width = Sizing::Grow(1.0);
        inner.layout.height = Sizing::Grow(1.0);
        inner.layout.float = None;
        if !(inner.layout.scroll_x || inner.layout.scroll_y) {
            inner.layout.clip = true;
        }
        inner.events = None;
        inner.access = None;
        inner.interact = None;
        inner.focusable = false;
        inner.initial_focus = false;
        inner.hoverable = false;
        inner.cursor = None;
        inner.window = None;
        self.ease_spec(key, &mut inner);
        self.tree.note(&inner, &crate::tree::NodeContent::Container);
        self.tree.specs[app] = inner;

        let mut outer = self.tree.specs[0].clone();
        outer.style.bg = spec.style.bg;
        outer.events = spec.events;
        outer.access = spec.access;
        outer.focusable = spec.focusable;
        outer.initial_focus = spec.initial_focus;
        outer.hoverable = spec.hoverable;
        outer.disabled = spec.disabled;
        outer.cursor = spec.cursor;
        outer.window = spec.window;
        self.tree.note(&outer, &crate::tree::NodeContent::Container);
        self.tree.specs[0] = outer;
    }

    /// `Ui::finish`, before the menu: closes the app container, builds the
    /// panel where it goes, and declares its window when it has one.
    pub(crate) fn devtools_finish(&mut self) {
        if self.dt_window {
            // The deferred root (decision 6): the same push `begin_frame`
            // makes, and then the panel is the whole tree.
            self.tree.push(
                crate::tree::NIL,
                Key::ROOT,
                OriginId::HOST,
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0)),
                crate::tree::NodeContent::Container,
            );
            self.stack.clear();
            self.stack.push(0);
            self.counters.clear();
            self.counters.push(0);
            self.ns_depth = usize::MAX;
            self.build_panel(Place::Window);
            return;
        }
        if self.env.window.id != WindowId::MAIN {
            return;
        }
        let (on, dock) = {
            let s = self.session.state();
            (s.devtools.on, s.devtools.dock)
        };
        if let Some(_app) = self.dt_app.take() {
            // The host's last node closed for it, as `finish_frame` does.
            self.stack.truncate(1);
            self.counters.truncate(1);
            self.ns_depth = usize::MAX;
        }
        if !on {
            return;
        }
        if dock == Dock::Window {
            let saved = self.origin;
            self.origin = OriginId::DEVTOOLS;
            self.declare_window(
                DEVTOOLS_WINDOW,
                WindowConfig::sized(WINDOW_SIZE.w, WINDOW_SIZE.h),
            );
            self.origin = saved;
        }
        if self.dt_built {
            return;
        }
        // A docked panel is built into the root this frame began with:
        // wrapped for this dock, it goes beside the app container. Turned
        // on, or moved to this dock, since `begin_frame` — an app's
        // `set_devtools` from its `view` — the root is not laid out for it
        // (a column, or a row for another side), and a panel built into it
        // anyway sat in the bottom-left corner, 340 wide and half the
        // height, until the next frame; so it waits for that frame, and
        // asks for it. Undocked, there is nothing to lay out around.
        if !dock.docked() || self.dt_dock == Some(dock) {
            self.build_panel(Place::Main(dock));
        } else {
            self.frame_requested = true;
        }
    }

    /// Opens the `kui-devtools` node under the current cursor and builds
    /// the panel — the dock, the outlines, the picker — inside it, with
    /// the session's state taken out for the length of the build.
    fn build_panel(&mut self, place: Place) {
        let saved_origin = self.origin;
        self.origin = OriginId::DEVTOOLS;
        // The facts the panel reads: this frame's, from this core, when it
        // is the main window; the main window's last, from the session,
        // in the panel's own.
        let mut state = std::mem::take(&mut self.session.state().devtools);
        // The nodes the tree tab lists: this core's last frame in the main
        // window, the main window's copy in the panel's own — taken out
        // for the build, since the build borrows the core.
        let nodes = match place {
            Place::Main(dock) => {
                // A left dock is built at `begin_frame`, before the host
                // has declared this frame's title: it reads the facts the
                // last frame left, as the panel's own window does.
                if dock != Dock::Left {
                    state.facts = self.collect_facts(state.inspect_key);
                    state.tabs.clone_from(&self.dt_tabs);
                } else {
                    // Except the window's size, which is this frame's and
                    // is what the pane's width is clamped by.
                    state.facts.viewport = self.viewport;
                }
                std::mem::take(&mut self.inspected)
            }
            Place::Window => std::mem::take(&mut state.nodes),
        };
        #[cfg(feature = "devtools")]
        {
            let mut ui = crate::ui::Ui::wrap(self);
            panel::build(&mut ui, &mut state, &nodes, place);
        }
        match place {
            Place::Main(_) => self.inspected = nodes,
            Place::Window => state.nodes = nodes,
        }
        // What the build asked of the windows.
        if std::mem::take(&mut state.focus_window)
            && let Some(id) = self.devtools_window_id()
        {
            self.interaction
                .window_commands
                .push(WindowCommand::Focus(id));
        }
        let redraw_main = std::mem::take(&mut state.redraw_main) && self.dt_window;
        self.session.state().devtools = state;
        self.origin = saved_origin;
        if redraw_main {
            self.devtools_redraw_others();
        }
    }

    /// The end of `finish_frame` in the main window: the facts and the
    /// nodes for the panel's own window, the warnings into the stream, the
    /// frame count.
    pub(crate) fn devtools_after_frame(&mut self) {
        if self.env.window.id != WindowId::MAIN || !self.session.state().devtools.on {
            return;
        }
        let tabs = std::mem::take(&mut self.dt_tabs);
        let raised = self.warnings_raised();
        let facts = self.collect_facts(self.devtools_key());
        let mut s = self.session.state();
        let d = &mut s.devtools;
        d.frames = self.frame_no;
        d.facts = facts;
        d.tabs = tabs;
        // The panel's own window draws the tree from a copy; docked, the
        // tab reads this core's list itself and the copy is not kept.
        // The first copy is what the panel's window is waiting for: it
        // drew its tree tab before there was one, so it is asked again.
        let mut wake_panel = false;
        if d.dock == Dock::Window && d.shown() == Shown::Builtin(Tab::Tree) {
            wake_panel = d.nodes.is_empty() && !self.inspected.is_empty();
            d.nodes = self.inspected.clone();
        } else if !d.nodes.is_empty() {
            d.nodes = Vec::new();
        }
        let new_warnings_empty = raised.len() <= d.warnings_seen;
        if raised.len() > d.warnings_seen {
            let new: Vec<(String, String)> = raised[d.warnings_seen..]
                .iter()
                .map(|w| (w.code.to_string(), w.message.clone()))
                .collect();
            d.warnings_seen = raised.len();
            let frame = d.frames;
            for (code, message) in new {
                d.push(Entry {
                    seq: 0,
                    frame,
                    kind: EntryKind::Warning,
                    window: WindowId::MAIN,
                    key: Key::ROOT,
                    label: None,
                    origin: OriginId::DEVTOOLS,
                    payload: Value::map([
                        ("kind", Value::str("warning")),
                        ("code", Value::str(code)),
                        ("message", Value::str(message)),
                    ]),
                    lines: 0,
                });
            }
        }
        // Warnings logged while popped out move the stream there too.
        wake_panel |= !new_warnings_empty && d.dock == Dock::Window;
        drop(s);
        if wake_panel {
            self.devtools_redraw_others();
        }
    }

    /// The start of `handle_input`: a chord, or `Escape` while picking, is
    /// the panel's and the press goes no further (decision 4).
    pub(crate) fn devtools_intercept(&mut self, ev: &InputEvent) -> bool {
        let press = match ev {
            InputEvent::KeyDown(press) => press,
            // The editor channel's Escape is the same key: a driver sends
            // both, and while the picker is up neither is anybody else's.
            InputEvent::Key(crate::EditKey::Escape, _) => {
                let mut s = self.session.state();
                let d = &mut s.devtools;
                return d.on && (d.pick || std::mem::take(&mut d.escape_owed));
            }
            _ => return false,
        };
        if !self.session.state().devtools.on {
            return false;
        }
        if press.code == KeyCode::Escape {
            let mut s = self.session.state();
            if s.devtools.pick {
                s.devtools.pick = false;
                s.devtools.pick_hover = None;
                s.devtools.pick_keep_tab = false;
                s.devtools.escape_owed = true;
                drop(s);
                self.devtools_redraw_others();
                return true;
            }
            return false;
        }
        // Any other press settles the owed half: a driver that never
        // sends the editor channel owes nothing.
        let inspect = {
            let mut s = self.session.state();
            s.devtools.escape_owed = false;
            s.devtools.inspect_key
        };
        let Some(what) = chord(press, inspect) else {
            return false;
        };
        self.devtools_act(what);
        true
    }

    /// One action, from a chord or a control, with what it asks of the
    /// core done after the state borrow ends.
    fn devtools_act(&mut self, what: &str) {
        let (acted, focus_main) = {
            let mut s = self.session.state();
            let acted = s.devtools.act(what);
            (acted, std::mem::take(&mut s.devtools.focus_main))
        };
        if focus_main {
            self.interaction
                .window_commands
                .push(WindowCommand::Focus(WindowId::MAIN));
        }
        if acted {
            self.devtools_redraw_others();
        }
    }

    /// The panel's controls in the batch are acted on and dropped, the
    /// panel's window's own `window` events too, and in the panel's
    /// window nothing at all is the host's (decision 4).
    pub(crate) fn devtools_consume(&mut self, out: &mut Vec<UiEvent>) {
        if out.is_empty() {
            return;
        }
        let on = self.session.state().devtools.on;
        if !on && !self.dt_window {
            // Off, a panel control can still be in the tree the input
            // resolves against — the frame the door owes is not drawn
            // yet — and its event is nobody's: not the app's, which
            // never declared the origin, and not an action either, since
            // the panel it would act on is gone (backlog RG2).
            out.retain(|ev| ev.origin != OriginId::DEVTOOLS);
            return;
        }
        let mut actions: Vec<String> = Vec::new();
        let mut picks = 0usize;
        let mut closed = false;
        let mut resize: Option<Vec2> = None;
        out.retain(|ev| {
            if ev.origin == OriginId::DEVTOOLS {
                // A click carries its payload flat; a drag's or a sink's
                // rides in `tag`; a select's choice is the menu row's
                // `item` (`widgets::select`).
                let what = ev
                    .payload
                    .get("dt")
                    .or_else(|| ev.payload.get("tag").and_then(|t| t.get("dt")))
                    .or_else(|| ev.payload.get("item").and_then(|t| t.get("dt")))
                    .and_then(Value::as_str);
                match what {
                    Some("picked") => picks += 1,
                    Some("tree-key") => {
                        // The tree list's sink: a press (not a release, not
                        // a repeat of a chord) becomes an action naming
                        // the key, for the build to move the cursor by.
                        let s = |k: &str| ev.payload.get(k).and_then(Value::as_str);
                        // Both modifiers ride every key payload, so each
                        // is asked on its own: a chord bubbles to the sink
                        // (ADR 0011) and must not move the cursor.
                        let held = |k: &str| matches!(ev.payload.get(k), Some(Value::Bool(true)));
                        let plain = !held("ctrl") && !held("super");
                        if s("phase") == Some("down")
                            && plain
                            && let Some(code) = s("code")
                        {
                            actions.push(format!("tree-key:{code}"));
                        }
                    }
                    Some("resize") => {
                        let f = |k: &str| ev.payload.get(k).and_then(Value::as_float);
                        if let (Some(x), Some(y)) = (f("x"), f("y")) {
                            resize = Some(Vec2::new(x as f32, y as f32));
                        }
                    }
                    Some(what) => actions.push(what.to_string()),
                    None => {}
                }
                // An edit's `changed` and the like: the build reads the
                // field's text; nothing to do here.
                return false;
            }
            if ev.payload.get("kind").and_then(Value::as_str) == Some("window")
                && ev.payload.get("name").and_then(Value::as_str) == Some(DEVTOOLS_WINDOW)
            {
                closed |= ev.payload.get("phase").and_then(Value::as_str) == Some("closed");
                return false;
            }
            !self.dt_window
        });
        for what in actions {
            self.devtools_act(&what);
        }
        if let Some(p) = resize {
            // The handle is dragged in the main window: the pane's extent
            // is what the pointer leaves between the edge and itself.
            let vp = self.viewport;
            let mut s = self.session.state();
            let d = &mut s.devtools;
            match d.dock {
                Dock::Left => d.side_w = pane_w(p.x, vp.w),
                Dock::Right => d.side_w = pane_w(vp.w - p.x, vp.w),
                Dock::Bottom => d.bottom_h = pane_h(vp.h - p.y, vp.h),
                _ => {}
            }
        }
        if picks > 0 {
            // The picker's overlay was pressed: the node it was showing is
            // the one picked.
            let mut s = self.session.state();
            let d = &mut s.devtools;
            d.pick = false;
            if let Some(k) = d.pick_hover.take() {
                if std::mem::take(&mut d.pick_keep_tab) {
                    d.selected = Some(k);
                    d.reveal = Some(k);
                } else {
                    d.select(k);
                }
            }
            drop(s);
            self.devtools_redraw_others();
        }
        if closed {
            // The user closed the panel's window — unless the panel had
            // already moved on (a placement button in that very window
            // takes it away, and the close that follows is our own).
            let mut s = self.session.state();
            if s.devtools.dock == Dock::Window {
                s.devtools.dock = Dock::Off;
                s.devtools.pick = false;
                s.devtools.note("dock: off (window closed)");
            }
        }
    }

    /// Every event handed to the host, into the stream — after the
    /// window stamp, so the row can say which window (decision 4).
    pub(crate) fn devtools_log(&mut self, out: &[UiEvent]) {
        if out.is_empty() {
            return;
        }
        let entries: Vec<Entry> = {
            let s = self.session.state();
            let d = &s.devtools;
            if !d.on || d.paused {
                return;
            }
            let frame = d.frames;
            out.iter()
                .map(|ev| Entry {
                    seq: 0,
                    frame,
                    kind: EntryKind::Event,
                    window: ev.window,
                    key: ev.key,
                    label: if ev.key == Key::ROOT {
                        Some("root".into())
                    } else {
                        self.label_of(ev.key).map(str::to_string)
                    },
                    origin: ev.origin,
                    payload: ev.payload.clone(),
                    lines: 0,
                })
                .collect()
        };
        let mut s = self.session.state();
        for e in entries {
            s.devtools.push(e);
        }
        let popped = s.devtools.dock == Dock::Window;
        drop(s);
        if popped && !self.dt_window {
            self.devtools_redraw_others();
        }
    }

    /// Asks the *other* window that shows the panel's effects to draw
    /// again (decision 7): the main window from the panel's own, the
    /// panel's own from anywhere else. Coalesced against the last command
    /// queued, since a hover storm is many events.
    fn devtools_redraw_others(&mut self) {
        let target = if self.dt_window {
            Some(WindowId::MAIN)
        } else {
            self.devtools_window_id()
        };
        let Some(id) = target else {
            return;
        };
        let cmds = &mut self.interaction.window_commands;
        if cmds.last() != Some(&WindowCommand::Redraw(id)) {
            cmds.push(WindowCommand::Redraw(id));
        }
    }

    /// The status block, read from the doors it comes from. `inspect` is
    /// the chord into the dock, handed in because the main window's build
    /// collects with the panel's state taken out of the session (and
    /// `devtools_key` would read the default).
    fn collect_facts(&self, inspect: Accel) -> Facts {
        let env = &self.env;
        let vp = self.viewport;
        let scale = self.scale;
        let name = |k: Key| match self.label_of(k) {
            _ if k == Key::ROOT => "root".to_string(),
            Some(l) => l.to_string(),
            None => format!("{:08x}", k.0 as u32),
        };
        let focus = self.focus;
        let focus_visible = self.focus_visible;
        let region = self.region();
        let inspect = inspect.display();
        let mods = self.modifiers();
        let windows: Vec<String> = self
            .windows()
            .iter()
            .map(|(id, name)| format!("{name}#{}", id.0))
            .collect();
        let source = match self.theme_source {
            ThemeSource::Derived => "derived",
            ThemeSource::DerivedWithAccent(_) => "derived + accent",
            ThemeSource::Pinned(_) => "pinned",
        };
        let opt = |s: Option<String>| s.unwrap_or_else(|| "—".into());
        let hex = |c: Color| format!("#{:06x}", c.to_hex() >> 8);
        let rows: Vec<(&'static str, String)> = vec![
            ("appearance", env.system.appearance.name().into()),
            ("motion", env.system.motion.name().into()),
            ("locale", opt(env.system.locale.map(|l| l.to_string()))),
            ("assistive", env.system.assistive.name().into()),
            (
                "theme",
                format!("{} · {source}", self.theme.appearance.name()),
            ),
            ("accent", hex(self.theme.accent)),
            (
                "menus",
                if self.native_menus { "native" } else { "drawn" }.into(),
            ),
            (
                "window",
                format!(
                    "#{}{}{}{}{} · {}",
                    env.window.id.0,
                    if env.window.custom_chrome {
                        " custom-chrome"
                    } else {
                        ""
                    },
                    if env.window.maximized {
                        " maximized"
                    } else {
                        ""
                    },
                    if env.window.fullscreen {
                        " fullscreen"
                    } else {
                        ""
                    },
                    if env.window.always_on_top {
                        " on-top"
                    } else {
                        ""
                    },
                    windows.join(" "),
                ),
            ),
            (
                "viewport",
                format!(
                    "{}{}×{} @{scale} · {}",
                    // The app's, then the window's when the dock has
                    // taken some of it.
                    if self.dt_area.w != vp.w || self.dt_area.h != vp.h {
                        format!("{}×{} of ", self.dt_area.w.round(), self.dt_area.h.round())
                    } else {
                        String::new()
                    },
                    vp.w.round(),
                    vp.h.round(),
                    opt(env.refresh_hz.map(|hz| format!("{hz:.0} Hz"))),
                ),
            ),
            (
                "keyboard",
                if env.focused {
                    "this window"
                } else {
                    "elsewhere"
                }
                .into(),
            ),
            (
                "focus",
                match focus.map(name) {
                    Some(l) if focus_visible => format!("{l} · ring"),
                    Some(l) => l,
                    None => "—".into(),
                },
            ),
            (
                "region",
                match region.map(name) {
                    Some(l) => format!("{l} · {inspect} leaves"),
                    None => format!("main · {inspect} enters the dock"),
                },
            ),
            ("modifiers", {
                let mut m = Vec::new();
                if mods.shift {
                    m.push("shift");
                }
                if mods.ctrl {
                    m.push("ctrl");
                }
                if mods.alt {
                    m.push("alt");
                }
                if mods.super_key {
                    m.push("super");
                }
                if m.is_empty() {
                    "—".into()
                } else {
                    m.join("+")
                }
            }),
            (
                "audio",
                format!("{} · {} live", env.audio.device.name(), env.audio.live),
            ),
            ("nodes", format!("{}", self.inspected.len())),
            ("fonts", {
                // Which installed faces the three generic families are on
                // this machine (backlog C32), so a wrong one says so on
                // screen.
                let [sans, serif, mono] = self.default_font_families();
                format!("{sans} · {serif} · {mono}")
            }),
        ];
        let mut origins: Vec<&OriginId> = self.tokens.keys().collect();
        origins.sort_by_key(|o| o.0);
        let mut tokens = Vec::new();
        for origin in origins {
            let table = &self.tokens[origin];
            for (i, (name, _)) in table.colors().iter().enumerate() {
                let (light, dark) = table.halves(i as u16, &self.theme);
                tokens.push(TokenFact {
                    origin: *origin,
                    name: name.clone(),
                    kind: crate::tokens::TokenKind::Color,
                    light,
                    dark,
                    resolved: table.resolve_color(i as u16, &self.theme),
                    length: 0.0,
                    recipe: table.recipe(i as u16),
                });
            }
            for (name, v) in table.lengths() {
                tokens.push(TokenFact {
                    origin: *origin,
                    name: name.clone(),
                    kind: crate::tokens::TokenKind::Length,
                    light: Color::TRANSPARENT,
                    dark: Color::TRANSPARENT,
                    resolved: Color::TRANSPARENT,
                    length: *v,
                    recipe: None,
                });
            }
        }
        Facts {
            title: self
                .window_title
                .clone()
                .unwrap_or_else(|| "kui".to_string()),
            rows,
            tokens,
            theme_source: self.app_theme_source(),
            app_appearance: self.app_theme_source().resolve(&env.system).appearance,
            viewport: vp,
            focus,
            focus_visible,
            region,
            hovered: self.interaction.hovered(),
            pressed: self.interaction.pressed_key(),
            cursor: self.interaction.cursor(),
        }
    }
}

/// A `Value` as one line of data: `{kind: click, n: 3}`.
pub fn fmt_value(v: &Value) -> String {
    let mut out = String::new();
    write_value(v, &mut out);
    out
}

fn write_value(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => {
            if f.fract() == 0.0 && f.abs() < 1e9 {
                out.push_str(&format!("{f:.0}"));
            } else {
                out.push_str(&format!("{f:.2}"));
            }
        }
        Value::Str(s) => {
            if s.chars().any(|c| c.is_whitespace() || c == ',' || c == '}') || s.is_empty() {
                out.push_str(&format!("{s:?}"));
            } else {
                out.push_str(s);
            }
        }
        Value::List(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(item, out);
            }
            out.push(']');
        }
        Value::Map(entries) => {
            out.push('{');
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(k);
                out.push_str(": ");
                write_value(v, out);
            }
            out.push('}');
        }
    }
}

/// How many lines `v` takes as an indented tree: a map's entries and a
/// list's items are one each, nested ones under theirs; a scalar is one
/// line, and a map or list is its entries alone (its own line is the
/// row's summary).
pub(crate) fn lines(v: &Value) -> u16 {
    fn count(v: &Value) -> usize {
        match v {
            Value::Map(entries) => entries.iter().map(|(_, v)| 1 + nested(v)).sum::<usize>(),
            Value::List(items) => items.iter().map(|v| 1 + nested(v)).sum::<usize>(),
            _ => 1,
        }
    }
    fn nested(v: &Value) -> usize {
        match v {
            Value::Map(_) | Value::List(_) => count(v),
            _ => 0,
        }
    }
    count(v).min(u16::MAX as usize) as u16
}

/// A side pane's width for a window `vw` wide: what the handle left,
/// within the pane's floor and what the app needs — the floor winning
/// when the window has room for neither.
fn pane_w(side_w: f32, vw: f32) -> f32 {
    side_w.min((vw - APP_MIN).max(SIDE_MIN_W)).max(SIDE_MIN_W)
}

fn pane_h(bottom_h: f32, vh: f32) -> f32 {
    bottom_h
        .min((vh - APP_MIN).max(BOTTOM_MIN_H))
        .max(BOTTOM_MIN_H)
}

/// Where a build is drawing the panel.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    /// The main window: the dock (or nothing, for `Window` and `Off`)
    /// plus the outlines and the picker.
    Main(Dock),
    /// The panel's own window: the panel is the whole tree.
    Window,
}
