//! A headless driver for an [`App`]: a `Core`, a viewport, the app's
//! extensions, and the inputs a test needs to press its keys, click its
//! nodes and read what it drew — through the real `view` and `on_event`,
//! with no window (backlog DX11).
//!
//! ```ignore
//! let mut d = Drive::new(Core::new(), 480.0, 320.0);
//! d.frame(&mut app);
//! let add = d.key_of("add").ok_or("no add button")?;
//! d.click_key(&mut app, add);
//! d.frame(&mut app);
//! assert_eq!(app.count, 1);
//! assert!(d.warnings().is_empty());
//! ```
//!
//! A gesture sends its inputs and stops; the caller frames when it wants
//! the view to catch up. [`Drive::framing`] frames after every gesture
//! instead, and once while a drag is held, for a test written as a list
//! of keystrokes against what is on screen.
//!
//! It owns its `Core` or borrows one (`Drive::new(&mut core, …)`), which
//! is how the examples' `--headless` drives run on the core their harness
//! built. Events go to the app the way the runner sends them: an
//! extension's to the extension ([`Extensions::route`]), the rest to
//! `on_event`, and each is logged.

use std::borrow::{Borrow, BorrowMut};

use crate::{
    AccessAction, AccessRequest, App, Core, Extension, Extensions, InputEvent, Key, KeyCode,
    KeyMods, KeyPress, MouseButton, Rect, Size, UiEvent, Vec2,
};

/// The headless driver; see the module docs.
pub struct Drive<C: BorrowMut<Core> = Core> {
    pub core: C,
    /// The extensions filling the frame's slots, routed as the runner
    /// routes them. Empty unless [`Drive::extension`] loaded one.
    pub exts: Extensions,
    pub viewport: Size,
    /// The display's scale: 1 unless a test sets it.
    pub scale: f32,
    framing: bool,
    frame: u64,
    now: f64,
    log: Vec<String>,
}

impl<C: BorrowMut<Core>> Drive<C> {
    /// A drive over `core`, with the node snapshot on (`Core::set_inspect`)
    /// so [`Self::texts_under`] and [`Self::rect_of`] have a frame to read.
    pub fn new(mut core: C, w: f32, h: f32) -> Self {
        core.borrow_mut().set_inspect(true);
        Drive {
            core,
            exts: Extensions::new(),
            viewport: Size::new(w, h),
            scale: 1.0,
            framing: false,
            frame: 0,
            now: 0.0,
            log: Vec::new(),
        }
    }

    /// Frames after every gesture (a click, a key, typed text, the wheel,
    /// a hover, a drag), and once while a drag is held.
    pub fn framing(mut self) -> Self {
        self.framing = true;
        self
    }

    fn core(&mut self) -> &mut Core {
        self.core.borrow_mut()
    }

    /// Loads an extension under `ns`, as `Launcher::extension_as` would.
    pub fn extension(&mut self, ns: &str, ext: impl Extension + 'static) -> Result<(), String> {
        self.exts.push_as(ns, Box::new(ext))
    }

    /// Builds one frame from `app.view`, then hands `app` whatever the
    /// frame produced on its own (a `resize`, a `layout`, a window event).
    pub fn frame(&mut self, app: &mut impl App) {
        self.frame += 1;
        let (viewport, scale, now) = (self.viewport, self.scale, self.now);
        let core = self.core.borrow_mut();
        core.set_time(now);
        let mut ui = core.frame_with(viewport, scale, &mut self.exts);
        app.view(&mut ui);
        ui.finish();
        let pending = self.core().take_pending_events();
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
        self.core().key_of(label)
    }

    /// Where the last frame put the node `key` names, in logical px:
    /// where it is hit when it takes input (clipped, on top), else where
    /// layout put it.
    pub fn rect_of(&self, key: Key) -> Option<Rect> {
        let core: &Core = self.core.borrow();
        let hit = core
            .interaction
            .hits()
            .iter()
            .rev()
            .find(|h| h.key == key)
            .map(|h| h.rect);
        hit.or_else(|| core.nodes().iter().find(|n| n.key == key).map(|n| n.rect))
    }

