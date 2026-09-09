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
        // Whoever is building right now owns it, unless the caller said
        // otherwise: an extension that opens a menu hears its rows come
        // back, the way it hears every other event it declared (ADR 0014
        // decision 6). A host calling this outside a frame is `HOST`,
        // which is what the origin already is there.
        if menu.origin == crate::tree::OriginId::HOST {
            menu.origin = self.origin();
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
            let has = self.selection_text().is_some_and(|t| !t.is_empty());
            return vec![
                MenuItem::role(MenuRole::Copy).enabled(has),
                MenuItem::role(MenuRole::SelectAll),
            ];
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
        match item.role {
            MenuRole::Separator => return,
            MenuRole::SelectAll => {
                // The one standard item that needs nobody: an editor
                // selects its own text, a scope selects its runs.
                match self.menu_editor {
                    Some(key) => {
                        self.set_focus(Some(key));
                        self.handle_input(crate::input::InputEvent::Key(
                            crate::input::EditKey::SelectAll,
                            crate::input::Mods::default(),
                        ));
                    }
                    None => {
                        let scope = self.selection().map_or(menu.target, |s| s.scope);
                        self.select_all_in(scope);
                    }
                }
            }
            MenuRole::Copy => {
                let text = match self.menu_editor {
                    Some(key) => self.edit.copy_selection(key),
                    None => self.selection_text().filter(|t| !t.is_empty()),
                };
                if let Some(text) = text {
                    self.menu_actions.push(MenuAction::SetClipboard(text));
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
                    self.menu_actions.push(MenuAction::SetClipboard(text));
                }
            }
            MenuRole::Paste => self.menu_actions.push(MenuAction::Paste),
            // Not offered without a host that can show one, so reaching
            // here means a host asked for it: it hears about it the way it
            // hears about a custom item, and answers with its own panel.
            MenuRole::LookUp | MenuRole::Custom => {}
        }
        // Every chosen item posts, including the standard ones: an app
        // that wants to know its editor was cut from does not have to
        // guess, and one that does not simply ignores the event.
        let payload = item.id.clone().unwrap_or_else(|| Value::str(item.text()));
        out.push(UiEvent {
            origin: menu.origin,
            window: WindowId::MAIN,
            key: menu.target,
            payload: Value::map([
                ("kind", Value::str("menu")),
                ("role", Value::str(item.role.name())),
                ("item", payload),
            ]),
        });
    }
}
