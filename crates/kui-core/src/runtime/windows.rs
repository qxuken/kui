//! The window this core draws and the windows a frame declares
//! (`docs/adr/0004-multi-window.md`): the declared set and its diff into
//! `Open` / `Close`, the command queue drivers drain, the title and the
//! window level.

use super::*;

impl Core {
    // -- Declared windows ---------------------------------------------------
    // `docs/adr/0004-multi-window.md`, decisions 4-6: a window's existence
    // is declared the way a title is, the session diffs the union of what
    // every live window's frame declared, and the diff's `Open` / `Close`
    // go out through the same queue chrome commands do.

    /// Declares that a window named `name` exists this frame.
    ///
    /// The window opens on the first frame any core declares it — that
    /// frame's `finish_frame` queues a [`crate::WindowCommand::Open`] with
    /// the id the core assigned and raises `{kind:"window",
    /// phase:"opened", name, id}` — and closes, with a `Close` and a
    /// `phase:"closed"`, on the first frame none does. Declared every
    /// frame it costs nothing after the first.
    ///
    /// `config` is read on that opening edge and never again: re-declaring
    /// a live window at another size changes nothing, because the user
    /// owns its geometry once it exists. Where two declarations of one
    /// name disagree on that edge, the lowest declaring window's first
    /// declaration wins and [`crate::diag::DUPLICATE_WINDOW_CONFIG`]
    /// says so. A window the user closed (see [`Self::window_closed`])
    /// does not reopen while it is still declared — the declaration has
    /// to stop and start again — and keeps raising
    /// [`crate::diag::WINDOW_DECLARED_WHILE_CLOSED`] until it does.
    /// `"main"` names the window the launcher opened and is always live.
    pub fn declare_window(&mut self, name: &str, mut config: WindowConfig) {
        // A popup's anchor is declared in the host's coordinates and
        // resolved by the driver against the window's (ADR 0024): the
        // dock's offset goes back on here.
        let shift = self.dt_shift();
        config.anchor.x += shift.x;
        config.anchor.y += shift.y;
        if let Some(d) = self.declared_windows.iter_mut().find(|d| &*d.name == name) {
            // First declaration wins within a frame; a disagreement is
            // remembered for the opening edge to report.
            d.conflict |= d.config != config;
            return;
        }
        self.declared_windows.push(WindowDecl {
            name: Rc::from(name),
            config,
            origin: self.origin,
            conflict: false,
        });
    }

    /// The windows declared while the current frame was built, for the
    /// scene corpus's coverage derivation: a declaration leaves no node.
    #[cfg(feature = "conformance")]
    pub(crate) fn declared_windows(&self) -> &[WindowDecl] {
        &self.declared_windows
    }

    /// The name of the window this core draws: `"main"` for
    /// `WindowId::MAIN`, else the name the declaration that opened
    /// `env.window.id` used. A driver that gave a core an id the session
    /// never opened gets the id spelled out, so the answer is never empty.
    pub fn window_name(&self) -> Rc<str> {
        let id = self.env.window.id;
        if id == WindowId::MAIN {
            return Rc::from(MAIN_WINDOW_NAME);
        }
        self.session
            .state()
            .windows
            .name_of(id)
            .unwrap_or_else(|| Rc::from(format!("window-{}", id.0).as_str()))
    }

    /// Every window of the session that is open right now — main first,
    /// then in the order they opened — as `(id, name)`. What the
    /// declaration diff has asked for, not what the driver has shown:
    /// the two agree once the driver drains its commands.
    pub fn windows(&self) -> Vec<(WindowId, Rc<str>)> {
        self.session.state().windows.live()
    }

