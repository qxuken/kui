//! `Core`'s menu surface: opening one, drawing the stock one into the
//! frame, and consuming the events its own rows produce.
//!
//! The menu the core opens is drawn by [`crate::widgets::context_menu`] —
//! the same public widget an app calls — through the moment `Ui::finish`
//! already reserves for content the view did not build (the `"root"`
//! fill). So there is no overlay layer here, no window of the core's own,
//! and nothing the conformance corpus cannot see: the menu is ordinary
//! nodes in an ordinary frame.
//!
//! Its rows post ordinary click events too, which is why this module
//! *takes them back*. A row the core built is found by key — recorded
//! while building, never guessed at from a payload — and the events those
//! keys produce never reach the app; what reaches the app is the item's
//! own payload, on the node the menu was opened over.

use crate::geom::{Size, Vec2};
/// The longest selection a definition panel is offered for. A word, a
/// term, a short phrase — past this it is a passage, and a dictionary has
/// nothing to say about a passage.
pub const LOOKUP_MAX: usize = 100;

use crate::input::{EventKind, UiEvent};
use crate::key::Key;
use crate::menu::{Menu, MenuAction, MenuItem, MenuRole};
use crate::runtime::Core;
use crate::tree::OriginId;
use crate::ui::Ui;
use crate::value::Value;

impl Core {
    /// The menu this window has open, if any.
    pub fn menu(&self) -> Option<&Menu> {
        self.menu.as_ref()
    }

    /// Tells the core that this host shows menus itself — an `NSMenu`, a
    /// `TrackPopupMenu`, whatever the platform has. The core then keeps the
    /// open menu as state and **does not
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
    /// `None` when nothing was taken: no menu is open, or row `i` cannot
    /// be chosen — a disabled row, a separator — in which case the menu
    /// stays open and nothing is posted, since a native menu never reports
    /// such a row and the drawn one has no click on it, so a door that
    /// names one (`activateMenuItem(1)` over a select whose second option
    /// is disabled) should be answered the way the pointer would be,
    /// rather than handing the app a choice it disabled.
    /// An index past the end closes the menu and posts nothing
    /// (`Some` and empty), which is what a host reporting a row this build
    /// does not know should do.
    pub fn activate_menu_item(&mut self, i: usize) -> Option<Vec<UiEvent>> {
        self.activate_menu_path(&[i])
    }

    /// [`Self::activate_menu_item`] for a row inside a submenu, by its
    /// path: `[2, 0]` is the first row of the third row's submenu
    /// ([`MenuItem::at_path`]) — what a host whose own menu nests them
    /// (an `NSMenu`'s submenus) reports (backlog F128). A row that opens a
    /// submenu is refused like a disabled one: the platform opens it and
    /// never reports it chosen. A path that names no row closes the menu
    /// and posts nothing, as an index past the end does.
    pub fn activate_menu_path(&mut self, path: &[usize]) -> Option<Vec<UiEvent>> {
        let menu = self.menu.as_ref()?;
        let mut out = Vec::new();
        match MenuItem::at_path(&menu.items, path) {
            Some(_) if !MenuItem::choosable_at(&menu.items, path) => return None,
            Some(_) => self.choose_menu_path(path, &mut out),
            None => {
                self.close_menu();
            }
        }
        // The same way out an input's events take: a row of the devtools'
        // own select is the panel's whichever menu showed it.
        self.outbound(&mut out);
        Some(out)
    }

    /// What the selection would be looked *up* as, or `None` when it is
    /// not something to look up.
    ///
    /// A definition panel answers a word or a short phrase. Handed a
    /// paragraph it draws the whole thing back over the page as one
    /// enormous highlighted strip and then says "No Results Found" — seen
    /// in the field, 2026-09-09 — so a selection that spans lines, or runs
    /// past 100 characters, is neither offered nor asked about.
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

    /// Opens one. A second replaces the first: a window has one menu, the
    /// way it has one selection and one focus.
    ///
    /// Nothing is drawn here. The next frame draws it, because the menu is
    /// state and the frame is a function of state — which is also what
    /// makes an app that never pumps another frame after a right-click a
    /// bug the app can see rather than a menu that appears out of turn.
    pub fn open_menu(&mut self, mut menu: Menu) {
        // `at` arrives in the host's coordinates (ADR 0024); the menu is
        // kept in the window's, which the drawn one and the native one
        // both place by.
        menu.at = menu.at.plus(self.dt_shift());
        self.open_menu_raw(menu);
    }

    /// `open_menu` for a point already in window coordinates — the
    /// right-click's own.
    pub(crate) fn open_menu_raw(&mut self, mut menu: Menu) {
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
        // (a press under `OriginId::MENU` is spared), but an editor's
        // lives with its focus.
        self.menu_editor = self.edit.focused();
        self.menu_focus = self.focus;
        // The accelerators in the platform's spelling, as the bar's are
        // (`declare_menu_bar`): a portable `"mod+shift+n"` is drawn `⇧⌘N`
        // or `Ctrl+Shift+N`, and a host that shows the menu itself reads
        // the same string back (backlog F127).
        MenuItem::normalize_accels(&mut menu.items);
        self.menu = Some(menu);
        // A new menu opens with none of its submenus open, drawn here —
        // or, where the host places menus, nowhere until it has said where
        // (`set_menu_popups`).
        self.menu_sub = Submenus::default();
        // A menu that asked to stay in the window is placed here at once.
        let in_window = self.menu.as_ref().is_some_and(|m| m.in_window);
        self.menu_where = if self.menu_popups && !in_window {
            MenuWhere::Pending
        } else {
            MenuWhere::Window
        };
    }

