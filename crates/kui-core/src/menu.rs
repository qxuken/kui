//! Context menus as data: what an item *is*, what the window has open,
//! and what a host has to do about the items the core cannot finish on its
//! own (`docs/adr/0017-selection-as-a-scope.md`, decision 5).
//!
//! A menu is a list of items and a point to open at. Nothing here draws:
//! the stock renderer is [`crate::widgets::context_menu`], which builds
//! ordinary nodes into the frame, and a host that has a native menu
//! renders the same list itself. That is the whole reason the list is
//! data — a menu whose items are a closure could only ever be drawn by
//! the code that wrote it.

use crate::geom::Vec2;
use crate::key::Key;
use crate::tree::OriginId;
use crate::value::Value;

/// What an item *means*, as far as anything outside the app is concerned.
///
/// The roles exist for two reasons and neither is decoration. A host with
/// a native menu maps them onto its own standard items, so Copy is the
/// platform's Copy — its wording, its accelerator, its position; and the
/// core acts on the ones it can act on without asking anybody
/// ([`MenuRole::SelectAll`]) or with one round trip through the host (the
/// clipboard three). [`MenuRole::Custom`] is an item the app invented,
/// which only the app can perform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuRole {
    /// The app's own item: choosing it posts the item's payload and the
    /// core does nothing else.
    #[default]
    Custom,
    /// A divider. Never focusable, never chosen, no payload.
    Separator,
    Cut,
    Copy,
    Paste,
    SelectAll,
    /// Show the platform's definition/Look Up panel for the selection.
    /// The core cannot draw one and never tries: with no host to answer
    /// it, the item is simply not offered (ADR 0017, decision 6).
    LookUp,
}

impl MenuRole {
    /// The wire name, for the bindings and the report.
    pub fn name(self) -> &'static str {
        match self {
            MenuRole::Custom => "custom",
            MenuRole::Separator => "separator",
            MenuRole::Cut => "cut",
            MenuRole::Copy => "copy",
            MenuRole::Paste => "paste",
            MenuRole::SelectAll => "selectAll",
            MenuRole::LookUp => "lookUp",
        }
    }

    /// The label the stock renderer draws when the item declares none.
    /// A native renderer ignores this and uses the platform's wording,
    /// which is the point of having a role at all.
    pub fn default_label(self) -> &'static str {
        match self {
            MenuRole::Custom | MenuRole::Separator => "",
            MenuRole::Cut => "Cut",
            MenuRole::Copy => "Copy",
            MenuRole::Paste => "Paste",
            MenuRole::SelectAll => "Select All",
            MenuRole::LookUp => "Look Up",
        }
    }

    /// The shortcut the stock renderer draws beside the row when the item
    /// declares none, spelled the way the platform spells it — Command on
    /// macOS, Control elsewhere, the same split [`KeyMods::primary`] makes
    /// for the key that produces it.
    ///
    /// Display only, like every accelerator here: the core binds nothing,
    /// and the row only names the key the host is already handling. Empty
    /// for the roles with no standard shortcut — `Custom` above all, since
    /// an app's own accelerator is the app's to declare
    /// ([`MenuItem::accel`]) — and empty for `LookUp`, whose shortcut
    /// belongs to the one platform that has the panel and draws its own
    /// menu anyway.
    ///
    /// [`KeyMods::primary`]: crate::input::KeyMods::primary
    pub fn default_accel(self) -> &'static str {
        let mac = cfg!(target_os = "macos");
        match self {
            MenuRole::Cut if mac => "⌘X",
            MenuRole::Copy if mac => "⌘C",
            MenuRole::Paste if mac => "⌘V",
            MenuRole::SelectAll if mac => "⌘A",
            MenuRole::Cut => "Ctrl+X",
            MenuRole::Copy => "Ctrl+C",
            MenuRole::Paste => "Ctrl+V",
            MenuRole::SelectAll => "Ctrl+A",
            MenuRole::Custom | MenuRole::Separator | MenuRole::LookUp => "",
        }
    }

    /// Whether the core performs this itself, or with one hand from the
    /// host. `false` is an item only the app can carry out.
    pub fn is_builtin(self) -> bool {
        !matches!(self, MenuRole::Custom | MenuRole::Separator)
    }
}

