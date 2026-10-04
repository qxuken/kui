//! `Core`'s application menu bar: the declaration, the one open menu the
//! drawn bar keeps, and the two doors a chosen item comes back through.
//!
//! The declaration is data and sticky, the way the window title is: a frame
//! that declares none leaves the last one in force and a frame that
//! declares an empty bar takes it away, so the driver diffs
//! [`Core::menu_bar_revision`] rather than a tree.
//!
//! Where the platform owns a bar the driver hands the declaration over and
//! reports back through [`Core::activate_menu_bar_item`]. Where it does not,
//! [`crate::widgets::menu_bar`] draws it: its titles and its rows are
//! ordinary nodes with ordinary click payloads, and this module *takes
//! those events back* by key — recorded while building, never guessed at
//! from a payload — exactly as `menu_api` does for the open context menu's
//! rows. What reaches the app is one `{kind:"menu", role, item}`, the same
//! event both menus have always posted.

use crate::input::UiEvent;
use crate::key::Key;
use crate::menu::{MenuBar, MenuItem};
use crate::runtime::Core;
use crate::tree::OriginId;

impl Core {
    /// Declares the application menu for this frame.
    ///
    /// Sticky, and diffed: declaring the same bar again costs one
    /// comparison and changes nothing, a different one bumps
    /// [`Self::menu_bar_revision`] for the driver to notice, and an empty
    /// [`MenuBar`] is how an app takes the bar away. A frame that says
    /// nothing leaves the last declaration standing — which is what lets a
    /// palette window declare no menu and leave the document window's bar
    /// alone.
    ///
    /// Every item's accelerator is normalized on the way in: a portable
    /// `"mod+s"` becomes the platform's own spelling (`"⌘S"`, `"Ctrl+S"`),
    /// so the drawn bar and the platform's read the same and the platform's
    /// can bind the key. A spelling kui cannot parse is left exactly as
    /// written and drawn as written — an app's own shortcut is the app's.
    pub fn declare_menu_bar(&mut self, bar: MenuBar) {
        let bar = normalize(bar);
        if self.menu_bar.as_ref() == Some(&bar) {
            return;
        }
        // An open menu describes a bar that may no longer exist. Identity
        // is the label at that index: an item list that changed under an
        // open menu is ordinary (a row enabling, a setting checking), but a
        // *different menu* at that index leaves the open index naming
        // something the user never opened — and the row keys are derived
        // from the index, so the next click would perform the new menu's
        // item at that position.
        let open_moved = self.menu_bar_open.is_some_and(|i| {
            fn label(b: &MenuBar, i: usize) -> Option<&str> {
                b.menus.get(i).map(|m| m.label.as_str())
            }
            self.menu_bar.as_ref().and_then(|old| label(old, i)) != label(&bar, i)
        });
        if open_moved {
            self.menu_bar_open = None;
        }
        self.menu_bar = Some(bar);
        self.menu_bar_origin = self.origin();
        self.menu_bar_rev += 1;
    }

    /// The declaration in force, if any frame has made one.
    pub fn menu_bar(&self) -> Option<&MenuBar> {
        self.menu_bar.as_ref()
    }

    /// Bumped whenever the declaration changes. A driver keeps the number
    /// it last applied and touches the platform only when they differ —
    /// the menu-bar analogue of the title diff, and the reason re-declaring
    /// an unchanged bar every frame costs nothing.
    pub fn menu_bar_revision(&self) -> u64 {
        self.menu_bar_rev
    }

    /// Tells the core that the platform owns the menu bar — macOS's, which
    /// is not in any window. The drawn bar then
    /// draws nothing, and the driver is the one that hands the declaration
    /// over and reports what was chosen.
    ///
    /// Off by default, like [`Self::set_native_menus`]: a host that says
    /// nothing draws its own bar, which is what every headless test and
    /// every binding without a runner sees.
    pub fn set_native_menu_bar(&mut self, on: bool) {
        self.native_menu_bar = on;
    }

    /// Whether the platform said the menu bar is its.
    pub fn native_menu_bar(&self) -> bool {
        self.native_menu_bar
    }

    /// Which menu of the drawn bar is open, if any. Retained by the core
    /// because it is the one thing the widget cannot derive from the frame
    /// — and always `None` while the platform draws
    /// the bar, since then the open menu is the platform's.
    pub fn menu_bar_open(&self) -> Option<usize> {
        self.menu_bar_open
    }

