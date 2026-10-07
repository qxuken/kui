//! Accessibility as data: the semantic tree of a frame, derived from what
//! nodes do and from the `role` and `label` props a view declares.
//!
//! A view rarely builds anything here. It sets `NodeSpec::role`,
//! `NodeSpec::label`, `description`, `live` and the value props; the core
//! derives an [`AccessTree`] of [`AccessNode`]s (roles from behaviour,
//! names from labels or text, plain boxes elided) that a driver reads with
//! [`crate::Core::access_tree`] and hands to the platform through
//! AccessKit. Requests from assistive technology come back as
//! [`crate::InputEvent::Access`] carrying an [`AccessRequest`] and resolve
//! inside the core: activating a button emits the same event a click
//! would. `Ui::announce` queues an [`Announcement`] for a one-off message
//! with no node behind it. Headless tests assert on the tree directly.
//!
//! ```rust
//! use kui_core::{AccessAction, Core, NodeSpec, Role, Size, TextStyle};
//!
//! let mut core = Core::new();
//! let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
//! ui.window_title("Demo");
//! // An `on_click` node is a button; `label` names it when it has no text.
//! ui.leaf_keyed("save", NodeSpec::row().size(24.0, 24.0).on_click("save").label("Save"));
//! ui.text("Ready", TextStyle::new(14.0));
//! ui.finish();
//!
//! let tree = core.access_tree();
//! assert_eq!(tree.root().map(|n| n.role), Some(Role::Window));
//! let button = tree.nodes.iter().find(|n| n.role == Role::Button).unwrap();
//! assert_eq!(button.name.as_deref(), Some("Save"));
//! assert!(button.supports(AccessAction::Click));
//! ```
//!
//! Text is the one place the tree goes below the node: an editor carries
//! its laid-out lines as [`AccessRun`]s and its caret and selection as
//! positions in them, which is what a screen reader needs to read by
//! character, word and line. An app that draws its own text in an
//! `on_key` sink gets the same by declaring `role="multilineTextInput"`
//! on the sink, `role="line"` on each line, and `caret` /
//! `selectionAnchor` byte offsets on the lines that hold them.

use std::sync::Arc;

use cosmic_text::Buffer;
use unicode_segmentation::UnicodeSegmentation;

use crate::display::{Clip, NO_CLIP};
use crate::edit::EditStore;
use crate::geom::{Rect, Size, Vec2};
use crate::key::Key;
use crate::scroll::ScrollStore;
use crate::spec::NodeSpec;
use crate::text::TextSystem;
use crate::tree::{NIL, NodeContent, OriginId, Tree};
use crate::value::{Handles, Value};
use crate::window::{WindowButton, WindowRole};

/// What a node is to assistive technology. Most of these a view declares
/// (`role` prop; `schema::ROLES` is that list, and the wire order); the
/// ones the core derives from a node's content and behaviour instead are
/// `schema::DERIVED_ONLY`, which says what derives each. Every variant is
/// on one list or the other — `schema`'s
/// `every_role_is_declarable_or_derived` fails when a new one is on
/// neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// Decorative: the node and its whole subtree leave the access tree.
    None,
    Button,
    Checkbox,
    Radio,
    Switch,
    Slider,
    Tab,
    TabList,
    Link,
    Heading,
    List,
    ListItem,
    Image,
    Dialog,
    Group,
    // -- Derived, and declarable on a custom editor ------------------------
    /// The root, named by the window title.
    Window,
    /// A `window="drag"` strip.
    TitleBar,
    /// A text node; its content is its name.
    StaticText,
    /// A single-line editor; the text is its value. Declared on an
    /// `on_key` sink that draws its own text, it makes that sink one.
    TextInput,
    /// A multiline editor (see [`Role::TextInput`]).
    MultilineTextInput,
    /// A container that scrolls.
    ScrollView,
    /// One line of a custom editor (a `role="textInput"` sink): the text
    /// nodes inside it, in order, are that line of the editor's value,
    /// and its `caret` / `selectionAnchor` are byte offsets into it. Not a
    /// node of its own.
    Line,
    // -- Appended later ----------------------------------------------------
    // At the tail, and in the order [`Role::ALL`] lists them, because the
    // tail is the only free position: `KUI_ROLE_*` is an `ALL` index plus
    // one and the Lua and Node wires carry the `ROLES` index, so a role
    // inserted anywhere else renumbers every role after it.
    /// A set of `radio`s: one Tab stop, arrows moving the checked one.
    RadioGroup,
    /// A menu: one Tab stop, arrows moving focus without activating.
    Menu,
    /// One item of a `menu`. A control, so it is focusable by its role and
    /// an unnamed one is reported.
    MenuItem,
    /// A cell grid (`crate::cells`): the screen of a terminal, its rows
    /// joined as the value. Derived from the node.
    Terminal,
}

impl Role {
    /// The camelCase spelling every binding uses.
    pub fn name(self) -> &'static str {
        match self {
            Role::None => "none",
            Role::Button => "button",
            Role::Checkbox => "checkbox",
            Role::Radio => "radio",
            Role::Switch => "switch",
            Role::Slider => "slider",
            Role::Tab => "tab",
            Role::TabList => "tabList",
            Role::Link => "link",
            Role::Heading => "heading",
            Role::List => "list",
            Role::ListItem => "listItem",
            Role::Image => "image",
            Role::Dialog => "dialog",
            Role::Group => "group",
            Role::RadioGroup => "radioGroup",
            Role::Menu => "menu",
            Role::MenuItem => "menuItem",
            Role::Terminal => "terminal",
            Role::Window => "window",
            Role::TitleBar => "titleBar",
            Role::StaticText => "staticText",
            Role::TextInput => "textInput",
            Role::MultilineTextInput => "multilineTextInput",
            Role::ScrollView => "scrollView",
            Role::Line => "line",
        }
    }

    pub fn parse(name: &str) -> Option<Role> {
        Role::ALL.iter().copied().find(|r| r.name() == name)
    }

    pub const ALL: [Role; 26] = [
        Role::None,
        Role::Button,
        Role::Checkbox,
        Role::Radio,
        Role::Switch,
        Role::Slider,
        Role::Tab,
        Role::TabList,
        Role::Link,
        Role::Heading,
        Role::List,
        Role::ListItem,
        Role::Image,
        Role::Dialog,
        Role::Group,
        Role::Window,
        Role::TitleBar,
        Role::StaticText,
        Role::TextInput,
        Role::MultilineTextInput,
        Role::ScrollView,
        Role::Line,
        Role::RadioGroup,
        Role::Menu,
        Role::MenuItem,
        Role::Terminal,
    ];

    /// A control needs a name; one without is reported as a warning.
    pub fn is_control(self) -> bool {
        matches!(
            self,
            Role::Button
                | Role::Checkbox
                | Role::Radio
                | Role::Switch
                | Role::Slider
                | Role::Tab
                | Role::MenuItem
                | Role::Link
                | Role::TextInput
                | Role::MultilineTextInput
        )
    }

    /// An editor role: carries text runs, a caret and a selection.
    pub fn is_editor(self) -> bool {
        matches!(self, Role::TextInput | Role::MultilineTextInput)
    }

    /// Roles whose name, absent a `label`, is the text inside them (ARIA's
    /// name-from-content), and whose children are presentational: the
    /// subtree is read as the control, not as separate items.
    pub(crate) fn presentational(self) -> bool {
        matches!(
            self,
            Role::Button
                | Role::Checkbox
                | Role::Radio
                | Role::Switch
                | Role::Slider
                | Role::Tab
                | Role::MenuItem
                | Role::Link
                | Role::Heading
                | Role::Image
        )
    }
}

/// How urgently a reader should read a change it was not asked to read:
/// ARIA's `aria-live`, AccessKit's `Live`. Declared on the node holding
/// the text (`live` prop) and, for a one-off with no node behind it, the
/// politeness of an [`Announcement`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Live {
    /// Not a live region: changes are read only when asked for.
    #[default]
    Off,
    /// Read at the next pause, without interrupting.
    Polite,
    /// Read now, interrupting whatever is being said.
    Assertive,
}

impl Live {
    /// Every politeness, in wire order: `schema::LIVE` is `ALL` by `name`.
    pub const ALL: &'static [Live] = &[Live::Off, Live::Polite, Live::Assertive];

