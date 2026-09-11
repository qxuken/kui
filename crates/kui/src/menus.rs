//! The platform's menus, pumped from the run loop: the native context
//! menu and menu bar where the OS draws them, and the actions choosing a
//! row leaves for the runner (`docs/adr/0018-a-menu-bar-the-app-declares.md`).
//! Split off `lib.rs` as a pure move.

use super::*;

impl<A: App> Shell<A> {
    /// Hands a menu the core opened to the platform, and hands the
    /// platform's answer back (ADR 0017, decision 5, step 3).
    ///
    /// Two turns, not one, because the platform's menu is modal and cannot
    /// be entered from inside a winit callback (see `macos_menu`): the
    /// first turn schedules it, and a later one — after the menu has come
    /// and gone — reads which row it reported. A host with no native menu
    /// does neither and the core draws its own.
    pub(super) fn pump_native_menu(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "macos")]
        {
            let Some(native) = self.native_menu.as_ref() else {
                return;
            };
            // Only the main window's menu: a popup window's owner is what
            // a context menu belongs to, and no pane but the main one has
            // asked for one yet. And only a core that still says the
            // platform shows menus: an app that opted out with
            // `set_native_menus(false)` is drawing this one itself, and
            // presenting it here as well put the two on screen together.
            let Some(i) = self
                .panes
                .iter()
                .position(|p| p.core.menu().is_some() && p.core.native_menus())
            else {
                self.menu_shown = false;
                return;
            };
            if !self.menu_shown {
                let menu = self.panes[i].core.menu().expect("checked").clone();
                let pane = &self.panes[i];
                self.menu_shown = native.present(&pane.window, menu.at, &menu.items);
                if !self.menu_shown {
                    // No view to show it over: fall back to the drawn
                    // menu rather than leaving one open that never shows.
                    self.panes[i].core.set_native_menus(false);
                    self.panes[i].window.request_redraw();
                }
                return;
            }
            if native.busy() {
                return;
            }
            self.menu_shown = false;
            let events = match native.take_chosen() {
                Some(row) => self.panes[i].core.activate_menu_item(row),
                None => {
                    self.panes[i].core.close_menu();
                    Vec::new()
                }
            };
            self.route_events(events);
            self.apply_menu_actions(event_loop, i);
            self.apply_window_commands(event_loop);
            self.panes[i].window.request_redraw();
        }
        #[cfg(not(target_os = "macos"))]
        let _ = event_loop;
    }

    /// Hands the frontmost window's menu-bar declaration to the platform,
    /// and the platform's answers back (ADR 0018, decisions 4 and 8).
    ///
    /// The bar belongs to the process and a `Core` to a window, so the one
    /// that holds the keyboard is the one whose declaration is up — and,
    /// when none does, whichever window declared one, since an app can be
    /// active with no key window and that is the state a menu is used in.
    /// A window that declares nothing leaves the last bar standing rather
    /// than blanking it. Diffed against `(window, revision)`, so
    /// re-declaring the same bar every frame rebuilds no `NSMenu`.
    pub(super) fn pump_menu_bar(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "macos")]
        {
            let Some(native) = self.native_menu_bar.as_ref() else {
                return;
            };
            // What the user chose, if anything — read *before* anything is
            // applied, because a pick names a row of the menu that was up
            // when it was made: applying first would rebuild the tag map
            // under it and resolve the pick through the new one.
            let chosen = native.take_chosen();
            // The window with the keyboard, and — when the app is active
            // with no key window, which is an ordinary macOS state and the
            // one a menu is chosen from — whichever window has declared
            // one. Without the second half the bar goes up at launch and
            // is never rebuilt again.
            let declared = |p: &Pane| p.core.menu_bar().is_some();
            let front = self
                .panes
                .iter()
                .position(|p| p.core.env.focused && declared(p))
                .or_else(|| self.panes.iter().position(declared));
            if let Some(i) = front {
                let pane = &self.panes[i];
                // A core told to draw the bar itself (`set_native_menu_bar(false)`
                // — a devtool comparing the two, a test) gets the platform's
                // taken away rather than both at once: an empty declaration
                // is how `MacMenuBar` is told there is no bar. Stamped as
                // revision 0, which a declared bar never is.
                let drawn = !pane.core.native_menu_bar();
                let revision = if drawn {
                    0
                } else {
                    pane.core.menu_bar_revision()
                };
                let stamp = (pane.core.env.window.id, revision);
                if self.applied_menu_bar != Some(stamp) {
                    if drawn {
                        native.apply(&kui_core::MenuBar::default());
                    } else {
                        native.apply(pane.core.menu_bar().expect("checked"));
                    }
                    self.applied_menu_bar = Some(stamp);
                }
            }
            // Reported to the window the bar was applied from, which is the
            // one the items are about.
            let Some((menu, item)) = chosen else {
                return;
            };
            let Some(i) = self
                .applied_menu_bar
                .and_then(|(w, _)| self.pane_of(w))
                .or_else(|| (!self.panes.is_empty()).then_some(0))
            else {
                return;
            };
            let events = self.panes[i].core.activate_menu_bar_item(menu, item);
            // A chosen row is input that reached the app, so the frame
            // after it waits for the host's answer where the host answers
            // late (`Launcher::deferred_events`) — otherwise it would
            // paint the model the pick was about to change.
            let reached_app = self.route_events(events);
            self.owe_for(reached_app);
            self.apply_menu_actions(event_loop, i);
            self.apply_window_commands(event_loop);
            self.panes[i].window.request_redraw();
        }
        #[cfg(not(target_os = "macos"))]
        let _ = event_loop;
    }

    /// What choosing a stock context-menu item left for the host: the
    /// clipboard, which is this driver's in the same way Cmd-C's is (ADR
    /// 0017, decision 5). Copy and Cut arrive as the text to put there —
    /// the core worked out *what*, which is the half only it can do — and
    /// Paste as a request for what is there, delivered back as typing so
    /// it takes exactly the path Cmd-V takes.
    pub(super) fn apply_menu_actions(&mut self, event_loop: &ActiveEventLoop, i: usize) {
        let Some(pane) = self.panes.get_mut(i) else {
            return;
        };
        for action in pane.core.take_menu_actions() {
            match action {
                MenuAction::SetClipboard { text, html } => {
                    set_clipboard(self.clipboard.as_mut(), text, html);
                }
                MenuAction::Paste => {
                    if let Some(text) = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok()) {
                        self.dispatch(event_loop, i, InputEvent::Text(text));
                    }
                }
                MenuAction::LookUp { text, at } => {
                    // Only macOS has a panel to show. Everywhere else the
                    // core never offers the row and never asks, so this is
                    // unreachable rather than merely unhandled.
                    #[cfg(target_os = "macos")]
                    if let Some(pane) = self.panes.get(i) {
                        macos_menu::show_definition(&pane.window, at, &text);
                    }
                    #[cfg(not(target_os = "macos"))]
                    let _ = (text, at);
                }
            }
        }
    }
}
