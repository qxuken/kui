//! Batteries-included runner: winit window + wgpu renderer around a
//! `kui_core::Core`, driving the Elm-ish loop — input becomes `UiEvent`s
//! routed to `App::on_event` (host) or extensions by origin, then `App::view`
//! rebuilds the frame.

use std::sync::Arc;

pub use kui_core::*;
pub use kui_core::widgets;

#[cfg(target_os = "windows")]
mod windows_nc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};
use winit::window::{CursorIcon, ResizeDirection, Window, WindowId};

pub trait App {
    fn view(&mut self, ui: &mut Ui<'_>);
    fn on_event(&mut self, _ev: UiEvent) {}
}

/// Who draws the window chrome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Chrome {
    /// The OS titlebar and buttons (the default).
    #[default]
    Native,
    /// The app draws its own titlebar (`widgets::titlebar`). On macOS the
    /// native traffic lights stay, overlaid on the content (their rect is
    /// reported in `env.window.native_controls`); elsewhere the window is
    /// undecorated and the runner synthesizes edge resizing, double-click
    /// maximize, and applies the `WindowCommand`s chrome nodes produce.
    Custom,
    /// No decorations and no chrome expectations (splash screens, popups).
    Borderless,
}

/// Entry point: `kui::app("title").custom_titlebar().run(my_app)`.
pub fn app(title: &str) -> Launcher {
    Launcher {
        title: title.to_string(),
        chrome: Chrome::Native,
        size: (960.0, 640.0),
        extensions: Vec::new(),
    }
}

/// Builder for the windowed runner.
pub struct Launcher {
    title: String,
    chrome: Chrome,
    size: (f64, f64),
    extensions: Vec<Box<dyn Extension>>,
}

impl Launcher {
    pub fn chrome(mut self, chrome: Chrome) -> Self {
        self.chrome = chrome;
        self
    }

    /// Shorthand for `.chrome(Chrome::Custom)`.
    pub fn custom_titlebar(self) -> Self {
        self.chrome(Chrome::Custom)
    }

    /// Shorthand for `.chrome(Chrome::Borderless)`.
    pub fn borderless(self) -> Self {
        self.chrome(Chrome::Borderless)
    }

    /// Initial inner size, logical px (`KUI_WINDOW=WxH` still overrides).
    pub fn size(mut self, w: f64, h: f64) -> Self {
        self.size = (w, h);
        self
    }

    pub fn extension(mut self, ext: impl Extension + 'static) -> Self {
        self.extensions.push(Box::new(ext));
        self
    }

    pub fn extensions(mut self, exts: Vec<Box<dyn Extension>>) -> Self {
        self.extensions.extend(exts);
        self
    }

    pub fn run<A: App>(self, app: A) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut shell = Shell {
            title: self.title.clone(),
            applied_title: self.title,
            chrome: self.chrome,
            size: self.size,
            app,
            extensions: self.extensions,
            core: Core::new(),
            window: None,
            renderer: None,
            modifiers: ModifiersState::empty(),
            clipboard: arboard::Clipboard::new().ok(),
            last_titlebar_press: None,
            cursor: Vec2::ZERO,
            last_click: None,
            blink_visible: true,
            blink_deadline: None,
            caret_stamp_seen: 0,
            resize_edge: None,
            exit_requested: false,
            #[cfg(target_os = "windows")]
            nc: None,
        };
        event_loop.run_app(&mut shell)?;
        Ok(())
    }
}

pub fn run<A: App>(
    title: &str,
    application: A,
    extensions: Vec<Box<dyn Extension>>,
) -> Result<(), Box<dyn std::error::Error>> {
    app(title).extensions(extensions).run(application)
}