    /// A select field built this frame (`widgets::select`): its key and
    /// the rows its menu will have. Read back by
    /// [`Self::consume_select_events`] when the field is clicked.
    pub(crate) fn declare_select(&mut self, key: Key, items: Vec<MenuItem>) {
        self.selects.push((key, items));
    }

    /// A click on a select field is not the app's: taken back here and
    /// answered with the field's menu, opened under its bottom-left
    /// corner — the core's own menu, so what follows (the rows, a choice,
    /// a dismissal) is `consume_menu_events`' as for any other. The
    /// field's node is the target, so the choice is posted on it.
    pub(crate) fn consume_select_events(&mut self, out: &mut Vec<UiEvent>) {
        if self.selects.is_empty() {
            return;
        }
        let mut clicked: Option<Key> = None;
        out.retain(|ev| {
            let is = ev.payload.get_bool("select") == Some(true)
                && self.selects.iter().any(|(k, _)| *k == ev.key);
            if is {
                clicked = Some(ev.key);
            }
            !is
        });
        let Some(key) = clicked else {
            return;
        };
        let Some((_, items)) = self.selects.iter().find(|(k, _)| *k == key) else {
            return;
        };
        let items = items.clone();
        // Under the field, in window coordinates: the last frame laid the
        // field out, which is the frame the click was made against.
        let at = self
            .tree
            .keys
            .iter()
            .position(|k| *k == key)
            .map(|i| {
                let (p, s) = (self.tree.pos[i], self.tree.size[i]);
                Vec2::new(p.x, p.y + s.h)
            })
            .unwrap_or_default();
        self.open_menu_raw(Menu::new(key, at, items));
    }

    /// Closes it, and every submenu open in it. Returns whether one was
    /// open.
    pub fn close_menu(&mut self) -> bool {
        self.menu_sub = Submenus::default();
        self.menu_where = MenuWhere::Window;
        self.menu.take().is_some()
    }

    /// The submenus open in the drawn context menu: the row opened at each
    /// level, outermost first — `[2]` is the third row's submenu, `[2, 0]`
    /// that and the submenu of its first row. Empty when none is, and
    /// always while the host shows menus itself (backlog F128).
    pub fn menu_submenus(&self) -> &[usize] {
        &self.menu_sub.open
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
        if ui.core().native_menus || ui.core().menu_where != MenuWhere::Window {
            // The host is showing it, in the platform's menu or in a
            // window of its own, or has yet to say where. Nothing is
            // drawn, so there are no rows to click and
            // `consume_menu_events` finds nothing to take back.
            return;
        }
        let Some(menu) = ui.core().menu.clone() else {
            return;
        };
        // The widget floats against the host's viewport, whose origin is
        // the dock's edge under a left dock (ADR 0024): the window point
        // becomes a host one. Its nodes are opened under `OriginId::MENU`,
        // which is how `consume_menu_events` knows them.
        let at = menu.at.minus(ui.core().dt_shift());
        let max = menu
            .max_size
            .unwrap_or(Size::new(f32::INFINITY, f32::INFINITY));
        crate::widgets::context_menu_within(ui, at, &menu.items, max);
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
    /// than by luck.
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
        self.open_menu_raw(Menu::new(target, at, items).origin(origin));
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
        let taken = Self::take_surface_events(out, OriginId::MENU);
        if let Some(path) = taken.row_path() {
            // A row that opens a submenu opens it — the click, Enter, a
            // reader's press — and stays open; any other row is chosen.
            let opens = self
                .menu
                .as_ref()
                .and_then(|m| MenuItem::at_path(&m.items, &path))
                .is_some_and(MenuItem::has_submenu);
            if opens {
                let keyboard = self.focus_visible;
                self.open_submenu(MenuSurface::Context, path, keyboard);
            } else {
                self.choose_menu_path(&path, out);
            }
        } else if taken.dismissed {
            // In a menu's popup window, Escape is said back to the host:
            // a submenu's goes back to its row, the menu's own closes it.
            if let Some(p) = self.menu_popup.as_mut() {
                p.answer = Some(if p.submenu {
                    MenuPopupAnswer::Back
                } else {
                    MenuPopupAnswer::Dismissed
                });
            }
            self.close_menu();
        }
    }

    /// Takes every event of one of the core's own surfaces out of `out`
    /// and reads what they said: a `dismiss` on the surface's modal root,
    /// a click on the `row`th item (its payload is `{row}`, the message
    /// `widgets::menu_row_tag` gave it, whether the pointer or Enter
    /// clicked it), a click on the `title`th menu of the bar (`{title}`).
    /// The one filter both menus consume through, so what a surface's
    /// nodes post is read in one place. Hover, focus and the rest of a
    /// surface's own events are taken with them: none of it is the app's.
    pub(crate) fn take_surface_events(out: &mut Vec<UiEvent>, origin: OriginId) -> Taken {
        let mut taken = Taken::default();
        let index = |v: &Value, name: &str| v.get(name).and_then(Value::as_int).map(|i| i as usize);
        out.retain(|ev| {
            if ev.origin != origin {
                return true;
            }
            if ev.is(EventKind::Dismiss) {
                taken.dismissed = true;
            } else if let Some(i) = index(&ev.payload, "row") {
                taken.row = Some(i);
                taken.path = row_path_of(&ev.payload);
            } else if let Some(i) = index(&ev.payload, "title") {
                taken.title = Some(i);
            }
            false
        });
        taken
    }