    /// The text of every text node under the first node labelled `label`
    /// in the last frame, in tree order; empty when none is labelled so.
    pub fn texts_under(&self, label: &str) -> Vec<String> {
        let nodes = Borrow::<Core>::borrow(&self.core).nodes();
        let Some(at) = nodes.iter().position(|n| n.label.as_deref() == Some(label)) else {
            return Vec::new();
        };
        nodes[at + 1..]
            .iter()
            .take_while(|n| n.depth > nodes[at].depth)
            .filter_map(|n| n.text.clone())
            .collect()
    }

    /// Every warning the core raised since the last call, as
    /// `code: message`: a test asserts it empty.
    pub fn warnings(&mut self) -> Vec<String> {
        self.core()
            .take_warnings()
            .into_iter()
            .map(|w| format!("{}: {}", w.code, w.message))
            .collect()
    }

    /// One input, its events to `app`, and back to the caller too.
    pub fn input(&mut self, app: &mut impl App, ev: InputEvent) -> Vec<UiEvent> {
        let out = self.core().handle_input(ev);
        self.deliver(app, out.clone());
        out
    }

    fn done(&mut self, app: &mut impl App, out: Vec<UiEvent>) -> Vec<UiEvent> {
        if self.framing {
            self.frame(app);
        }
        out
    }

    /// The pointer to a point, nothing pressed.
    pub fn move_to(&mut self, app: &mut impl App, x: f32, y: f32) -> Vec<UiEvent> {
        let out = self.input(app, InputEvent::CursorMoved(Vec2::new(x, y)));
        self.done(app, out)
    }

    /// The pointer to the middle of the node `key` names.
    pub fn hover(&mut self, app: &mut impl App, key: Key) -> Vec<UiEvent> {
        let Some(c) = self.rect_of(key).map(|r| r.center()) else {
            return Vec::new();
        };
        self.move_to(app, c.x, c.y)
    }

    fn press_at(&mut self, app: &mut impl App, at: Vec2, clicks: u8) -> Vec<UiEvent> {
        let mut out = self.input(app, InputEvent::CursorMoved(at));
        out.extend(self.input(app, InputEvent::mouse_down(clicks)));
        out.extend(self.input(app, InputEvent::mouse_up()));
        out
    }

    /// A primary click at a point: move, press, release.
    pub fn click(&mut self, app: &mut impl App, x: f32, y: f32) -> Vec<UiEvent> {
        let out = self.press_at(app, Vec2::new(x, y), 1);
        self.done(app, out)
    }

    /// Two clicks at a point, the second counted as the second, as the OS
    /// counts a double click into the press.
    pub fn double_click(&mut self, app: &mut impl App, x: f32, y: f32) -> Vec<UiEvent> {
        let at = Vec2::new(x, y);
        let mut out = self.press_at(app, at, 1);
        out.extend(self.press_at(app, at, 2));
        self.done(app, out)
    }

    /// A click on the node `key` names, the way assistive technology
    /// presses it — no geometry needed.
    pub fn click_key(&mut self, app: &mut impl App, key: Key) -> Vec<UiEvent> {
        let out = self.input(
            app,
            InputEvent::Access(AccessRequest::new(key, AccessAction::Click)),
        );
        self.done(app, out)
    }

    /// A press at `from`, the pointer taken past the click slop and on to
    /// `to`, the release: an `on_drag` node hears start, moves and end,
    /// and the release is no click.
    pub fn drag(&mut self, app: &mut impl App, from: Vec2, to: Vec2) -> Vec<UiEvent> {
        let mut out = self.input(app, InputEvent::CursorMoved(from));
        out.extend(self.input(app, InputEvent::mouse_down(1)));
        let past = Vec2::new(from.x + 8.0, from.y + 8.0);
        out.extend(self.input(app, InputEvent::CursorMoved(past)));
        out.extend(self.input(app, InputEvent::CursorMoved(to)));
        if self.framing {
            self.frame(app);
        }
        out.extend(self.input(app, InputEvent::mouse_up()));
        self.done(app, out)
    }

    /// The wheel over a point.
    pub fn wheel(&mut self, app: &mut impl App, x: f32, y: f32, dx: f32, dy: f32) -> Vec<UiEvent> {
        let mut out = self.input(app, InputEvent::CursorMoved(Vec2::new(x, y)));
        out.extend(self.input(app, InputEvent::Scroll(Vec2::new(dx, dy))));
        self.done(app, out)
    }

