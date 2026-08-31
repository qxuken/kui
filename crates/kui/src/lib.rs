//! Batteries-included runner: winit window + wgpu renderer around a
//! `kui_core::Core`, driving the Elm-ish loop — input becomes `UiEvent`s
//! routed to `App::on_event` (host) or extensions by origin, then `App::view`
//! rebuilds the frame.

use std::sync::Arc;

pub use kui_core::*;
pub use kui_core::widgets;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
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
        app,
        extensions,
        core: Core::new(),
        window: None,
        renderer: None,
    };
    event_loop.run_app(&mut shell)?;
    Ok(())
}

struct Shell<A: App> {
    title: String,
    app: A,
    extensions: Vec<Box<dyn Extension>>,
    core: Core,
    window: Option<Arc<Window>>,
    renderer: Option<kui_wgpu::Renderer>,
}

impl<A: App> Shell<A> {
    fn dispatch(&mut self, ev: InputEvent) {
        let events = self.core.handle_input(ev);
        let had_events = !events.is_empty();
        for ev in events {
            if ev.origin == OriginId::HOST {
                self.app.on_event(ev);
            } else if let Some(ext) = self.extensions.get_mut(ev.origin.0 as usize - 1) {
                ext.on_event(&ev);
            }
        }
        // Hover styling depends on input too, so any input redraws. A damage
        // pass can tighten this later.
        let _ = had_events;
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn redraw(&mut self) {
        let (Some(window), Some(renderer)) = (&self.window, &mut self.renderer) else {
            return;
        };
        let scale = window.scale_factor() as f32;
        let size = window.inner_size();
        let viewport = Size::new(size.width as f32 / scale, size.height as f32 / scale);

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
        ui.finish();

        let (dl, atlas) = self.core.output();
        match renderer.render(dl, atlas) {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                renderer.resize(size.width, size.height);
                window.request_redraw();
            }
            Err(err) => eprintln!("kui: render error: {err}"),
        }
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
pub use kui_wgpu::{Renderer, wgpu};