    /// The camelCase spelling every binding uses.
    pub fn name(self) -> &'static str {
        match self {
            Live::Off => "off",
            Live::Polite => "polite",
            Live::Assertive => "assertive",
        }
    }

    /// The variant `schema::LIVE` index `i` names; `Off` for an index
    /// this build lacks.
    pub fn from_index(i: usize) -> Live {
        Self::ALL.get(i).copied().unwrap_or_default()
    }
}

/// One thing to say once, with no node behind it: "Saved", "3 results".
/// Queued by `Core::announce` and drained by `Core::take_announcements`,
/// the way window commands, audio commands and warnings are: an
/// announcement is an event on a timeline, and the frame's tree has no
/// place to keep one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Announcement {
    pub text: String,
    /// Never [`Live::Off`]: `Core::announce` drops those rather than
    /// queueing something no reader would say.
    pub live: Live,
}

/// How a container arranges its items, for the platform to announce
/// (`AXOrientation`, UIA's `Orientation`). Derived from the container's
/// `dir` and never declared: the layout is what arranges the items, so a
/// row that says it is a column would be a fact with two owners. It is an
/// announcement and not a gate — the arrows move both ways whatever this
/// says — so a container whose visual arrangement does not match its `dir`
/// costs a less precise announcement rather than a dead keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

impl Orientation {
    pub fn name(self) -> &'static str {
        match self {
            Orientation::Horizontal => "horizontal",
            Orientation::Vertical => "vertical",
        }
    }

    pub fn parse(name: &str) -> Option<Orientation> {
        Orientation::ALL.iter().copied().find(|o| o.name() == name)
    }

    pub const ALL: [Orientation; 2] = [Orientation::Horizontal, Orientation::Vertical];
}

/// What assistive technology can ask of a node. Each node advertises the
/// subset it supports ([`AccessNode::actions`]), and a request for one
/// arrives as [`crate::InputEvent::Access`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccessAction {
    /// Activate: the node's `on_click` payload is emitted (a window button
    /// issues its command; an editor or key sink takes focus). On a node
    /// behind the frame's modal it is the press outside: a `dismiss` on
    /// the modal, nothing on the node.
    Click,
    /// Give the node keyboard focus — any focusable node (an editor, a key
    /// sink, a control, a `focusable` box); it shows, as after Tab.
    Focus,
    Blur,
    /// Replace an editor's text (`AccessRequest::value`); a `changed` event
    /// follows when it differs. On a custom editor it arrives as
    /// `{kind="access", action="setValue", text, tag}`.
    SetValue,
    /// Nudge a slider. The core cannot know what a step means, so these
    /// reach the app as `{kind="access", action, tag}` on the node.
    Increment,
    Decrement,
    /// Scroll the nearest scrolling ancestor so the node is visible.
    ScrollIntoView,
    /// Scroll a scroll view by most of a page.
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    /// Move an editor's caret and selection (`AccessRequest::anchor` /
    /// `focus`). On a custom editor it arrives as `{kind="access",
    /// action="setTextSelection", anchor={line, offset}, focus={line,
    /// offset}, tag}`.
    SetTextSelection,
    /// Type over an editor's selection (`AccessRequest::value`). On a
    /// custom editor: `{kind="access", action="replaceSelectedText",
    /// text, tag}`.
    ReplaceSelectedText,
}

impl AccessAction {
    pub const ALL: [AccessAction; 13] = [
        AccessAction::Click,
        AccessAction::Focus,
        AccessAction::Blur,
        AccessAction::SetValue,
        AccessAction::Increment,
        AccessAction::Decrement,
        AccessAction::ScrollIntoView,
        AccessAction::ScrollUp,
        AccessAction::ScrollDown,
        AccessAction::ScrollLeft,
        AccessAction::ScrollRight,
        AccessAction::SetTextSelection,
        AccessAction::ReplaceSelectedText,
    ];

    /// The action's bit in [`AccessNode::actions`].
    pub fn bit(self) -> u32 {
        1 << (self as u32)
    }

    pub fn name(self) -> &'static str {
        match self {
            AccessAction::Click => "click",
            AccessAction::Focus => "focus",
            AccessAction::Blur => "blur",
            AccessAction::SetValue => "setValue",
            AccessAction::Increment => "increment",
            AccessAction::Decrement => "decrement",
            AccessAction::ScrollIntoView => "scrollIntoView",
            AccessAction::ScrollUp => "scrollUp",
            AccessAction::ScrollDown => "scrollDown",
            AccessAction::ScrollLeft => "scrollLeft",
            AccessAction::ScrollRight => "scrollRight",
            AccessAction::SetTextSelection => "setTextSelection",
            AccessAction::ReplaceSelectedText => "replaceSelectedText",
        }
    }

    pub fn parse(name: &str) -> Option<AccessAction> {
        AccessAction::ALL.iter().copied().find(|a| a.name() == name)
    }
}

/// A position in an editor's text: a run and a character index into it
/// (`character == char count` is the end of the run).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextPos {
    pub run: Key,
    pub character: usize,
}

impl TextPos {
    /// `{run, character}`, the run spelled by `h`.
    pub fn to_value(self, h: Handles) -> Value {
        Value::map([
            ("run", (h.key)(self.run)),
            ("character", Value::Int(self.character as i64)),
        ])
    }
}

/// A request from assistive technology, delivered as
/// [`crate::InputEvent::Access`].
#[derive(Clone, Debug, PartialEq)]
pub struct AccessRequest {
    pub key: Key,
    pub action: AccessAction,
    /// The new text for [`AccessAction::SetValue`] /
    /// [`AccessAction::ReplaceSelectedText`].
    pub value: Option<String>,
    /// The selection for [`AccessAction::SetTextSelection`]: `anchor` is
    /// the end that stays put, `focus` the caret.
    pub anchor: Option<TextPos>,
    pub focus: Option<TextPos>,
}

impl AccessRequest {
    pub fn new(key: Key, action: AccessAction) -> Self {
        AccessRequest {
            key,
            action,
            value: None,
            anchor: None,
            focus: None,
        }
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn with_selection(mut self, anchor: TextPos, focus: TextPos) -> Self {
        self.anchor = Some(anchor);
        self.focus = Some(focus);
        self
    }
}

/// A scroll view's offsets and range (logical px).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollState {
    pub x: f32,
    pub y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl ScrollState {
    /// `{x, y, max_x, max_y}`.
    pub fn to_value(self) -> Value {
        Value::map([
            ("x", Value::float(self.x)),
            ("y", Value::float(self.y)),
            ("max_x", Value::float(self.max_x)),
            ("max_y", Value::float(self.max_y)),
        ])
    }
}

/// One visual line (or a piece of one) of an editor's text, with what a
/// screen reader needs to read it by character and word and to place a
/// caret: every character's byte length, x position and width. A line
/// that continues into another ends with its `"\n"`, counted as a
/// character of zero width. Runs longer than [`RUN_CHARS`] characters are
/// split, so indices fit the platform's byte-sized ones.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessRun {
    /// The run's own id (derived from the editor's key).
    pub key: Key,
    /// The line it belongs to: a buffer line for a built-in editor, the
    /// ordinal of the `role="line"` node for a custom one.
    pub line: usize,
    /// Byte range of the run inside the line's text (the `"\n"` excluded).
    pub start: usize,
    pub end: usize,
    pub text: String,
    /// Logical px, viewport coordinates, cut to the clip the text is
    /// drawn under as a node's `rect` is; `char_positions` still place
    /// every character where it is drawn.
    pub rect: Rect,
    pub char_lengths: Vec<u8>,
    /// Each character's x relative to `rect.x`, and its width.
    pub char_positions: Vec<f32>,
    pub char_widths: Vec<f32>,
    /// Character indices where words start.
    pub word_starts: Vec<u8>,
    pub rtl: bool,
}

impl AccessRun {
    /// The run as plain data, its key spelled by `h`.
    pub fn to_value(&self, h: Handles) -> Value {
        let bytes = |v: &[u8]| Value::list(v.iter().map(|b| Value::Int(*b as i64)));
        Value::map([
            ("key", (h.key)(self.key)),
            ("line", Value::Int(self.line as i64)),
            ("start", Value::Int(self.start as i64)),
            ("end", Value::Int(self.end as i64)),
            ("text", Value::Str(self.text.clone())),
            ("rect", self.rect.to_value()),
            ("char_lengths", bytes(&self.char_lengths)),
            ("char_positions", Value::floats(&self.char_positions)),
            ("char_widths", Value::floats(&self.char_widths)),
            ("word_starts", bytes(&self.word_starts)),
            ("rtl", Value::Bool(self.rtl)),
        ])
    }
}

