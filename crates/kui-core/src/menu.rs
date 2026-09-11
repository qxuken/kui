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
    /// Every role, in wire order: the index a binding that spells roles as
    /// numbers sends (C's `KUI_MENU_*`), pinned there by name. Append-only,
    /// like every list a C enum restates.
    pub const ALL: [MenuRole; 7] = [
        MenuRole::Custom,
        MenuRole::Separator,
        MenuRole::Cut,
        MenuRole::Copy,
        MenuRole::Paste,
        MenuRole::SelectAll,
        MenuRole::LookUp,
    ];

    /// The role a wire name spells, for the bindings that take roles as
    /// strings: the inverse of [`Self::name`], so a binding cannot accept
    /// a spelling the event will not report back. `None` for a name that
    /// is no role.
    pub fn from_name(name: &str) -> Option<MenuRole> {
        Self::ALL.into_iter().find(|r| r.name() == name)
    }

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
    /// Drawn with a checkmark, and the platform's own check state where a
    /// host renders the menu itself. A setting the row *is* rather than a
    /// command it runs — View ▸ Show Sidebar — and inert for every other
    /// row, which is why it is a flag beside the label and not a role.
    pub checked: bool,
    /// Drawn right-aligned and dimmed; the core binds nothing to it. The
    /// keyboard shortcut is the app's or the platform's, and an
    /// accelerator here only says which one it is. A standard row that
    /// declares none takes its role's ([`MenuRole::default_accel`]), the
    /// way an empty label takes the role's wording.
    pub accel: Option<String>,
}

impl MenuItem {
    /// A row from plain data: a map with `label`, `role` (a wire name;
    /// absent is `custom`), `enabled` (default true), `checked` (default
    /// false), `id` and `accel`. A custom row needs a label, since the
    /// label is what it posts when it has no `id`. Every binding funnels
    /// its rows through here — `openMenu`'s list and a menu bar's alike —
    /// so a row can never mean two things.
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let Value::Map(_) = v else {
            return Err("each menu item is an object".into());
        };
        let role = match v.get("role").and_then(Value::as_str) {
            None => MenuRole::Custom,
            Some(name) => MenuRole::from_name(name)
                .ok_or_else(|| format!("unknown menu item role {name:?}"))?,
        };
        let label = v
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if label.is_empty() && role == MenuRole::Custom {
            return Err("a custom menu item needs a label".into());
        }
        Ok(MenuItem {
            label,
            role,
            enabled: v.get("enabled").and_then(Value::as_bool).unwrap_or(true),
            checked: v.get("checked").and_then(Value::as_bool).unwrap_or(false),
            id: v.get("id").filter(|id| **id != Value::Null).cloned(),
            accel: v.get("accel").and_then(Value::as_str).map(str::to_string),
        })
    }

    /// A menu's rows from plain data: a list of [`Self::from_value`] maps.
    pub fn list_from_value(v: &Value) -> Result<Vec<Self>, String> {
        let Value::List(rows) = v else {
            return Err("a menu's items are an array".into());
        };
        rows.iter().map(Self::from_value).collect()
    }

    /// An item of the app's own, by label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            role: MenuRole::Custom,
            enabled: true,
            checked: false,
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
            checked: false,
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

    /// Draws a checkmark beside the row (and sets the platform's check
    /// state where a host renders the menu).
    pub fn checked(mut self, on: bool) -> Self {
        self.checked = on;
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

// -- The application menu bar -----------------------------------------------
// `docs/adr/0018-a-menu-bar-the-app-declares.md`. The bar is the same rows
// one level up: a list of menus, each a label and the `MenuItem`s above, so
// an Edit menu's Copy is the *same item* the context menu's Copy is and the
// core performs it the same way.

/// One menu of the bar: what the bar reads, and what drops out of it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BarMenu {
    /// What the bar shows. On macOS the first menu is the application menu
    /// and the platform titles that one with the app's own name, whatever
    /// this says.
    pub label: String,
    pub items: Vec<MenuItem>,
    /// A disabled menu is dimmed and opens nothing.
    pub enabled: bool,
}