    /// Performs one item and closes the menu. A standard role the core can
    /// finish it finishes; the clipboard three become a [`MenuAction`] for
    /// the host; everything else is the app's, and reaches it as an event
    /// on the node the menu was opened over.
    fn choose_menu_path(&mut self, path: &[usize], out: &mut Vec<UiEvent>) {
        let Some(menu) = self.menu.clone() else {
            return;
        };
        // A menu's popup window performs nothing: the menu is its owner's,
        // which the host hands the row to (`take_menu_popup_answer`).
        if let Some(p) = self.menu_popup.as_mut() {
            p.answer = Some(MenuPopupAnswer::Chose(path.to_vec()));
            self.close_menu();
            return;
        }
        let Some(item) = MenuItem::at_path(&menu.items, path).cloned() else {
            return;
        };
        self.close_menu();
        self.perform_menu_item(&item, menu.target, menu.origin, false, out);
    }

    /// A key press while one of the core's own menus is open (backlog
    /// C98): the press matching an enabled row's accelerator chooses that
    /// row — its event posted as a click on it would post it, the menu
    /// closed — as a native menu does with its items' key equivalents.
    /// Kit's Actions menu open, ⌘C on its Copy link row did nothing and
    /// the menu stayed. True when a row took the press, which then goes
    /// nowhere else.
    ///
    /// The context menu's rows first, then the drawn bar's open menu; a
    /// menu the host shows itself has its own key equivalents, and a row
    /// that opens a submenu, a dead one or one under a dead row is never
    /// chosen, as the pointer could not choose it.
    pub(crate) fn menu_accel(
        &mut self,
        kp: &crate::input::KeyPress,
        out: &mut Vec<UiEvent>,
    ) -> bool {
        // Only a menu drawn here: one in a popup has the popup's keyboard
        // (the host lends it the keys), and one not placed yet is not up.
        if let Some(menu) = self
            .menu
            .as_ref()
            .filter(|_| !self.native_menus && self.menu_where == MenuWhere::Window)
            && let Some(path) = accel_path(&menu.items, kp)
        {
            self.choose_menu_path(&path, out);
            return true;
        }
        let Some(open) = self.menu_bar_open.filter(|_| !self.native_menu_bar) else {
            return false;
        };
        let Some(item) = self.menu_bar.as_ref().and_then(|b| {
            let items = &b.menus.get(open).filter(|m| m.enabled)?.items;
            MenuItem::at_path(items, &accel_path(items, kp)?).cloned()
        }) else {
            return false;
        };
        // Closed first, as a press on the row closes it before performing:
        // a row that replays its chord plays it to the window under the
        // menu, not back into this.
        self.set_menu_bar_open(None);
        let (target, origin) = (
            self.menu_bar_root.unwrap_or(Key::ROOT),
            self.menu_bar_origin,
        );
        self.perform_menu_item(&item, target, origin, true, out);
        true
    }

    /// One item, performed and posted, wherever it was chosen from: the
    /// open context menu's row, a host's native menu, or a menu of the
    /// application menu bar. The two callers differ only in what they close first
    /// and what node the event lands on, so everything after that is here
    /// and cannot drift between them.
    ///
    /// `target` is the node the event is posted on and `origin` who hears
    /// it; the standard roles act on [`Core::menu_editor`], which each
    /// caller sets when its menu opens — an editor's selection lives with
    /// its focus, and a menu's rows take that focus. `bar` says the menu
    /// is the application menu bar's, whose choice arrives as the bar's
    /// own `on_menu` message (ADR 0048, decision 3).
    pub(crate) fn perform_menu_item(
        &mut self,
        item: &MenuItem,
        target: Key,
        origin: crate::tree::OriginId,
        bar: bool,
        out: &mut Vec<UiEvent>,
    ) {
        // A row that is its chord plays it instead (backlog F151).
        if let Some(kp) = item.replayed() {
            self.replay_chord(kp, out);
            return;
        }
        if item.role == MenuRole::Separator {
            return;
        }
        self.perform_role(item.role, target, out);
        // Every chosen item posts, including the standard ones, to a node
        // that declared `on_menu` and from a bar that declared one: an app that wants
        // to know its editor was cut from does not have to guess, and one
        // that does not simply ignores the event. A menu opened for a node
        // that declared none — the stock right-click menu, whose roles the
        // core has just performed — posts nothing (ADR 0047, decision 5).
        let picked = item.id.clone().unwrap_or_else(|| Value::str(item.text()));
        let data = Value::map([("role", Value::str(item.role.name())), ("item", picked)]);
        let message = if bar {
            self.menu_bar.as_ref().and_then(|b| b.on_menu.as_ref())
        } else {
            self.tree
                .index_of(target)
                .and_then(|i| self.tree.specs[i].events().on_menu())
        };
        if let Some(message) = message {
            out.push(UiEvent::declared(
                EventKind::Menu,
                origin,
                target,
                Some(message),
                data,
            ));
        }
    }