/// Longest run, in characters (the platform indexes them in a byte).
pub const RUN_CHARS: usize = 200;

/// One semantic node of a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessNode {
    pub key: Key,
    /// The nearest semantic ancestor; None for the root.
    pub parent: Option<Key>,
    pub origin: OriginId,
    pub role: Role,
    /// The accessible name: `label`, else the node's own text, else (for
    /// presentational roles) the text inside it, else the window title
    /// for the root.
    pub name: Option<String>,
    /// `description` on the spec — the `description` prop, or a `tooltip`.
    pub description: Option<String>,
    /// Final laid-out rect, logical px, viewport coordinates, cut to the
    /// clip the node is drawn under — the one its hit region carries
    /// — so a reader's hover finds only what a pointer
    /// could. A node wholly clipped away is a zero-size rect on the clip's
    /// edge nearest it: still in the tree, still actionable, never hit.
    pub rect: Rect,
    /// The node's string value, which the platform has exactly one slot
    /// for: an editor's committed text (a custom editor's: the lines it
    /// draws, joined by `"\n"`), or a slider's declared `value_text`.
    /// A slider that names its reading has *only* that reading — the
    /// string wins over the number wherever both could be said, which is
    /// what `aria-valuetext` means and what `accesskit_macos` does with
    /// `AXValue`. `min` / `max` are unaffected,
    /// and so are the increment actions.
    pub value: Option<String>,
    /// An editor's caret, a byte offset into `value`.
    pub caret: Option<usize>,
    /// An editor's non-empty selection as byte offsets into `value`.
    pub selection: Option<(usize, usize)>,
    /// An editor's laid-out text, run by run (see [`AccessRun`]).
    pub runs: Vec<AccessRun>,
    /// The caret (`focus`) and the other end of the selection (`anchor`,
    /// equal to `focus` without one) as run positions.
    pub anchor: Option<TextPos>,
    pub focus: Option<TextPos>,
    /// `checked` for checkbox / radio / switch roles.
    pub checked: Option<bool>,
    /// A checkbox that is neither on nor off:
    /// reported as mixed whatever `checked` says.
    pub mixed: bool,
    /// The current one of a set: every `tab` carries it, a `listItem` or a
    /// `link` only where the view set it (an ordinary list is not a
    /// selection, and "not selected" on every row of one is noise).
    /// None = the node has no such state.
    pub selected: Option<bool>,
    /// A disclosure's state, exactly as declared. None = it does not
    /// expand, and a reader says nothing about it.
    pub expanded: Option<bool>,
    /// "3 of 7": this node's zero-based ordinal among the items of the
    /// `list` / `tabList` holding it, with `set_size` on that container.
    /// Derived, never declared — the core counts the semantic children it
    /// already has (see `set_size`).
    pub pos_in_set: Option<usize>,
    /// How a composite container arranges its items, from its `dir` (see
    /// [`Orientation`]). None = the node is not one of the four composite
    /// containers, and says nothing about arrangement.
    pub orientation: Option<Orientation>,
    /// On a `list` / `tabList`: how many items it holds. AccessKit puts
    /// the count on the container and the ordinal on the item, unlike
    /// ARIA's `aria-setsize` on every item; this follows AccessKit.
    pub set_size: Option<usize>,
    /// `valueNow` / `valueMin` / `valueMax` for a slider. What the
    /// position *reads as* is `valueText`, which lands in `value` above
    /// because the platform has one string slot for both.
    pub number: Option<f32>,
    pub min: Option<f32>,
    pub max: Option<f32>,
    /// A slider's `valueStep`, where it declared one.
    pub step: Option<f32>,
    /// Holds keyboard focus (`Core::focus`).
    pub focused: bool,
    /// Declared `disabled`: inert, and not in the Tab ring.
    pub disabled: bool,
    /// The frame's modal surface (`aria-modal`): the Tab ring and every
    /// pointer are confined to it, and everything else is inert. Only the
    /// modal in effect carries it — the last one declared — so a confirm
    /// inside a dialog leaves the dialog an ordinary node.
    pub modal: bool,
    pub scroll: Option<ScrollState>,
    /// Bitset of [`AccessAction::bit`].
    pub actions: u32,
    /// Declared `live`: when the text inside this node changes, a reader
    /// reads the change without being asked. Carried exactly where the
    /// view declared it — the platform consumer inherits it down the
    /// subtree, and duplicating that here would be a second copy of a
    /// rule kui does not own.
    pub live: Live,
}

impl AccessNode {
    /// The node as plain data, every field under its snake_case name and
    /// every key spelled by `h`. The slider's numbers are `value_now`,
    /// `value_min`, `value_max` — the rows that set them, not the
    /// fields that hold them; `actions` is the list of
    /// action names, `live` and `role` and `orientation` their schema
    /// names.
    pub fn to_value(&self, h: Handles) -> Value {
        Value::map([
            ("key", (h.key)(self.key)),
            ("parent", h.opt_key(self.parent)),
            ("origin", Value::Int(self.origin.0 as i64)),
            ("role", Value::str(self.role.name())),
            ("name", Value::opt_str(&self.name)),
            ("description", Value::opt_str(&self.description)),
            ("rect", self.rect.to_value()),
            ("value", Value::opt_str(&self.value)),
            ("caret", Value::opt_usize(self.caret)),
            (
                "selection",
                Value::opt(self.selection, |(a, b)| {
                    Value::list([Value::Int(a as i64), Value::Int(b as i64)])
                }),
            ),
            ("anchor", Value::opt(self.anchor, |p| p.to_value(h))),
            ("focus", Value::opt(self.focus, |p| p.to_value(h))),
            ("runs", Value::list(self.runs.iter().map(|r| r.to_value(h)))),
            ("checked", Value::opt_bool(self.checked)),
            ("mixed", Value::Bool(self.mixed)),
            ("selected", Value::opt_bool(self.selected)),
            ("expanded", Value::opt_bool(self.expanded)),
            ("pos_in_set", Value::opt_usize(self.pos_in_set)),
            ("set_size", Value::opt_usize(self.set_size)),
            (
                "orientation",
                Value::opt(self.orientation, |o| Value::str(o.name())),
            ),
            ("live", Value::str(self.live.name())),
            ("value_now", Value::opt_float(self.number)),
            ("value_min", Value::opt_float(self.min)),
            ("value_max", Value::opt_float(self.max)),
            ("value_step", Value::opt_float(self.step)),
            ("focused", Value::Bool(self.focused)),
            ("disabled", Value::Bool(self.disabled)),
            ("modal", Value::Bool(self.modal)),
            ("scroll", Value::opt(self.scroll, ScrollState::to_value)),
            (
                "actions",
                Value::list(self.action_list().into_iter().map(|a| Value::str(a.name()))),
            ),
        ])
    }

    pub fn supports(&self, action: AccessAction) -> bool {
        self.actions & action.bit() != 0
    }

    /// The actions this node advertises.
    pub fn action_list(&self) -> Vec<AccessAction> {
        AccessAction::ALL
            .iter()
            .copied()
            .filter(|a| self.supports(*a))
            .collect()
    }

    /// Turns a run position back into the line it is on and a byte offset
    /// into that line's text (the `"\n"` counting as the line's end).
    pub fn line_offset(&self, pos: TextPos) -> Option<(usize, usize)> {
        let run = self.runs.iter().find(|r| r.key == pos.run)?;
        let within = run
            .text
            .char_indices()
            .nth(pos.character)
            .map_or(run.text.len(), |(b, _)| b);
        Some((run.line, run.start + within.min(run.end - run.start)))
    }

    /// The run position of a byte offset into line `line`'s text: the
    /// run holding it, or the line's last run for its end. An offset inside
    /// a character is the character's start: a custom editor's `caret` is
    /// the app's number, and slicing there panicked, which emptied the
    /// whole tree for every frame it stood.
    pub fn text_pos(&self, line: usize, offset: usize) -> Option<TextPos> {
        let chars_before =
            |r: &AccessRun, at: usize| r.text[..r.text.floor_char_boundary(at)].chars().count();
        let mut last = None;
        for r in self.runs.iter().filter(|r| r.line == line) {
            if offset >= r.start && offset < r.end {
                return Some(TextPos {
                    run: r.key,
                    character: chars_before(r, offset - r.start),
                });
            }
            last = Some(r);
        }
        let r = last?;
        Some(TextPos {
            run: r.key,
            character: chars_before(r, (offset.max(r.start) - r.start).min(r.end - r.start)),
        })
    }
}