    /// The driver reports that the OS closed window `id` — its close
    /// button, a keyboard shortcut, the window manager. The window is gone
    /// and stays gone while its name is still declared (the app has to stop
    /// declaring it and start again to reopen it; see
    /// [`Self::declare_window`]); its own declarations leave the union, so
    /// whatever only it declared closes too. Raises `{kind:"window",
    /// phase:"closed", name, id}` for the app, pending like a resize.
    /// Nothing happens for the main window (closing it ends the app) or
    /// for a window the diff already closed.
    pub fn window_closed(&mut self, id: WindowId) {
        let mut changes = Vec::new();
        let name = {
            let sess = &mut *self.session.state();
            let Some(name) = sess.windows.os_closed(id) else {
                return;
            };
            sess.windows.diff(&mut changes);
            name
        };
        self.push_window_event("closed", &name, id);
        self.apply_window_changes(changes);
    }

    /// The driver reports that window `id` was asked to go away — a press
    /// landed outside it, or Escape reached it (see
    /// [`crate::window::DismissReason`]). Raises `{kind:"dismiss", reason,
    /// name, id}` on the root, pending like a resize, and **closes
    /// nothing**: only the app can stop declaring the window, and it does
    /// that on the frame it decides to, which is ADR 0003 decision 6 one
    /// level up. So an app that graduates a dropdown from a `modal` float
    /// to a popup window changes its declaration and keeps its handler.
    ///
    /// Both facts behind it are the OS's — a press outside a window lands
    /// in another surface, and a non-activating popup never holds the
    /// keyboard — so the core cannot notice either; a driver reports them
    /// the way it reports a close ([`Self::window_closed`]). Nothing
    /// happens for a window the session has not opened.
    pub fn dismiss_window(&mut self, id: WindowId, reason: crate::window::DismissReason) {
        let Some(name) = self.session.state().windows.name_of(id) else {
            return;
        };
        self.pending.push(UiEvent {
            origin: OriginId::HOST,
            window: WindowId::MAIN,
            key: Key::ROOT,
            payload: Value::map([
                ("kind", Value::str("dismiss")),
                ("reason", Value::str(reason.as_str())),
                ("name", Value::str(&*name)),
                ("id", Value::Int(id.0 as i64)),
            ]),
        });
    }

    /// Hands this frame's declarations to the session and takes back what
    /// the union's diff decided. Runs at `finish_frame`; skipped whole when
    /// this frame declared what the last one did and no window is sitting
    /// closed-but-declared, which is every frame of a single-window app.
    pub(crate) fn sync_windows(&mut self) {
        let changed = self.declared_windows != self.declared_windows_last;
        let mut changes = Vec::new();
        let mut still_closed: Vec<Rc<str>> = Vec::new();
        {
            let sess = &mut *self.session.state();
            let reg = &mut sess.windows;
            if !changed && !reg.any_closed() {
                return;
            }
            if changed {
                reg.set_slot(self.env.window.id, &self.declared_windows);
                reg.diff(&mut changes);
            }
            still_closed.extend(reg.closed_among(&self.declared_windows));
        }
        for name in still_closed {
            self.diag
                .raise(crate::diag::window_declared_while_closed(&name));
        }
        self.apply_window_changes(changes);
    }

    /// Turns the diff's decisions into what a driver and an app see: a
    /// command in the queue, a `{kind:"window"}` event, and the conflict
    /// warning where the opening edge found one.
    fn apply_window_changes(&mut self, changes: Vec<WindowChange>) {
        for change in changes {
            match change {
                WindowChange::Opened {
                    id,
                    name,
                    owner,
                    origin,
                    config,
                    conflict,
                } => {
                    self.interaction
                        .window_commands
                        .push(crate::window::WindowCommand::Open {
                            id,
                            owner,
                            origin,
                            config,
                        });
                    self.push_window_event("opened", &name, id);
                    if conflict {
                        self.diag.raise(crate::diag::duplicate_window_config(&name));
                    }
                }
                WindowChange::Closed { id, name } => {
                    self.interaction
                        .window_commands
                        .push(crate::window::WindowCommand::Close(id));
                    self.push_window_event("closed", &name, id);
                }
            }
        }
    }

