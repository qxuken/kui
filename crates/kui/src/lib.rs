//! Batteries-included runner: winit window + wgpu renderer around a
//! `kui_core::Core`, driving the Elm-ish loop — input becomes `UiEvent`s
//! routed to `App::on_event` (host) or extensions by origin, then `App::view`
//! rebuilds the frame.

use std::sync::Arc;

pub use kui_core::*;
pub use kui_core::widgets;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

pub trait App {
    fn view(&mut self, ui: &mut Ui<'_>);
    fn on_event(&mut self, _ev: UiEvent) {}
}

pub fn run<A: App>(
    title: &str,
    app: A,
    extensions: Vec<Box<dyn Extension>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut shell = Shell {
        title: title.to_string(),
        applied_title: title.to_string(),
        app,
        extensions,
        core: Core::new(),
        window: None,
        renderer: None,
        modifiers: ModifiersState::empty(),
        clipboard: arboard::Clipboard::new().ok(),
    };
    event_loop.run_app(&mut shell)?;
    Ok(())
}

struct Shell<A: App> {
    title: String,
    /// Last title actually set on the window; views declare per frame and
    /// we only touch the window on change.
    applied_title: String,
    app: A,
    extensions: Vec<Box<dyn Extension>>,
    core: Core,
    window: Option<Arc<Window>>,
    renderer: Option<kui_wgpu::Renderer>,
    modifiers: ModifiersState,
    clipboard: Option<arboard::Clipboard>,
}

impl<A: App> Shell<A> {
    fn dispatch(&mut self, ev: InputEvent) {
        let t0 = std::time::Instant::now();
        let events = self.core.handle_input(ev);
        self.route_events(events);
        // Hover styling depends on input too, so any input redraws. A damage
        // pass can tighten this later.
        self.core.stats.pending_input_ms += t0.elapsed().as_secs_f32() * 1e3;
        if let Some(w) = &self.window {
            w.request_redraw();
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
        // Clipboard + select-all shortcuts.
        if self.primary()
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

        if let Some(t) = self.core.window_title()
            && t != self.applied_title
        {
            self.applied_title = t.to_string();
            window.set_title(&self.applied_title);
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
            .unwrap_or((960.0, 640.0));
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(&self.title)
                        .with_inner_size(LogicalSize::new(w, h)),
                )
                .expect("create window"),
        );
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
                self.dispatch(InputEvent::CursorMoved(Vec2::new(
                    position.x as f32 / scale,
                    position.y as f32 / scale,
                )));
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
                self.dispatch(match state {
                    ElementState::Pressed => InputEvent::MouseDown,
                    ElementState::Released => InputEvent::MouseUp,
                });
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }
}

// Re-exported so apps can reach the renderer without depending on kui-wgpu.
pub use kui_wgpu::{RenderError, Renderer, wgpu};