    /// What a standard role does, against [`Core::menu_editor`] or the
    /// window's selection: everything [`Self::perform_menu_item`] does
    /// but post the `menu` event.
    fn perform_role(&mut self, role: MenuRole, target: Key, out: &mut Vec<UiEvent>) {
        match role {
            MenuRole::Separator => {}
            MenuRole::SelectAll => {
                // The one standard item that needs nobody: an editor
                // selects its own text, a scope selects its runs.
                match self.menu_editor {
                    Some(key) => {
                        self.move_focus(Some(key));
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
                    && let Some(text) = self.cut_editor(key)
                {
                    self.move_focus(Some(key));
                    // No `html`: an editor's text is one style, and what
                    // was cut is gone anyway.
                    self.menu_actions
                        .push(MenuAction::SetClipboard { text, html: None });
                    // And the edit it is: every other mutation posts one
                    // (`apply_text`, `apply_key`, a reader's `setValue`),
                    // so an app mirroring the field hears this one too
                    // (AR15).
                    self.push_edit_event(key, EventKind::Change, out);
                }
            }
            MenuRole::Paste => {
                // The answer is a paste to the focused editor, and a host
                // sends it straight after this input, before a frame has
                // handed focus back from the closing menu (the native
                // runner does). So focus goes back to the field now, as
                // Cut and Select All take it back.
                if let Some(key) = self.menu_editor {
                    self.move_focus(Some(key));
                }
                self.queue_paste();
            }
            MenuRole::LookUp => {
                if let Some(action) = self.lookup_action() {
                    self.menu_actions.push(action);
                }
            }
            MenuRole::Custom => {}
            // A Mac's application performs these where it draws the menu;
            // anywhere else the event the row posts is the app's to act on
            // (backlog F152).
            MenuRole::About
            | MenuRole::Hide
            | MenuRole::HideOthers
            | MenuRole::ShowAll
            | MenuRole::Quit => {}
        }
    }
}

impl Core {
    /// A `replay` row chosen from a menu the core draws, or reported by a
    /// host's: its chord, played where the keyboard was before the menu
    /// took it, as the keyboard would have sent it — the press to the key
    /// sink, the editing key or the text it maps to, the release — and
    /// for the clipboard and undo chords a driver performs itself
    /// (`Shell::edit_chord`), the same thing done here, against the editor
    /// or the selection: the core has no driver to ask (backlog F151).
    pub(crate) fn replay_chord(&mut self, kp: crate::input::KeyPress, out: &mut Vec<UiEvent>) {
        use crate::input::{EditKey, InputEvent, KeyCode, Mods};
        // The menu that was chosen from is still the last frame's modal
        // scope, and everything outside it inert, until the next frame
        // lays the window out without it. The chord is for the window
        // under it, so it is routed as that frame will see it: under the
        // app's own modal if there is one, else under none.
        if let Some((i, ..)) = self.modal
            && matches!(self.tree.origins[i], OriginId::MENU | OriginId::MENU_BAR)
        {
            self.modal = (0..i)
                .rev()
                .find(|&j| {
                    self.tree.specs[j].events().modal().is_some()
                        && !self.tree.origins[j].is_core_surface()
                })
                .map(|j| (j, self.tree.subtree_end(j), self.tree.keys[j]));
        }
        if let Some(k) = self.menu_focus.take()
            && self.tree.keys.contains(&k)
        {
            self.move_focus(Some(k));
        }
        out.extend(self.route_input(InputEvent::KeyDown(kp.clone())));
        let letter = match kp.code {
            KeyCode::Char(c) if kp.mods.primary() => Some(c.to_ascii_lowercase()),
            _ => None,
        };
        let editor = self.edit.focused();
        let scope = self.selection().is_some() || self.cell_selection().is_some();
        let role = match letter {
            _ if editor.is_none() && !scope => None,
            Some('c') => Some(MenuRole::Copy),
            Some('x') => Some(MenuRole::Cut),
            Some('v') => Some(MenuRole::Paste),
            Some('a') => Some(MenuRole::SelectAll),
            _ => None,
        };
        let history = match letter {
            Some('z') if kp.mods.shift => Some(EditKey::Redo),
            Some('z') => Some(EditKey::Undo),
            Some('y') => Some(EditKey::Redo),
            _ => None,
        };
        if let Some(role) = role {
            self.menu_editor = editor;
            let target = editor.unwrap_or(Key::ROOT);
            self.perform_role(role, target, out);
        } else if let (Some(ek), Some(_)) = (history, editor) {
            out.extend(self.route_input(InputEvent::Key(ek, Mods::default())));
        } else if let Some(ev) = kp.edit_event() {
            out.extend(self.route_input(ev));
        }
        out.extend(self.route_input(InputEvent::KeyUp(kp.released())));
    }
}

/// What one input said to one of the core's surfaces (`Core::take_surface_events`).
#[derive(Default)]
pub(crate) struct Taken {
    pub dismissed: bool,
    pub row: Option<usize>,
    /// The rows opened on the way to `row`, for a row of a submenu.
    pub path: Vec<usize>,
    pub title: Option<usize>,
}

impl Taken {
    /// The clicked row's whole path, `path` then `row`.
    pub fn row_path(&self) -> Option<Vec<usize>> {
        let row = self.row?;
        Some(self.path.iter().copied().chain([row]).collect())
    }
}

/// The path of the first row in `items`, submenus included, that the press
/// `kp` is the accelerator of and that can be chosen
/// ([`MenuItem::choosable_at`]): the row's own accelerator or its role's,
/// in any spelling [`crate::menu::Accel::parse`] reads. A row whose
/// accelerator kui cannot parse is the app's to match, and is passed.
fn accel_path(items: &[MenuItem], kp: &crate::input::KeyPress) -> Option<Vec<usize>> {
    fn walk(
        all: &[MenuItem],
        level: &[MenuItem],
        path: &mut Vec<usize>,
        kp: &crate::input::KeyPress,
    ) -> bool {
        for (i, item) in level.iter().enumerate() {
            path.push(i);
            let hit = item
                .accel_text()
                .and_then(crate::menu::Accel::parse)
                .is_some_and(|a| a.matches(kp))
                && MenuItem::choosable_at(all, path);
            if hit || (item.selectable() && walk(all, &item.submenu, path, kp)) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut path = Vec::new();
    walk(items, items, &mut path, kp).then_some(path)
}

/// The `path` a submenu row's tag carries (`widgets::menu_row_tag`);
/// empty for a top-level row, which carries none.
fn row_path_of(payload: &Value) -> Vec<usize> {
    match payload.get("path") {
        Some(Value::List(ps)) => ps
            .iter()
            .filter_map(Value::as_int)
            .map(|p| p as usize)
            .collect(),
        _ => Vec::new(),
    }
}

// -- Submenus (backlog F128) --------------------------------------------------
// A row with a submenu opens it beside itself; the rows inside are drawn by
// the same `menu_panel`, under the same origin, so their clicks come back
// through `take_surface_events` like any row's, carrying the path that
// says which submenu they are in. What is open is the core's, per drawn
// menu: the frame cannot derive which row the pointer last rested on, or
// that the keyboard closed what the pointer opened.

/// Which of the core's two drawn menus a row belongs to, told by the
/// origin its nodes were opened under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuSurface {
    /// The context menu (`open_menu`), and a select's list.
    Context,
    /// The drawn menu bar's open menu.
    Bar,
}

impl MenuSurface {
    pub(crate) fn of(origin: OriginId) -> Option<Self> {
        match origin {
            OriginId::MENU => Some(MenuSurface::Context),
            OriginId::MENU_BAR => Some(MenuSurface::Bar),
            _ => None,
        }
    }

    fn origin(self) -> OriginId {
        match self {
            MenuSurface::Context => OriginId::MENU,
            MenuSurface::Bar => OriginId::MENU_BAR,
        }
    }
}

/// The submenus open in one of the core's drawn menus.
#[derive(Clone, Debug, Default)]
pub(crate) struct Submenus {
    /// The row opened at each level, outermost first: `[2, 0]` is the
    /// third row's submenu and, inside it, its first row's. Empty: none.
    pub open: Vec<usize>,
    /// The row the pointer was last seen on, as a path. The pointer opens
    /// and closes submenus when this *changes*, so a keyboard that moved on
    /// is not undone by a pointer resting where it was.
    pub hovered: Option<Vec<usize>>,
    /// The keyboard opened the innermost submenu: its first row takes
    /// focus on the frame that draws it (`submenu_drawn`).
    pub focus_first: bool,
    /// A switch away from an open submenu the pointer asked for, waiting
    /// [`SUBMENU_SWITCH_DELAY`] in case it is only passing over the row on
    /// its way into that submenu (backlog RG150).
    pub pending: Option<PendingSwitch>,
    /// Some row of this menu was under the pointer during the frame's
    /// build: a pass that sees none forgets `hovered`, so coming back to
    /// a row the keyboard closed opens it again.
    pub seen: bool,
    /// The menu was built this frame (`submenu_pass` ran). A frame that
    /// builds no menu on this surface — an app that stopped drawing the
    /// bar, or handed it to the platform — has no rows for a switch to
    /// ripen on, so a pending one is dropped at the frame's end rather
    /// than owing frames for good (backlog RG154).
    pub built: bool,
}

/// A row the pointer moved onto while a submenu beside another was open.
#[derive(Clone, Debug)]
pub(crate) struct PendingSwitch {
    pub row: Vec<usize>,
    pub opens: bool,
    /// The frame clock's reading when the pointer arrived.
    pub since: f64,
}

/// How long the pointer rests on another row before an open submenu gives
/// way to it. Travelling diagonally from a row to a lower row of its
/// submenu crosses the rows below it; without the wait each one closed
/// the submenu the pointer was heading for.
pub(crate) const SUBMENU_SWITCH_DELAY: f64 = 0.3;

impl Submenus {
    /// What the pointer on `row` means for what is open: a row with a
    /// submenu opens it, closing whatever was open beside it; any other
    /// row closes the submenus below its own level.
    fn switch_to(&mut self, row: &[usize], opens: bool) {
        let level = row.len() - 1;
        if opens {
            // Kept when it is already the open one, deeper submenus and all:
            // the pointer coming back to its row from inside it.
            if !self.open.starts_with(row) {
                self.open = row.to_vec();
            }
        } else if self.open.len() > level && self.open[..level] == row[..level] {
            self.open.truncate(level);
        }
    }
}

impl Core {
    fn submenus(&mut self, s: MenuSurface) -> &mut Submenus {
        match s {
            MenuSurface::Context => &mut self.menu_sub,
            MenuSurface::Bar => &mut self.menu_bar_sub,
        }
    }

    /// The rows of the menu `s` has open: the context menu's, or the bar's
    /// open menu's.
    fn surface_items(&self, s: MenuSurface) -> Option<&[MenuItem]> {
        match s {
            MenuSurface::Context => self.menu.as_ref().map(|m| m.items.as_slice()),
            MenuSurface::Bar => {
                let open = self.menu_bar_open?;
                Some(self.menu_bar.as_ref()?.menus.get(open)?.items.as_slice())
            }
        }
    }

    /// Opens the submenu of the row at `path` — and with it the ones on the
    /// way, closing any other — the keyboard's way when `keyboard`, which
    /// moves focus into it once it is drawn.
    pub(crate) fn open_submenu(&mut self, s: MenuSurface, path: Vec<usize>, keyboard: bool) {
        let sub = self.submenus(s);
        sub.open = path;
        sub.focus_first = keyboard;
        sub.pending = None;
    }

    /// The row in the submenu at `path` that is open, if one is: what the
    /// panel at that level draws its submenu beside.
    pub(crate) fn submenu_open_at(&mut self, s: MenuSurface, path: &[usize]) -> Option<usize> {
        let open = &self.submenus(s).open;
        (open.len() > path.len() && open[..path.len()] == *path).then(|| open[path.len()])
    }

    /// The pointer is on the row at `row` (its whole path) this frame. On a
    /// change of row: a row with a submenu opens it, closing whatever was
    /// open beside it, and any other row closes the submenus below its own
    /// level — the way every platform's menus follow the pointer.
    ///
    /// A row that would close a submenu open beside another — a sibling of
    /// the row that opened it, or a row further out — waits
    /// [`SUBMENU_SWITCH_DELAY`] on the frame clock first, and moving into
    /// that submenu meanwhile cancels it. A driver that sets no clock
    /// switches at once, as its transitions snap.
    pub(crate) fn submenu_hovered(&mut self, s: MenuSurface, row: &[usize], opens: bool) {
        let now = self.anim.time();
        let sub = self.submenus(s);
        sub.seen = true;
        if sub.hovered.as_deref() == Some(row) {
            // Still resting there: a switch it is waiting on ripens.
            if let Some(p) = &sub.pending
                && p.row == row
                && now.is_none_or(|t| t - p.since >= SUBMENU_SWITCH_DELAY)
            {
                let p = sub.pending.take().unwrap();
                sub.switch_to(&p.row, p.opens);
            }
            return;
        }
        sub.hovered = Some(row.to_vec());
        sub.focus_first = false;
        let level = row.len() - 1;
        let away = sub.open.len() > level && !sub.open.starts_with(row);
        match now {
            Some(since) if away => {
                sub.pending = Some(PendingSwitch {
                    row: row.to_vec(),
                    opens,
                    since,
                });
            }
            _ => {
                sub.pending = None;
                sub.switch_to(row, opens);
            }
        }
    }

    /// The build of the menu on `s` begins (`begin`) or ends: a build in
    /// which the pointer was on none of its rows forgets the row it was
    /// last on, and any switch waiting on it.
    pub(crate) fn submenu_pass(&mut self, s: MenuSurface, begin: bool) {
        let sub = self.submenus(s);
        if begin {
            sub.seen = false;
            sub.built = true;
        } else if !sub.seen {
            sub.hovered = None;
            sub.pending = None;
        }
    }

    /// The frame is over: a switch waiting on a menu the frame did not
    /// build has no rows to ripen on and is dropped, so the frames it
    /// owed are not owed by a bar the app stopped drawing (backlog
    /// RG154). The context menu is built by every `Ui::finish` that has
    /// one open; the bar only where `widgets::menu_bar` is called.
    pub(crate) fn submenu_frame_end(&mut self) {
        for sub in [&mut self.menu_sub, &mut self.menu_bar_sub] {
            if !sub.built {
                sub.pending = None;
            }
            sub.built = false;
        }
    }

    /// Whether a switch is waiting on the clock: the driver owes the
    /// frames that let it ripen.
    pub(crate) fn submenu_waiting(&self) -> bool {
        self.menu_sub.pending.is_some() || self.menu_bar_sub.pending.is_some()
    }

    /// The submenu at `path` was drawn this frame, its first row that can
    /// take focus at `first`: where focus lands when the keyboard opened it.
    pub(crate) fn submenu_drawn(&mut self, s: MenuSurface, path: &[usize], first: Key) {
        let sub = self.submenus(s);
        if sub.focus_first && sub.open == path {
            sub.focus_first = false;
            self.move_focus(Some(first));
            self.focus_visible = true;
        }
    }

    /// The keyboard inside one of the core's drawn menus, where a submenu
    /// changes what a key means: the Right arrow on a row with a submenu
    /// opens it, focus on its first row; the Left arrow inside a submenu
    /// closes it, focus back on its row; Escape closes the innermost open
    /// submenu rather than the whole menu. Returns whether the key was
    /// taken; every other key goes on as before (the arrows walk the rows
    /// of whichever panel holds focus, `composite_step`).
    pub(crate) fn submenu_key(&mut self, ek: crate::input::EditKey) -> bool {
        use crate::input::EditKey;
        if !matches!(ek, EditKey::Left | EditKey::Right | EditKey::Escape) {
            return false;
        }
        // No menu of the core's open, no submenu to work: an editor's
        // arrows skip the walk below, and an app drawing `context_menu`
        // itself — whose hovers still note a submenu — cannot leave one
        // behind for a later Escape to be spent on.
        if self.menu.is_none() && self.menu_bar_open.is_none() {
            return false;
        }
        // The focused row, if it is one of the core's: which menu, and
        // where in it.
        let focused = self.focus.and_then(|k| {
            let i = self.tree.keys.iter().position(|key| *key == k)?;
            let s = MenuSurface::of(self.tree.origins[i])?;
            let tag = self.tree.specs[i].events().on_click()?;
            let row = tag.get("row").and_then(Value::as_int)? as usize;
            let mut path = row_path_of(tag);
            path.push(row);
            Some((s, path))
        });
        match (ek, focused) {
            (EditKey::Right, Some((s, path))) => {
                let opens = self
                    .surface_items(s)
                    .and_then(|items| MenuItem::at_path(items, &path))
                    .is_some_and(|item| item.enabled && item.has_submenu());
                if opens {
                    self.open_submenu(s, path, true);
                }
                opens
            }
            // A row at `[a, b, c]` is in the submenu `[a, b]` opened; what
            // closes is that one, down to `[a]`.
            (EditKey::Left, Some((s, path))) if path.len() > 1 => {
                self.close_submenu(s, path.len() - 2);
                true
            }
            // A submenu shown in a popup window of its own has its rows at
            // the top level here, and the row it hangs off in the window
            // before: going back to it is the host's (`MenuPopupAnswer`).
            (EditKey::Left, Some((MenuSurface::Context, _)))
                if self.menu_popup.as_ref().is_some_and(|p| p.submenu) =>
            {
                if let Some(p) = self.menu_popup.as_mut() {
                    p.answer = Some(MenuPopupAnswer::Back);
                }
                true
            }
            (EditKey::Escape, focused) => {
                // The menu the focus is in, or else whichever has a submenu
                // open: the pointer may have opened one the keyboard is not in.
                let s = focused.map(|(s, _)| s).or_else(|| {
                    [MenuSurface::Context, MenuSurface::Bar]
                        .into_iter()
                        .find(|&s| !self.submenus(s).open.is_empty())
                });
                let Some(s) = s else { return false };
                let depth = self.submenus(s).open.len();
                if depth == 0 {
                    return false;
                }
                self.close_submenu(s, depth - 1);
                true
            }
            _ => false,
        }
    }

    /// Closes the submenus of `s` past the first `keep` — the one the row
    /// at `open[..=keep]` opened and every one inside it — and puts focus
    /// on that row, which is where the closed one hung.
    fn close_submenu(&mut self, s: MenuSurface, keep: usize) {
        let sub = self.submenus(s);
        if sub.open.len() <= keep {
            return;
        }
        let row = sub.open[..=keep].to_vec();
        sub.open.truncate(keep);
        sub.focus_first = false;
        sub.pending = None;
        // The row the closed submenu hangs from, found by the tag its click
        // carries; it is in the last frame's tree, since its submenu was.
        let (parent, i) = row.split_at(keep);
        let origin = s.origin();
        let at = (0..self.tree.len()).find(|&n| {
            self.tree.origins[n] == origin
                && self.tree.specs[n].events().on_click().is_some_and(|tag| {
                    tag.get("row").and_then(Value::as_int) == Some(i[0] as i64)
                        && row_path_of(tag) == parent
                })
        });
        if let Some(n) = at {
            self.land_focus(n);
        }
    }
}

// -- In a popup window (backlog F207, ADR 0004 amended 2026-10-27) ----------
// A host that can open a borderless window anywhere on the display shows a
// context menu that does not fit its window in one: `kui-native` does, on
// Windows, macOS and X11. Two cores take part. The window's own keeps the
// menu as state and draws none of it (`set_menu_in_popup`), as it does for
// a platform's menu; the popup's is a core of its own whose frame is the
// menu and nothing else (`open_menu_popup`), drawn by the same
// `widgets::context_menu`, so its rows, keyboard, scrolling and access
// tree are the in-window menu's. What it does not do is perform a row: the
// row, Escape and the Left arrow are said back to the host
// (`take_menu_popup_answer`), which hands a row to the window's core
// through `activate_menu_path`, as it does a platform menu's. A submenu is
// a further popup beside its row, with a core of its own showing the
// submenu's rows; the row it hangs off is `menu_popup_submenu`.

/// What a core that is a menu's popup window has to say back to the host
/// ([`Core::take_menu_popup_answer`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuPopupAnswer {
    /// The row at this path in the items the popup was opened with was
    /// chosen — the click, Enter, its accelerator. Prefix the path of the
    /// submenu the popup shows to hand it to the menu's own core.
    Chose(Vec<usize>),
    /// Escape or the Left arrow in a submenu's popup: it closes, and the
    /// keyboard goes back to the row it hangs off
    /// ([`Core::close_menu_popup_submenu`] on the popup before it).
    Back,
    /// Escape in the menu's own popup: the menu is dismissed.
    Dismissed,
}

/// The row of a menu's popup whose submenu is open
/// ([`Core::menu_popup_submenu`]): where the host opens the submenu's
/// popup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuPopupSubmenu {
    /// Its index in the items the popup was opened with.
    pub row: usize,
    /// Its box, in this window's logical px, as the last layout placed it.
    pub rect: crate::geom::Rect,
    /// The keyboard opened it (Enter, the Right arrow), so its first row
    /// shows focus; the pointer resting on the row opens one that does not.
    pub keyboard: bool,
}

/// A core that is a menu's popup window: what it was opened with, and
/// what it has said and the host has not taken.
#[derive(Clone, Debug)]
pub(crate) struct MenuPopup {
    room: crate::geom::Size,
    submenu: bool,
    answer: Option<MenuPopupAnswer>,
}

/// Where the open menu is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MenuWhere {
    /// In this window, as a float.
    #[default]
    Window,
    /// In a popup window the host opened for it.
    Popup,
    /// Nowhere yet: the host places menus (`set_menu_popups`) and has not
    /// said where this one goes.
    Pending,
}

