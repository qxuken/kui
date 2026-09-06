//! Accessibility as data (see `docs/adr/0001-accessibility-as-data.md`).
//!
//! The frame's tree knows what nodes *do* (`on_click`, editors, scroll
//! containers, window chrome); two props, `role` and `label`, let a view
//! say what they *are*. From both the core derives an [`AccessTree`]: the
//! semantic nodes of the frame, in tree order, each with its role, name,
//! rect, state and the actions it supports. Plain boxes are elided — their
//! semantic descendants attach to the nearest semantic ancestor — so a
//! frame of ten thousand rects yields a tree of a handful of nodes.
//!
//! Text is the one place the tree goes below the node: an editor carries
//! its laid-out lines as [`AccessRun`]s (one per visual line, with every
//! character's position), and its caret and selection as positions in
//! them, which is what a screen reader needs to read by character, word
//! and line and to report where the caret went. The built-in editors get
//! this from their buffers; an app that owns its text (an `on_key` sink
//! drawing lines itself) declares `role="multilineTextInput"` on the
//! sink, `role="line"` on each line it draws, and `caret` /
//! `selectionAnchor` byte offsets on the lines that hold them — and gets
//! the same tree, with selection requests coming back as events.
//!
//! The tree is data a driver asks for ([`crate::Core::access_tree`]); the
//! windowed runners translate it into the platform accessibility API
//! through AccessKit, headless tests assert on it directly. Requests from
//! assistive technology come back in as input
//! ([`crate::InputEvent::Access`]) and resolve inside the core: activating
//! a button emits the same event a pointer click would.

use std::sync::Arc;

use cosmic_text::Buffer;
use unicode_segmentation::UnicodeSegmentation;

use crate::edit::EditStore;
use crate::geom::{Rect, Size, Vec2};
use crate::key::Key;
use crate::scroll::ScrollStore;
use crate::spec::NodeSpec;
use crate::text::TextSystem;
use crate::tree::{NIL, NodeContent, OriginId, Tree};
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
    // -- Appended by ADR 0007 ----------------------------------------------
    // At the tail, and in the order [`Role::ALL`] lists them, because the
    // tail is the only free position: `KUI_ROLE_*` is an `ALL` index plus
    // one and the Lua and Node wires carry the `ROLES` index, so a role
    // inserted anywhere else renumbers every role after it
    // (`docs/adr/0006-c-abi-versioning.md`).
    /// A set of `radio`s: one Tab stop, arrows moving the checked one.
    RadioGroup,
    /// A menu: one Tab stop, arrows moving focus without activating.
    Menu,
    /// One item of a `menu`. A control, so it is focusable by its role and
    /// an unnamed one is reported.
    MenuItem,
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

    pub const ALL: [Role; 25] = [
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
/// politeness of a [`Announcement`]. See
/// `docs/adr/0008-live-regions-and-announcements.md`.
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
    /// The camelCase spelling every binding uses, and `schema::LIVE`'s
    /// wire order.
    pub fn name(self) -> &'static str {
        match self {
            Live::Off => "off",
            Live::Polite => "polite",
            Live::Assertive => "assertive",
        }
    }

    /// The variant `schema::LIVE` index `i` names.
    pub fn from_index(i: usize) -> Live {
        match i {
            1 => Live::Polite,
            2 => Live::Assertive,
            _ => Live::Off,
        }
    }
}

