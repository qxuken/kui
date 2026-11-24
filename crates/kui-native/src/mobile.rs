//! Touchscreen host for Android activities and UIKit windows. Build with
//! `default-features = false` and the `mobile` feature.
use crate::mobile_touch::{Contacts, GestureAction};
use crate::{
    App, Core, Edges, Extensions, InputEvent, KeyCode, KeyMods, KeyPress, Size, UiEvent, Vec2,
};
use kui_wgpu::{RenderError, Renderer};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, Ime, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[cfg(target_os = "android")]
pub use winit::platform::android::activity::AndroidApp;

#[cfg(target_os = "android")]
#[path = "mobile_android_editor.rs"]
mod android_editor;

#[path = "mobile_access.rs"]
mod access;
#[path = "mobile_fonts.rs"]
mod fonts;
pub use fonts::session_with_japanese_fallback;
#[cfg(target_os = "ios")]
#[path = "mobile_ios_editor.rs"]
mod ios_editor;
#[path = "mobile_text.rs"]
mod mobile_text;
use mobile_text::{Document, NativeText};

#[derive(Debug)]
enum MobileEvent {
    #[cfg(target_os = "ios")]
    Scene(String, Option<String>),
    #[cfg(target_os = "ios")]
    SceneForeground(String, bool),
    #[cfg(target_os = "ios")]
    Routed(u64, Box<MobileEvent>),
    AccessActive(bool),
    Access(crate::AccessRequest),
    Wake,
    Ime(i64, NativeText),
    #[cfg(target_os = "ios")]
    EditorKey(i64, KeyCode),
    #[cfg(target_os = "ios")]
    HardwareKey(i64, KeyPress, bool),
}

#[derive(Clone)]
pub struct Waker(Proxy);

#[derive(Clone)]
struct Proxy {
    events: EventLoopProxy<MobileEvent>,
    #[cfg(target_os = "ios")]
    scene: u64,
}

impl Proxy {
    fn send_event(
        &self,
        event: MobileEvent,
    ) -> Result<(), winit::event_loop::EventLoopClosed<MobileEvent>> {
        #[cfg(target_os = "ios")]
        let event = if self.scene == 0 {
            event
        } else {
            MobileEvent::Routed(self.scene, Box::new(event))
        };
        self.events.send_event(event)
    }
}

impl Waker {
    pub fn wake(&self) {
        let _ = self.0.send_event(MobileEvent::Wake);
    }
}

pub fn run(
    #[cfg(target_os = "android")] activity: AndroidApp,
    core: Core,
    app: impl App,
) -> Result<(), Box<dyn std::error::Error>> {
    run_with_extensions(
        #[cfg(target_os = "android")]
        activity,
        core,
        app,
        Extensions::new(),
    )
}

/// Run a mobile app with the same component instances and event routing used
/// by the desktop and web hosts.
pub fn run_with_extensions(
    #[cfg(target_os = "android")] activity: AndroidApp,
    core: Core,
    app: impl App,
    extensions: Extensions,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = EventLoop::with_user_event();
    #[cfg(target_os = "android")]
    {
        use winit::platform::android::EventLoopBuilderExtAndroid;
        builder.with_android_app(activity.clone());
    }
    let event_loop = builder.build()?;
    let proxy = Proxy {
        events: event_loop.create_proxy(),
        #[cfg(target_os = "ios")]
        scene: 0,
    };
    let mut host = Host::new(
        #[cfg(target_os = "android")]
        activity,
        core,
        app,
        extensions,
        proxy,
    )?;
    event_loop.run_app(&mut host)?;
    if let Some(error) = host.error {
        return Err(error);
    }
    Ok(())
}