impl Core {
    /// Tells the core that the host can show a context menu that does not
    /// fit the window in a popup window of its own, and will say where
    /// each one goes before it is drawn — `kui-native` does, where the
    /// platform lets it place a window. A menu then opens drawn nowhere
    /// ([`Self::menu_awaits_host`]) until the host answers with
    /// [`Self::set_menu_in_popup`]: the frame that opened it does not
    /// show it in the window first and then in the popup.
    ///
    /// Declared once by the driver, as [`Self::set_native_menus`] is. Off
    /// by default, and off in every host that opens no windows: a menu is
    /// drawn in the window the frame it opens.
    pub fn set_menu_popups(&mut self, on: bool) {
        self.menu_popups = on;
        if !on && self.menu_where == MenuWhere::Pending {
            self.menu_where = MenuWhere::Window;
        }
    }

    /// Whether the host said it places menus (`set_menu_popups`).
    pub fn menu_popups(&self) -> bool {
        self.menu_popups
    }

    /// Whether a menu is open that the host has yet to place: drawn
    /// nowhere until [`Self::set_menu_in_popup`] says where.
    pub fn menu_awaits_host(&self) -> bool {
        self.menu.is_some() && self.menu_where == MenuWhere::Pending
    }

    /// The host's answer for the open menu: `true` shows it in a popup
    /// window the host opened for it (`kui-native` does, for a context
    /// menu that does not fit the window) — the core keeps it as state and
    /// draws none of it, as under [`Self::set_native_menus`], and the host
    /// reports back with [`Self::activate_menu_path`] or
    /// [`Self::close_menu`]; `false` draws it in the window.
    ///
    /// For this menu only: the next one waits for the host again, or is
    /// drawn here where the host does not place menus. Nothing with no
    /// menu open.
    pub fn set_menu_in_popup(&mut self, on: bool) {
        if self.menu.is_some() {
            self.menu_where = if on {
                MenuWhere::Popup
            } else {
                MenuWhere::Window
            };
        }
    }