/// One thing to say once, with no node behind it: "Saved", "3 results".
/// Queued by `Core::announce` and drained by `Core::take_announcements`,
/// the way window commands, audio commands and warnings are — an
/// announcement is an event on a timeline, and the frame's tree has no
/// place to keep one (see
/// `docs/adr/0008-live-regions-and-announcements.md`).
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
/// row that says it is a column would be a fact with two owners (see
/// `docs/adr/0007-composite-keyboard-patterns.md`, decision 7). It is an
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
    /// issues its command; an editor or key sink takes focus).
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
    /// Logical px, viewport coordinates.
    pub rect: Rect,
    pub char_lengths: Vec<u8>,
    /// Each character's x relative to `rect.x`, and its width.
    pub char_positions: Vec<f32>,
    pub char_widths: Vec<f32>,
    /// Character indices where words start.
    pub word_starts: Vec<u8>,
    pub rtl: bool,
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
    /// `description` on the spec — what the `tooltip` prop sets.
    pub description: Option<String>,
    /// Final laid-out rect, logical px, viewport coordinates.
    pub rect: Rect,
    /// An editor's committed text (a custom editor's: the lines it draws,
    /// joined by `"\n"`).
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
    /// `valueNow` / `valueMin` / `valueMax` for a slider.
    pub number: Option<f32>,
    pub min: Option<f32>,
    pub max: Option<f32>,
    /// Holds keyboard focus (`Core::focus`; see
    /// `docs/adr/0002-keyboard-focus-as-data.md`).
    pub focused: bool,
    /// Declared `disabled`: inert, and not in the Tab ring.
    pub disabled: bool,
    /// The frame's modal surface (`aria-modal`): the Tab ring and every
    /// pointer are confined to it, and everything else is inert. Only the
    /// modal in effect carries it — the last one declared — so a confirm
    /// inside a dialog leaves the dialog an ordinary node
    /// (`docs/adr/0003-modal-surfaces.md`).
    pub modal: bool,
    pub scroll: Option<ScrollState>,
    /// Bitset of [`AccessAction::bit`].
    pub actions: u32,
    /// Declared `live`: when the text inside this node changes, a reader
    /// reads the change without being asked. Carried exactly where the
    /// view declared it — the platform consumer inherits it down the
    /// subtree, and duplicating that here would be a second copy of a
    /// rule kui does not own (see
    /// `docs/adr/0008-live-regions-and-announcements.md`).
    pub live: Live,
}

