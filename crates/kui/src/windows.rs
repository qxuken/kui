//! The shell's windows: finding a pane by either id, and applying the
//! window commands every core queues — chrome intents on the window they
//! name, and the `Open` / `Close` the declared set's diff produced
//! (`docs/adr/0004-multi-window.md`). Split off `lib.rs` as a pure move.

use super::*;

impl<A: App> Shell<A> {
    pub(super) fn pane_index(&self, id: WinitWindowId) -> Option<usize> {
        self.panes.iter().position(|p| p.window.id() == id)
    }

    pub(super) fn pane_of(&self, id: WindowId) -> Option<usize> {
        self.panes.iter().position(|p| p.id == id)
    }

    /// Applies every window command every core queued: chrome intents on
    /// the window they name, and the `Open` / `Close` the declared set's
    /// diff produced.
    pub(super) fn apply_window_commands(&mut self, event_loop: &ActiveEventLoop) {
        let mut cmds = Vec::new();
        for p in &mut self.panes {
            cmds.append(&mut p.core.take_window_commands());
        }
        self.apply_commands(event_loop, cmds);
    }

    pub(super) fn apply_commands(
        &mut self,
        event_loop: &ActiveEventLoop,
        cmds: Vec<WindowCommand>,
    ) {
        for cmd in cmds {
            match cmd {
                WindowCommand::StartDrag(id) => {
                    let Some(i) = self.pane_of(id) else { continue };
                    let pane = &mut self.panes[i];
                    let now = std::time::Instant::now();
                    let double = pane
                        .last_titlebar_press
                        .take()
                        .is_some_and(|t| now.duration_since(t).as_millis() < DOUBLE_CLICK_MS);
                    if double {
                        pane.window.set_maximized(!pane.window.is_maximized());
                    } else {
                        pane.last_titlebar_press = Some(now);
                        let _ = pane.window.drag_window();
                    }
                }
                WindowCommand::Close(id) if id == WindowId::MAIN => self.exit_requested = true,
                // The chrome close button: the user closed it, as far as
                // the declared set is concerned, so it stays closed while
                // still declared. A `Close` the diff produced has already
                // been taken out of the set, and reporting it again is a
                // no-op there.
                WindowCommand::Close(id) => self.close_pane(event_loop, id),
                WindowCommand::Minimize(id) => {
                    if let Some(i) = self.pane_of(id) {
                        self.panes[i].window.set_minimized(true);
                    }
                }
                WindowCommand::ToggleMaximize(id) => {
                    if let Some(i) = self.pane_of(id) {
                        let w = &self.panes[i].window;
                        w.set_maximized(!w.is_maximized());
                    }
                }
                // Every origin may open a window here; a host that wants
                // to refuse an extension's checks `origin` before this.
                // The devtools' own window is the core's, not the app's:
                // it gets the OS's chrome whatever the launcher asked
                // for, since nothing in it draws a titlebar — under
                // `Chrome::Custom` it opened undecorated, and could not
                // be moved (the pomodoro's report, 2026-09-12).
                WindowCommand::Open {
                    id,
                    owner,
                    origin,
                    config,
                } => {
                    if self.pane_of(id).is_none() {
                        let chrome = if origin == OriginId::DEVTOOLS {
                            Chrome::Native
                        } else {
                            self.chrome
                        };
                        self.open_pane(event_loop, id, owner, config, chrome);
                    }
                }
                // The app asking, rather than the declaration: a live
                // window's config is never re-read, so this is the only
                // way its size moves from inside the app (ADR 0004
                // decision 5). winit reports the size it actually applied
                // at once on some platforms and as a later `Resized` on
                // others; either way it reaches the app as the ordinary
                // `resize`, so nothing here writes the pane's viewport.
                WindowCommand::SetSize { window, size } => {
                    if let Some(i) = self.pane_of(window) {
                        let _ = self.panes[i]
                            .window
                            .request_inner_size(LogicalSize::new(size.w as f64, size.h as f64));
                    }
                }
                WindowCommand::Focus(id) => {
                    if let Some(i) = self.pane_of(id) {
                        self.panes[i].window.focus_window();
                    }
                }
                // Another window's input changed what this one shows
                // (ADR 0024, decision 7): the same request an event that
                // reached the app makes of every pane, for one pane.
                WindowCommand::Redraw(id) => {
                    if let Some(i) = self.pane_of(id) {
                        self.panes[i].window.request_redraw();
                    }
                }
            }
        }
    }

