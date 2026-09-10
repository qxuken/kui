//! `Core`'s menu surface: opening one, drawing the stock one into the
//! frame, and consuming the events its own rows produce
//! (`docs/adr/0017-selection-as-a-scope.md`, decision 5).
//!
//! The menu the core opens is drawn by [`crate::widgets::context_menu`] —
//! the same public widget an app calls — through the moment `Ui::finish`
//! already reserves for content the view did not build (ADR 0014's `"root"`
//! fill). So there is no overlay layer here, no window of the core's own,
//! and nothing the conformance corpus cannot see: the menu is ordinary
//! nodes in an ordinary frame.
//!
//! Its rows post ordinary click events too, which is why this module
//! *takes them back*. A row the core built is found by key — recorded
//! while building, never guessed at from a payload — and the events those
//! keys produce never reach the app; what reaches the app is the item's
//! own payload, on the node the menu was opened over.

use crate::geom::Vec2;
/// The longest selection a definition panel is offered for. A word, a
/// term, a short phrase — past this it is a passage, and a dictionary has
/// nothing to say about a passage.
pub const LOOKUP_MAX: usize = 100;

use crate::input::UiEvent;
use crate::key::Key;
use crate::menu::{Menu, MenuAction, MenuItem, MenuRole};
use crate::runtime::Core;
use crate::ui::Ui;
use crate::value::Value;
use crate::window::WindowId;

impl Core {
    /// The menu this window has open, if any.
    pub fn menu(&self) -> Option<&Menu> {
        self.menu.as_ref()
    }

    /// Tells the core that this host shows menus itself — an `NSMenu`, a
    /// `TrackPopupMenu`, whatever the platform has (ADR 0017, decision 5,
    /// step 3). The core then keeps the open menu as state and **does not
    /// draw it**: the host reads `menu()`, shows it, and reports back with
    /// [`Self::activate_menu_item`] or [`Self::close_menu`].
    ///
    /// Declared once by the driver, not per menu, because it is a fact
    /// about the host and not about any one menu. Off by default: a host
    /// that says nothing gets the drawn menu, which is every binding's
    /// starting point and the only thing a headless test can see.
    pub fn set_native_menus(&mut self, on: bool) {
        self.native_menus = on;
    }

    /// Tells the core that this host can show the platform's definition
    /// panel — macOS's Look Up. The standard Look Up row is then offered
    /// where it means something, and a force click over text asks for
    /// one; without it the core neither offers nor asks, because an item
    /// that does nothing is worse than an item that is not there.
    pub fn set_lookup_available(&mut self, on: bool) {
        self.lookup_available = on;
    }

    /// Whether the host said it can show a definition panel.
    pub fn lookup_available(&self) -> bool {
        self.lookup_available
    }

    /// Whether the host said it shows menus itself.
    pub fn native_menus(&self) -> bool {
        self.native_menus
    }

    /// The host's menu reports that item `i` was chosen: the same path a
    /// press on the drawn menu's row takes — the core performs what it
    /// can, queues what the host must do, and returns the events the app
    /// hears (one `menu` event on the node the menu was about).
    ///
    /// An index past the end closes the menu and posts nothing, which is
    /// what a host reporting a row this build does not know should do.
    pub fn activate_menu_item(&mut self, i: usize) -> Vec<UiEvent> {
        let mut out = Vec::new();
        if self.menu.as_ref().is_some_and(|m| i < m.items.len()) {
            self.choose_menu_item(i, &mut out);
        } else {
            self.close_menu();
        }
        out
    }

    /// What the selection would be looked *up* as, or `None` when it is
    /// not something to look up.
    ///
    /// A definition panel answers a word or a short phrase. Handed a
    /// paragraph it draws the whole thing back over the page as one
    /// enormous highlighted strip and then says "No Results Found" — seen
    /// in the field, 2026-09-09 — so a selection that spans lines, or runs
    /// past [`LOOKUP_MAX`] characters, is neither offered nor asked about.
    /// A force click always passes: it selects one word.
    pub fn lookup_text(&self) -> Option<String> {
        let text = self.copy_selection().filter(|t| !t.trim().is_empty())?;
        let one_line = !text.contains('\n');
        (one_line && text.chars().count() <= LOOKUP_MAX).then_some(text)
    }