impl AccessNode {
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
    /// run holding it, or the line's last run for its end.
    pub fn text_pos(&self, line: usize, offset: usize) -> Option<TextPos> {
        let mut last = None;
        for r in self.runs.iter().filter(|r| r.line == line) {
            if offset >= r.start && offset < r.end {
                return Some(TextPos {
                    run: r.key,
                    character: r.text[..offset - r.start].chars().count(),
                });
            }
            last = Some(r);
        }
        let r = last?;
        Some(TextPos {
            run: r.key,
            character: r.text[..(offset.max(r.start) - r.start).min(r.end - r.start)]
                .chars()
                .count(),
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
        NodeContent::Image(_) => return Some(Role::Image),
        // A stroke is decoration and takes no input, so nothing below —
        // not even an `on_click` it ignores — can make it a control
        // (`docs/adr/0010-a-segment-primitive.md`, decision 7).
        NodeContent::Line(_) => return None,
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
        // change would go unread (ADR 0008, decision 2).
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

/// Whether node `i` can hold keyboard focus (see
/// `docs/adr/0002-keyboard-focus-as-data.md`): an editor, a key sink, a
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
    // announcing the changed descendant — is what ADR 0008 first built,
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
fn content_name(tree: &Tree, text: &TextSystem, i: usize) -> Option<String> {
    let end = tree.subtree_end(i);
    let mut out = String::new();
    for j in i..end {
        if let NodeContent::Text(id) = tree.content[j] {
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
    pub edit: &'a EditStore,
    pub scroll: &'a ScrollStore,
    pub title: Option<&'a str>,
    /// The core's one keyboard focus (see `Core::focus`).
    pub focus: Option<Key>,
    /// The frame's modal in effect (see `Core::modal`).
    pub modal: Option<Key>,
    pub viewport: Size,
    pub scale: f32,
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
        let rect = if i == 0 {
            Rect::new(0.0, 0.0, src.viewport.w, src.viewport.h)
        } else {
            Rect::from_pos_size(tree.pos[i], tree.size[i])
        };
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
            selected: None,
            expanded: None,
            orientation: crate::composite::orientation(tree, i),
            pos_in_set: None,
            set_size: None,
            number: None,
            min: None,
            max: None,
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
        }
        // A disclosure names its own state, so it lands wherever it is
        // declared: no role means "shows and hides something".
        let ax = spec.access();
        node.expanded = ax.expanded;
        match sem.role {
            Role::Checkbox | Role::Radio | Role::Switch => node.checked = Some(ax.checked),
            // A tab is one of a set by definition, so it reports either
            // state; a row or a link reports only the one it declares,
            // since most lists and every navigation bar are not
            // selections and "not selected" on each of their nodes is the
            // noise AccessKit warns about.
            Role::Tab => node.selected = Some(ax.selected),
            Role::ListItem | Role::Link if ax.selected => node.selected = Some(true),
            Role::Slider => {
                node.number = ax.value_now;
                node.min = ax.value_min;
                node.max = ax.value_max;
                if !spec.disabled {
                    actions |= AccessAction::Increment.bit() | AccessAction::Decrement.bit();
                }
            }
            _ => {}
        }
        if spec.layout.scroll_x || spec.layout.scroll_y {
            let off = src.scroll.offset(key);
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
/// arrow keys walk (`docs/adr/0007-composite-keyboard-patterns.md`,
/// decision 3): "3 of 7" and that walk must be the same seven in the same
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

/// A custom editor's text: its `role="line"` descendants in order, each
/// line the text nodes inside it concatenated, runs from their buffers,
/// caret and anchor from the lines that declare them. Subtrees under
/// `role="none"` (a gutter) do not count.
fn custom_editor(tree: &Tree, src: &Sources<'_>, i: usize, node: &mut AccessNode) {
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
            src.text.with_buffer(id, |b| {
                runs_of_buffer(
                    b,
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
            });
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
    let scale = src.scale.max(f32::EPSILON);
    let line_count = buffer.lines.len();
    let runs: Vec<_> = buffer.layout_runs().collect();
    for (r, run) in runs.iter().enumerate() {
        let last_of_line = runs.get(r + 1).is_none_or(|next| next.line_i != run.line_i);
        let newline = last_of_line && (run.line_i + 1 < line_count || src.newline_after_last);
        // The slice of the line this run lays out.
        let (start, end) = run.glyphs.iter().fold((usize::MAX, 0usize), |(s, e), g| {
            (s.min(g.start), e.max(g.end))
        });
        let (start, end) = if run.glyphs.is_empty() {
            (0, 0)
        } else {
            (start, end)
        };
        let slice = &run.text[start..end];
        // Where the run starts: its leftmost glyph (0 for an empty line).
        let x0 = run.glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
        let x0 = if x0.is_finite() { x0 } else { 0.0 };
        // Every character: position and width from the glyph cluster
        // covering it (split evenly by bytes inside a cluster).
        let mut chars: Vec<(usize, u8, f32, f32)> = Vec::new(); // (byte, len, x, w) physical
        for (rel, c) in slice.char_indices() {
            let idx = start + rel;
            let len = c.len_utf8();
            let (x, w) = match run.glyphs.iter().find(|g| g.start <= idx && idx < g.end) {
                Some(g) => {
                    let span = (g.end - g.start).max(1) as f32;
                    (
                        g.x + g.w * (idx - g.start) as f32 / span,
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
            let mut text = run.text[byte_start.min(end)..byte_end].to_string();
            if newline && chunk_end == chars.len() {
                text.push('\n');
            }
            out.push(AccessRun {
                key: run_key(src.key, *run_no),
                line: src.line + run.line_i,
                start: src.byte_base + byte_start.min(end),
                end: src.byte_base + byte_end,
                text,
                rect: Rect::new(
                    src.origin.x + chunk_x / scale,
                    src.origin.y + run.line_top / scale,
                    (chunk_end_x - chunk_x) / scale,
                    run.line_height / scale,
                ),
                char_lengths: chunk.iter().map(|c| c.1).collect(),
                char_positions: chunk.iter().map(|c| (c.2 - chunk_x) / scale).collect(),
                char_widths: chunk.iter().map(|c| c.3 / scale).collect(),
                word_starts: starts
                    .iter()
                    .filter(|&&s| s >= at && s < chunk_end)
                    .map(|&s| (s - at) as u8)
                    .collect(),
                rtl: run.rtl,
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
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
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
        mix(&[n.checked.map_or(2, |c| c as u8)]);
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