impl BarMenu {
    pub fn new(label: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self {
            label: label.into(),
            items,
            enabled: true,
        }
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
}

/// The application menu: what a frame declares, in order
/// (`Core::declare_menu_bar`).
///
/// Declared and not commanded, like the window title: a frame that declares
/// none leaves the last one in force, and a frame that declares an empty
/// bar takes it away. Where the platform owns a menu bar the driver hands
/// this over (macOS: `NSApp.mainMenu`); everywhere else
/// [`crate::widgets::menu_bar`] draws it, and each of its titles opens the
/// ordinary [`Menu`] machinery — so the dropdown, its keyboard, its
/// dismissal and its access tree are the ones already built for the context
/// menu.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuBar {
    pub menus: Vec<BarMenu>,
}

impl MenuBar {
    pub fn new(menus: Vec<BarMenu>) -> Self {
        Self { menus }
    }

    /// A bar from plain data: a list of `{ label, items, enabled? }`,
    /// whose `items` are the rows `openMenu` takes
    /// (`docs/adr/0018-a-menu-bar-the-app-declares.md`). A menu with no
    /// `items` is a shape error and not an empty menu: the two read the
    /// same on screen and only one of them was meant.
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let Value::List(menus) = v else {
            return Err("menu is an array of menus".into());
        };
        let mut out = Vec::with_capacity(menus.len());
        for entry in menus {
            let Value::Map(_) = entry else {
                return Err("each menu is an object { label, items }".into());
            };
            let label = entry
                .get("label")
                .and_then(Value::as_str)
                .ok_or("each menu needs a label")?
                .to_string();
            let items = entry
                .get("items")
                .ok_or_else(|| format!("menu entry `{label}` needs `items` (a list of rows)"))?;
            out.push(BarMenu {
                label,
                items: MenuItem::list_from_value(items)?,
                enabled: entry
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            });
        }
        Ok(Self::new(out))
    }

    /// Nothing declared: the bar the platform is asked to take away.
    pub fn is_empty(&self) -> bool {
        self.menus.is_empty()
    }

    /// The item at `(menu, item)`, if it is there.
    pub fn item(&self, menu: usize, item: usize) -> Option<&MenuItem> {
        self.menus.get(menu)?.items.get(item)
    }
}

/// A keyboard shortcut, parsed out of the string an item declares.
///
/// The core binds nothing to it and never has ([`MenuItem::accel`] is
/// display); this exists for the one consumer that needs the parts rather
/// than the words — a platform menu bar, which sets a real key equivalent
/// and then matches it before the window ever sees the key.
///
/// Both spellings parse, because both are written in the field: the
/// portable one (`"mod+shift+s"`, where `mod` is Command on macOS and
/// Control elsewhere) and the platform one a menu is read in (`"⇧⌘S"`,
/// `"Ctrl+Shift+S"`). [`Accel::display`] is the second, which is what
/// `declare_menu_bar` normalizes a declaration into so the drawn bar and
/// the platform's read the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accel {
    pub code: crate::input::KeyCode,
    pub mods: crate::input::KeyMods,
}

