//! The shell's windows: finding a pane by either id, and applying the
//! window commands every core queues — chrome intents on the window they
//! name, and the `Open` / `Close` the declared set's diff produced
//!. Split off `lib.rs` as a pure move.

use super::*;

impl DynShell<'_> {
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
                        // A popup is a menu surface: no chrome whatever the
                        // launcher asked for (backlog AR32 — under `Custom`
                        // its outer 6 px answered resize cursors and the
                        // app's caption regions).
                        let chrome = if origin == OriginId::DEVTOOLS {
                            Chrome::Native
                        } else if config.kind == WindowKind::Popup {
                            Chrome::Borderless
                        } else {
                            self.chrome
                        };
                        // The app's backdrop on the app's windows, and the
                        // same two exceptions: the devtools draw an opaque
                        // panel, and a popup is a menu surface.
                        let backdrop =
                            if origin == OriginId::DEVTOOLS || config.kind == WindowKind::Popup {
                                Backdrop::Opaque
                            } else {
                                self.backdrop
                            };
                        self.open_pane(event_loop, id, owner, config, chrome, backdrop);
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
                        self.panes[i].redraw_for(FrameCause::ELSEWHERE);
                    }
                }
            }
        }
    }

    /// Opens the window a frame declared, on the shared session and device,
    /// with `chrome` and `backdrop` — the launcher's for the app's windows.
    pub(super) fn open_pane(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        owner: WindowId,
        config: WindowConfig,
        chrome: Chrome,
        backdrop: Backdrop,
    ) {
        let size = if config.size.w > 0.0 && config.size.h > 0.0 {
            config.size
        } else {
            WindowConfig::DEFAULT_SIZE
        };
        // Untitled until its first frame's `window_title` lands (ADR 0004
        // decision 5): the declaration carries no string.
        let mut attrs = self
            .window_attrs("", (size.w as f64, size.h as f64), chrome, backdrop)
            .with_active(config.activates);
        if config.kind == WindowKind::Popup {
            // A menu surface, not a window with the app's chrome: no
            // decorations whatever the launcher asked for, not resizable
            // (AppKit keeps edge resizing on a hidden-titlebar window and
            // Win32 keeps `WS_THICKFRAME` undecorated, so a menu's edge
            // could be dragged — backlog AR32), above its owner, and placed
            // against the anchor rather than wherever the window manager
            // would have put a new window.
            attrs = undecorated(attrs)
                .with_resizable(false)
                .with_window_level(winit::window::WindowLevel::AlwaysOnTop);
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
                self.refuse_pane(event_loop, id);
                return;
            }
        };
        self.icon.set_on(&window);
        // The Window menu lists windows the user would switch to, and a
        // popup is not one (ADR 0030).
        #[cfg(target_os = "macos")]
        if config.kind == WindowKind::Popup {
            macos_menu::exclude_from_windows_menu(&window);
        }
        let Some(gpu) = self.gpu.clone() else {
            eprintln!("kui: cannot open window {}: no device yet", id.0);
            self.refuse_pane(event_loop, id);
            return;
        };
        let px = window.inner_size();
        let renderer = match kui_wgpu::Renderer::new_in_with(
            &gpu,
            window.clone(),
            px.width,
            px.height,
            backdrop.is_translucent(),
        ) {
            Ok(mut r) => {
                r.set_frame_latency(self.frame_latency);
                r
            }
            Err(err) => {
                eprintln!("kui: cannot open window {}: {err}", id.0);
                self.refuse_pane(event_loop, id);
                return;
            }
        };
        let applied = crate::backdrop::apply(&window, &renderer, backdrop);
        // Before it is shown: AppKit makes a window key on ordering it
        // front and on every press, and asks `canBecomeKeyWindow` first —
        // so a non-activating window answers NO (`macos_key`), and its
        // owner's chrome never greys for the batch the hand-back below
        // used to take (backlog W18). The hand-back stays for a platform
        // that makes it key anyway.
        #[cfg(target_os = "macos")]
        if !config.activates {
            macos_key::refuse_key(&window);
        }
        let mut core = Core::new_in(&self.session);
        core.set_diagnostics(self.diagnostics);
        core.set_subpixel_text(self.subpixel);
        core.env.window.id = id;
        self.push_pane(
            event_loop, id, config, owner, chrome, core, window, renderer, applied,
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
    ) -> Option<winit::dpi::Position> {
        let pane = self.panes.get(self.pane_of(owner)?)?;
        let scale = pane.window.scale_factor();
        // Worked in the platform's own frame (`retarget`'s module doc):
        // points on macOS, physical pixels elsewhere — a logical position
        // handed to winit on Windows resolves against the monitor the
        // window is *created* on, which is not the owner's when the two
        // differ in scale (backlog AR33). `unit` is the frame's units per
        // logical px of the owner.
        let points = cfg!(target_os = "macos");
        let unit = if points { 1.0 } else { scale };
        let raw = pane.window.inner_position().ok()?;
        let origin = if points {
            let l = raw.to_logical::<f64>(scale);
            (l.x, l.y)
        } else {
            (raw.x as f64, raw.y as f64)
        };
        let a = config.anchor;
        let x = origin.0 + a.x as f64 * unit;
        let below = origin.1 + (a.y + a.h) as f64 * unit;
        // The monitor the owner is on, in the same frame. With no monitor
        // to ask, "below" is the answer and the WM may move it.
        let y = match pane.window.current_monitor() {
            Some(m) => {
                let (top, height) = if points {
                    (
                        m.position().to_logical::<f64>(scale).y,
                        m.size().to_logical::<f64>(scale).height,
                    )
                } else {
                    (m.position().y as f64, m.size().height as f64)
                };
                if below + size.h as f64 * unit > top + height {
                    // Above the anchor instead, unless there is even less
                    // room up there — then stay below and let it clip.
                    let above = origin.1 + a.y as f64 * unit - size.h as f64 * unit;
                    if above >= top { above } else { below }
                } else {
                    below
                }
            }
            None => below,
        };
        Some(if points {
            LogicalPosition::new(x, y).into()
        } else {
            winit::dpi::PhysicalPosition::new(x, y).into()
        })
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
        applied: crate::backdrop::Applied,
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
        // The titlebar the launcher asked for, where the strip under
        // custom chrome is the app's alone: its height is the
        // `titlebar_h` metric, and the launcher's ask is that window's
        // platform height (backlog W22). On macOS it is AppKit's
        // titlebar instead, set on the window below.
        #[cfg(not(target_os = "macos"))]
        if chrome == Chrome::Custom && config.kind != WindowKind::Popup {
            core.set_platform_titlebar_h(self.titlebar.strip_h());
        }
        let access = self
            .proxy
            .clone()
            .and_then(|proxy| access_bridge::Bridge::new(event_loop, &window, proxy));
        // The non-client hit test answers resize borders and the caption
        // for a window with the app's chrome; a popup has neither (AR32).
        #[cfg(target_os = "windows")]
        let nc = (chrome != Chrome::Native && config.kind != WindowKind::Popup)
            .then(|| windows_nc::NcHitTest::install(&window, true))
            .flatten();
        #[cfg(target_os = "windows")]
        let anim_timer = windows_anim::AnimTimer::new(&window);
        // Every window, popups too: each hears the switch on its own copy
        // of the broadcast, and winit sets each one's dark mode (RG106).
        #[cfg(target_os = "windows")]
        windows_theme::install(&window);
        // Ask for the deep-click stage before the window is shown: the
        // press that reaches stage 2 is what a force click *is*, and a
        // view that never asked never gets one (ADR 0017, decision 6).
        #[cfg(target_os = "macos")]
        macos_force::configure(&window);
        // The titlebar's height under custom chrome (backlog W22), before
        // the window is shown so the lights do not jump, and before they
        // are measured below.
        #[cfg(target_os = "macos")]
        if chrome == Chrome::Custom {
            macos_chrome::set_titlebar(&window, self.titlebar);
        }
        window.set_visible(true);
        window.set_ime_allowed(true);
        // Answer the palette and dictation, which winit's view does not
        // (W15); the class is patched once, the view registered per window.
        #[cfg(target_os = "macos")]
        macos_text_input::attach(&window);
        // And answer a file drag with where it is, which winit's delegate
        // does not (ADR 0031); patched once, the delegate registered per
        // window.
        #[cfg(target_os = "macos")]
        macos_drop::attach(&window);
        window.request_redraw();
        let appearance = appearance_of(&window);
        // Where the OS's own controls are, now that there is a window to
        // ask: winit has no getter, and the numbers moved between macOS
        // releases (backlog W17).
        #[cfg(target_os = "macos")]
        let native_controls =
            (chrome == Chrome::Custom).then(|| macos_chrome::native_controls(&window));
        #[cfg(not(target_os = "macos"))]
        let native_controls = None;
        let pacer = crate::pacer::Pacer::new(&window, !self.pumped);
        self.panes.push(Pane {
            id,
            kind: config.kind,
            owner,
            activates: config.activates,
            chrome,
            backdrop: applied.got,
            // Read on a thread; the loop is woken when it lands.
            ground: applied.ground.map(|source| {
                let proxy = self.proxy.clone();
                crate::ground::Ground::new(crate::ground::spawn(source, move || {
                    if let Some(p) = proxy {
                        let _ = p.send_event(access_bridge::UserEvent::Wake);
                    }
                }))
            }),
            _backdrop_keep: applied.keep,
            native_controls,
            applied_min: None,
            anchor: config.anchor,
            // A popup opened on top (see above); everything else opens Normal.
            applied_on_top: config.kind == WindowKind::Popup,
            applied_option_as_alt: kui_core::OptionAsAlt::None,
            applied_ime_off: false,
            ime_off_held: Vec::new(),
            preedit_open: false,
            level_supported: level_supported(&window),
            core,
            window,
            renderer: Some(renderer),
            surface_tries: 0,
            awaits_device: false,
            applied_title: String::new(),
            modifiers: ModifiersState::empty(),
            modifier_keys_down: Vec::new(),
            alt_held: (false, false),
            last_titlebar_press: None,
            cursor: Vec2::ZERO,
            last_click: None,
            blink_visible: true,
            blink_deadline: None,
            caret_stamp_seen: 0,
            resize_edge: None,
            pressure_stage: 0,
            axis_lock: Default::default(),
            scroll_gesture: Default::default(),
            file_drag: Vec::new(),
            file_drag_pending: None,
            cursor_icon: CursorIcon::Default,
            os_focused: false,
            appearance,
            handed_back: false,
            retry: crate::retry::Retry::new(std::time::Instant::now()),
            deferred_frame: false,
            cause: crate::pane::Causes::new(FrameCause::FIRST),
            pacer,
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

    /// The attributes a window of this app is created with: `chrome` and
    /// `backdrop` — the launcher's for the app's own windows — the
    /// launcher's icon, and hidden until the accessibility adapter has
    /// hooked it (the platform adapters must see the window before it is
    /// shown).
    pub(super) fn window_attrs(
        &self,
        title: &str,
        (w, h): (f64, f64),
        chrome: Chrome,
        backdrop: Backdrop,
    ) -> winit::window::WindowAttributes {
        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(LogicalSize::new(w, h))
            .with_visible(false);
        attrs = self.icon.apply(attrs);
        attrs = crate::backdrop::attrs(attrs, backdrop);
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
                // Windows 11 draws an undecorated window square and flat;
                // an app window drawing its own titlebar keeps the corners
                // and the shadow every other window has (backlog RG51).
                #[cfg(target_os = "windows")]
                {
                    use winit::platform::windows::{CornerPreference, WindowAttributesExtWindows};
                    attrs = attrs
                        .with_undecorated_shadow(true)
                        .with_corner_preference(CornerPreference::Round);
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
    /// The OS (or the device) refused a window the registry has opened
    /// and the app has already heard `opened` for: told to the registry
    /// as a close, the way an OS close is, so the app hears `closed`,
    /// `windows()` stops listing it, and declaring the name again reopens
    /// it — instead of an id that is live forever and answers nothing
    /// (AR22). Through the owner's core, since no pane of its own exists.
    fn refuse_pane(&mut self, event_loop: &ActiveEventLoop, id: WindowId) {
        let Some(core) = self.panes.first_mut().map(|p| &mut p.core) else {
            return;
        };
        core.window_closed(id);
        let events = core.take_pending_events();
        let cmds = core.take_window_commands();
        self.route_events(events);
        self.apply_commands(event_loop, cmds);
    }

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
        #[cfg(target_os = "macos")]
        {
            macos_text_input::detach(&self.panes[i].window);
            macos_drop::detach(&self.panes[i].window);
            macos_key::release(&self.panes[i].window);
        }
        self.panes.remove(i);
        if let Some(j) = hand_back.and_then(|o| self.pane_of(o)) {
            self.panes[j].os_focused = true;
            self.panes[j].window.focus_window();
        }
        self.settle_focus();
        self.route_events(events);
        self.apply_commands(event_loop, cmds);
        for p in &self.panes {
            p.redraw_for(FrameCause::ELSEWHERE);
        }
    }
}