    /// A definition-panel request for whatever is selected: the text, and
    /// the baseline origin of its first line to anchor the panel at.
    pub(crate) fn lookup_action(&self) -> Option<MenuAction> {
        if !self.lookup_available {
            return None;
        }
        let text = self.lookup_text()?;
        let at = self.selection_anchor()?;
        Some(MenuAction::LookUp { text, at })
    }

    /// Whether `key` is a node of the stock menu — its root or one of its
    /// rows. What tells a press inside the core's own menu from a press in
    /// the app.
    pub(crate) fn in_menu(&self, key: Key) -> bool {
        Some(key) == self.menu_root || self.menu_items.contains(&key)
    }

    /// Opens one. A second replaces the first: a window has one menu, the
    /// way it has one selection and one focus.
    ///
    /// Nothing is drawn here. The next frame draws it, because the menu is
    /// state and the frame is a function of state — which is also what
    /// makes an app that never pumps another frame after a right-click a
    /// bug the app can see rather than a menu that appears out of turn.
    pub fn open_menu(&mut self, mut menu: Menu) {
        // Whose menu it is, unless the caller said: the node it is about.
        // An extension that opens a menu over its own node hears the rows
        // come back, the way it hears every other event it declared (ADR
        // 0014 decision 6).
        //
        // The node and not `self.origin()`, which is the *builder's*: it
        // is right during a view and stale afterwards — whatever ran last
        // in the frame that finished, an extension in any app that loads
        // one — so a host opening a menu from its own event handler would
        // have the rows answered to somebody else.
        if menu.origin == crate::tree::OriginId::HOST {
            menu.origin = self
                .tree
                .keys
                .iter()
                .position(|k| *k == menu.target)
                .map_or_else(|| self.origin(), |i| self.tree.origins[i]);
        }
        // Which editor the menu is about, remembered now rather than
        // looked up when a row is chosen: the menu's rows are focusable
        // (they have to be — the arrow keys are the composite's), so by
        // the time Copy runs, focus has moved off the field the user
        // right-clicked. The window's *selection* survives the press
        // (see `in_menu`), but an editor's lives with its focus.
        self.menu_editor = self.edit.focused();
        self.menu = Some(menu);
        self.menu_items.clear();
    }

    /// Closes it. Returns whether one was open.
    pub fn close_menu(&mut self) -> bool {
        self.menu_items.clear();
        self.menu.take().is_some()
    }

    /// What choosing an item left for the host: clipboard work, which is
    /// the host's in this library. Drained like the window and audio
    /// commands, and empty on every frame of an app whose menus are all
    /// its own.
    pub fn take_menu_actions(&mut self) -> Vec<MenuAction> {
        std::mem::take(&mut self.menu_actions)
    }

    /// Draws the open menu into the frame being built, if there is one.
    /// Called by `Ui::finish` after the extensions have filled their
    /// slots, so the menu is the last thing declared and therefore the
    /// frame's modal scope and its topmost float.
    pub(crate) fn build_menu(ui: &mut Ui<'_>) {
        if ui.core().native_menus {
            // The host is showing it. Nothing is drawn, and the rows the
            // stock renderer would have keyed are not there to be clicked
            // — which is why `menu_items` stays empty and
            // `consume_menu_events` finds nothing to take back.
            return;
        }
        let Some(menu) = ui.core().menu.clone() else {
            return;
        };
        let nodes = crate::widgets::context_menu(ui, menu.at, &menu.items);
        // The keys the rows actually took, reported by the widget rather
        // than recomputed here: a key is a hash of a path, and a second
        // derivation of one is a second thing to keep in step.
        let core = ui.core();
        core.menu_root = Some(nodes.root);
        core.menu_items = nodes.rows;
    }