impl<A: App> Host<A> {
    fn new(
        #[cfg(target_os = "android")] activity: AndroidApp,
        core: Core,
        mut app: A,
        extensions: Extensions,
        proxy: Proxy,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(target_os = "android")]
        let editor = android_editor::Editor::new(&activity, proxy.clone())?;
        #[cfg(target_os = "ios")]
        let editor = ios_editor::Editor::new(proxy.clone());
        app.setup(Waker(proxy.clone()));
        Ok(Self {
            #[cfg(target_os = "ios")]
            scene_id: None,
            access: None,
            access_feed: Default::default(),
            proxy,
            #[cfg(target_os = "android")]
            activity,
            editor,
            document: Document::default(),
            editor_stamp: None,
            editor_focus: None,
            session: 0,
            ime_off: false,
            key_mods: KeyMods::default(),
            #[cfg(target_os = "android")]
            ime: AndroidInsets::default(),
            core,
            app,
            extensions,
            window: None,
            renderer: None,
            contacts: Contacts::default(),
            keyboard: false,
            layout: (Size::ZERO, Edges::default()),
            show_editor: false,
            epoch: Instant::now(),
            next_draw: None,
            error: None,
        })
    }
}

struct Host<A> {
    #[cfg(target_os = "ios")]
    scene_id: Option<String>,
    access: Option<access::Adapter>,
    access_feed: kui_access::Feed,
    proxy: Proxy,
    #[cfg(target_os = "android")]
    activity: AndroidApp,
    #[cfg(target_os = "android")]
    ime: AndroidInsets,
    #[cfg(target_os = "android")]
    editor: android_editor::Editor,
    #[cfg(target_os = "ios")]
    editor: ios_editor::Editor,
    document: Document,
    editor_stamp: Option<(u64, u64)>,
    editor_focus: Option<(crate::Key, i32)>,
    session: i64,
    ime_off: bool,
    key_mods: KeyMods,
    core: Core,
    app: A,
    extensions: Extensions,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    contacts: Contacts,
    keyboard: bool,
    layout: (Size, Edges),
    show_editor: bool,
    epoch: Instant,
    next_draw: Option<Instant>,
    error: Option<Box<dyn std::error::Error>>,
}

impl<A: App> Host<A> {
    fn refresh_motion(&mut self) {
        #[cfg(target_os = "ios")]
        let reduced = {
            unsafe extern "C" {
                fn UIAccessibilityIsReduceMotionEnabled() -> bool;
            }
            Some(unsafe { UIAccessibilityIsReduceMotionEnabled() })
        };
        #[cfg(target_os = "android")]
        let reduced = android_editor::with_activity(&self.activity, |env, _| {
            use jni::{jni_sig, jni_str};
            env.call_static_method(
                jni_str!("android/animation/ValueAnimator"),
                jni_str!("areAnimatorsEnabled"),
                jni_sig!("()Z"),
                &[],
            )?
            .z()
            .map(|enabled| !enabled)
        })
        .ok();
        if let Some(reduced) = reduced {
            self.core.env.system.motion = if reduced {
                crate::MotionPref::Reduced
            } else {
                crate::MotionPref::Full
            };
        }
    }

    fn editor_key(&self) -> Option<crate::Key> {
        // A custom text sink can have a caret without a stock edit state.
        self.core.edit.focused().or_else(|| self.core.focus())
    }

    fn owns_editor(&self, session: i64) -> bool {
        !self.core.ime_off_requested()
            && self.session == session
            && self
                .editor_focus
                .is_some_and(|(key, _)| Some(key) == self.editor_key())
    }

    fn input(&mut self, input: InputEvent) {
        dispatch_input(
            &mut self.app,
            &mut self.core,
            &mut self.extensions,
            self.epoch.elapsed().as_secs_f64(),
            input,
            false,
        );
    }

    fn key(&mut self, key: KeyPress, pressed: bool) {
        if pressed {
            let edit = key.edit_event_on(self.core.platform());
            self.input(InputEvent::KeyDown(key));
            if let Some(edit) = edit {
                self.input(edit);
            }
        } else {
            self.input(InputEvent::KeyUp(key.released()));
        }
    }