    /// Whether the open menu is in a popup window the host opened for it.
    pub fn menu_in_popup(&self) -> bool {
        self.menu.is_some() && self.menu_where == MenuWhere::Popup
    }

    /// Makes this core a menu's popup window: its frame is `items`, as the
    /// in-window menu draws them, at the window's top-left, and nothing
    /// else — a host builds its frames without a view. A menu taller than
    /// the window is cut to it and scrolls; `room` is the display's work
    /// area (logical px) a row's width is held to, since the window is
    /// only as wide as the panel. `max_size` is the menu's own
    /// ([`Menu::max_size`](crate::menu::Menu#structfield.max_size)), which a
    /// submenu's popup carries too. `submenu` says it shows a submenu,
    /// whose Escape and Left arrow go back rather than dismiss.
    ///
    /// Nothing chosen here is performed: see [`MenuPopupAnswer`]. A host
    /// measuring what window to open lays one frame out at the room's size
    /// and reads [`Self::menu_popup_size`].
    pub fn open_menu_popup(
        &mut self,
        items: Vec<MenuItem>,
        max_size: Option<Size>,
        room: Size,
        submenu: bool,
    ) {
        self.menu_popup = Some(MenuPopup {
            room,
            submenu,
            answer: None,
        });
        let mut menu = Menu::new(Key::ROOT, Vec2::ZERO, items);
        menu.max_size = max_size;
        self.open_menu_raw(menu);
    }