    /// Opens one of the drawn bar's menus, closes it (`None`), and is what
    /// a press on a title goes through. Public because a keymap is as good
    /// a reason to open the File menu as a click is; out of range closes.
    pub fn set_menu_bar_open(&mut self, menu: Option<usize>) {
        let menu = menu.filter(|i| {
            self.menu_bar
                .as_ref()
                .and_then(|b| b.menus.get(*i))
                .is_some_and(|m| m.enabled && !m.items.is_empty())
        });
        if menu == self.menu_bar_open {
            return;
        }
        // Which editor the menu is about, remembered as the menu opens for
        // the reason `open_menu` remembers it: the rows are focusable, so
        // by the time Copy runs the field the user was in has lost focus.
        //
        // Only when there *is* one, and never `None` over an answer already
        // recorded: a press on a title has moved focus to the title before
        // this runs (`Core::note_menu_bar_editor` is where the field was
        // caught), and hovering across to another title runs it again with
        // focus inside the open menu. Either would otherwise erase the
        // field the menu is about and leave Cut with nothing to cut.
        if menu.is_some()
            && let Some(editor) = self.edit.focused()
        {
            self.menu_editor = Some(editor);
        }
        self.menu_bar_open = menu;
    }

    /// The platform's menu bar reports that an item was chosen: the same
    /// path a press on the drawn bar's row takes. The core performs the
    /// standard roles it can, queues what the host must do
    /// (`take_menu_actions`) and returns the events the app hears.
    ///
    /// An index past the end does nothing, which is what a host reporting a
    /// row this build does not have should do.
    pub fn activate_menu_bar_item(&mut self, menu: usize, item: usize) -> Vec<UiEvent> {
        let mut out = Vec::new();
        let Some(item) = self
            .menu_bar
            .as_ref()
            .and_then(|b| b.item(menu, item))
            .cloned()
        else {
            return out;
        };
        // Nothing displaced focus — the platform's menu bar is not in this
        // window and takes none — so the editor is simply whichever one has
        // it now.
        self.menu_editor = self.edit.focused();
        let (target, origin) = (self.menu_bar_target(), self.menu_bar_origin);
        self.perform_menu_item(&item, target, origin, &mut out);
        out
    }

    /// Where a menu-bar event lands: the bar's own root when one is drawn,
    /// and the frame root when the platform draws it and there is no node.
    fn menu_bar_target(&self) -> Key {
        self.menu_bar_root.unwrap_or(Key::ROOT)
    }

    /// The drawn bar reports its root this frame (`widgets::menu_bar`):
    /// where a menu-bar event lands (`menu_bar_target`). Its titles and
    /// rows are known by their origin, not by key.
    pub(crate) fn set_menu_bar_root(&mut self, root: Key) {
        self.menu_bar_root = Some(root);
    }

    /// Filters the events one input produced: anything belonging to the
    /// drawn bar is consumed and acted on — a title opens or closes its
    /// menu, a row performs its item, a dismissal closes what is open — and
    /// what the app hears instead is the item's own payload.
    ///
    /// Runs beside `consume_menu_events` on the way out of `handle_input`,
    /// so a bar the app never declared can leak nothing and an app that
    /// declared one never sees its plumbing.
    pub(crate) fn consume_menu_bar_events(&mut self, out: &mut Vec<UiEvent>) {
        if self.menu_bar_root.is_none() {
            return;
        }
        // A `dismiss` is the bar's own: Escape, or a press outside it while
        // a menu is open (the bar is the modal scope then, so its titles
        // stay live and the app below does not).
        let taken = Self::take_surface_events(out, OriginId::MENU_BAR);
        let (title, row, dismissed) = (taken.title, taken.row, taken.dismissed);
        if let Some(i) = title {
            // A press on the open menu's own title closes it, which is
            // what every menu bar does and what makes the title a toggle.
            let next = (self.menu_bar_open != Some(i)).then_some(i);
            self.set_menu_bar_open(next);
        } else if let Some(i) = row {
            let Some(menu) = self.menu_bar_open else {
                return;
            };
            let item = self
                .menu_bar
                .as_ref()
                .and_then(|b| b.item(menu, i))
                .cloned();
            self.set_menu_bar_open(None);
            if let Some(item) = item {
                let (target, origin) = (self.menu_bar_target(), self.menu_bar_origin);
                self.perform_menu_item(&item, target, origin, out);
            }
        } else if dismissed {
            self.set_menu_bar_open(None);
        }
    }

    /// A press has landed on a node of the drawn bar: remember the editor
    /// it is about *now*, because the press is what moves focus onto the
    /// title and the menu does not open until the release.
    ///
    /// The truth at press time, `None` included — a menu opened with no
    /// field focused is about no field, and must not inherit the one an
    /// earlier menu was about.
    pub(crate) fn note_menu_bar_editor(&mut self) {
        self.menu_editor = self.edit.focused();
    }
}

/// Rewrites every accelerator kui can parse into the platform's own
/// spelling, and leaves the rest exactly as declared. Runs once per
/// changed declaration and not per frame, which is why `declare_menu_bar`
/// compares after normalizing rather than before.
fn normalize(mut bar: MenuBar) -> MenuBar {
    for menu in &mut bar.menus {
        for item in &mut menu.items {
            normalize_item(item);
        }
    }
    bar
}

fn normalize_item(item: &mut MenuItem) {
    let Some(accel) = &item.accel else { return };
    if let Some(parsed) = crate::menu::Accel::parse(accel) {
        item.accel = Some(parsed.display());
    }
}