impl Accel {
    /// Parses `"mod+s"`, `"ctrl+shift+p"`, `"f5"`, `"⇧⌘S"`. `None` for
    /// anything this vocabulary cannot name — the caller then leaves the
    /// string alone and draws it as written, since a shortcut kui cannot
    /// parse is still a shortcut the app's own keymap runs.
    pub fn parse(s: &str) -> Option<Accel> {
        use crate::input::{KeyCode, KeyMods};
        let mut mods = KeyMods::default();
        let mut rest = s.trim();
        // The glyph spelling has no separators: ⌃⌥⇧⌘ in that order, then
        // the key. Stripped first so `"⌘S"` and `"cmd+s"` land together.
        loop {
            let mut chars = rest.chars();
            match chars.next() {
                Some('\u{2303}') => mods.ctrl = true,
                Some('\u{2325}') => mods.alt = true,
                Some('\u{21e7}') => mods.shift = true,
                Some('\u{2318}') => mods.super_key = true,
                _ => break,
            }
            rest = chars.as_str();
        }
        let mut code = None;
        for part in rest.split('+') {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            let lower = part.to_ascii_lowercase();
            match lower.as_str() {
                // The one token that is not a key on any keyboard:
                // whichever modifier this platform puts shortcuts behind.
                "mod" | "cmdorctrl" => {
                    if cfg!(target_os = "macos") {
                        mods.super_key = true;
                    } else {
                        mods.ctrl = true;
                    }
                }
                "cmd" | "command" | "super" | "meta" | "win" => mods.super_key = true,
                "ctrl" | "control" => mods.ctrl = true,
                "alt" | "option" | "opt" => mods.alt = true,
                "shift" => mods.shift = true,
                // The key, and only one of them: `"s+s"` is a typo.
                _ if code.is_some() => return None,
                _ => {
                    code = Some(if part.chars().count() == 1 {
                        KeyCode::Char(part.chars().next().unwrap())
                    } else {
                        KeyCode::from_name(&lower)?
                    });
                }
            }
        }
        let code = code?;
        (code != KeyCode::Unknown).then_some(Accel { code, mods })
    }

    /// How the platform writes it: the macOS glyph run (`⇧⌘S`, in AppKit's
    /// order, no separators) or the spelled form (`Ctrl+Shift+S`).
    pub fn display(&self) -> String {
        let key = key_label(self.code);
        if cfg!(target_os = "macos") {
            let mut out = String::new();
            for (on, glyph) in [
                (self.mods.ctrl, '\u{2303}'),
                (self.mods.alt, '\u{2325}'),
                (self.mods.shift, '\u{21e7}'),
                (self.mods.super_key, '\u{2318}'),
            ] {
                if on {
                    out.push(glyph);
                }
            }
            out.push_str(&key);
            out
        } else {
            let mut parts = Vec::new();
            for (on, name) in [
                (self.mods.ctrl, "Ctrl"),
                (self.mods.super_key, "Super"),
                (self.mods.alt, "Alt"),
                (self.mods.shift, "Shift"),
            ] {
                if on {
                    parts.push(name);
                }
            }
            parts.push(&key);
            parts.join("+")
        }
    }

    /// The key equivalent an `NSMenuItem` takes: the character, lowercased
    /// (AppKit reads an uppercase one as Shift being held), or `None` for a
    /// key AppKit spells with a function-key code this does not carry.
    pub fn key_equivalent(&self) -> Option<String> {
        match self.code {
            crate::input::KeyCode::Char(c) => Some(c.to_lowercase().to_string()),
            crate::input::KeyCode::Space => Some(" ".into()),
            crate::input::KeyCode::Enter => Some("\r".into()),
            crate::input::KeyCode::Tab => Some("\t".into()),
            crate::input::KeyCode::Backspace => Some("\u{8}".into()),
            crate::input::KeyCode::Delete => Some("\u{7f}".into()),
            crate::input::KeyCode::Escape => Some("\u{1b}".into()),
            _ => None,
        }
    }
}