/// Traffic-light keep-out rect under macOS custom chrome (logical px,
/// window coords); content starts at its right edge. AppKit gives no stable
/// public metric, so this uses gpui's measured TRAFFIC_LIGHT_PADDING: 78
/// with the macOS 26 SDK (71 on older SDKs — the extra pixel on the left is
/// the window border). We assume a current SDK.
const MACOS_TRAFFIC_LIGHTS: Rect = Rect { x: 0.0, y: 0.0, w: 78.0, h: 28.0 };
/// Width of the invisible resize band synthesized on undecorated windows.
const RESIZE_BAND: f32 = 6.0;
/// A second titlebar press within this window toggles maximize.
const DOUBLE_CLICK_MS: u128 = 350;
/// Presses within this window (and `MULTI_CLICK_SLOP` px) count up the
/// multi-click sent with `InputEvent::MouseDown` (double = word select).
const MULTI_CLICK_MS: u128 = 400;
const MULTI_CLICK_SLOP: f32 = 4.0;
/// Caret blink half-period while an edit widget is focused.
const BLINK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

struct Shell<A: App> {
    title: String,
    /// Last title actually set on the window; views declare per frame and
    /// we only touch the window on change.
    applied_title: String,
    chrome: Chrome,
    size: (f64, f64),
    app: A,
    extensions: Vec<Box<dyn Extension>>,
    core: Core,
    window: Option<Arc<Window>>,
    renderer: Option<kui_wgpu::Renderer>,
    modifiers: ModifiersState,
    clipboard: Option<arboard::Clipboard>,
    /// Time of the last titlebar press, for double-click maximize.
    last_titlebar_press: Option<std::time::Instant>,
    /// Last cursor position (logical px), for multi-click distance checks.
    cursor: Vec2,
    /// Last primary press: time, position, and its click count.
    last_click: Option<(std::time::Instant, Vec2, u8)>,
    /// Caret blink phase mirror + next toggle time; the clock lives here,
    /// the core only stores the visible flag.
    blink_visible: bool,
    blink_deadline: Option<std::time::Instant>,
    caret_stamp_seen: u64,
    /// Resize edge currently under the cursor (undecorated windows only).
    resize_edge: Option<ResizeDirection>,
    /// Set by `WindowCommand::Close`; honored at the end of the event.
    exit_requested: bool,
    /// Windows: answers WM_NCHITTEST from the frame's chrome regions, which
    /// enables snap layouts + native caption behavior over drawn controls.
    #[cfg(target_os = "windows")]
    nc: Option<windows_nc::NcHitTest>,
}

