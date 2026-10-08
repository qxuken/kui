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

use crate::geom::Vec2;
/// The longest selection a definition panel is offered for. A word, a
/// term, a short phrase — past this it is a passage, and a dictionary has
/// nothing to say about a passage.
pub const LOOKUP_MAX: usize = 100;

use crate::input::UiEvent;
use crate::key::Key;
use crate::menu::{Menu, MenuAction, MenuItem, MenuRole};
use crate::runtime::Core;
use crate::tree::OriginId;
use crate::ui::Ui;
use crate::value::Value;
use crate::window::WindowId;

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
        // The accelerators in the platform's spelling, as the bar's are
        // (`declare_menu_bar`): a portable `"mod+shift+n"` is drawn `⇧⌘N`
        // or `Ctrl+Shift+N`, and a host that shows the menu itself reads
        // the same string back (backlog F127).
        MenuItem::normalize_accels(&mut menu.items);
        self.menu = Some(menu);
        // A new menu opens with none of its submenus open.
        self.menu_sub = Submenus::default();
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
        if ui.core().native_menus {
            // The host is showing it. Nothing is drawn, so there are no
            // rows to click and `consume_menu_events` finds nothing to
            // take back.
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
        crate::widgets::context_menu(ui, at, &menu.items);
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
            if ev.kind() == Some("dismiss") {
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
        let Some(item) = MenuItem::at_path(&menu.items, path).cloned() else {
            return;
        };
        self.close_menu();
        self.perform_menu_item(&item, menu.target, menu.origin, out);
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
                    self.push_edit_event(key, "changed", out);
                }
            }
            MenuRole::Paste => self.queue_paste(),
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
            slot: None,
        });
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
    pub(crate) fn submenu_hovered(&mut self, s: MenuSurface, row: &[usize], opens: bool) {
        let sub = self.submenus(s);
        if sub.hovered.as_deref() == Some(row) {
            return;
        }
        sub.hovered = Some(row.to_vec());
        sub.focus_first = false;
        let level = row.len() - 1;
        if opens {
            // Kept when it is already the open one, deeper submenus and all:
            // the pointer coming back to its row from inside it.
            if !sub.open.starts_with(row) {
                sub.open = row.to_vec();
            }
        } else if sub.open.len() > level && sub.open[..level] == row[..level] {
            sub.open.truncate(level);
        }
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
            let tag = self.tree.specs[i].events().on_click.as_ref()?;
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
        // The row the closed submenu hangs from, found by the tag its click
        // carries; it is in the last frame's tree, since its submenu was.
        let (parent, i) = row.split_at(keep);
        let origin = s.origin();
        let at = (0..self.tree.len()).find(|&n| {
            self.tree.origins[n] == origin
                && self.tree.specs[n]
                    .events()
                    .on_click
                    .as_ref()
                    .is_some_and(|tag| {
                        tag.get("row").and_then(Value::as_int) == Some(i[0] as i64)
                            && row_path_of(tag) == parent
                    })
        });
        if let Some(n) = at {
            self.land_focus(n);
        }
    }
}
