//! The AccessKit bridge: the core's access tree (data) becomes the
//! platform accessibility tree (UIA, NSAccessibility, AT-SPI) through
//! `accesskit_winit`, and the platform's requests come back as
//! `InputEvent::Access`.
//!
//! Nothing here runs until assistive technology asks for the tree: the
//! adapter reports activation through the event loop, and only then does
//! the shell derive and push trees — on frames whose tree hash changed.
//!
//! The translation is `kui-access`'s, shared with the bridge a host
//! attaches to a window of its own (backlog F269): the tree as AccessKit
//! nodes (the platform role is `kui_access::role_of`), a request as the
//! core's, and the `Feed` that decides when to send. What stays here is
//! winit's half: the adapter on a winit window, its events through the
//! loop's proxy.

#[cfg(feature = "accesskit")]
mod imp {
    use accesskit_winit::{Adapter, Event, WindowEvent as AkWindowEvent};
    use kui_access::{Feed, request_of};
    use kui_core::{AccessRequest, AccessTree, Announcement, Assistive};
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::window::Window;

    /// The runner's event-loop user event: what AccessKit sends back, and
    /// the one an app's own thread sends through a [`crate::Waker`].
    #[derive(Debug)]
    pub enum UserEvent {
        Access(Event),
        /// Something the app owns changed off the loop's thread: draw.
        Wake,
        /// Draw at this time (`Waker::wake_at`, backlog F135).
        WakeAt(std::time::Instant),
        /// The system's installed fonts changed (`mod system_fonts`).
        /// Only macOS and Windows say so; elsewhere nothing sends it.
        #[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
        FontsChanged,
        /// A file dialog shown for `window` was answered.
        /// Only the `dialogs` feature shows one.
        #[cfg_attr(not(feature = "dialogs"), allow(dead_code))]
        Files {
            window: winit::window::WindowId,
            paths: Vec<String>,
        },
    }

    impl From<Event> for UserEvent {
        fn from(e: Event) -> Self {
            UserEvent::Access(e)
        }
    }

    /// Which window the platform is talking about: every adapter is one
    /// window's, and the shell routes to that window's bridge. A wake is
    /// nobody's window.
    pub fn window_of(ev: &UserEvent) -> Option<winit::window::WindowId> {
        match ev {
            UserEvent::Access(ev) => Some(ev.window_id),
            UserEvent::Files { window, .. } => Some(*window),
            UserEvent::Wake | UserEvent::WakeAt(_) | UserEvent::FontsChanged => None,
        }
    }

    pub struct Bridge {
        adapter: Adapter,
        /// Whether a client listens, what was sent last, and the
        /// announcements standing in the tree (`kui_access::Feed`).
        feed: Feed,
    }

    impl Bridge {
        /// Must run before the window is shown (the platform adapters hook
        /// the window at creation).
        pub fn new(
            event_loop: &ActiveEventLoop,
            window: &Window,
            proxy: EventLoopProxy<UserEvent>,
        ) -> Option<Self> {
            Some(Bridge {
                adapter: Adapter::with_event_loop_proxy(event_loop, window, proxy),
                feed: Feed::new(),
            })
        }

        pub fn process_event(&mut self, window: &Window, event: &WindowEvent) {
            self.adapter.process_event(window, event);
        }

        pub fn active(&self) -> bool {
            self.feed.active()
        }

        /// The reading `env.system.assistive` carries: a
        /// client has asked for the tree, or none has. Whether it ever
        /// falls back is the adapter's: only `accesskit_unix` sends
        /// `AccessibilityDeactivated`; the macOS and Windows adapters in
        /// `accesskit_winit` take the deactivation handler and never call
        /// it, so there the reading rises once and stays.
        pub fn assistive(&self) -> Assistive {
            self.feed.assistive()
        }

        /// Folds an AccessKit event in; an action request comes back as
        /// the core's input to dispatch.
        pub fn on_event(&mut self, ev: UserEvent) -> Option<AccessRequest> {
            let UserEvent::Access(ev) = ev else {
                return None;
            };
            match ev.window_event {
                AkWindowEvent::InitialTreeRequested => {
                    self.feed.activate();
                    None
                }
                AkWindowEvent::AccessibilityDeactivated => {
                    self.feed.deactivate();
                    None
                }
                AkWindowEvent::ActionRequested(req) => request_of(req),
            }
        }

        /// Pushes the tree when assistive technology is attached and the
        /// tree differs from the last one sent.
        pub fn publish(&mut self, tree: &AccessTree, scale: f32, said: &[Announcement]) {
            let adapter = &mut self.adapter;
            self.feed
                .publish(tree, scale, said, |u| adapter.update_if_active(|| u));
        }
    }
}

#[cfg(not(feature = "accesskit"))]
mod imp {
    use kui_core::{AccessRequest, AccessTree, Announcement, Assistive};
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::window::Window;

    /// Without the `accesskit` feature the only user event is the wake an
    /// app's own thread sends through a [`crate::Waker`].
    #[derive(Debug)]
    pub enum UserEvent {
        /// Something the app owns changed off the loop's thread: draw.
        Wake,
        /// Draw at this time (`Waker::wake_at`, backlog F135).
        WakeAt(std::time::Instant),
        /// The system's installed fonts changed (`mod system_fonts`).
        /// Only macOS and Windows say so; elsewhere nothing sends it.
        #[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
        FontsChanged,
        /// A file dialog shown for `window` was answered.
        /// Only the `dialogs` feature shows one.
        #[cfg_attr(not(feature = "dialogs"), allow(dead_code))]
        Files {
            window: winit::window::WindowId,
            paths: Vec<String>,
        },
    }

    pub fn window_of(ev: &UserEvent) -> Option<winit::window::WindowId> {
        match ev {
            UserEvent::Files { window, .. } => Some(*window),
            UserEvent::Wake | UserEvent::WakeAt(_) | UserEvent::FontsChanged => None,
        }
    }

    pub struct Bridge {}

    impl Bridge {
        pub fn new(
            _event_loop: &ActiveEventLoop,
            _window: &Window,
            _proxy: EventLoopProxy<UserEvent>,
        ) -> Option<Self> {
            None
        }

        pub fn process_event(&mut self, _window: &Window, _event: &WindowEvent) {}

        pub fn active(&self) -> bool {
            false
        }

        /// Never constructed (`new` answers `None`), so a pane built
        /// without the feature reads `Unknown` through the `map_or` in
        /// `sync_env` rather than through here.
        pub fn assistive(&self) -> Assistive {
            Assistive::Unknown
        }

        pub fn on_event(&mut self, ev: UserEvent) -> Option<AccessRequest> {
            match ev {
                // Handled by the shell before a bridge is asked.
                UserEvent::Wake
                | UserEvent::WakeAt(_)
                | UserEvent::FontsChanged
                | UserEvent::Files { .. } => None,
            }
        }

        pub fn publish(&mut self, _tree: &AccessTree, _scale: f32, _said: &[Announcement]) {}
    }
}

pub use imp::{Bridge, UserEvent, window_of};