    /// Whether this core is a menu's popup window (`open_menu_popup`).
    pub fn is_menu_popup(&self) -> bool {
        self.menu_popup.is_some()
    }

    /// What this popup's menu said since the last call: a row chosen,
    /// Escape, the Left arrow. Taken once. `None` while it has said
    /// nothing, and for a core that is no menu's popup.
    pub fn take_menu_popup_answer(&mut self) -> Option<MenuPopupAnswer> {
        self.menu_popup.as_mut()?.answer.take()
    }

    /// The size the last layout gave this popup's menu panel, logical px:
    /// the size its window should be. `None` before a frame has drawn it.
    pub fn menu_popup_size(&self) -> Option<Size> {
        self.menu_popup.as_ref()?;
        let i = (0..self.tree.len()).find(|&i| self.tree.origins[i] == OriginId::MENU)?;
        Some(self.tree.size[i])
    }

    /// The row of this popup whose submenu is open, if one is: the row the
    /// pointer rests on, or the keyboard opened. Its submenu is not drawn
    /// here — the host opens a popup for it beside `rect`.
    pub fn menu_popup_submenu(&self) -> Option<MenuPopupSubmenu> {
        self.menu_popup.as_ref()?;
        let row = *self.menu_sub.open.first()?;
        let i = (0..self.tree.len()).find(|&n| {
            self.tree.origins[n] == OriginId::MENU
                && self.tree.specs[n].events().on_click().is_some_and(|tag| {
                    tag.get("row").and_then(Value::as_int) == Some(row as i64)
                        && row_path_of(tag).is_empty()
                })
        })?;
        Some(MenuPopupSubmenu {
            row,
            rect: crate::geom::Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]),
            keyboard: self.menu_sub.focus_first,
        })
    }

    /// The keyboard opened this popup's submenu (Enter or the Right arrow
    /// on its row, [`MenuPopupSubmenu::keyboard`]): its first row shows
    /// focus, as a submenu the keyboard opens in the window does.
    pub fn menu_popup_by_keyboard(&mut self) {
        if self.menu_popup.is_some() {
            self.focus_visible = true;
        }
    }

    /// Closes this popup's open submenu, putting focus back on its row:
    /// what the host does when the submenu's popup answers
    /// [`MenuPopupAnswer::Back`].
    pub fn close_menu_popup_submenu(&mut self) {
        if !self.menu_sub.open.is_empty() {
            self.close_submenu(MenuSurface::Context, 0);
        }
    }

    /// The work area a menu's popup was opened with: what its rows' width
    /// is held to in place of the window's.
    pub(crate) fn menu_popup_room(&self) -> Option<Size> {
        Some(self.menu_popup.as_ref()?.room)
    }
}