    /// `{kind:"window", phase, name, id}` on the root, pending for the
    /// driver to route after the frame (or with the next input). `id` is
    /// in the payload because `UiEvent::window` says which core reported
    /// it, and the diff runs on whichever core finished its frame.
    fn push_window_event(&mut self, phase: &str, name: &str, id: WindowId) {
        self.pending.push(UiEvent {
            origin: OriginId::HOST,
            window: WindowId::MAIN,
            key: Key::ROOT,
            payload: Value::map([
                ("kind", Value::str("window")),
                ("phase", Value::str(phase)),
                ("name", Value::str(name)),
                ("id", Value::Int(id.0 as i64)),
            ]),
        });
    }

    /// Queues a window command as if chrome had produced it, so apps can
    /// close/minimize/maximize from a keymap or command line. Drained by
    /// the frame driver with the rest.
    pub fn push_window_command(&mut self, cmd: crate::window::WindowCommand) {
        self.interaction.window_commands.push(cmd);
    }

    /// Asks the driver to resize `window` to `size` (logical px). Queued
    /// the way `reveal` and `play` queue theirs: a request the driver
    /// applies on its next pump — after this input dispatch if called from
    /// a handler, after this frame if called from a view — and one a
    /// headless driver never applies, since it never drains. The window
    /// answers through the ordinary `resize` event, with the size it
    /// actually became. Until ADR 0004's step 3 lets a frame declare more
    /// windows, `WindowId::MAIN` is the only one there is.
    pub fn set_window_size(&mut self, window: crate::window::WindowId, size: crate::geom::Size) {
        self.interaction
            .window_commands
            .push(crate::window::WindowCommand::SetSize { window, size });
    }

    /// Asks the driver to give `window` keyboard focus; queued like
    /// [`Core::set_window_size`]. Whether the window manager agrees shows
    /// up as `env.focused` on the frames that follow, not as a reply.
    pub fn focus_window(&mut self, window: crate::window::WindowId) {
        self.interaction
            .window_commands
            .push(crate::window::WindowCommand::Focus(window));
    }

    /// Drains window intents queued since the last drain — by chrome nodes,
    /// by `push_window_command`, `set_window_size` and `focus_window` — in
    /// the order they were queued. Frame drivers call this after each input
    /// dispatch and each frame and apply the commands to the real window;
    /// headless drivers may simply never call.
    pub fn take_window_commands(&mut self) -> Vec<crate::window::WindowCommand> {
        std::mem::take(&mut self.interaction.window_commands)
    }

    /// Declares this frame's window title. Like all frame state it's data:
    /// the driver diffs against what's applied and only then touches the
    /// window. Undeclared frames leave the title alone; last writer wins.
    pub fn set_window_title(&mut self, title: &str) {
        self.window_title = Some(title.to_string());
    }

    /// The title declared this frame, if any (for the frame driver).
    pub fn window_title(&self) -> Option<&str> {
        self.window_title.as_deref()
    }

    /// Declares that this frame wants the window above every other app's
    /// (backlog C30): a floating palette, a picture-in-picture player, a
    /// timer. Frame state like the title, and the driver applies it the
    /// same way — `set_window_level` when it differs from what is applied,
    /// nothing when it does not — but it defaults to `false` rather than
    /// "leave as-is", so a frame that stops declaring it lowers the window
    /// again and a pin button is a toggle on the app's own state. Whether
    /// the platform has a level to set is `env.window.always_on_top`: the
    /// driver's record of what it set, false on Wayland (where winit has
    /// no call for it) however often the app asks — and not a query, so a
    /// level the OS dropped afterwards (a fullscreen space, a tiling
    /// manager) is not reported. A popup's level is its own whatever its
    /// owner declares.
    pub fn set_always_on_top(&mut self, on_top: bool) {
        self.always_on_top = on_top;
    }

    /// Whether this frame asked for the window to stay on top (for the
    /// frame driver); false for a frame that never said.
    pub fn always_on_top(&self) -> bool {
        self.always_on_top
    }
}