/// The semantic nodes of a finished frame, in tree order (a parent always
/// precedes its descendants; the root comes first).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccessTree {
    pub nodes: Vec<AccessNode>,
    /// The node holding keyboard focus, if any.
    pub focus: Option<Key>,
    /// A hash of everything above: a driver that sent the tree once sends
    /// it again only when this changes.
    pub hash: u64,
}

impl AccessTree {
    /// `{nodes, focus, hash}`, every node by [`AccessNode::to_value`],
    /// the focus key spelled by `h` and the hash as sixteen hex digits.
    pub fn to_value(&self, h: Handles) -> Value {
        Value::map([
            (
                "nodes",
                Value::list(self.nodes.iter().map(|n| n.to_value(h))),
            ),
            ("focus", h.opt_key(self.focus)),
            ("hash", Value::Str(format!("{:016x}", self.hash))),
        ])
    }

    pub fn get(&self, key: Key) -> Option<&AccessNode> {
        self.nodes.iter().find(|n| n.key == key)
    }

    /// The nodes whose `parent` is `key`, in order.
    pub fn children(&self, key: Key) -> impl Iterator<Item = &AccessNode> {
        self.nodes.iter().filter(move |n| n.parent == Some(key))
    }

    /// The root (the window), when a frame has been built.
    pub fn root(&self) -> Option<&AccessNode> {
        self.nodes.first()
    }
}

/// What the tree walk decided for one node.
pub(crate) struct Semantic {
    pub role: Role,
    pub name: Option<String>,
    /// Descendants are read as part of this node, not as their own nodes.
    pub presentational: bool,
}

/// The role a node's spec and content imply, before elision. None = plain
/// structure (elided; its descendants still appear).
pub(crate) fn derived_role(tree: &Tree, i: usize) -> Option<Role> {
    let spec = &tree.specs[i];
    if let Some(r) = spec.access().role {
        // A line is structure of the editor around it; on its own it is
        // nothing.
        return (r != Role::Line).then_some(r);
    }
    if i == 0 {
        return Some(Role::Window);
    }
    match tree.content[i] {
        NodeContent::Text(_) => return Some(Role::StaticText),
        NodeContent::Edit(_) => return Some(Role::TextInput),
        NodeContent::Image(..) => return Some(Role::Image),
        // A stroke or a fill is decoration on its own and elided like
        // plain structure; one that takes input is hit by its shape, so
        // the derivation below reaches it as it reaches a box —
        // a clickable wedge is a button, a draggable connector a control.
        NodeContent::Line(_) | NodeContent::Polygon(_) | NodeContent::Path(_) => {}
        NodeContent::Cells(_) => return Some(Role::Terminal),
        // A fragment is paint. On its own it is decoration and is elided
        // like plain structure, but unlike a line it does take input, so
        // the derivation below still reaches it: a fragment with an
        // `on_click` is a button, and one that means something says so
        // with its own `role` and `label`.
        NodeContent::Fragment(_) => {}
        NodeContent::Container => {}
    }
    match spec.window {
        Some(WindowRole::Drag) => return Some(Role::TitleBar),
        Some(WindowRole::Button(_)) => return Some(Role::Button),
        None => {}
    }
    if spec.events().modal.is_some() {
        // A modal surface is a dialog to assistive technology; anything
        // else it might be, the view says with an explicit role.
        return Some(Role::Dialog);
    }
    if spec.events().on_click.is_some() {
        return Some(Role::Button);
    }
    if spec.layout.scroll_x || spec.layout.scroll_y {
        return Some(Role::ScrollView);
    }
    if spec.events().on_key.is_some() || spec.focusable {
        // A key sink or a focusable box is at least somewhere focus can
        // land, so a reader has to be able to see it there.
        return Some(Role::Group);
    }
    if spec.access().live != Live::Off {
        // A live region that was elided would carry its liveness
        // nowhere: its text would inherit the window's instead, and the
        // change would go unread.
        return Some(Role::Group);
    }
    None
}

/// Whether node `i` is an editor the app draws itself: an editor role on
/// something other than a built-in edit node.
pub(crate) fn is_custom_editor(tree: &Tree, i: usize) -> bool {
    tree.specs[i].access().role.is_some_and(Role::is_editor)
        && !matches!(tree.content[i], NodeContent::Edit(_))
}

/// Whether node `i` can hold keyboard focus: an editor, a key sink, a
/// control role, a derived button (`on_click`), or a node declaring
/// `focusable` — never a disabled node, decoration or window chrome. The
/// Tab ring is these nodes in tree order; a `role="none"` subtree is
/// skipped by the walk, not here.
pub(crate) fn focusable(tree: &Tree, i: usize) -> bool {
    let spec = &tree.specs[i];
    if spec.disabled || spec.access().role == Some(Role::None) || spec.window.is_some() {
        return false;
    }
    if spec.focusable
        || spec.events().on_key.is_some()
        || matches!(tree.content[i], NodeContent::Edit(_))
    {
        return true;
    }
    match spec.access().role {
        Some(r) => r.is_control(),
        None => spec.events().on_click.is_some(),
    }
}

/// The role, name and presentation of node `i`, or None when it is plain
/// structure. Shared by the tree build and the diagnostics check so both
/// agree on what "unnamed" means.
pub(crate) fn semantic(
    tree: &Tree,
    text: &TextSystem,
    edit: &EditStore,
    title: Option<&str>,
    i: usize,
) -> Option<Semantic> {
    let mut role = derived_role(tree, i)?;
    let spec = &tree.specs[i];
    if role == Role::TextInput
        && let NodeContent::Edit(key) = tree.content[i]
        && edit.is_multiline(key)
    {
        role = Role::MultilineTextInput;
    }
    // Window buttons read as one control; the drag strip keeps its
    // children (the buttons sit inside it). A custom editor's lines are
    // its text, not children.
    //
    // A live region joins them: it reads as **one message**, named by the
    // text inside it, and that is what changes when the message does. The
    // alternative — the region carrying only liveness and each platform
    // announcing the changed descendant — is what was first built,
    // and macOS does not deliver it: `accesskit_macos` derives a live
    // node's announcement from `NodeWrapper::label()`, which for a
    // `Role::Label` reads the node's *value*, so a live static text
    // announces nothing at all. One named node is also one announcement
    // on all three platforms rather than one per live descendant.
    let presentational = role.presentational()
        || spec.access().live != Live::Off
        || matches!(spec.window, Some(WindowRole::Button(_)))
        || is_custom_editor(tree, i);
    let name = match (&spec.access().label, spec.window) {
        (Some(label), _) => Some(label.to_string()),
        (None, Some(WindowRole::Button(b))) => Some(
            match b {
                WindowButton::Close => "Close",
                WindowButton::Minimize => "Minimize",
                WindowButton::Maximize => "Maximize",
            }
            .to_string(),
        ),
        (None, _) => match role {
            Role::StaticText => match tree.content[i] {
                NodeContent::Text(id) => Some(text.content(id).to_string()),
                _ => None,
            },
            // The window carries the title; a drawn titlebar naming
            // itself the same thing would have a screen reader read it
            // twice (it keeps its children, so its own text is read).
            Role::Window => title.map(str::to_string),
            _ if role.presentational() || spec.access().live != Live::Off => {
                content_name(tree, text, i)
            }
            _ => None,
        },
    };
    Some(Semantic {
        role,
        name,
        presentational,
    })
}

/// Whether a live region has anything a reader could ever say: a `label`
/// of its own, or text somewhere inside it (which is where the string
/// actually comes from — liveness inherits, and the changed descendant is
/// what gets announced). Shared with the diagnostics so both agree on
/// what a silent live region is.
pub(crate) fn live_region_speaks(tree: &Tree, text: &TextSystem, i: usize) -> bool {
    tree.specs[i].access().label.is_some() || content_name(tree, text, i).is_some()
}