/// What a menu writes a key as: the macOS glyph a user reads a shortcut by
/// (`⇧`, `⌫`, `↩`), and the spelled word everywhere else. A key name is
/// wire vocabulary (`"pageup"`); this is the label beside a row.
fn key_label(code: crate::input::KeyCode) -> String {
    use crate::input::KeyCode;
    let mac = cfg!(target_os = "macos");
    match code {
        KeyCode::Char(c) => return c.to_uppercase().to_string(),
        KeyCode::F(n) => return format!("F{n}"),
        _ => {}
    }
    let s = match (code, mac) {
        (KeyCode::Left, true) => "\u{2190}",
        (KeyCode::Right, true) => "\u{2192}",
        (KeyCode::Up, true) => "\u{2191}",
        (KeyCode::Down, true) => "\u{2193}",
        (KeyCode::Home, true) => "\u{2196}",
        (KeyCode::End, true) => "\u{2198}",
        (KeyCode::PageUp, true) => "\u{21de}",
        (KeyCode::PageDown, true) => "\u{21df}",
        (KeyCode::Backspace, true) => "\u{232b}",
        (KeyCode::Delete, true) => "\u{2326}",
        (KeyCode::Enter, true) => "\u{21a9}",
        (KeyCode::Tab, true) => "\u{21e5}",
        (KeyCode::Escape, true) => "\u{238b}",
        (KeyCode::Space, true) => "\u{2423}",
        (KeyCode::Left, _) => "Left",
        (KeyCode::Right, _) => "Right",
        (KeyCode::Up, _) => "Up",
        (KeyCode::Down, _) => "Down",
        (KeyCode::Home, _) => "Home",
        (KeyCode::End, _) => "End",
        (KeyCode::PageUp, _) => "Page Up",
        (KeyCode::PageDown, _) => "Page Down",
        (KeyCode::Backspace, _) => "Backspace",
        (KeyCode::Delete, _) => "Delete",
        (KeyCode::Enter, _) => "Enter",
        (KeyCode::Tab, _) => "Tab",
        (KeyCode::Escape, _) => "Esc",
        (KeyCode::Space, _) => "Space",
        (KeyCode::Insert, _) => "Insert",
        // Neither reachable from a parsed accelerator nor worth a lie.
        (KeyCode::Unknown, _) | (KeyCode::Char(_), _) | (KeyCode::F(_), _) => "",
    };
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyMods};

    /// `ALL` is what C indexes, what Node's generated `MenuItemRole` is
    /// spelled from and what `from_name` searches, so a variant it lacks
    /// is one no binding can say. The match is exhaustive on purpose: a
    /// variant added to the enum fails to compile here until it is placed
    /// in `ALL` too.
    #[test]
    fn all_names_every_role_once() {
        let mut seen = 0;
        for role in MenuRole::ALL {
            match role {
                MenuRole::Custom
                | MenuRole::Separator
                | MenuRole::Cut
                | MenuRole::Copy
                | MenuRole::Paste
                | MenuRole::SelectAll
                | MenuRole::LookUp => seen += 1,
            }
            assert_eq!(MenuRole::from_name(role.name()), Some(role));
            assert_eq!(
                MenuRole::ALL.iter().filter(|r| **r == role).count(),
                1,
                "{role:?} is listed more than once"
            );
        }
        assert_eq!(seen, MenuRole::ALL.len());
    }

    #[test]
    fn mod_is_the_platform_primary() {
        let a = Accel::parse("mod+s").unwrap();
        assert_eq!(a.mods.super_key, cfg!(target_os = "macos"));
        assert_eq!(a.mods.ctrl, !cfg!(target_os = "macos"));
        assert_eq!(a.code, KeyCode::Char('s'));
    }

    #[test]
    fn the_glyph_spelling_parses_back() {
        let a = Accel::parse("\u{21e7}\u{2318}S").unwrap();
        assert_eq!(
            a,
            Accel {
                code: KeyCode::Char('S'),
                mods: KeyMods {
                    shift: true,
                    super_key: true,
                    ..KeyMods::default()
                },
            }
        );
    }

    #[test]
    fn a_named_key_parses_and_reads_back_capitalised() {
        let a = Accel::parse("ctrl+pageup").unwrap();
        assert_eq!(a.code, KeyCode::PageUp);
        let expect = if cfg!(target_os = "macos") {
            "\u{21de}"
        } else {
            "Page Up"
        };
        assert!(a.display().ends_with(expect), "{}", a.display());
    }

    #[test]
    fn a_shortcut_kui_cannot_name_is_not_a_shortcut() {
        assert!(Accel::parse("mod+nope").is_none());
        assert!(Accel::parse("s+s").is_none());
        assert!(Accel::parse("").is_none());
    }

    #[test]
    fn a_key_equivalent_is_lowercase() {
        // AppKit reads an uppercase key equivalent as Shift being held, so
        // ⌘S must be sent as "s" with the Command flag and nothing else.
        assert_eq!(
            Accel::parse("cmd+S").unwrap().key_equivalent().as_deref(),
            Some("s")
        );
    }
}