    /// The stock items for a right-click on `region`, or none when there
    /// is nothing standard to offer there. An editor gets the four every
    /// platform's field has; a selection scope gets the two that mean
    /// anything without a text model behind them — nothing owns the text
    /// under a label, so it cannot be cut into or pasted over.
    ///
    /// Every item is always present and only its *enabling* moves: a menu
    /// whose rows shuffle depending on what happens to be possible is one
    /// nobody can use without reading it every time.
    pub(crate) fn default_menu_items(
        &self,
        editor: Option<Key>,
        scope: Option<Key>,
    ) -> Vec<MenuItem> {
        if let Some(key) = editor {
            let has = self.edit.copy_selection(key).is_some_and(|t| !t.is_empty());
            return vec![
                MenuItem::role(MenuRole::Cut).enabled(has),
                MenuItem::role(MenuRole::Copy).enabled(has),
                // The core cannot see a clipboard, so it cannot know
                // whether there is anything to paste; the host that can
                // will say so when it renders this natively (step 3).
                MenuItem::role(MenuRole::Paste),
                MenuItem::separator(),
                MenuItem::role(MenuRole::SelectAll),
            ];
        }
        if scope.is_some() {
            // `copy_selection`, not `selection_text`: a `cells` grid is a
            // scope too, and its selection is in cells rather than in
            // runs — reading only the text one left Copy dimmed over a
            // terminal with half its screen selected.
            let has = self.copy_selection().is_some_and(|t| !t.is_empty());
            let mut items = vec![
                MenuItem::role(MenuRole::Copy).enabled(has),
                MenuItem::role(MenuRole::SelectAll),
            ];
            // Only where the host can show one, and only with something
            // to look up: the row is the platform's, and a dead one would
            // be a promise this library cannot keep.
            if self.lookup_available {
                // Enabled only for something a dictionary can answer —
                // dimmed for a passage, and dimmed rather than missing, so
                // the rows a reader reaches for stay where they were.
                let can = self.lookup_text().is_some();
                items.insert(0, MenuItem::role(MenuRole::LookUp).enabled(has && can));
                items.insert(1, MenuItem::separator());
            }
            return items;
        }
        Vec::new()
    }

    /// A secondary press the app did not claim: opens the stock menu when
    /// the press landed on something with standard items, and does nothing
    /// at all otherwise — a right-click on a plain box has never opened a
    /// menu and does not start now.
    ///
    /// `claimed` is whether the node under the pointer declared
    /// `onContextMenu`. That declaration wins: the app asked to own the
    /// menu there, and one of the two has to win by declaration rather
    /// than by luck (ADR 0017, decision 5).
    pub(crate) fn auto_menu(&mut self, at: Vec2, claimed: bool) {
        if claimed {
            return;
        }
        let Some(region) = self.interaction.hit_at(at) else {
            return;
        };
        let (key, origin) = (region.key, region.origin);
        let editor = region.edit_origin.map(|_| key);
        let scope = region.select_scope;
        let items = self.default_menu_items(editor, scope);
        if items.is_empty() {
            return;
        }
        let target = editor.or(scope).unwrap_or(key);
        self.open_menu(Menu::new(target, at, items).origin(origin));
        // The press told us which editor this is about, which is better
        // than what held focus: a right-click moves no focus (it must
        // leave a selection alone), so the field under the pointer is not
        // necessarily the focused one.
        self.menu_editor = editor;
    }

    /// Filters the events one input produced: anything belonging to the
    /// core's own menu is consumed and acted on, and what the app hears
    /// instead is the item's payload on the node the menu was about.
    ///
    /// Runs on the way out of `handle_input`, so a menu row cannot leak a
    /// click into an app that never declared one.
    pub(crate) fn consume_menu_events(&mut self, out: &mut Vec<UiEvent>) {
        if self.menu.is_none() {
            return;
        }
        let root = self.menu_root;
        let mut chosen: Option<usize> = None;
        let mut dismissed = false;
        out.retain(|ev| {
            if Some(ev.key) == root {
                // The modal's own dismissal (a press outside, Escape).
                dismissed |= ev.payload.get("kind").and_then(Value::as_str) == Some("dismiss");
                return false;
            }
            match self.menu_items.iter().position(|k| *k == ev.key) {
                Some(i) => {
                    chosen = Some(i);
                    false
                }
                None => true,
            }
        });
        if let Some(i) = chosen {
            self.choose_menu_item(i, out);
        } else if dismissed {
            self.close_menu();
        }
    }