/// The text inside node `i`, in order, joined by spaces — ARIA's
/// name-from-content. None when there is none.
///
/// A subtree under `role="none"` is not content, as it is not for a
/// custom editor's lines ([`lines_under`]): it is hidden from assistive
/// technology, and what it draws is not what the control is called. The
/// `tooltip` prop's hint is one (`widgets::hover_hint`) — built only while
/// the pointer is over the node, it made a hovered button's name its label
/// and its hint both.
fn content_name(tree: &Tree, text: &TextSystem, i: usize) -> Option<String> {
    let end = tree.subtree_end(i);
    let mut out = String::new();
    let mut j = i;
    while j < end {
        if j > i && tree.specs[j].access().role == Some(Role::None) {
            j = tree.subtree_end(j);
            continue;
        }
        let at = j;
        j += 1;
        if let NodeContent::Text(id) = tree.content[at] {
            let s = text.content(id).trim();
            if s.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(s);
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Everything the build reads besides the tree.
pub(crate) struct Sources<'a> {
    pub text: &'a TextSystem,
    pub cells: &'a crate::cells::CellStore,
    pub edit: &'a EditStore,
    pub scroll: &'a ScrollStore,
    pub title: Option<&'a str>,
    /// The core's one keyboard focus (see `Core::focus`).
    pub focus: Option<Key>,
    /// The frame's modal in effect (see `Core::modal`).
    pub modal: Option<Key>,
    pub viewport: Size,
    pub scale: f32,
    /// The clip each node was emitted under, by tree index: its
    /// ancestors' only, and the one its hit region carries, so a float
    /// that escapes has none and a `clip` float has its parent's.
    /// Empty when the frame clipped nothing.
    pub clips: &'a [Clip],
}

/// The clip node `i` was emitted under (see [`Sources::clips`]).
fn clip_of(src: &Sources<'_>, i: usize) -> Rect {
    src.clips.get(i).map_or(NO_CLIP, |c| c.rect)
}

/// `rect` cut to `clip`, so assistive technology finds and highlights
/// only what is drawn. A rect wholly outside becomes a
/// zero-size one on the clip's edge nearest it: the node keeps its place
/// in reading order and its actions (a reader's "scroll into view" goes by
/// key), but a point never lands in it. The clip is a rect even where the
/// clipper's corners are round, as the hit test's is.
fn clipped(rect: Rect, clip: Rect) -> Rect {
    let x0 = rect.x.max(clip.x);
    let y0 = rect.y.max(clip.y);
    let x1 = (rect.x + rect.w).min(clip.x + clip.w);
    let y1 = (rect.y + rect.h).min(clip.y + clip.h);
    // Gone along an axis when nothing of it is left, which is emission's
    // cull: past the edge, or on it with no extent inside. An axis the
    // rect never had extent on (a caret-wide run) is not gone for that.
    let gone = |lo: f32, hi: f32, extent: f32| hi < lo || (hi == lo && extent > 0.0);
    if gone(x0, x1, rect.w) || gone(y0, y1, rect.h) {
        // `max` then `min` rather than `clamp`, which panics on a clip
        // whose edges cross.
        let x = rect.x.max(clip.x).min(clip.x + clip.w);
        let y = rect.y.max(clip.y).min(clip.y + clip.h);
        return Rect::new(x, y, 0.0, 0.0);
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// Node `i`'s access rect: the root's viewport, everyone else's box cut
/// to the clip it was emitted under.
fn node_rect(tree: &Tree, src: &Sources<'_>, i: usize) -> Rect {
    if i == 0 {
        Rect::new(0.0, 0.0, src.viewport.w, src.viewport.h)
    } else {
        clipped(
            Rect::from_pos_size(tree.pos[i], tree.size[i]),
            clip_of(src, i),
        )
    }
}

/// What an editor's runs are cut to: the node's clip and, for a field
/// that does not fold to its width, its content box across, as emission
/// cuts its glyphs — a scrolled field's text past its edge is not
/// drawn, so a reader should not find it there either.
fn edit_run_clip(tree: &Tree, src: &Sources<'_>, i: usize, edit_key: Key) -> Rect {
    let clip = clip_of(src, i);
    if src.edit.folds(edit_key) {
        return clip;
    }
    let pad = tree.specs[i].layout.padding;
    let across = Rect::new(
        tree.pos[i].x + pad.l,
        clip.y,
        (tree.size[i].w - pad.x()).max(0.0),
        clip.h,
    );
    clip.intersect(&across)
}

/// Cuts runs to `clip` the way [`clipped`] cuts a node, keeping each
/// character where it is: `char_positions` are relative to the run's x,
/// so they move by what the x did.
fn clip_runs(runs: &mut [AccessRun], clip: Rect) {
    if clip == NO_CLIP {
        return;
    }
    for run in runs {
        let cut = clipped(run.rect, clip);
        let dx = run.rect.x - cut.x;
        if dx != 0.0 {
            for p in &mut run.char_positions {
                *p += dx;
            }
        }
        run.rect = cut;
    }
}

/// A hash of every input [`build`] reads, taken by the same walk with
/// nothing built. `None` means "cannot answer, rebuild" — see the custom
/// editor note below.
///
/// **The invariant this rests on**: everything `build` reads must be mixed
/// in here. Miss one and a frame that changed only that thing serves the
/// previous frame's tree, which is a bug with no symptom in the core, no
/// failing test, and a wrong reading on somebody's screen. Two things hold
/// it. The walk is deliberately the *same* walk — the same skip rules
/// calling the same [`semantic`], [`focusable`] and
/// [`composite::orientation`](crate::composite::orientation) — so what can
/// drift is only the fields `build` reads directly out of the spec and the
/// sources; and `runtime::dispatch`'s tests mutate each of those in turn
/// and assert the tree moved.
///
/// It is worth taking because the walk is the cheap quarter of `build`:
/// **105 µs against 480 µs** over a 10,000-node frame on an M3 Pro, because
/// three quarters of that function is constructing `AccessNode`s and
/// pushing them, which is exactly what a cache hit skips.
///
/// A custom editor (`is_custom_editor`) answers `None` rather than a hash:
/// `custom_editor` fills a node from the `line` children's own text and
/// runs, and there is no reading of those inputs that does not amount to
/// building the node. Such a view is one whose text is changing anyway, so
/// it is the case a cache would miss on regardless.
pub(crate) fn inputs_hash(tree: &Tree, src: &Sources<'_>) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let mut h = rustc_hash::FxHasher::default();
    let f = |h: &mut rustc_hash::FxHasher, v: f32| v.to_bits().hash(h);

    tree.len().hash(&mut h);
    src.focus.map(|k| k.0).hash(&mut h);
    src.modal.map(|k| k.0).hash(&mut h);
    src.title.hash(&mut h);
    f(&mut h, src.viewport.w);
    f(&mut h, src.viewport.h);
    f(&mut h, src.scale);

    let mut skip_until = 0usize;
    let mut i = 0usize;
    while i < tree.len() {
        if i < skip_until {
            i += 1;
            continue;
        }
        let Some(sem) = semantic(tree, src.text, src.edit, src.title, i) else {
            i += 1;
            continue;
        };
        if sem.role == Role::None {
            skip_until = tree.subtree_end(i);
            i += 1;
            continue;
        }
        if is_custom_editor(tree, i) {
            return None;
        }
        let key = tree.keys[i];
        let spec = &tree.specs[i];
        let ax = spec.access();

        // Identity and place in the tree.
        i.hash(&mut h);
        key.0.hash(&mut h);
        tree.parent[i].hash(&mut h);
        tree.origins[i].0.hash(&mut h);

        // What `semantic` decided, and the node's own strings.
        sem.role.hash(&mut h);
        sem.name.hash(&mut h);
        sem.presentational.hash(&mut h);
        ax.description.as_deref().hash(&mut h);

        // The rect, which is the root's viewport and everyone else's box
        // cut to its clip.
        let rect = node_rect(tree, src, i);
        for v in [rect.x, rect.y, rect.w, rect.h] {
            f(&mut h, v);
        }

        // State the node reports, and everything the action bits read.
        (src.focus == Some(key)).hash(&mut h);
        (src.modal == Some(key)).hash(&mut h);
        spec.disabled.hash(&mut h);
        ax.live.hash(&mut h);
        spec.window.hash(&mut h);
        spec.events().on_click.is_some().hash(&mut h);
        focusable(tree, i).hash(&mut h);
        crate::composite::orientation(tree, i).hash(&mut h);
        ax.expanded.hash(&mut h);
        ax.checked.hash(&mut h);
        ax.mixed.hash(&mut h);
        ax.selected.hash(&mut h);
        ax.value_text.as_deref().hash(&mut h);
        for v in [ax.value_now, ax.value_min, ax.value_max, ax.value_step] {
            v.is_some().hash(&mut h);
            f(&mut h, v.unwrap_or(0.0));
        }

        // The content's own value, through the same accessors `build` uses.
        match tree.content[i] {
            NodeContent::Edit(edit_key) => {
                let pad = spec.layout.padding;
                f(&mut h, tree.pos[i].x + pad.l);
                f(&mut h, tree.pos[i].y + pad.t);
                src.edit.text(edit_key).hash(&mut h);
                src.edit.caret_and_selection(edit_key).hash(&mut h);
                src.edit.selection_cursors(edit_key).hash(&mut h);
                // `runs` is the editor's shaped layout, which the text and
                // the box above do not fully determine — a font arriving
                // between frames reshapes it. The store's own version bumps
                // on every mutation that could, so it stands in for reading
                // the runs, which would cost what building them costs.
                src.edit.version(edit_key).hash(&mut h);
                let clip = edit_run_clip(tree, src, i, edit_key);
                for v in [clip.x, clip.y, clip.w, clip.h] {
                    f(&mut h, v);
                }
            }
            NodeContent::Cells(id) => src.cells.value(id).hash(&mut h),
            _ => {}
        }

        // Scrolling: the offset is retained state, the max a layout output.
        if spec.layout.scroll_x || spec.layout.scroll_y {
            spec.layout.scroll_x.hash(&mut h);
            spec.layout.scroll_y.hash(&mut h);
            let off = src.scroll.drawn(key);
            let max = tree.scroll_max[i];
            for v in [off.x, off.y, max.x, max.y] {
                f(&mut h, v);
            }
        }

        if sem.presentational {
            skip_until = tree.subtree_end(i);
        }
        i += 1;
    }
    Some(h.finish())
}

/// Derives the access tree of a laid-out frame.
pub(crate) fn build(tree: &Tree, src: &Sources<'_>) -> AccessTree {
    let mut out = AccessTree::default();
    if tree.is_empty() {
        return out;
    }
    // Per tree node: the key of the nearest semantic ancestor (for elided
    // nodes, inherited from the parent).
    let mut parents: Vec<Option<Key>> = vec![None; tree.len()];
    let mut skip_until = 0usize;
    let mut i = 0usize;
    while i < tree.len() {
        if i < skip_until {
            i += 1;
            continue;
        }
        let parent = tree.parent[i];
        let inherited = if parent == NIL {
            None
        } else {
            parents[parent as usize]
        };
        let Some(sem) = semantic(tree, src.text, src.edit, src.title, i) else {
            parents[i] = inherited;
            i += 1;
            continue;
        };
        if sem.role == Role::None {
            skip_until = tree.subtree_end(i);
            i += 1;
            continue;
        }
        let key = tree.keys[i];
        parents[i] = Some(key);
        let spec: &NodeSpec = &tree.specs[i];
        let rect = node_rect(tree, src, i);
        let mut node = AccessNode {
            key,
            parent: inherited,
            origin: tree.origins[i],
            role: sem.role,
            name: sem.name,
            description: spec.access().description.as_ref().map(|d| d.to_string()),
            rect,
            value: None,
            caret: None,
            selection: None,
            runs: Vec::new(),
            anchor: None,
            focus: None,
            checked: None,
            mixed: false,
            selected: None,
            expanded: None,
            orientation: crate::composite::orientation(tree, i),
            pos_in_set: None,
            set_size: None,
            number: None,
            min: None,
            max: None,
            step: None,
            focused: src.focus == Some(key),
            disabled: spec.disabled,
            modal: src.modal == Some(key),
            scroll: None,
            actions: 0,
            live: spec.access().live,
        };
        let mut actions = 0u32;
        match spec.window {
            Some(WindowRole::Button(_)) => actions |= AccessAction::Click.bit(),
            Some(WindowRole::Drag) => {}
            None => {
                if spec.events().on_click.is_some() && !spec.disabled {
                    actions |= AccessAction::Click.bit();
                }
            }
        }
        // Anything the keyboard can reach, assistive technology can focus
        // (and AccessKit calls focusable only what supports `Focus`).
        if focusable(tree, i) {
            actions |= AccessAction::Focus.bit() | AccessAction::Blur.bit();
        }
        if let NodeContent::Edit(edit_key) = tree.content[i] {
            let pad = spec.layout.padding;
            let origin = Vec2::new(tree.pos[i].x + pad.l, tree.pos[i].y + pad.t);
            node.value = src.edit.text(edit_key);
            node.runs = src.edit.runs(edit_key, key, origin, src.scale);
            clip_runs(&mut node.runs, edit_run_clip(tree, src, i, edit_key));
            if let Some((caret, selection)) = src.edit.caret_and_selection(edit_key) {
                node.caret = Some(caret);
                node.selection = selection;
            }
            if let Some((anchor, focus)) = src.edit.selection_cursors(edit_key) {
                node.anchor = node.text_pos(anchor.0, anchor.1);
                node.focus = node.text_pos(focus.0, focus.1);
            }
            if !spec.disabled {
                actions |= AccessAction::Click.bit()
                    | AccessAction::SetValue.bit()
                    | AccessAction::SetTextSelection.bit()
                    | AccessAction::ReplaceSelectedText.bit();
            }
        } else if is_custom_editor(tree, i) {
            custom_editor(tree, src, i, &mut node);
            if !spec.disabled {
                actions |= AccessAction::SetValue.bit()
                    | AccessAction::SetTextSelection.bit()
                    | AccessAction::ReplaceSelectedText.bit();
            }
        } else if let NodeContent::Cells(id) = tree.content[i] {
            // A terminal's value is its screen, rows joined.
            node.value = Some(src.cells.value(id));
        }
        // A disclosure names its own state, so it lands wherever it is
        // declared: no role means "shows and hides something".
        let ax = spec.access();
        node.expanded = ax.expanded;
        match sem.role {
            Role::Checkbox | Role::Radio | Role::Switch => {
                node.checked = Some(ax.checked);
                node.mixed = ax.mixed && sem.role == Role::Checkbox;
            }
            // A tab is one of a set by definition, so it reports either
            // state; a row or a link reports only the one it declares,
            // since most lists and every navigation bar are not
            // selections and "not selected" on each of their nodes is the
            // noise AccessKit warns about.
            Role::Tab => node.selected = Some(ax.selected),
            Role::ListItem | Role::Link if ax.selected => node.selected = Some(true),
            // A menu row that is a setting reads as checked (the drawn
            // menu's checkmark, `widgets::menu_panel`); every other row is
            // a command and says nothing, the way a list row says nothing
            // about a selection it is not part of. Before this the widget
            // declared the fact and the tree dropped it, so a reader heard
            // "Wrap" where a sighted user saw "✓ Wrap".
            Role::MenuItem if ax.checked => node.checked = Some(true),
            Role::Slider => {
                node.number = ax.value_now;
                node.min = ax.value_min;
                node.max = ax.value_max;
                node.step = ax.value_step;
                // The declared reading, in the one string slot the
                // platform gives a node (see `value`). Slider-only, like
                // the three numbers: a `group` shaped like a progress bar
                // carries none of them either, and a role whose numbers
                // are ignored should not have a reading that is not.
                node.value = ax.value_text.as_deref().map(str::to_owned);
                // SetValue too: Windows' UI Automation has no increment,
                // and moves a slider only by setting it.
                if !spec.disabled {
                    actions |= AccessAction::Increment.bit()
                        | AccessAction::Decrement.bit()
                        | AccessAction::SetValue.bit();
                }
            }
            _ => {}
        }
        if spec.layout.scroll_x || spec.layout.scroll_y {
            let off = src.scroll.drawn(key);
            let max = tree.scroll_max[i];
            node.scroll = Some(ScrollState {
                x: off.x,
                y: off.y,
                max_x: max.x,
                max_y: max.y,
            });
            if spec.layout.scroll_y {
                actions |= AccessAction::ScrollUp.bit() | AccessAction::ScrollDown.bit();
            }
            if spec.layout.scroll_x {
                actions |= AccessAction::ScrollLeft.bit() | AccessAction::ScrollRight.bit();
            }
        }
        if i != 0 {
            actions |= AccessAction::ScrollIntoView.bit();
        }
        node.actions = actions;
        if node.focused {
            out.focus = Some(key);
        }
        out.nodes.push(node);
        if sem.presentational {
            skip_until = tree.subtree_end(i);
        }
        i += 1;
    }
    set_positions(tree, &mut out);
    out.hash = hash_of(&out);
    out
}

/// "3 of 7", derived rather than declared: a composite container already
/// holds its items, so the core counts them and numbers each one instead
/// of making every view repeat itself (and get it wrong the moment a row
/// is filtered out). The count lands on the container and the zero-based
/// ordinal on the item, which is how AccessKit models a set.
///
/// The items come from `composite::items`, which is also the order the
/// arrow keys walk: "3 of 7" and that walk must be the same seven in the same
/// order or the announcement is a lie, so they are one function rather
/// than two that agree by inspection. A disabled item is still one of the
/// set — "2 of 3" is what a reader should hear on a disabled tab — even
/// though the arrows step over it, as the Tab ring does.
fn set_positions(tree: &Tree, out: &mut AccessTree) {
    let containers: Vec<(usize, Role)> = (0..tree.len())
        .filter_map(|i| {
            let item = crate::composite::item_role(tree.specs[i].access().role?)?;
            Some((i, item))
        })
        .collect();
    let mut items: Vec<usize> = Vec::new();
    for (container, item) in containers {
        crate::composite::items(tree, container, item, &mut items);
        if items.is_empty() {
            continue;
        }
        let size = items.len();
        let container = tree.keys[container];
        let keys: Vec<Key> = items.iter().map(|&i| tree.keys[i]).collect();
        for n in &mut out.nodes {
            if n.key == container {
                n.set_size = Some(size);
            } else if let Some(pos) = keys.iter().position(|k| *k == n.key) {
                n.pos_in_set = Some(pos);
            }
        }
    }
}

/// The `role="line"` nodes under `i`, in tree order — the numbering a
/// custom editor's `access` events use for `line`, and the one a press
/// inside a key sink carries, so the two agree. Subtrees
/// under `role="none"` (a gutter) do not count, and a line's own subtree
/// is not searched for lines.
pub(crate) fn lines_under(tree: &Tree, i: usize) -> Vec<usize> {
    let end = tree.subtree_end(i);
    let mut lines: Vec<usize> = Vec::new();
    let mut j = i + 1;
    while j < end {
        let spec = &tree.specs[j];
        if spec.access().role == Some(Role::None) {
            j = tree.subtree_end(j);
            continue;
        }
        if spec.access().role == Some(Role::Line) {
            lines.push(j);
            j = tree.subtree_end(j);
            continue;
        }
        j += 1;
    }
    lines
}

/// A custom editor's text: its `role="line"` descendants in order, each
/// line the text nodes inside it concatenated, runs from their buffers,
/// caret and anchor from the lines that declare them. Subtrees under
/// `role="none"` (a gutter) do not count.
fn custom_editor(tree: &Tree, src: &Sources<'_>, i: usize, node: &mut AccessNode) {
    let lines = lines_under(tree, i);
    let mut value = String::new();
    let mut run_no = 0usize;
    let mut caret: Option<(usize, usize)> = None;
    let mut anchor: Option<(usize, usize)> = None;
    for (ln, &l) in lines.iter().enumerate() {
        let last = ln + 1 == lines.len();
        let mut line_text = String::new();
        let mut texts: Vec<usize> = Vec::new();
        let lend = tree.subtree_end(l);
        let mut t = l;
        while t < lend {
            if tree.specs[t].access().role == Some(Role::None) {
                t = tree.subtree_end(t);
                continue;
            }
            if let NodeContent::Text(_) = tree.content[t] {
                texts.push(t);
            }
            t += 1;
        }
        for (k, &t) in texts.iter().enumerate() {
            let NodeContent::Text(id) = tree.content[t] else {
                continue;
            };
            let base = line_text.len();
            line_text.push_str(src.text.content(id));
            let newline = !last && k + 1 == texts.len();
            let first = node.runs.len();
            src.text.access_runs(
                id,
                RunSource {
                    key: node.key,
                    line: ln,
                    byte_base: base,
                    origin: tree.pos[t],
                    scale: src.scale,
                    newline_after_last: newline,
                },
                &mut run_no,
                &mut node.runs,
            );
            clip_runs(&mut node.runs[first..], clip_of(src, t));
        }
        if texts.is_empty() {
            // An empty line still has a place for the caret.
            node.runs.push(empty_run(
                node.key,
                run_no,
                ln,
                Rect::from_pos_size(tree.pos[l], tree.size[l]),
                !last,
            ));
            let at = node.runs.len() - 1;
            clip_runs(&mut node.runs[at..], clip_of(src, l));
            run_no += 1;
        }
        if let Some(c) = tree.specs[l].access().caret {
            caret = Some((ln, (c as usize).min(line_text.len())));
        }
        if let Some(a) = tree.specs[l].access().selection_anchor {
            anchor = Some((ln, (a as usize).min(line_text.len())));
        }
        if ln > 0 {
            value.push('\n');
        }
        value.push_str(&line_text);
    }
    node.value = Some(value);
    if let Some((line, offset)) = caret {
        node.focus = node.text_pos(line, offset);
        let anchor = anchor.unwrap_or((line, offset));
        node.anchor = node.text_pos(anchor.0, anchor.1);
        // Byte offsets into the joined value, for the flat view of it.
        let flat = |(line, offset): (usize, usize)| {
            node.value
                .as_ref()
                .map(|v| v.split('\n').take(line).map(|l| l.len() + 1).sum::<usize>() + offset)
        };
        node.caret = flat((line, offset));
        if anchor != (line, offset)
            && let (Some(a), Some(c)) = (flat(anchor), node.caret)
        {
            node.selection = Some((a.min(c), a.max(c)));
        }
    }
}

/// Where a buffer's runs belong: which line and byte base of the editor
/// they land in, and where the buffer draws.
pub(crate) struct RunSource {
    pub key: Key,
    /// The line index the buffer's first line maps to.
    pub line: usize,
    /// Byte offset of the buffer's text inside that line (a line drawn as
    /// several text nodes).
    pub byte_base: usize,
    /// Where the buffer's (0, 0) sits, logical px, viewport coordinates.
    pub origin: Vec2,
    pub scale: f32,
    /// Whether the buffer's last line continues into another line of the
    /// editor (its last run then ends with `"\n"`).
    pub newline_after_last: bool,
}

/// The id of run `n` of editor `key`; never a node key (the tag byte
/// keeps it out of the label and index namespaces).
fn run_key(key: Key, n: usize) -> Key {
    key.str("\u{1}run").index(n as u64)
}

fn empty_run(key: Key, n: usize, line: usize, rect: Rect, newline: bool) -> AccessRun {
    AccessRun {
        key: run_key(key, n),
        line,
        start: 0,
        end: 0,
        text: if newline { "\n".into() } else { String::new() },
        rect: Rect::new(rect.x, rect.y, 0.0, rect.h),
        char_lengths: if newline { vec![1] } else { Vec::new() },
        char_positions: if newline { vec![0.0] } else { Vec::new() },
        char_widths: if newline { vec![0.0] } else { Vec::new() },
        word_starts: Vec::new(),
        rtl: false,
    }
}

/// The runs of a laid-out buffer (one per visual line, split past
/// [`RUN_CHARS`] characters), appended to `out`. Only laid-out lines
/// produce runs: a long document's off-screen lines are in the value
/// but not walked.
pub(crate) fn runs_of_buffer(
    buffer: &Buffer,
    src: RunSource,
    run_no: &mut usize,
    out: &mut Vec<AccessRun>,
) {
    let line_count = buffer.lines.len();
    let runs: Vec<_> = buffer.layout_runs().collect();
    for (r, run) in runs.iter().enumerate() {
        let last_of_line = runs.get(r + 1).is_none_or(|next| next.line_i != run.line_i);
        let newline = last_of_line && (run.line_i + 1 < line_count || src.newline_after_last);
        push_row_runs(
            &RowGlyphs {
                glyphs: run.glyphs,
                text: run.text,
                line: run.line_i,
                top: run.line_top,
                height: run.line_height,
                rtl: run.rtl,
                dx: 0.0,
                base: 0,
                newline,
            },
            &src,
            run_no,
            out,
        );
    }
}

/// One visual row's glyphs, for [`push_row_runs`]: a buffer's laid-out
/// run, or one row of a long line's chunk (`TextSystem::access_runs`),
/// whose glyphs sit `dx` physical px from where its unwrapped run put
/// them and whose `text` starts `base` bytes into the source's.
pub(crate) struct RowGlyphs<'a> {
    pub glyphs: &'a [cosmic_text::LayoutGlyph],
    /// The paragraph the glyphs index into.
    pub text: &'a str,
    /// The source's line this row belongs to, less `RunSource::line`.
    pub line: usize,
    pub top: f32,
    pub height: f32,
    pub rtl: bool,
    pub dx: f32,
    pub base: usize,
    /// Whether the row ends a line that continues into another.
    pub newline: bool,
}

/// The runs of one visual row (split past [`RUN_CHARS`] characters),
/// appended to `out`.
pub(crate) fn push_row_runs(
    row: &RowGlyphs<'_>,
    src: &RunSource,
    run_no: &mut usize,
    out: &mut Vec<AccessRun>,
) {
    let scale = src.scale.max(f32::EPSILON);
    let newline = row.newline;
    {
        // The slice of the line this row lays out.
        let (start, end) = row.glyphs.iter().fold((usize::MAX, 0usize), |(s, e), g| {
            (s.min(g.start), e.max(g.end))
        });
        let (start, end) = if row.glyphs.is_empty() {
            (0, 0)
        } else {
            (start, end)
        };
        let slice = &row.text[start..end];
        // Where the run starts: its leftmost glyph (0 for an empty line).
        let x0 = row
            .glyphs
            .iter()
            .map(|g| g.x + row.dx)
            .fold(f32::INFINITY, f32::min);
        let x0 = if x0.is_finite() { x0 } else { 0.0 };
        // Every character: position and width from the glyph cluster
        // covering it (split evenly by bytes inside a cluster).
        let mut chars: Vec<(usize, u8, f32, f32)> = Vec::new(); // (byte, len, x, w) physical
        for (rel, c) in slice.char_indices() {
            let idx = start + rel;
            let len = c.len_utf8();
            let (x, w) = match row.glyphs.iter().find(|g| g.start <= idx && idx < g.end) {
                Some(g) => {
                    let span = (g.end - g.start).max(1) as f32;
                    (
                        g.x + row.dx + g.w * (idx - g.start) as f32 / span,
                        g.w * len as f32 / span,
                    )
                }
                None => chars.last().map_or((x0, 0.0), |&(_, _, x, w)| (x + w, 0.0)),
            };
            chars.push((idx, len as u8, x, w));
        }
        let line_end_x = chars.last().map_or(x0, |&(_, _, x, w)| x + w);
        if newline {
            chars.push((end, 1, line_end_x, 0.0));
        }
        // Word starts over the run's text (the newline is never one).
        let starts: Vec<usize> = slice
            .split_word_bound_indices()
            .filter(|(_, w)| !w.chars().all(char::is_whitespace))
            .map(|(b, _)| slice[..b].chars().count())
            .collect();
        // Chunk so character indices fit a byte.
        let mut at = 0usize;
        loop {
            let chunk_end = (at + RUN_CHARS).min(chars.len());
            let chunk = &chars[at..chunk_end];
            let chunk_x = chunk.first().map_or(x0, |c| c.2);
            let chunk_end_x = chunk.last().map_or(chunk_x, |c| c.2 + c.3);
            let byte_start = chunk.first().map_or(end, |c| c.0);
            let byte_end = chunk
                .last()
                .map_or(end, |c| {
                    if newline && c.0 == end {
                        end
                    } else {
                        c.0 + c.1 as usize
                    }
                })
                .min(end);
            let mut text = row.text[byte_start.min(end)..byte_end].to_string();
            if newline && chunk_end == chars.len() {
                text.push('\n');
            }
            out.push(AccessRun {
                key: run_key(src.key, *run_no),
                line: src.line + row.line,
                start: src.byte_base + row.base + byte_start.min(end),
                end: src.byte_base + row.base + byte_end,
                text,
                rect: Rect::new(
                    src.origin.x + chunk_x / scale,
                    src.origin.y + row.top / scale,
                    (chunk_end_x - chunk_x) / scale,
                    row.height / scale,
                ),
                char_lengths: chunk.iter().map(|c| c.1).collect(),
                char_positions: chunk.iter().map(|c| (c.2 - chunk_x) / scale).collect(),
                char_widths: chunk.iter().map(|c| c.3 / scale).collect(),
                word_starts: starts
                    .iter()
                    .filter(|&&s| s >= at && s < chunk_end)
                    .map(|&s| (s - at) as u8)
                    .collect(),
                rtl: row.rtl,
            });
            *run_no += 1;
            at = chunk_end;
            if at >= chars.len() {
                break;
            }
        }
    }
}

fn hash_of(tree: &AccessTree) -> u64 {
    let mut h = crate::key::FNV_OFFSET;
    let mut mix = |bytes: &[u8]| h = crate::key::fnv(h, bytes);
    let mix_str = |mix: &mut dyn FnMut(&[u8]), s: &Option<String>| match s {
        Some(s) => {
            mix(&[1]);
            mix(s.as_bytes());
            mix(&[0]);
        }
        None => mix(&[0]),
    };
    let mix_f32 = |mix: &mut dyn FnMut(&[u8]), v: Option<f32>| match v {
        Some(v) => {
            mix(&[1]);
            mix(&v.to_bits().to_le_bytes());
        }
        None => mix(&[0]),
    };
    let mix_pos = |mix: &mut dyn FnMut(&[u8]), p: Option<TextPos>| match p {
        Some(p) => {
            mix(&p.run.0.to_le_bytes());
            mix(&p.character.to_le_bytes());
        }
        None => mix(&[0]),
    };
    for n in &tree.nodes {
        mix(&n.key.0.to_le_bytes());
        mix(&n.parent.map_or(0, |k| k.0).to_le_bytes());
        mix(&[n.role as u8]);
        mix_str(&mut mix, &n.name);
        mix_str(&mut mix, &n.description);
        for v in [n.rect.x, n.rect.y, n.rect.w, n.rect.h] {
            mix(&v.to_bits().to_le_bytes());
        }
        mix_str(&mut mix, &n.value);
        mix(&n.caret.unwrap_or(usize::MAX).to_le_bytes());
        let (a, b) = n.selection.unwrap_or((usize::MAX, usize::MAX));
        mix(&a.to_le_bytes());
        mix(&b.to_le_bytes());
        for r in &n.runs {
            mix(&r.key.0.to_le_bytes());
            mix(r.text.as_bytes());
            mix(&[0]);
            for v in [r.rect.x, r.rect.y, r.rect.w, r.rect.h] {
                mix(&v.to_bits().to_le_bytes());
            }
            for v in &r.char_positions {
                mix(&v.to_bits().to_le_bytes());
            }
        }
        mix_pos(&mut mix, n.anchor);
        mix_pos(&mut mix, n.focus);
        mix(&[n.checked.map_or(2, |c| c as u8), n.mixed as u8]);
        mix(&n.step.map_or(u32::MAX, f32::to_bits).to_le_bytes());
        mix(&[n.selected.map_or(2, |c| c as u8)]);
        mix(&[n.expanded.map_or(2, |c| c as u8)]);
        mix(&n.pos_in_set.unwrap_or(usize::MAX).to_le_bytes());
        mix(&n.set_size.unwrap_or(usize::MAX).to_le_bytes());
        mix_f32(&mut mix, n.number);
        mix_f32(&mut mix, n.min);
        mix_f32(&mut mix, n.max);
        mix(&[n.focused as u8, n.disabled as u8, n.modal as u8]);
        if let Some(s) = n.scroll {
            for v in [s.x, s.y, s.max_x, s.max_y] {
                mix(&v.to_bits().to_le_bytes());
            }
        }
        mix(&n.actions.to_le_bytes());
        mix(&[n.live as u8]);
    }
    mix(&tree.focus.map_or(0, |k| k.0).to_le_bytes());
    h
}

/// The label type on `NodeSpec`: shared, so cloning a spec is a refcount
/// bump and a Rust view can keep one `Arc<str>` across frames.
pub type Label = Arc<str>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_names_round_trip() {
        for r in Role::ALL {
            assert_eq!(Role::parse(r.name()), Some(r));
        }
        assert_eq!(Role::parse("nope"), None);
    }

    #[test]
    fn action_bits_are_distinct_and_named() {
        let mut seen = 0u32;
        for a in AccessAction::ALL {
            assert_eq!(seen & a.bit(), 0);
            seen |= a.bit();
            assert_eq!(AccessAction::parse(a.name()), Some(a));
        }
    }

    #[test]
    fn run_keys_never_collide_with_node_keys() {
        let k = Key::ROOT.str("editor");
        assert_ne!(run_key(k, 0), k.index(0));
        assert_ne!(run_key(k, 0), k.str("run"));
        assert_ne!(run_key(k, 0), run_key(k, 1));
    }
}