/// One row of a menu.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    /// What the row reads. Empty takes the role's default wording.
    pub label: String,
    pub role: MenuRole,
    /// A disabled row is drawn dimmed, is not focusable, and cannot be
    /// chosen — Paste with an empty clipboard, Copy with no selection.
    /// Present rather than absent on purpose: a menu whose rows move
    /// depending on what is possible is a menu nobody builds muscle
    /// memory for.
    pub enabled: bool,
    /// Posted as the event payload when the row is chosen. A `Custom`
    /// item without one posts its label.
    pub id: Option<Value>,
    /// Drawn right-aligned and dimmed; the core binds nothing to it. The
    /// keyboard shortcut is the app's or the platform's, and an
    /// accelerator here only says which one it is. A standard row that
    /// declares none takes its role's ([`MenuRole::default_accel`]), the
    /// way an empty label takes the role's wording.
    pub accel: Option<String>,
}

impl MenuItem {
    /// An item of the app's own, by label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            role: MenuRole::Custom,
            enabled: true,
            id: None,
            accel: None,
        }
    }

    /// One of the standard items, with the role's own wording.
    pub fn role(role: MenuRole) -> Self {
        Self {
            label: String::new(),
            role,
            enabled: true,
            id: None,
            accel: None,
        }
    }

    pub fn separator() -> Self {
        Self::role(MenuRole::Separator)
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    pub fn id(mut self, id: impl Into<Value>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn accel(mut self, a: impl Into<String>) -> Self {
        self.accel = Some(a.into());
        self
    }

    /// What the row reads: its own label, or the role's.
    pub fn text(&self) -> &str {
        if self.label.is_empty() {
            self.role.default_label()
        } else {
            &self.label
        }
    }

    /// What the row draws on its right: its own accelerator, or the
    /// role's. `None` is a row with neither, which is every `Custom` one
    /// the app did not spell a shortcut for.
    pub fn accel_text(&self) -> Option<&str> {
        match &self.accel {
            Some(accel) => Some(accel),
            None => Some(self.role.default_accel()).filter(|a| !a.is_empty()),
        }
    }

    /// Whether the row takes focus and can be chosen.
    pub fn selectable(&self) -> bool {
        self.enabled && self.role != MenuRole::Separator
    }
}

/// The menu a window has open: its items, where it opened, and what it is
/// about. One per window — opening a second closes the first, the way one
/// selection closes the last.
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    /// The node the menu was opened over. Chosen items post their event
    /// on it, so an app reads a menu the way it reads a click.
    pub target: Key,
    /// Where it opens, logical viewport px — the press point, which is
    /// where every platform puts a context menu.
    pub at: Vec2,
    /// Who asked for it: the host, or the extension whose node it is
    /// about. Carried onto the events the items post.
    pub origin: OriginId,
    pub items: Vec<MenuItem>,
}

impl Menu {
    pub fn new(target: Key, at: Vec2, items: Vec<MenuItem>) -> Self {
        Self {
            target,
            at,
            origin: OriginId::HOST,
            items,
        }
    }

    pub fn origin(mut self, origin: OriginId) -> Self {
        self.origin = origin;
        self
    }
}

/// What choosing an item leaves for the host to do, drained with
/// [`crate::runtime::Core::take_menu_actions`] the way window and audio
/// commands are.
///
/// The clipboard is the host's in this library — the runner already reads
/// and writes it for Cmd-C/X/V — and nothing here changes that. The core
/// works out *what* to copy, which is the half it is uniquely able to do,
/// and hands over a string.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuAction {
    /// Put this on the system clipboard. Both Copy and Cut produce one;
    /// Cut has already removed the text by the time it arrives.
    ///
    /// `html` is the same selection with the formatting the core knows
    /// about — bold, italic, a span's declared colour (ADR 0017, decision
    /// 7) — for a host that can offer a second flavour. It is an
    /// *addition* to `text` and never a replacement: a clipboard whose
    /// only flavour is HTML pastes markup into every plain-text field on
    /// the machine.
    SetClipboard { text: String, html: Option<String> },
    /// Read the clipboard and deliver it as `InputEvent::Text`, exactly
    /// as the host does for Cmd-V. The core cannot read a clipboard, so
    /// Paste is the one standard item it can only ask for.
    Paste,
    /// Show the platform's definition panel for `text`, anchored at
    /// `rect` (logical viewport px — the word's own box, which is what
    /// macOS's `showDefinitionForAttributedString:atPoint:` wants). Both
    /// the Look Up row and a force click over text produce one; a host
    /// that cannot show a panel drops it, and is never offered the row in
    /// the first place (`Core::set_lookup_available`).
    LookUp {
        text: String,
        /// The **baseline origin of the selection's first line**, logical
        /// viewport px — the point
        /// `showDefinitionForAttributedString:atPoint:` takes and draws
        /// the term back over. Not a box's corner: a box's bottom puts the
        /// term a line low, and a multi-run selection's union puts it
        /// under the last line while the panel shows the first.
        at: crate::geom::Vec2,
    },
}
