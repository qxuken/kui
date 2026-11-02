//! Replicator Bloom over a host-owned wgpu world: one encoder, one submit.
//! Includes the stock latency HUD fed by host input, view, layout, render and wait spans.
//!
//! Run: cargo run -p kui-native --example host_pass --features host-pass

mod game;
mod hud;
mod input;
mod render;

use std::sync::Arc;
use std::time::Instant;

use game::Game;
use kui_access::HostBridge;
use kui_core::{Core, FrameSample, Size};
use render::Render;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Arc<Window>>,
    render: Option<Render>,
    access: Option<HostBridge>,
    core: Core,
    game: Game,
    input: input::Input,
    started: Instant,
    previous: Instant,
    remaining: Option<u64>,
    frames: u64,
}

impl App {
    fn new(max_frames: Option<u64>) -> Self {
        let mut core = Core::new();
        core.arm_host_hit();
        core.set_focused(true);
        Self {
            window: None,
            render: None,
            access: None,
            core,
            game: Game::new(1100.0, 700.0),
            input: input::Input::default(),
            started: Instant::now(),
            previous: Instant::now(),
            remaining: max_frames,
            frames: 0,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Kui host pass")
            .with_inner_size(LogicalSize::new(1100.0, 700.0))
            .with_visible(false);
        let window = Arc::new(event_loop.create_window(attributes).expect("create window"));
        self.access = input::attach_access(&window);
        window.set_visible(true);
        let size = window.inner_size();
        let render =
            Render::new(window.clone(), size.width, size.height).expect("initialize renderer");
        self.core.set_subpixel_text(render.subpixel_text());
        self.render = Some(render);
        self.window = Some(window);
        self.previous = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
            return;
        }
        if let WindowEvent::Resized(size) = event
            && let Some(render) = &mut self.render
        {
            render.resize(size.width, size.height);
        }
        let input_started = Instant::now();
        self.core.set_time(self.started.elapsed().as_secs_f64());
        if let Some(bridge) = &mut self.access {
            let events = bridge.apply(&mut self.core);
            hud::apply(&mut self.game, &events);
        }
        let outcome = self.input.handle(
            &mut self.core,
            &mut self.game,
            &event,
            window.scale_factor() as f32,
        );
        hud::apply(&mut self.game, &outcome.events);
        self.core.stats.pending_input_ms += input_started.elapsed().as_secs_f32() * 1000.0;
        if outcome.close {
            event_loop.exit();
            return;
        }
        if !matches!(event, WindowEvent::RedrawRequested) {
            return;
        }
        if self.remaining == Some(0) {
            event_loop.exit();
            return;
        }
        let now = Instant::now();
        self.game
            .tick(now.duration_since(self.previous).as_secs_f32());
        self.previous = now;
        let scale = window.scale_factor() as f32;
        let logical = window.inner_size().to_logical::<f32>(scale.into());
        let mut ui = self
            .core
            .frame(Size::new(logical.width, logical.height), scale);
        let view_started = Instant::now();
        hud::view(&self.game, &mut ui);
        let view_ms = view_started.elapsed().as_secs_f32() * 1000.0;
        let layout_started = Instant::now();
        ui.finish();
        let events = self.core.take_pending_events();
        hud::apply(&mut self.game, &events);
        let layout_ms = layout_started.elapsed().as_secs_f32() * 1000.0;
        if let Some(bridge) = &mut self.access {
            bridge.publish(&mut self.core);
        }
        if let Some(render) = &mut self.render {
            match render.frame(&mut self.core, &self.game, logical.width, logical.height) {
                Ok(times) => self.core.stats.push(FrameSample {
                    input_ms: 0.0,
                    view_ms,
                    layout_ms,
                    render_ms: times.render_ms,
                    wait_ms: times.wait_ms,
                }),
                Err(error) => {
                    eprintln!("{error}");
                    event_loop.exit();
                    return;
                }
            }
        }
        self.frames += 1;
        if let Some(left) = &mut self.remaining {
            *left = left.saturating_sub(1);
            if *left == 0 {
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        println!(
            "game: {} frames, gold {}, drones {}",
            self.frames, self.game.gold, self.game.drones
        );
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let max_frames = std::env::args()
        .skip(1)
        .position(|arg| arg == "--frames")
        .and_then(|index| std::env::args().nth(index + 2))
        .and_then(|value| value.parse::<u64>().ok());
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(max_frames);
    event_loop.run_app(&mut app)?;
    Ok(())
}