    /// One event of a scroll gesture over a point (backlog F107): `begins`
    /// on its first, and the rest go to the target it picked, wherever
    /// the pointer or the content has gone since — the latching a native
    /// swipe gets. [`Self::wheel`] is a gesture of its own.
    pub fn scroll_gesture(
        &mut self,
        app: &mut impl App,
        x: f32,
        y: f32,
        delta: Vec2,
        begins: bool,
    ) -> Vec<UiEvent> {
        let mut out = self.input(app, InputEvent::CursorMoved(Vec2::new(x, y)));
        out.extend(self.input(app, InputEvent::ScrollGesture { delta, begins }));
        self.done(app, out)
    }

    /// A non-primary button pressed and released at a point: what an
    /// `on_button` node claiming it hears as `press` and `release`
    /// (backlog F105, RG75).
    pub fn button_click(
        &mut self,
        app: &mut impl App,
        x: f32,
        y: f32,
        button: MouseButton,
    ) -> Vec<UiEvent> {
        let mut out = self.input(app, InputEvent::CursorMoved(Vec2::new(x, y)));
        out.extend(self.input(app, InputEvent::MouseDown { button, clicks: 1 }));
        out.extend(self.input(app, InputEvent::MouseUp { button }));
        self.done(app, out)
    }

    /// A key pressed and released, by the name a binding spells it
    /// (`"a"`, `"enter"`, `"f2"`): the raw press — carrying the character
    /// (or the space) as its text when no chord modifier is held, as a
    /// keyboard's would — then the editor event the same press means
    /// (`KeyPress::edit_event`, the one table every driver sends from),
    /// then the release.
    pub fn key(&mut self, app: &mut impl App, name: &str, mods: KeyMods) -> Vec<UiEvent> {
        let code = KeyCode::from_name(name).unwrap_or(KeyCode::Unknown);
        let mut press = KeyPress::new(code, mods);
        let chord = mods.ctrl || mods.alt || mods.super_key;
        match code {
            KeyCode::Char(c) if !chord => press = press.with_text(c.to_string()),
            KeyCode::Space if !chord => press = press.with_text(" "),
            _ => {}
        }
        let mut out = self.input(app, InputEvent::KeyDown(press.clone()));
        if let Some(ev) = press.edit_event() {
            out.extend(self.input(app, ev));
        }
        out.extend(self.input(app, InputEvent::KeyUp(press.released())));
        self.done(app, out)
    }

    /// `keys(app, "jj ww")`: [`Self::key`] for each character, unmodified,
    /// a space being the space key.
    pub fn keys(&mut self, app: &mut impl App, seq: &str) -> Vec<UiEvent> {
        let mut out = Vec::new();
        for c in seq.chars() {
            let name = if c == ' ' {
                "space".to_string()
            } else {
                c.to_string()
            };
            out.extend(self.key(app, &name, KeyMods::NONE));
        }
        out
    }

    /// Typed text, as the OS delivers it to the focused editor.
    pub fn text(&mut self, app: &mut impl App, s: &str) -> Vec<UiEvent> {
        let out = self.input(app, InputEvent::Text(s.to_string()));
        self.done(app, out)
    }

    /// Text that did not come from a key press — an IME's commit — as the
    /// OS delivers it: to a focused editor, or to a key sink as `text`.
    pub fn commit(&mut self, app: &mut impl App, s: &str) -> Vec<UiEvent> {
        let out = self.input(app, InputEvent::Commit(s.to_string()));
        self.done(app, out)
    }

    /// Focuses the node `key` names, as Tab or a screen reader would.
    pub fn focus(&mut self, app: &mut impl App, key: Key) -> Vec<UiEvent> {
        let out = self.input(
            app,
            InputEvent::Access(AccessRequest::new(key, AccessAction::Focus)),
        );
        self.done(app, out)
    }

    /// `Ok` when `cond` holds, else `Err(what)` — the shape an example's
    /// `headless` returns, so a drive reads as a list of these.
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
        for ev in &events {
            self.log.push(format!(
                "{:>4} {:08x} {}",
                self.frame,
                ev.key.0 as u32,
                crate::devtools::fmt_value(&ev.payload)
            ));
        }
        let core = self.core.borrow_mut();
        self.exts.route(events, |ev| app.on_event_with(ev, core));
    }
}
