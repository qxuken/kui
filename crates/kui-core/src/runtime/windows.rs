//! The window this core draws and the windows a frame declares:
//! the declared set and its diff into
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
        self.taint_kept("it declared a window");
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
        self.release_window_audio(id);
        self.apply_window_changes(changes);
    }

    /// A window that will finish no more frames leaves its `audio` mounts
    /// behind: they are reconciled against that window's frames alone,
    /// so nothing else would ever stop them — a popup's looped bed
    /// played on after the popup closed. Reconciling the window against
    /// the nothing it now declares stops each mount by the rule a removed
    /// node follows (`finish` releases a one-shot, a loop stops).
    fn release_window_audio(&mut self, id: WindowId) {
        self.session.state().audio.reconcile(id);
    }

    /// The driver reports that window `id` was asked to go away — a press
    /// landed outside it, or Escape reached it (see
    /// [`crate::window::DismissReason`]). Raises `{kind:"dismiss", reason,
    /// name, id}` on the root, pending like a resize, and **closes
    /// nothing**: only the app can stop declaring the window, and it does
    /// that on the frame it decides to, the way a modal closes. So an app that
    /// graduates a dropdown from a `modal` float
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
            slot: None,
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
                    self.release_window_audio(id);
                }
            }
        }
    }

    /// `{kind:"window", phase, name, id}` on the root, pending for the
    /// driver to route after the frame (or with the next input). `id` is
    /// in the payload because `UiEvent::window` says which core reported
    /// it, and the diff runs on whichever core finished its frame.
    pub(crate) fn push_window_event(&mut self, phase: &str, name: &str, id: WindowId) {
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
            slot: None,
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
    /// actually became.
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
        self.taint_kept("it declared the window's title");
        self.window_title = Some(title.to_string());
    }

    /// The title declared this frame, if any (for the frame driver).
    pub fn window_title(&self) -> Option<&str> {
        self.window_title.as_deref()
    }

    /// Declares that this frame wants the window above every other app's:
    /// a floating palette, a picture-in-picture player, a
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
        self.taint_kept("it declared always-on-top");
        self.always_on_top = on_top;
    }

    /// Whether this frame asked for the window to stay on top (for the
    /// frame driver); false for a frame that never said.
    pub fn always_on_top(&self) -> bool {
        self.always_on_top
    }

    /// Declares that this frame wants the keyboard to this window kept
    /// from other processes while the window has it — macOS's Secure
    /// Keyboard Entry, what a terminal turns on at a password prompt.
    /// Frame state like `always_on_top`: a frame that stops
    /// declaring it turns it off, so an app asks on every frame the
    /// prompt is up and never has to remember to undo it.
    ///
    /// The runner owns the platform call and its balance: it enables
    /// secure input only while a window whose frame asked has the
    /// keyboard, and disables it when that window loses the keyboard,
    /// closes, stops asking, or the app exits — Apple's rule for it,
    /// since while it is on no other process can read the keyboard at
    /// all (a launcher's hotkey, a text expander, an accessibility tool).
    /// `EnableSecureEventInput` is process-wide and counted, and the
    /// runner holds at most one count however many windows ask. Nothing
    /// happens on other platforms, which have no such switch.
    pub fn set_secure_input(&mut self, on: bool) {
        self.taint_kept("it declared secure input");
        self.secure_input = on;
    }

    /// Whether this frame asked for secure keyboard entry (for the frame
    /// driver); false for a frame that never said.
    pub fn secure_input(&self) -> bool {
        self.secure_input
    }

    /// Declares which Option keys act as Alt in this window on macOS:
    /// a dead key under that Option — ⌥u, ⌥e, ⌥i, ⌥n,
    /// ⌥\` — then arrives as the chord `<A-u>` rather than starting an
    /// accent the app never hears, and a key under it types nothing, as
    /// under Control. Frame state like `always_on_top`, default
    /// [`OptionAsAlt::None`](crate::OptionAsAlt::None): a frame that stops
    /// declaring it gives the Option keys back to the layout, so an app
    /// declares it on every frame — from a setting, say — and never has
    /// to undo it.
    ///
    /// The runner applies it to the window on change and never per frame.
    /// A popup never has the keyboard on macOS — its keys come through its
    /// owner, which stays key — so the owner's declaration is the one a
    /// popup's keys are read under. Nothing happens on other platforms,
    /// whose Alt composes nothing.
    pub fn set_option_as_alt(&mut self, option_as_alt: crate::OptionAsAlt) {
        self.taint_kept("it declared option-as-alt");
        self.option_as_alt = option_as_alt;
    }

    /// Which Option keys this frame asked to act as Alt (for the frame
    /// driver); `None` for a frame that never said.
    pub fn option_as_alt(&self) -> crate::OptionAsAlt {
        self.option_as_alt
    }

    /// Declares that this window takes the keyboard as keys, with the
    /// platform's input method off: no composition and no candidate
    /// window, and on a Mac no dead key waiting for the next one and no
    /// press-and-hold — which is an input method too, so a held letter
    /// repeats instead of opening the accent picker, whatever the user's
    /// `ApplePressAndHoldEnabled` says. A key's `text` is still the
    /// layout's character; what goes is everything the OS would have
    /// composed from it. What a modal editor's normal mode wants, where
    /// `jjjj` is how one moves and a Japanese IME left on eats the
    /// keymap; its insert mode stops declaring it and gets both back.
    /// Frame state like `always_on_top`, default off: a frame that stops
    /// declaring it gives the window its input method back, so an app
    /// declares it on every frame its mode wants it and never has to
    /// undo it.
    ///
    /// The runner applies it to the window on change and never per
    /// frame (winit's `set_ime_allowed`); a composition in progress when
    /// it turns off is ended without a commit, as an empty `preedit`. It
    /// is the window's, not a node's: a stock editor focused under it
    /// composes nothing either. A popup's keys arrive through its owner,
    /// so the owner's declaration is the one they are read under. On
    /// Windows and Linux the window's IME is disabled the same way, and
    /// that is all: their dead keys are the layout's (`WM_DEADCHAR`, xkb
    /// compose), winit composes them whatever the IME says, and they
    /// still compose.
    pub fn set_ime_off(&mut self, off: bool) {
        self.taint_kept("it declared the input method off");
        self.ime_off = off;
    }

    /// Whether this frame asked for the input method off (for the frame
    /// driver); false for a frame that never said.
    pub fn ime_off(&self) -> bool {
        self.ime_off
    }
}
