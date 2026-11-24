//! UIKit delivers each session through the scene delegate declared in Info.plist.
use super::*;
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::{CStr, CString, c_char},
};

thread_local! {
    static EVENTS: RefCell<Option<EventLoopProxy<MobileEvent>>> = const { RefCell::new(None) };
}

#[unsafe(no_mangle)]
unsafe extern "C" fn kui_scene_changed(session: *const c_char, note: *const c_char) {
    let id = unsafe { CStr::from_ptr(session) }
        .to_string_lossy()
        .into_owned();
    let note = (!note.is_null()).then(|| {
        unsafe { CStr::from_ptr(note) }
            .to_string_lossy()
            .into_owned()
    });
    EVENTS.with_borrow(|proxy| {
        if let Some(proxy) = proxy {
            let _ = proxy.send_event(MobileEvent::Scene(id, note));
        }
    });
}

#[unsafe(no_mangle)]
unsafe extern "C" fn kui_scene_foreground(session: *const c_char, foreground: bool) {
    let id = unsafe { CStr::from_ptr(session) }
        .to_string_lossy()
        .into_owned();
    EVENTS.with_borrow(|proxy| {
        if let Some(proxy) = proxy {
            let _ = proxy.send_event(MobileEvent::SceneForeground(id, foreground));
        }
    });
}

/// Request a new scene presenting the specified document.
pub fn open_scene(source_session: &str, document: &str) {
    unsafe extern "C" {
        fn kui_open_scene(source: *const c_char, document: *const c_char);
    }
    if let (Ok(source), Ok(name)) = (CString::new(source_session), CString::new(document)) {
        unsafe { kui_open_scene(source.as_ptr(), name.as_ptr()) };
    }
}

/// Create a core and application view for each connected iPad scene.
/// The factory receives UIKit's persistent session ID and the requested document.
pub fn run_scenes<A: App>(
    make: impl FnMut(&str, &str) -> (Core, A),
) -> Result<(), Box<dyn std::error::Error>> {
    let events = EventLoop::with_user_event().build()?;
    EVENTS.set(Some(events.create_proxy()));
    let mut scenes = Scenes {
        make,
        events: events.create_proxy(),
        hosts: HashMap::new(),
        next: 0,
        error: None,
    };
    events.run_app(&mut scenes)?;
    EVENTS.set(None);
    scenes.error.map_or(Ok(()), Err)
}

struct Scenes<A, F> {
    make: F,
    events: EventLoopProxy<MobileEvent>,
    hosts: HashMap<u64, Host<A>>,
    next: u64,
    error: Option<Box<dyn std::error::Error>>,
}

impl<A: App, F: FnMut(&str, &str) -> (Core, A)> ApplicationHandler<MobileEvent> for Scenes<A, F> {
    // UIKit reports foreground/background for each scene independently.
    fn resumed(&mut self, _: &ActiveEventLoop) {}

    fn new_events(&mut self, el: &ActiveEventLoop, cause: StartCause) {
        for host in self.hosts.values_mut() {
            host.new_events(el, cause);
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        for host in self.hosts.values_mut() {
            host.about_to_wait(el);
        }
        let next = self.hosts.values().filter_map(|h| h.next_draw).min();
        el.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }

    fn exiting(&mut self, el: &ActiveEventLoop) {
        for (_, mut host) in self.hosts.drain() {
            host.exiting(el);
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Some(host) = self
            .hosts
            .values_mut()
            .find(|h| h.window.as_ref().is_some_and(|w| w.id() == id))
        {
            host.window_event(el, id, event);
        }
    }

    fn user_event(&mut self, el: &ActiveEventLoop, event: MobileEvent) {
        match event {
            MobileEvent::Scene(id, note) => {
                let existing = self
                    .hosts
                    .iter()
                    .find(|(_, h)| h.scene_id.as_ref() == Some(&id))
                    .map(|(key, _)| *key);
                if let Some(key) = existing {
                    if note.is_none()
                        && let Some(mut host) = self.hosts.remove(&key)
                    {
                        host.exiting(el);
                    }
                } else if let Some(note) = note {
                    self.next += 1;
                    let proxy = Proxy {
                        events: self.events.clone(),
                        scene: self.next,
                    };
                    let (core, app) = (self.make)(&id, &note);
                    match Host::new(core, app, crate::Extensions::new(), proxy) {
                        Ok(mut host) => {
                            host.scene_id = Some(id);
                            host.resumed(el);
                            self.error = host.error.take();
                            self.hosts.insert(self.next, host);
                        }
                        Err(e) => {
                            self.error = Some(e);
                            el.exit();
                        }
                    }
                }
            }
            MobileEvent::SceneForeground(id, foreground) => {
                if let Some(host) = self
                    .hosts
                    .values_mut()
                    .find(|h| h.scene_id.as_ref() == Some(&id))
                {
                    if foreground {
                        host.resumed(el);
                    } else {
                        host.suspended(el);
                    }
                    if host.error.is_some() {
                        self.error = host.error.take();
                    }
                }
            }
            MobileEvent::Routed(scene, event) => {
                if let Some(host) = self.hosts.get_mut(&scene) {
                    host.user_event(el, *event);
                }
            }
            _ => {}
        }
    }
}
