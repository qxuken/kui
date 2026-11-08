//! A single-canvas browser host for kui. The app and its core stay in Wasm.
//! The browser supplies Canvas imports or a canvas for the shared wgpu renderer.

#![cfg(target_arch = "wasm32")]

mod access;
#[cfg(not(feature = "rust-wgpu"))]
mod canvas;
#[cfg(feature = "rust-wgpu")]
mod wgpu_painter;

#[cfg(feature = "rust-wgpu")]
pub use access::set_browser_callbacks;
#[cfg(feature = "rust-wgpu")]
pub use kui_wgpu::RenderError;
#[cfg(feature = "rust-wgpu")]
pub use wgpu_painter::WgpuPainter;

use kui_core::{
    AccessAction, AccessRequest, Core, CursorShape, InputEvent, Key, KeyPress, Platform, Session,
    Size, Ui, UiEvent,
};

pub trait App {
    fn view(&mut self, ui: &mut Ui<'_>);
    fn on_event(&mut self, _event: UiEvent) {}
    fn on_event_with(&mut self, event: UiEvent, _core: &mut Core) {
        self.on_event(event);
    }
    fn before_frame(&mut self, _core: &mut Core) {}
}

pub struct Runner<A> {
    app: A,
    core: Core,
    access: access::Mirror,
    #[cfg(not(feature = "rust-wgpu"))]
    painter: canvas::Painter,
    #[cfg(feature = "rust-wgpu")]
    painter: Option<WgpuPainter>,
}

impl<A: App> Runner<A> {
    /// The caller supplies a session with bundled fonts and platform policy.
    pub fn new(app: A, session: &Session) -> Self {
        Self {
            app,
            core: Core::new_in(session),
            access: access::Mirror::default(),
            #[cfg(not(feature = "rust-wgpu"))]
            painter: canvas::Painter::default(),
            #[cfg(feature = "rust-wgpu")]
            painter: None,
        }
    }

    pub fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Routes input through the last frame's hit tree, using browser time in seconds.
    pub fn input(&mut self, event: InputEvent, time: f64) {
        self.core.set_time(time);
        if matches!(event, InputEvent::MouseDown { .. }) {
            self.core.set_focused(true);
        }
        let events = self.core.handle_input(event);
        self.route(events);
    }

    /// A complete keystroke, including its semantic action in the core.
    pub fn keystroke(&mut self, key: KeyPress, time: f64) {
        self.core.set_time(time);
        let events = self.core.press(key.clone());
        self.route(events);
        let events = self.core.release(key);
        self.route(events);
    }

    /// Delivers a browser key transition without synthesizing its release.
    pub fn key_event(&mut self, key: KeyPress, time: f64, down: bool) {
        self.core.set_time(time);
        let events = if down {
            self.core.press(key)
        } else {
            self.core.release(key)
        };
        self.route(events);
    }

    fn route(&mut self, events: Vec<UiEvent>) {
        for event in events {
            self.app.on_event_with(event, &mut self.core);
        }
    }

    pub fn pointer_cursor(&self) -> bool {
        self.core.cursor_shape() == CursorShape::Pointer
    }

    /// Whether a touch moved from this point should feed the scroll path.
    pub fn prefers_scroll_at(&self, point: kui_core::Vec2) -> bool {
        self.core.interaction.prefers_scroll_at(point)
    }

    pub fn set_platform(&mut self, platform: Platform) {
        self.core.set_platform(platform);
    }

    pub fn blur(&mut self, time: f64) {
        self.window_focus(false, time);
        self.core.set_focus(None);
    }

    pub fn window_focus(&mut self, focused: bool, time: f64) {
        self.core.set_time(time);
        self.core.set_focused(focused);
    }

    pub fn focus(&mut self, key: Key, time: f64) {
        self.core.set_focused(true);
        self.input(
            InputEvent::Access(AccessRequest::new(key, AccessAction::Focus)),
            time,
        );
        if self.core.key_focus() == Some(key) {
            self.core.reveal(key);
        }
    }

    pub fn activate(&mut self, key: Key, time: f64) {
        self.input(
            InputEvent::Access(AccessRequest::new(key, AccessAction::Click)),
            time,
        );
    }

    pub fn clipboard_selection(&mut self, cut: bool, time: f64) {
        self.core.set_time(time);
        let text = if cut {
            self.core.cut_selection()
        } else {
            self.core.copy_selection()
        };
        if let Some(text) = text {
            access::copy_to_browser(&text);
        }
    }

    pub fn replace_editor_value(&mut self, value: String, time: f64) {
        if let Some(key) = self.core.key_focus() {
            self.input(
                InputEvent::Access(
                    AccessRequest::new(key, AccessAction::SetValue).with_value(value),
                ),
                time,
            );
        }
    }

    pub fn select_editor_text(&mut self, anchor: usize, focus: usize, time: f64) {
        let Some(key) = self.core.key_focus() else {
            return;
        };
        let Some(node) = self.core.access_tree().get(key) else {
            return;
        };
        let (Some(anchor), Some(focus)) = (
            access::text_position(node, anchor),
            access::text_position(node, focus),
        ) else {
            return;
        };
        self.input(
            InputEvent::Access(
                AccessRequest::new(key, AccessAction::SetTextSelection)
                    .with_selection(anchor, focus),
            ),
            time,
        );
    }

    /// Publishes changes to the DOM mirror after the display list is built.
    pub fn mirror_access(&mut self) {
        self.access.publish(&mut self.core);
    }

    /// The browser replaced or restored the canvas backing store.
    #[cfg(not(feature = "rust-wgpu"))]
    pub fn invalidate_canvas(&mut self) {
        self.painter.invalidate();
    }

    /// Builds and paints a frame. Returns whether the browser owes another frame.
    #[cfg(not(feature = "rust-wgpu"))]
    pub fn frame(&mut self, viewport: Size, scale: f32, time: f64) -> bool {
        let next = self.build_frame(viewport, scale, time);
        self.painter.paint(&mut self.core);
        next
    }

    /// Attaches a new renderer, including after browser GPU device loss.
    #[cfg(feature = "rust-wgpu")]
    pub fn attach_wgpu(&mut self, painter: WgpuPainter) {
        self.painter = Some(painter);
    }

    /// Builds a complete frame and renders it directly to the browser surface.
    #[cfg(feature = "rust-wgpu")]
    pub fn frame_wgpu(
        &mut self,
        viewport: Size,
        scale: f32,
        time: f64,
        backing_size: [u32; 2],
    ) -> Result<bool, RenderError> {
        let next = self.build_frame(viewport, scale, time);
        let painter = self
            .painter
            .as_mut()
            .expect("wgpu renderer attached at startup");
        Ok(painter.paint(&mut self.core, backing_size)? || next)
    }

    pub fn next_frame_at(&self) -> Option<f64> {
        self.core.next_frame_at()
    }

    fn build_frame(&mut self, viewport: Size, scale: f32, time: f64) -> bool {
        self.core.set_time(time);
        self.app.before_frame(&mut self.core);
        let mut ui = self.core.frame(viewport, scale);
        self.app.view(&mut ui);
        ui.finish();
        let events = self.core.take_pending_events();
        let app_received_events = !events.is_empty();
        self.route(events);
        app_received_events || self.core.animating()
    }
}
