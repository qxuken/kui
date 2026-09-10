//! The scaffolding a headless drive is built from: a bare `Core`, a
//! viewport, and the handful of inputs an example's `--headless` needs to
//! click its own buttons and read the answer. Every input is handed to
//! the example's `on_event` and logged, so what the drive prints is the
//! same stream the dock shows.
//!
//! ```ignore
//! fn headless(&mut self, core: &mut Core) -> Result<(), String> {
//!     let mut d = Drive::new(core, 480.0, 320.0);
//!     d.frame(self);
//!     let add = d.key_of("add").ok_or("no add button")?;
//!     d.click_key(self, add);
//!     d.frame(self);
//!     d.check(self.count == 1, "one click counts once")
//! }
//! ```

use kui::{
    AccessAction, AccessRequest, App, Core, InputEvent, Key, KeyCode, KeyMods, KeyPress, Rect,
    Size, UiEvent, Vec2,
};

/// A headless driver over a borrowed `Core`.
pub struct Drive<'c> {
    pub core: &'c mut Core,
    viewport: Size,
    scale: f32,
    frame: u64,
    /// Seconds on the frame clock; `advance` moves it.
    now: f64,
    log: Vec<String>,
}

impl<'c> Drive<'c> {
    pub fn new(core: &'c mut Core, w: f32, h: f32) -> Self {
        Drive {
            core,
            viewport: Size::new(w, h),
            scale: 1.0,
            frame: 0,
            now: 0.0,
            log: Vec::new(),
        }
    }

    /// Builds one frame from `app.view`, then hands `app` whatever the
    /// frame produced on its own (a `resize`, a `layout`, a window event).
    pub fn frame(&mut self, app: &mut impl App) {
        self.frame += 1;
        self.core.set_time(self.now);
        let mut ui = self.core.frame(self.viewport, self.scale);
        app.view(&mut ui);
        ui.finish();
        let pending = self.core.take_pending_events();
        self.deliver(app, pending);
    }

    /// Moves the frame clock `secs` forward; the next `frame` sees it.
    pub fn advance(&mut self, secs: f64) {
        self.now += secs;
    }

    /// The frames built so far.
    pub fn frames(&self) -> u64 {
        self.frame
    }

    /// The key a node was opened under `label`, from the last frame.
    pub fn key_of(&mut self, label: &str) -> Option<Key> {
        self.core.key_of(label)
    }

    /// Where the last frame put an interactive node (one that hovers,
    /// clicks, drags or edits — the hit list is theirs), in logical px.
    pub fn rect_of(&self, key: Key) -> Option<Rect> {
        self.core
            .interaction
            .hits()
            .iter()
            .rev()
            .find(|h| h.key == key)
            .map(|h| h.rect)
    }

    /// Moves the pointer to the middle of the node `key` names.
    pub fn hover(&mut self, app: &mut impl App, key: Key) -> Vec<UiEvent> {
        let Some(r) = self.rect_of(key) else {
            return Vec::new();
        };
        self.input(
            app,
            InputEvent::CursorMoved(Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0)),
        )
    }

    /// One input, its events to `app`, and back to the caller too.
    pub fn input(&mut self, app: &mut impl App, ev: InputEvent) -> Vec<UiEvent> {
        let out = self.core.handle_input(ev);
        self.deliver(app, out.clone());
        out
    }

    /// A primary click at a point: move, press, release.
    pub fn click(&mut self, app: &mut impl App, x: f32, y: f32) -> Vec<UiEvent> {
        self.input(app, InputEvent::CursorMoved(Vec2::new(x, y)));
        self.input(app, InputEvent::mouse_down(1));
        self.input(app, InputEvent::mouse_up())
    }

    /// A click on the node `key` names, the way assistive technology
    /// presses it — no geometry needed.
    pub fn click_key(&mut self, app: &mut impl App, key: Key) -> Vec<UiEvent> {
        self.input(
            app,
            InputEvent::Access(AccessRequest::new(key, AccessAction::Click)),
        )
    }

    /// The wheel over a point.
    pub fn wheel(&mut self, app: &mut impl App, x: f32, y: f32, dx: f32, dy: f32) -> Vec<UiEvent> {
        self.input(app, InputEvent::CursorMoved(Vec2::new(x, y)));
        self.input(app, InputEvent::Scroll(Vec2::new(dx, dy)))
    }

    /// A key pressed and released, by the name a binding spells it
    /// (`"a"`, `"enter"`, `"f2"`): the raw press, then the editor event
    /// the same press means (`KeyPress::edit_event`, the one table every
    /// driver sends from — Escape dismissing a modal is that channel),
    /// then the release.
    pub fn key(&mut self, app: &mut impl App, name: &str, mods: KeyMods) -> Vec<UiEvent> {
        let code = KeyCode::from_name(name).unwrap_or(KeyCode::Unknown);
        let press = KeyPress::new(code, mods);
        let mut out = self.input(app, InputEvent::KeyDown(press.clone()));
        if let Some(ev) = press.edit_event() {
            out.extend(self.input(app, ev));
        }
        out.extend(self.input(app, InputEvent::KeyUp(press.released())));
        out
    }

    /// Typed text, as the OS delivers it to the focused editor.
    pub fn text(&mut self, app: &mut impl App, s: &str) -> Vec<UiEvent> {
        self.input(app, InputEvent::Text(s.to_string()))
    }

    /// Focuses the node `key` names, as Tab or a screen reader would.
    pub fn focus(&mut self, app: &mut impl App, key: Key) -> Vec<UiEvent> {
        self.input(
            app,
            InputEvent::Access(AccessRequest::new(key, AccessAction::Focus)),
        )
    }

    /// `Ok` when `cond` holds, else `Err(what)` — the shape `headless`
    /// returns, so a drive reads as a list of these.
    pub fn check(&self, cond: bool, what: &str) -> Result<(), String> {
        if cond {
            println!("  ok   {what}");
            Ok(())
        } else {
            Err(what.to_string())
        }
    }

    /// Everything delivered so far, one line each: `frame key payload`.
    pub fn log(&self) -> &[String] {
        &self.log
    }

    fn deliver(&mut self, app: &mut impl App, events: Vec<UiEvent>) {
        for ev in events {
            self.log.push(format!(
                "{:>4} {:08x} {}",
                self.frame,
                ev.key.0 as u32,
                kui::devtools::fmt_value(&ev.payload)
            ));
            app.on_event(ev);
        }
    }
}