    fn draw(&mut self, window: &Window) -> Result<(), RenderError> {
        let size = surface_size(window);
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let scale = window.scale_factor();
        let logical = size.to_logical::<f32>(scale);
        #[cfg(target_os = "ios")]
        let (height, edges) = view_layout(window, logical.height);
        #[cfg(target_os = "android")]
        let (height, edges) = self.ime.layout(window, logical.width, logical.height);
        let layout = (Size::new(logical.width, height), edges);
        let changed = layout != self.layout;
        self.layout = layout;
        self.core.set_safe_area(edges);
        let time = self.epoch.elapsed().as_secs_f64();
        if self.core.env.system.motion.is_reduced() {
            self.contacts.stop_glide();
        }
        let gliding = self.contacts.advance(time, |input| {
            dispatch_input(
                &mut self.app,
                &mut self.core,
                &mut self.extensions,
                time,
                input,
                false,
            )
        });
        self.core.set_time(time);
        self.app.before_frame(&mut self.core);
        let reveal = changed && self.core.ime_rect().is_some();
        let mut ui = self.core.frame_with(
            Size::new(logical.width, height),
            scale as f32,
            &mut self.extensions,
        );
        if reveal && let Some(key) = ui.focused() {
            ui.reveal(key);
        }
        self.app.view(&mut ui);
        ui.finish();
        let said = self.core.take_announcements();
        if self.access_feed.active()
            && let Some(adapter) = &mut self.access
        {
            access::publish(
                &mut self.access_feed,
                adapter,
                self.core.access_tree(),
                scale as f32,
                &said,
            );
        }
        let ime_off = self.core.ime_off_requested();
        let ime_changed = ime_off != self.ime_off;
        let editing = !ime_off && self.core.ime_rect().is_some();
        if ime_off && ime_changed {
            // Dropping the native responder discards its marked buffer. End
            // the preview in Core too, including for a custom keyboard sink.
            if self.editor_focus.is_some() {
                self.input(InputEvent::Preedit(String::new(), None));
            } else if cfg!(target_os = "android") {
                window.set_ime_allowed(false);
            }
        }
        self.ime_off = ime_off;
        // System Back can dismiss the Android IME while the editor retains
        // focus. A fresh tap must be able to open it again.
        let show = std::mem::take(&mut self.show_editor);
        {
            let focus = self.editor_key().filter(|_| editing).map(|key| {
                // InputType.TYPE_CLASS_TEXT with password or auto-correction
                // variation, and FLAG_MULTI_LINE where the field accepts it.
                let mut kind = if self.core.edit.is_secure(key) {
                    0x81
                } else {
                    0x8001
                };
                if self.core.edit.is_multiline(key) {
                    kind |= 0x20000;
                }
                (key, kind)
            });
            let changed = self.editor_focus != focus;
            if changed || ime_changed {
                self.session = self.session.wrapping_add(1);
            }
            let stamp =
                focus.map(|(key, _)| (self.core.edit.version(key), self.core.edit.caret_stamp()));
            if changed || show || self.editor_stamp != stamp {
                let mut refresh = changed;
                if let Some((key, _)) = focus
                    && self.core.edit.contains(key)
                    && !self.core.edit.is_secure(key)
                {
                    let doc = Document::read(&self.core, key);
                    refresh |= self.document != doc;
                    self.document = doc;
                } else if changed || show {
                    self.document = Document::default();
                    refresh = true;
                }
                if refresh || (editing && show) {
                    let kind = focus.map_or(0, |(_, ty)| ty);
                    #[cfg(target_os = "android")]
                    self.editor
                        .set(self.session, kind, &self.document, changed || show, refresh);
                    #[cfg(target_os = "ios")]
                    self.editor.set(
                        window,
                        self.session,
                        kind,
                        &self.document,
                        changed || show,
                        refresh,
                    );
                }
                self.editor_stamp = stamp;
            }
            self.editor_focus = focus;
        }
        #[cfg(target_os = "ios")]
        if ime_changed {
            self.editor.key_mode(window, self.session, ime_off);
        }
        self.keyboard = editing;
        let events = self.core.take_pending_events();
        let again = gliding || !events.is_empty() || self.core.animating();
        route_events(&mut self.app, &mut self.core, &mut self.extensions, events);
        // UIKit can coalesce a redraw requested inside its drawing callback
        // with the frame already being presented. Wake the loop for the next
        // frame and request it from outside that callback.
        self.next_draw = if again {
            Some(Instant::now() + Duration::from_secs_f64(1. / 120.))
        } else {
            self.core.next_frame_at().and_then(|at| {
                self.epoch
                    .checked_add(Duration::try_from_secs_f64(at).ok()?)
            })
        };
        let (list, atlas) = self
            .core
            .output_for_surface(Size::new(size.width as f32, size.height as f32));
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.render(list, atlas)?;
        }
        Ok(())
    }
}