impl<A: App> Shell<A> {
    fn dispatch(&mut self, ev: InputEvent) {
        let t0 = std::time::Instant::now();
        let events = self.core.handle_input(ev);
        self.route_events(events);
        self.apply_window_commands();
        // Hover styling depends on input too, so any input redraws. A damage
        // pass can tighten this later.
        self.core.stats.pending_input_ms += t0.elapsed().as_secs_f32() * 1e3;
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Applies window intents produced by chrome nodes (`window_drag`,
    /// `window_button`) to the real window.
    fn apply_window_commands(&mut self) {
        for cmd in self.core.take_window_commands() {
            let Some(w) = &self.window else { break };
            match cmd {
                WindowCommand::StartDrag => {
                    let now = std::time::Instant::now();
                    let double = self
                        .last_titlebar_press
                        .take()
                        .is_some_and(|t| now.duration_since(t).as_millis() < DOUBLE_CLICK_MS);
                    if double {
                        w.set_maximized(!w.is_maximized());
                    } else {
                        self.last_titlebar_press = Some(now);
                        let _ = w.drag_window();
                    }
                }
                WindowCommand::Close => self.exit_requested = true,
                WindowCommand::Minimize => w.set_minimized(true),
                WindowCommand::ToggleMaximize => w.set_maximized(!w.is_maximized()),
            }
        }
    }

    /// Undecorated windows get no OS resize borders; the runner synthesizes
    /// them from a band inside the window edges (macOS custom chrome keeps
    /// native edge resizing, and on Windows the non-client subclass answers
    /// WM_NCHITTEST with real border codes instead, so neither synthesizes).
    fn synthesizes_resize(&self) -> bool {
        #[cfg(target_os = "windows")]
        if self.nc.is_some() {
            return false;
        }
        self.chrome != Chrome::Native && !cfg!(target_os = "macos")
    }

    fn resize_edge_at(&self, p: Vec2) -> Option<ResizeDirection> {
        let w = self.window.as_ref()?;
        if w.is_maximized() || w.fullscreen().is_some() {
            return None;
        }
        let scale = w.scale_factor() as f32;
        let size = w.inner_size();
        let (sw, sh) = (size.width as f32 / scale, size.height as f32 / scale);
        let (l, r) = (p.x < RESIZE_BAND, p.x > sw - RESIZE_BAND);
        let (t, b) = (p.y < RESIZE_BAND, p.y > sh - RESIZE_BAND);
        Some(match (l, r, t, b) {
            (true, _, true, _) => ResizeDirection::NorthWest,
            (_, true, true, _) => ResizeDirection::NorthEast,
            (true, _, _, true) => ResizeDirection::SouthWest,
            (_, true, _, true) => ResizeDirection::SouthEast,
            (true, ..) => ResizeDirection::West,
            (_, true, ..) => ResizeDirection::East,
            (_, _, true, _) => ResizeDirection::North,
            (_, _, _, true) => ResizeDirection::South,
            _ => return None,
        })
    }

    fn update_resize_cursor(&mut self, p: Vec2) {
        let edge = self.resize_edge_at(p);
        if edge == self.resize_edge {
            return;
        }
        self.resize_edge = edge;
        if let Some(w) = &self.window {
            w.set_cursor(match edge {
                Some(ResizeDirection::West | ResizeDirection::East) => CursorIcon::EwResize,
                Some(ResizeDirection::North | ResizeDirection::South) => CursorIcon::NsResize,
                Some(ResizeDirection::NorthWest | ResizeDirection::SouthEast) => {
                    CursorIcon::NwseResize
                }
                Some(ResizeDirection::NorthEast | ResizeDirection::SouthWest) => {
                    CursorIcon::NeswResize
                }
                None => CursorIcon::Default,
            });
        }
    }

    fn route_events(&mut self, events: Vec<UiEvent>) {
        for ev in events {
            if ev.origin == OriginId::HOST {
                self.app.on_event(ev);
            } else if let Some(ext) = self.extensions.get_mut(ev.origin.0 as usize - 1) {
                ext.on_event(&ev);
            }
        }
    }

    /// Direct edits (cut) mutate the document outside handle_input, so they
    /// must notify + redraw explicitly.
    fn after_direct_edit(&mut self) {
        if let Some(key) = self.core.edit.focused() {
            let origin = self.core.edit.origin_of(key).unwrap_or(OriginId::HOST);
            self.route_events(vec![UiEvent {
                origin,
                key,
                payload: Value::map([("kind", "changed".into())]),
            }]);
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn mods(&self) -> Mods {
        Mods {
            shift: self.modifiers.shift_key(),
            word: self.modifiers.alt_key(),
            doc: if cfg!(target_os = "macos") {
                self.modifiers.super_key()
            } else {
                self.modifiers.control_key()
            },
        }
    }

    /// The platform primary shortcut modifier (Cmd on macOS, Ctrl elsewhere).
    fn primary(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.modifiers.super_key()
        } else {
            self.modifiers.control_key()
        }
    }

    fn on_key(&mut self, event: winit::event::KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }
        // Full-keyboard path: every press also travels as data to the
        // key-focused sink (`NodeSpec::on_key`); the core drops it when
        // an edit widget holds focus instead.
        let kmods = KeyMods {
            shift: self.modifiers.shift_key(),
            ctrl: self.modifiers.control_key(),
            alt: self.modifiers.alt_key(),
            super_key: self.modifiers.super_key(),
        };
        let plain = !kmods.ctrl && !kmods.alt && !kmods.super_key;
        // With Alt held the logical key is the composed character on some
        // layouts (macOS ⌥o → "ø"); chords want the layout key, so report
        // the modifier-stripped one instead.
        let logical = if kmods.alt {
            use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
            event.key_without_modifiers()
        } else {
            event.logical_key.clone()
        };
        let (code, ktext) = match &logical {
            WinitKey::Character(s) => (
                KeyCode::Char(s.chars().next().unwrap_or('\u{fffd}')),
                plain.then(|| s.to_string()),
            ),
            WinitKey::Named(n) => (
                match n {
                    NamedKey::Space => KeyCode::Space,
                    NamedKey::ArrowLeft => KeyCode::Left,
                    NamedKey::ArrowRight => KeyCode::Right,
                    NamedKey::ArrowUp => KeyCode::Up,
                    NamedKey::ArrowDown => KeyCode::Down,
                    NamedKey::Home => KeyCode::Home,
                    NamedKey::End => KeyCode::End,
                    NamedKey::PageUp => KeyCode::PageUp,
                    NamedKey::PageDown => KeyCode::PageDown,
                    NamedKey::Backspace => KeyCode::Backspace,
                    NamedKey::Delete => KeyCode::Delete,
                    NamedKey::Enter => KeyCode::Enter,
                    NamedKey::Tab => KeyCode::Tab,
                    NamedKey::Escape => KeyCode::Escape,
                    NamedKey::Insert => KeyCode::Insert,
                    NamedKey::F1 => KeyCode::F(1),
                    NamedKey::F2 => KeyCode::F(2),
                    NamedKey::F3 => KeyCode::F(3),
                    NamedKey::F4 => KeyCode::F(4),
                    NamedKey::F5 => KeyCode::F(5),
                    NamedKey::F6 => KeyCode::F(6),
                    NamedKey::F7 => KeyCode::F(7),
                    NamedKey::F8 => KeyCode::F(8),
                    NamedKey::F9 => KeyCode::F(9),
                    NamedKey::F10 => KeyCode::F(10),
                    NamedKey::F11 => KeyCode::F(11),
                    NamedKey::F12 => KeyCode::F(12),
                    _ => KeyCode::Unknown,
                },
                (plain && *n == NamedKey::Space).then(|| " ".to_string()),
            ),
            _ => (KeyCode::Unknown, None),
        };
        if code != KeyCode::Unknown {
            self.dispatch(InputEvent::KeyDown(KeyPress {
                code,
                mods: kmods,
                text: ktext,
                repeat: event.repeat,
            }));
        }

        // Clipboard + select-all shortcuts (edit widgets only — a key
        // sink gets the raw chord and brings its own bindings).
        if self.core.edit.focused().is_some()
            && self.primary()
            && let WinitKey::Character(c) = &event.logical_key
        {
            match c.to_lowercase().as_str() {
                "c" => {
                    if let (Some(text), Some(cb)) =
                        (self.core.copy_selection(), self.clipboard.as_mut())
                    {
                        let _ = cb.set_text(text);
                    }
                    return;
                }
                "x" => {
                    if let Some(text) = self.core.cut_selection() {
                        if let Some(cb) = self.clipboard.as_mut() {
                            let _ = cb.set_text(text);
                        }
                        self.after_direct_edit();
                    }
                    return;
                }
                "v" => {
                    if let Some(text) = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok()) {
                        self.dispatch(InputEvent::Text(text));
                    }
                    return;
                }
                "a" => {
                    self.dispatch(InputEvent::Key(EditKey::SelectAll, Mods::default()));
                    return;
                }
                _ => {}
            }
        }

        let named = match &event.logical_key {
            WinitKey::Named(n) => Some(match n {
                NamedKey::ArrowLeft => EditKey::Left,
                NamedKey::ArrowRight => EditKey::Right,
                NamedKey::ArrowUp => EditKey::Up,
                NamedKey::ArrowDown => EditKey::Down,
                NamedKey::Home => EditKey::Home,
                NamedKey::End => EditKey::End,
                NamedKey::PageUp => EditKey::PageUp,
                NamedKey::PageDown => EditKey::PageDown,
                NamedKey::Backspace => EditKey::Backspace,
                NamedKey::Delete => EditKey::Delete,
                NamedKey::Enter => EditKey::Enter,
                NamedKey::Tab => EditKey::Tab,
                NamedKey::Escape => EditKey::Escape,
                NamedKey::Space => {
                    self.dispatch(InputEvent::Text(" ".to_string()));
                    return;
                }
                _ => return,
            }),
            _ => None,
        };
        if let Some(key) = named {
            let mods = self.mods();
            self.dispatch(InputEvent::Key(key, mods));
            return;
        }
        // Plain typed text (IME commits arrive via WindowEvent::Ime).
        if !self.primary()
            && !self.modifiers.control_key()
            && let Some(text) = &event.text
            && text.chars().any(|c| !c.is_control())
        {
            self.dispatch(InputEvent::Text(text.to_string()));
        }
    }

    fn redraw(&mut self) {
        let (Some(window), Some(renderer)) = (&self.window, &mut self.renderer) else {
            return;
        };
        let scale = window.scale_factor() as f32;
        let size = window.inner_size();
        let viewport = Size::new(size.width as f32 / scale, size.height as f32 / scale);
        // Per-frame so it self-corrects when the window moves to another
        // monitor.
        self.core.env.refresh_hz = window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .map(|mhz| mhz as f32 / 1000.0);
        self.core.env.window = WindowEnv {
            custom_chrome: self.chrome != Chrome::Native,
            maximized: window.is_maximized(),
            fullscreen: window.fullscreen().is_some(),
            native_controls: (cfg!(target_os = "macos") && self.chrome == Chrome::Custom)
                .then_some(MACOS_TRAFFIC_LIGHTS),
        };

        let t_view = std::time::Instant::now();
        let mut ui = self.core.frame(viewport, scale);
        self.app.view(&mut ui);
        for (i, ext) in self.extensions.iter_mut().enumerate() {
            ui.set_origin(OriginId(i as u16 + 1));
            if let Err(err) = ext.view(&mut ui) {
                eprintln!("kui: extension '{}' view error: {err}", ext.name());
                ui.text(
                    &format!("[{}] {err}", ext.name()),
                    TextStyle::new(13.0).color(Color::rgb8(0xe8, 0x5d, 0x5d)),
                );
            }
        }
        ui.set_origin(OriginId::HOST);
        let view_ms = t_view.elapsed().as_secs_f32() * 1e3;

        let t_layout = std::time::Instant::now();
        ui.finish();
        let layout_ms = t_layout.elapsed().as_secs_f32() * 1e3;

        // Mirror this frame's hit regions into the WM_NCHITTEST answerer.
        #[cfg(target_os = "windows")]
        if let Some(nc) = &self.nc {
            nc.update(
                scale,
                window.is_maximized(),
                self.core
                    .interaction
                    .hits()
                    .iter()
                    .map(|h| (h.window, h.rect.intersect(&h.clip))),
            );
        }

        if let Some(t) = self.core.window_title()
            && t != self.applied_title
        {
            self.applied_title = t.to_string();
            window.set_title(&self.applied_title);
        }

        // Anchor the OS IME candidate window at the focused caret.
        if let Some(r) = self.core.ime_rect() {
            window.set_ime_cursor_area(
                winit::dpi::LogicalPosition::new(r.x, r.y),
                winit::dpi::LogicalSize::new(r.w.max(1.0), r.h),
            );
        }

        let t_render = std::time::Instant::now();
        let (dl, atlas) = self.core.output();
        let mut wait_ms = 0.0;
        match renderer.render(dl, atlas) {
            Ok(report) => wait_ms = report.vsync_wait_ms,
            Err(kui_wgpu::RenderError::Reconfigure) => {
                renderer.resize(size.width, size.height);
                window.request_redraw();
            }
            // Occluded or timed out: nothing to present, try next frame.
            Err(kui_wgpu::RenderError::Skip) => {}
            Err(err) => eprintln!("kui: render error: {err}"),
        }
        let render_ms = (t_render.elapsed().as_secs_f32() * 1e3 - wait_ms).max(0.0);

        self.core.stats.push(FrameSample { input_ms: 0.0, view_ms, layout_ms, render_ms, wait_ms });
    }
}

impl<A: App> ApplicationHandler for Shell<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // KUI_WINDOW=WxH overrides the initial size (useful for testing).
        let (w, h) = std::env::var("KUI_WINDOW")
            .ok()
            .and_then(|s| {
                let (w, h) = s.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            })
            .unwrap_or(self.size);
        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes()
            .with_title(&self.title)
            .with_inner_size(LogicalSize::new(w, h));
        match self.chrome {
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
            Chrome::Borderless => attrs = attrs.with_decorations(false),
        }
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        #[cfg(target_os = "windows")]
        if self.chrome != Chrome::Native {
            self.nc = windows_nc::NcHitTest::install(&window, true);
        }
        window.set_ime_allowed(true);
        let size = window.inner_size();
        let renderer =
            pollster::block_on(kui_wgpu::Renderer::new(window.clone(), size.width, size.height))
                .expect("init renderer");
        self.window = Some(window);
        self.renderer = Some(renderer);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.window.as_ref().map_or(1.0, |w| w.scale_factor()) as f32;
                let p = Vec2::new(position.x as f32 / scale, position.y as f32 / scale);
                if self.synthesizes_resize() {
                    self.update_resize_cursor(p);
                }
                self.cursor = p;
                self.dispatch(InputEvent::CursorMoved(p));
            }
            WindowEvent::CursorLeft { .. } => self.dispatch(InputEvent::CursorLeft),
            WindowEvent::Focused(focused) => {
                self.core.env.focused = focused;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, .. } => self.on_key(event),
            WindowEvent::Ime(Ime::Commit(text)) => self.dispatch(InputEvent::Text(text)),
            WindowEvent::Ime(Ime::Preedit(text, cursor)) => {
                self.dispatch(InputEvent::Preedit(text, cursor));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scale = self.window.as_ref().map_or(1.0, |w| w.scale_factor()) as f32;
                let d = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        Vec2::new(Core::lines_to_px(x), Core::lines_to_px(y))
                    }
                    winit::event::MouseScrollDelta::PixelDelta(p) => {
                        Vec2::new(p.x as f32 / scale, p.y as f32 / scale)
                    }
                };
                self.dispatch(InputEvent::Scroll(d));
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                // A press on the synthesized resize band starts an OS resize
                // instead of reaching the UI.
                if state == ElementState::Pressed
                    && let Some(dir) = self.resize_edge
                    && let Some(w) = &self.window
                {
                    let _ = w.drag_resize_window(dir);
                    return;
                }
                let ev = match state {
                    ElementState::Pressed => {
                        let now = std::time::Instant::now();
                        let clicks = match self.last_click {
                            Some((t, p, n))
                                if now.duration_since(t).as_millis() < MULTI_CLICK_MS
                                    && (p.x - self.cursor.x).abs() < MULTI_CLICK_SLOP
                                    && (p.y - self.cursor.y).abs() < MULTI_CLICK_SLOP =>
                            {
                                // Cycle 1 → 2 → 3 → 1 like most editors.
                                n % 3 + 1
                            }
                            _ => 1,
                        };
                        self.last_click = Some((now, self.cursor, clicks));
                        InputEvent::MouseDown(clicks)
                    }
                    ElementState::Released => InputEvent::MouseUp,
                };
                self.dispatch(ev);
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                // Views can declare window commands too (ui.window_command);
                // apply them the same frame they were declared.
                self.apply_window_commands();
            }
            _ => {}
        }
        if self.exit_requested {
            event_loop.exit();
        }
    }

    /// Runs after every event batch (including timer wake-ups): the caret
    /// blink clock. Any caret activity re-arms the timer with the caret
    /// solid; each expiry toggles the phase and schedules the next.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.core.edit.focused().is_none() {
            if !self.blink_visible {
                self.blink_visible = true;
                self.core.edit.set_blink_visible(true);
            }
            self.blink_deadline = None;
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let now = std::time::Instant::now();
        let stamp = self.core.edit.caret_stamp();
        if stamp != self.caret_stamp_seen || self.blink_deadline.is_none() {
            self.caret_stamp_seen = stamp;
            self.blink_deadline = Some(now + BLINK_INTERVAL);
            if !self.blink_visible {
                self.blink_visible = true;
                self.core.edit.set_blink_visible(true);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
        } else if now >= self.blink_deadline.unwrap() {
            self.blink_visible = !self.blink_visible;
            self.core.edit.set_blink_visible(self.blink_visible);
            self.blink_deadline = Some(now + BLINK_INTERVAL);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.blink_deadline.unwrap()));
    }
}

// Re-exported so apps can reach the renderer without depending on kui-wgpu.
pub use kui_wgpu::{RenderError, Renderer, wgpu};