    /// Performs one item and closes the menu. A standard role the core can
    /// finish it finishes; the clipboard three become a [`MenuAction`] for
    /// the host; everything else is the app's, and reaches it as an event
    /// on the node the menu was opened over.
    fn choose_menu_item(&mut self, i: usize, out: &mut Vec<UiEvent>) {
        let Some(menu) = self.menu.clone() else {
            return;
        };
        let Some(item) = menu.items.get(i).cloned() else {
            return;
        };
        self.close_menu();
        self.perform_menu_item(&item, menu.target, menu.origin, out);
    }

    /// One item, performed and posted, wherever it was chosen from: the
    /// open context menu's row, a host's native menu, or a menu of the
    /// application menu bar (`docs/adr/0018-a-menu-bar-the-app-declares.md`,
    /// decision 3). The two callers differ only in what they close first
    /// and what node the event lands on, so everything after that is here
    /// and cannot drift between them.
    ///
    /// `target` is the node the event is posted on and `origin` who hears
    /// it; the standard roles act on [`Core::menu_editor`], which each
    /// caller sets when its menu opens — an editor's selection lives with
    /// its focus, and a menu's rows take that focus.
    pub(crate) fn perform_menu_item(
        &mut self,
        item: &MenuItem,
        target: Key,
        origin: crate::tree::OriginId,
        out: &mut Vec<UiEvent>,
    ) {
        match item.role {
            MenuRole::Separator => return,
            MenuRole::SelectAll => {
                // The one standard item that needs nobody: an editor
                // selects its own text, a scope selects its runs.
                match self.menu_editor {
                    Some(key) => {
                        self.set_focus(Some(key));
                        // The editor directly, not back through
                        // `handle_input`: this runs *inside* one already,
                        // and the events the nested call returned were
                        // dropped on the floor.
                        self.edit_with_fonts(|edit, fs| {
                            edit.apply_key(
                                key,
                                crate::input::EditKey::SelectAll,
                                crate::input::Mods::default(),
                                fs,
                            )
                        });
                    }
                    None => {
                        let scope = self.selection().map_or(target, |s| s.scope);
                        self.select_all_in(scope);
                    }
                }
            }
            MenuRole::Copy => {
                // `copy_selection` again, for the reason it is used to
                // enable the row: a `cells` grid's selection is in cells,
                // and reading only the text one left Copy lit over a
                // terminal and then copied nothing when it was chosen.
                let text = match self.menu_editor {
                    Some(key) => self.edit.copy_selection(key),
                    None => self.copy_selection().filter(|t| !t.is_empty()),
                };
                if let Some(text) = text {
                    let html = self.selection_html();
                    self.menu_actions
                        .push(MenuAction::SetClipboard { text, html });
                }
            }
            MenuRole::Cut => {
                // Only an editor can be cut from: nothing owns the text
                // behind a static selection, so there is nothing to take
                // it out of. The item is simply not offered there.
                if let Some(key) = self.menu_editor
                    && let Some(text) = self.edit.copy_selection(key)
                {
                    self.set_focus(Some(key));
                    self.edit_with_fonts(|edit, fs| edit.delete_selection(key, fs));
                    // No `html`: an editor's text is one style, and what
                    // was cut is gone anyway.
                    self.menu_actions
                        .push(MenuAction::SetClipboard { text, html: None });
                }
            }
            MenuRole::Paste => self.menu_actions.push(MenuAction::Paste),
            MenuRole::LookUp => {
                if let Some(action) = self.lookup_action() {
                    self.menu_actions.push(action);
                }
            }
            MenuRole::Custom => {}
        }
        // Every chosen item posts, including the standard ones: an app
        // that wants to know its editor was cut from does not have to
        // guess, and one that does not simply ignores the event.
        let payload = item.id.clone().unwrap_or_else(|| Value::str(item.text()));
        out.push(UiEvent {
            origin,
            window: WindowId::MAIN,
            key: target,
            payload: Value::map([
                ("kind", Value::str("menu")),
                ("role", Value::str(item.role.name())),
                ("item", payload),
            ]),
        });
    }
}