impl<A: App> ApplicationHandler<MobileEvent> for Host<A> {
    fn new_events(&mut self, _: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. })
            && self.next_draw.is_some_and(|due| due <= Instant::now())
        {
            self.next_draw = None;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref().filter(|_| self.renderer.is_some()) else {
            return;
        };
        let deadline = self.next_draw;
        // The usable area can change while the surface itself stays the same size.
        #[cfg(target_os = "ios")]
        {
            let full = surface_size(window)
                .to_logical::<f32>(window.scale_factor())
                .height;
            if view_layout(window, full) != (self.layout.0.h, self.layout.1) {
                window.request_redraw();
            }
        }
        #[cfg(target_os = "android")]
        let deadline = self
            .ime
            .check(&self.activity, window, self.keyboard)
            .into_iter()
            .chain(deadline)
            .min();
        event_loop.set_control_flow(deadline.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let opened = (|| -> Result<_, Box<dyn std::error::Error>> {
            let window = match &self.window {
                Some(window) => window.clone(),
                None => {
                    let attrs = Window::default_attributes().with_title("KUI Mobile");
                    #[cfg(target_os = "ios")]
                    let attrs = if let Some(id) = &self.scene_id {
                        use winit::platform::ios::WindowAttributesExtIOS;
                        attrs.with_scene_session(id.clone())
                    } else {
                        attrs
                    };
                    Arc::new(event_loop.create_window(attrs)?)
                }
            };
            if self.access.is_none() {
                self.access = Some(access::attach(
                    #[cfg(target_os = "ios")]
                    &window,
                    #[cfg(target_os = "android")]
                    &self.activity,
                    self.proxy.clone(),
                )?);
            }
            let size = surface_size(&window);
            let renderer = pollster::block_on(Renderer::new(
                window.clone(),
                size.width.max(1),
                size.height.max(1),
            ))?;
            Ok((window, renderer))
        })();
        match opened {
            Ok((window, renderer)) => {
                #[cfg(target_os = "android")]
                self.ime.invalidate();
                self.refresh_motion();
                self.core.set_focused(true);
                window.request_redraw();
                self.window = Some(window);
                self.renderer = Some(renderer);
            }
            Err(error) => {
                eprintln!("kui mobile startup: {error}");
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
        let time = self.epoch.elapsed().as_secs_f64();
        self.contacts.reset(|input| {
            dispatch_input(
                &mut self.app,
                &mut self.core,
                &mut self.extensions,
                time,
                input,
                false,
            )
        });
        self.core.set_focused(false);
        self.renderer = None;
        self.next_draw = None;
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.renderer = None;
        self.app.teardown();
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: MobileEvent) {
        match event {
            #[cfg(target_os = "ios")]
            MobileEvent::Scene(..) | MobileEvent::SceneForeground(..) | MobileEvent::Routed(..) => {
                return;
            }
            MobileEvent::AccessActive(active) => {
                if active {
                    self.access_feed.activate();
                } else {
                    self.access_feed.deactivate();
                }
                let mut system = self.core.env.system;
                system.assistive = self.access_feed.assistive();
                self.core.set_system(system);
            }
            MobileEvent::Access(request) => {
                if matches!(
                    request.action,
                    crate::AccessAction::Click | crate::AccessAction::Focus
                ) && self.core.edit.contains(request.key)
                {
                    self.core.reveal(request.key);
                    self.show_editor = true;
                }
                self.input(InputEvent::Access(request));
            }
            MobileEvent::Wake => self.app.on_wake(),
            #[cfg(target_os = "ios")]
            MobileEvent::EditorKey(session, code) => {
                if !self.owns_editor(session) {
                    return;
                }
                let key = KeyPress::new(code, KeyMods::default());
                self.key(key.clone(), true);
                self.key(key, false);
            }
            #[cfg(target_os = "ios")]
            MobileEvent::HardwareKey(session, key, pressed) => {
                if self.session != session || !self.core.ime_off_requested() {
                    return;
                }
                self.key(key, pressed);
            }
            MobileEvent::Ime(session, input) => {
                if !self.owns_editor(session) {
                    return;
                }
                self.native_text(self.editor_focus.unwrap().0, input);
            }
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if self.renderer.is_none() {
            return;
        }
        match event {
            WindowEvent::RedrawRequested => {
                match self.draw(&window) {
                    Ok(()) => {}
                    Err(RenderError::Skip) => window.request_redraw(),
                    Err(RenderError::Reconfigure | RenderError::Validation) => {
                        let size = surface_size(&window);
                        self.renderer
                            .as_mut()
                            .unwrap()
                            .resize(size.width, size.height);
                        window.request_redraw();
                    }
                    Err(RenderError::DeviceLost) => {
                        self.renderer = None;
                        self.resumed(event_loop);
                    }
                }
                return;
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                #[cfg(target_os = "android")]
                self.ime.invalidate();
                let size = surface_size(&window);
                self.renderer
                    .as_mut()
                    .unwrap()
                    .resize(size.width, size.height);
            }
            WindowEvent::Touch(touch) => {
                if touch.phase == winit::event::TouchPhase::Started {
                    self.refresh_motion();
                }
                let pos = touch.location.to_logical::<f32>(window.scale_factor());
                let at = Vec2::new(pos.x, pos.y);
                let action = if touch.phase != winit::event::TouchPhase::Started {
                    GestureAction::TapOnly
                } else {
                    GestureAction::at(&self.core.interaction, at)
                };
                let time = self.epoch.elapsed().as_secs_f64();
                let mut dispatched = false;
                self.show_editor |= self.contacts.handle(
                    touch.phase,
                    touch.id,
                    at,
                    action,
                    time,
                    |input, tap_press| {
                        dispatched = true;
                        dispatch_input(
                            &mut self.app,
                            &mut self.core,
                            &mut self.extensions,
                            time,
                            input,
                            tap_press,
                        )
                    },
                );
                // Down and sub-slop moves only update the recognizer. A
                // synchronous present here delays the motion we need to sample.
                if !dispatched {
                    return;
                }
            }
            WindowEvent::Focused(focused) => {
                if focused {
                    self.refresh_motion();
                    #[cfg(target_os = "ios")]
                    {
                        if self.ime_off {
                            self.editor.key_mode(&window, self.session, true);
                        } else if self.scene_id.is_some() && self.editor_focus.is_some() {
                            // A restored field may have asked to become
                            // first responder before its window was key.
                            self.show_editor = true;
                        }
                    }
                } else {
                    self.key_mods = KeyMods::default();
                    let time = self.epoch.elapsed().as_secs_f64();
                    self.contacts.reset(|input| {
                        dispatch_input(
                            &mut self.app,
                            &mut self.core,
                            &mut self.extensions,
                            time,
                            input,
                            false,
                        )
                    });
                }
                self.core.set_focused(focused);
                self.app.after_input(&mut self.core);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.key_mods = KeyMods {
                    shift: state.shift_key(),
                    ctrl: state.control_key(),
                    alt: state.alt_key(),
                    super_key: state.super_key(),
                };
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let code = match &event.logical_key {
                    Key::Named(NamedKey::Backspace) => Some(KeyCode::Backspace),
                    Key::Named(NamedKey::Enter) => Some(KeyCode::Enter),
                    Key::Named(NamedKey::BrowserBack | NamedKey::Escape) => Some(KeyCode::Escape),
                    Key::Named(NamedKey::ArrowLeft) => Some(KeyCode::Left),
                    Key::Named(NamedKey::ArrowRight) => Some(KeyCode::Right),
                    Key::Named(NamedKey::ArrowUp) => Some(KeyCode::Up),
                    Key::Named(NamedKey::ArrowDown) => Some(KeyCode::Down),
                    Key::Named(NamedKey::Tab) => Some(KeyCode::Tab),
                    Key::Named(NamedKey::Delete) => Some(KeyCode::Delete),
                    Key::Named(NamedKey::Home) => Some(KeyCode::Home),
                    Key::Named(NamedKey::End) => Some(KeyCode::End),
                    Key::Named(NamedKey::PageUp) => Some(KeyCode::PageUp),
                    Key::Named(NamedKey::PageDown) => Some(KeyCode::PageDown),
                    Key::Character(text) if self.ime_off => KeyCode::from_name(text),
                    _ => None,
                };
                if let Some(code) = code {
                    let mut key = KeyPress::new(code, self.key_mods);
                    key.repeat = event.repeat;
                    key.text = event.text.map(|s| s.to_string());
                    self.key(key, event.state == ElementState::Pressed);
                } else if event.state == ElementState::Pressed
                    && let Some(text) = event.text
                {
                    self.input(InputEvent::Text(text.to_string()));
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) if !self.core.ime_off() => {
                self.input(InputEvent::Commit(text))
            }
            WindowEvent::Ime(Ime::Preedit(text, range)) if !self.core.ime_off() => {
                self.input(InputEvent::Preedit(text, range))
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => return,
        }
        window.request_redraw();
    }
}

// winit reports the iOS safe area as inner_size, but its touch coordinates
// and the Metal layer both cover the entire view.
fn surface_size(window: &Window) -> winit::dpi::PhysicalSize<u32> {
    #[cfg(target_os = "ios")]
    {
        window.outer_size()
    }
    #[cfg(not(target_os = "ios"))]
    {
        window.inner_size()
    }
}

#[cfg(target_os = "android")]
#[path = "mobile_android_insets.rs"]
mod android_insets;
#[cfg(target_os = "android")]
use android_insets::AndroidInsets;

#[cfg(target_os = "ios")]
fn view_layout(window: &Window, full: f32) -> (f32, Edges) {
    use objc2_ui_kit::UIView;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else {
        return (full, Edges::default());
    };
    let RawWindowHandle::UiKit(handle) = handle.as_raw() else {
        return (full, Edges::default());
    };
    // SAFETY: winit keeps the view alive with its window, on this UI thread.
    let view = unsafe { handle.ui_view.cast::<UIView>().as_ref() };
    let safe = view.safeAreaInsets();
    let mut edges = Edges {
        l: safe.left as f32,
        r: safe.right as f32,
        t: safe.top as f32,
        b: safe.bottom as f32,
    };
    if objc2::available!(ios = 15.0) {
        let guide = view.keyboardLayoutGuide().layoutFrame();
        if guide.size.height > safe.bottom && guide.origin.y > 0. {
            let height = full.min(guide.origin.y as f32);
            edges.b = (edges.b - (full - height)).max(0.);
            return (height, edges);
        }
    }
    (full, edges)
}

fn dispatch_input(
    app: &mut impl App,
    core: &mut Core,
    extensions: &mut Extensions,
    time: f64,
    input: InputEvent,
    tap_press: bool,
) -> bool {
    core.set_time(time);
    if app.on_input(&input) {
        return true;
    }
    let (events, claimed) = match input {
        InputEvent::ScrollGesture { delta, begins } => core.handle_scroll(delta, begins),
        _ if tap_press => (core.handle_tap_press(), true),
        other => (core.handle_input(other), true),
    };
    route_events(app, core, extensions, events);
    app.after_input(core);
    claimed
}

fn route_events(
    app: &mut impl App,
    core: &mut Core,
    extensions: &mut Extensions,
    events: Vec<UiEvent>,
) {
    extensions.route(events, |event| app.on_event_with(event, core));
}

#[cfg(target_os = "ios")]
#[path = "mobile_scenes.rs"]
mod scenes;
#[cfg(target_os = "ios")]
pub use scenes::{open_scene, run_scenes};