    /// Opens the window a frame declared, on the shared session and device,
    /// with `chrome` — the launcher's for the app's windows.
    pub(super) fn open_pane(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        owner: WindowId,
        config: WindowConfig,
        chrome: Chrome,
    ) {
        let size = if config.size.w > 0.0 && config.size.h > 0.0 {
            config.size
        } else {
            WindowConfig::DEFAULT_SIZE
        };
        // Untitled until its first frame's `window_title` lands (ADR 0004
        // decision 5): the declaration carries no string.
        let mut attrs = self
            .window_attrs("", (size.w as f64, size.h as f64), chrome)
            .with_active(config.activates);
        if config.kind == WindowKind::Popup {
            // A menu surface, not a window with the app's chrome: no
            // decorations whatever the launcher asked for, above its owner,
            // and placed against the anchor rather than wherever the window
            // manager would have put a new window.
            attrs = undecorated(attrs).with_window_level(winit::window::WindowLevel::AlwaysOnTop);
            if let Some(pos) = self.popup_position(owner, config, size) {
                attrs = attrs.with_position(pos);
            }
            // "Absent from the taskbar" is a Windows and X11 fact; macOS
            // has no per-window taskbar entry to be absent from (the Dock
            // is per application), so there is nothing to ask for there.
            #[cfg(target_os = "windows")]
            {
                use winit::platform::windows::WindowAttributesExtWindows;
                attrs = attrs.with_skip_taskbar(true);
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                use winit::platform::x11::{WindowAttributesExtX11, WindowType};
                attrs = attrs.with_x11_window_type(vec![WindowType::PopupMenu]);
            }
        }
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(err) => {
                eprintln!("kui: cannot open window {}: {err}", id.0);
                return;
            }
        };
        let Some(gpu) = self.gpu.clone() else {
            eprintln!("kui: cannot open window {}: no device yet", id.0);
            return;
        };
        let px = window.inner_size();
        let renderer = match kui_wgpu::Renderer::new_in(&gpu, window.clone(), px.width, px.height) {
            Ok(r) => r,
            Err(err) => {
                eprintln!("kui: cannot open window {}: {err}", id.0);
                return;
            }
        };
        let mut core = Core::new_in(&self.session);
        core.set_diagnostics(self.diagnostics);
        core.set_subpixel_text(self.subpixel);
        core.env.window.id = id;
        self.push_pane(
            event_loop, id, config, owner, chrome, core, window, renderer,
        );
        // ADR 0009 decision 1: a non-activating popup that opens while the
        // primary button is down **joins that press**. Evaluated once, here,
        // with no geometry — and it is tight because of the press-outside
        // rule, which guarantees that a press in the owner while a popup is
        // up dismisses that popup: a popup opening under a held button was
        // opened by that button, unless the app declined a dismissal, in
        // which case it is still the popup the user is pressing towards. A
        // second popup opening during the same press (a submenu the app
        // opened from a retargeted hover) joins the same press, whoever owns
        // it, and is on top of the ones before it.
        if config.kind == WindowKind::Popup && !config.activates && self.primary_down.is_some() {
            self.armed.push(Armed { id, inside: false });
        }
        if !config.activates {
            // Hand the keyboard back before the platform has even said it
            // took it. Ordering a window front is enough to make it key on
            // some platforms whatever `with_active(false)` asked for, and
            // `settle_focus` will ask again when it sees that happen — but
            // that is a batch later, and a batch is long enough for the
            // window behind to be drawn without its key styling. Asking
            // eagerly here closes the gap at the one moment it is
            // predictable; `settle_focus` covers every other way a popup
            // can end up with the keyboard, a press on it above all.
            if let Some(j) = self.pane_of(owner) {
                self.panes[j].window.focus_window();
            }
        }
    }

    /// Where a popup's top-left goes, in screen coordinates: below the
    /// anchor its owner reported, flipped above it when the monitor's
    /// bottom edge is nearer than the popup is tall — the placement a menu
    /// wants, and the one `FloatConfig::fit` cannot make, since it clamps
    /// into the window instead of leaving it.
    ///
    /// The anchor arrives in the owner's own logical coordinates (it is a
    /// rect an `onLayout` node reported), so this is where it stops being a
    /// window fact and becomes a screen one. `None` when the owner is gone
    /// or the platform will not say where it is; the window manager then
    /// places the popup and the app is no worse off than a float.
    pub(super) fn popup_position(
        &self,
        owner: WindowId,
        config: WindowConfig,
        size: Size,
    ) -> Option<LogicalPosition<f64>> {
        let pane = self.panes.get(self.pane_of(owner)?)?;
        let scale = pane.window.scale_factor();
        let origin = pane.window.inner_position().ok()?.to_logical::<f64>(scale);
        let a = config.anchor;
        let x = origin.x + a.x as f64;
        let below = origin.y + (a.y + a.h) as f64;
        // The monitor the owner is on, in its own logical coordinates. With
        // no monitor to ask, "below" is the answer and the WM may move it.
        let y = match pane.window.current_monitor() {
            Some(m) => {
                let top = m.position().to_logical::<f64>(scale).y;
                let height = m.size().to_logical::<f64>(scale).height;
                if below + size.h as f64 > top + height {
                    // Above the anchor instead, unless there is even less
                    // room up there — then stay below and let it clip.
                    let above = origin.y + a.y as f64 - size.h as f64;
                    if above >= top { above } else { below }
                } else {
                    below
                }
            }
            None => below,
        };
        Some(LogicalPosition::new(x, y))
    }

    /// Finishes a window whose surface and renderer exist: the platform
    /// hooks, the pane, and showing it.
    #[allow(
        clippy::too_many_arguments,
        reason = "one private call site; every argument is a distinct fact about the \
                  window being adopted, and a struct to carry them would be built and \
                  destructured in the same breath"
    )]
    pub(super) fn push_pane(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        config: WindowConfig,
        owner: WindowId,
        chrome: Chrome,
        mut core: Core,
        window: Arc<Window>,
        renderer: kui_wgpu::Renderer,
    ) {
        // macOS is the one platform whose own menu is worth the app's
        // appearance, because it is the one with rows that cannot be drawn
        // at all — Look Up, Services — so there the core keeps the open
        // menu as state and draws none of it. Everywhere else, Windows now
        // included, it draws the menu it always drew (ADR 0017, decision 5,
        // amended 2026-09-10).
        #[cfg(target_os = "macos")]
        let native_menus = self.native_menu.is_some();
        #[cfg(not(target_os = "macos"))]
        let native_menus = false;
        core.set_native_menus(native_menus);
        // And the menu *bar*, which is the platform's on macOS and drawn
        // by `widgets::menu_bar` everywhere else (ADR 0018, decision 4).
        #[cfg(target_os = "macos")]
        core.set_native_menu_bar(self.native_menu_bar.is_some());
        // macOS is the one platform with a definition panel to show, so
        // it is the one where the Look Up row is offered and a force click
        // asks for one (ADR 0017, decision 6).
        core.set_lookup_available(cfg!(target_os = "macos"));
        let access = self
            .proxy
            .clone()
            .and_then(|proxy| access_bridge::Bridge::new(event_loop, &window, proxy));
        #[cfg(target_os = "windows")]
        let nc = (chrome != Chrome::Native)
            .then(|| windows_nc::NcHitTest::install(&window, true))
            .flatten();
        #[cfg(target_os = "windows")]
        let anim_timer = windows_anim::AnimTimer::new(&window);
        // Ask for the deep-click stage before the window is shown: the
        // press that reaches stage 2 is what a force click *is*, and a
        // view that never asked never gets one (ADR 0017, decision 6).
        #[cfg(target_os = "macos")]
        macos_force::configure(&window);
        window.set_visible(true);
        window.set_ime_allowed(true);
        window.request_redraw();
        let appearance = appearance_of(&window);
        self.panes.push(Pane {
            id,
            kind: config.kind,
            owner,
            activates: config.activates,
            chrome,
            applied_min: None,
            anchor: config.anchor,
            core,
            window,
            renderer,
            applied_title: String::new(),
            modifiers: ModifiersState::empty(),
            last_titlebar_press: None,
            cursor: Vec2::ZERO,
            last_click: None,
            blink_visible: true,
            blink_deadline: None,
            caret_stamp_seen: 0,
            resize_edge: None,
            pressure_stage: 0,
            cursor_icon: CursorIcon::Default,
            os_focused: false,
            appearance,
            handed_back: false,
            first_frame: Some((FIRST_FRAME_RETRIES, std::time::Instant::now())),
            deferred_frame: false,
            access,
            #[cfg(target_os = "windows")]
            anim_timer,
            #[cfg(target_os = "windows")]
            nc,
        });
        // Before anything can ask: a host that drives its own loop runs
        // its view as soon as the window exists, which is before the
        // first frame (backlog F39).
        let audio = self.audio.env();
        let pane = self.panes.last_mut().expect("just pushed");
        sync_env(pane, &self.system, self.pinned_system, audio);
    }

    /// The attributes a window of this app is created with: `chrome` —
    /// the launcher's for the app's own windows — and hidden until the
    /// accessibility adapter has hooked it (the platform adapters must see
    /// the window before it is shown).
    pub(super) fn window_attrs(
        &self,
        title: &str,
        (w, h): (f64, f64),
        chrome: Chrome,
    ) -> winit::window::WindowAttributes {
        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(LogicalSize::new(w, h))
            .with_visible(false);
        match chrome {
            Chrome::Native => {}
            Chrome::Custom => {
                // macOS: keep the native traffic lights, drawn over our
                // content; everywhere else drop decorations entirely.
                #[cfg(target_os = "macos")]
                {
                    use winit::platform::macos::WindowAttributesExtMacOS;
                    attrs = attrs
                        .with_titlebar_transparent(true)
                        .with_fullsize_content_view(true)
                        .with_title_hidden(true);
                }
                #[cfg(not(target_os = "macos"))]
                {
                    attrs = attrs.with_decorations(false);
                }
            }
            Chrome::Borderless => attrs = undecorated(attrs),
        }
        attrs
    }

    /// Closes a window other than the main one: the user did (its chrome
    /// or OS close button), or the diff stopped declaring it. The core is
    /// told either way — a window the diff already closed is a no-op there
    /// — and whatever it queues in answer (the `closed` event, and the
    /// `Close` of anything only this window declared) is routed before the
    /// pane goes.
    pub(super) fn close_pane(&mut self, event_loop: &ActiveEventLoop, id: WindowId) {
        let Some(i) = self.pane_of(id) else { return };
        // Whatever this press was about, it is not about this window any
        // more (ADR 0009): a popup chosen from closes here, and its release
        // must not be classified against a surface that has stopped
        // existing.
        self.armed.retain(|a| a.id != id);
        // A non-activating popup that holds the keyboard is about to stop
        // existing, so the keyboard is going *somewhere*: say where, rather
        // than letting the owner read as unfocused until the platform gets
        // round to confirming it. Without this, choosing an item from a
        // menu — the click that closes it — flashes the window behind it
        // unfocused for a frame or two.
        let hand_back = (self.panes[i].kind == WindowKind::Popup
            && !self.panes[i].activates
            && self.panes[i].os_focused)
            .then_some(self.panes[i].owner);
        self.panes[i].core.window_closed(id);
        let events = self.panes[i].core.take_pending_events();
        let cmds = self.panes[i].core.take_window_commands();
        self.panes.remove(i);
        if let Some(j) = hand_back.and_then(|o| self.pane_of(o)) {
            self.panes[j].os_focused = true;
            self.panes[j].window.focus_window();
        }
        self.settle_focus();
        self.route_events(events);
        self.apply_commands(event_loop, cmds);
        for p in &self.panes {
            p.window.request_redraw();
        }
    }
}
