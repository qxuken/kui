//! A `Blur` backdrop asked of a Linux compositor (backlog F126): the
//! compositor blurs what is behind the window's translucent pixels, where
//! it has a way to be asked.
//!
//! - **Wayland**: the staging `ext-background-effect-v1`, when the
//!   compositor advertises it with its blur capability — a cross-compositor
//!   protocol, written by KWin's developers and implemented by KWin; others
//!   may follow — and else KWin's own `org_kde_kwin_blur`. Both are bound
//!   on a connection of kui's own over the display winit opened
//!   (`Backend::from_foreign_display`), so nothing of winit's changes —
//!   once per process, for the first window that asks, since neither the
//!   registry nor KWin's manager can be given back; each window's effect
//!   object is its own and destroyed with it. The blur state is
//!   double-buffered and lands with the next frame's commit.
//! - **X11**: `_KDE_NET_WM_BLUR_BEHIND_REGION` on the window, an empty
//!   region meaning all of it, where the window manager lists the atom in
//!   `_NET_SUPPORTED` (KWin does).
//! - Elsewhere — GNOME, which has no blur protocol, wlroots compositors —
//!   nothing is asked, and the backdrop falls back to the wallpaper kui
//!   draws (`mod ground`). Hyprland, SwayFX and picom blur translucent
//!   windows themselves when configured to, so an app there asks for
//!   `Transparent` and gets the blur from the compositor.

use std::any::Any;

use winit::raw_window_handle::{
    HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use winit::window::Window;

/// Asks the compositor to blur what is behind `window`. `Some` holds what
/// must live as long as the window for the blur to stay; `None` is a
/// compositor with no way to be asked.
pub(crate) fn request(window: &Window) -> Option<Box<dyn Any>> {
    let display = window.display_handle().ok()?.as_raw();
    let handle = window.window_handle().ok()?.as_raw();
    match (display, handle) {
        (RawDisplayHandle::Wayland(d), RawWindowHandle::Wayland(w)) => {
            // SAFETY: winit's display and surface, live for as long as the
            // window — and the pane keeps what this returns no longer; the
            // display lives as long as the event loop, which makes every
            // window there will be.
            unsafe { wayland::request(d.display.as_ptr(), w.surface.as_ptr()) }
        }
        (_, RawWindowHandle::Xlib(h)) => x11::request(h.window as u32),
        (_, RawWindowHandle::Xcb(h)) => x11::request(h.window.get()),
        _ => None,
    }
}

/// Whether a window there can be see-through at all: always under
/// Wayland, and under X11 only while a compositing manager runs (one owns
/// `_NET_WM_CM_S<screen>`) — without one an ARGB window's transparent
/// pixels are black.
pub(crate) fn composited(window: &Window) -> bool {
    match window.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Xlib(_)) | Ok(RawWindowHandle::Xcb(_)) => x11::composited(),
        _ => true,
    }
}

mod wayland {
    use std::any::Any;
    use std::ffi::c_void;
    use std::sync::Mutex;

    use wayland_backend::client::{Backend, ObjectId};
    use wayland_client::globals::{GlobalListContents, registry_queue_init};
    use wayland_client::protocol::{
        wl_compositor::WlCompositor, wl_region::WlRegion, wl_registry::WlRegistry,
        wl_surface::WlSurface,
    };
    use wayland_client::{
        Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, delegate_noop,
    };
    use wayland_protocols::ext::background_effect::v1::client::{
        ext_background_effect_manager_v1::{self, ExtBackgroundEffectManagerV1},
        ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
    };
    use wayland_protocols_plasma::blur::client::{
        org_kde_kwin_blur::OrgKdeKwinBlur, org_kde_kwin_blur_manager::OrgKdeKwinBlurManager,
    };

    /// What the bound globals told us: whether the background-effect
    /// manager offers blur.
    #[derive(Default)]
    pub(super) struct State {
        blur: bool,
    }

    impl Dispatch<WlRegistry, GlobalListContents> for State {
        fn event(
            _: &mut Self,
            _: &WlRegistry,
            _: wayland_client::protocol::wl_registry::Event,
            _: &GlobalListContents,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    impl Dispatch<ExtBackgroundEffectManagerV1, ()> for State {
        fn event(
            state: &mut Self,
            _: &ExtBackgroundEffectManagerV1,
            event: ext_background_effect_manager_v1::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let ext_background_effect_manager_v1::Event::Capabilities { flags } = event {
                state.blur = match flags {
                    WEnum::Value(c) => {
                        c.contains(ext_background_effect_manager_v1::Capability::Blur)
                    }
                    WEnum::Unknown(bits) => bits & 1 != 0,
                };
            }
        }
    }

    delegate_noop!(State: WlCompositor);
    delegate_noop!(State: WlRegion);
    delegate_noop!(State: ExtBackgroundEffectSurfaceV1);
    delegate_noop!(State: OrgKdeKwinBlurManager);
    delegate_noop!(State: OrgKdeKwinBlur);

    /// The blur asked for, kept for the window's life and taken back when
    /// the pane goes.
    enum Effect {
        Ext(ExtBackgroundEffectSurfaceV1),
        Kde(OrgKdeKwinBlur),
    }

    struct Kept {
        conn: Connection,
        effect: Effect,
    }

    impl Drop for Kept {
        fn drop(&mut self) {
            match &self.effect {
                Effect::Ext(e) => e.destroy(),
                Effect::Kde(b) => b.release(),
            }
            let _ = self.conn.flush();
        }
    }

    /// What every window's blur is asked through, bound once per process
    /// on kui's own queue over winit's display: the registry, and the
    /// managers and compositor the compositor offers. None of them can be
    /// given back one window at a time — `wl_registry`, `wl_compositor`
    /// and `org_kde_kwin_blur_manager` have no destructor, and an
    /// `ext_background_effect_manager_v1` destroyed and bound again per
    /// window would cost a roundtrip each — so they are bound for the
    /// first window that asks and kept, with the queue their events land
    /// on.
    struct Shared {
        /// The `wl_display` they were bound on.
        display: usize,
        conn: Connection,
        queue: EventQueue<State>,
        state: State,
        compositor: Option<WlCompositor>,
        ext: Option<ExtBackgroundEffectManagerV1>,
        kde: Option<OrgKdeKwinBlurManager>,
    }

    /// Never dropped: a static's value outlives the display, and dropping
    /// the queue destroys a `wl_event_queue` on it.
    static SHARED: Mutex<Option<Shared>> = Mutex::new(None);

    impl Shared {
        /// Binds the globals on `display`.
        ///
        /// # Safety
        ///
        /// `display` is a live `wl_display` that outlives every use of
        /// what this returns — winit's, which lives as long as the event
        /// loop, after which no window is made.
        unsafe fn bind(display: *mut c_void) -> Option<Self> {
            // SAFETY: the caller's.
            let conn =
                Connection::from_backend(unsafe { Backend::from_foreign_display(display.cast()) });
            let (globals, mut queue) = registry_queue_init::<State>(&conn).ok()?;
            let qh = queue.handle();
            let mut state = State::default();
            let ext = globals
                .bind::<ExtBackgroundEffectManagerV1, _, _>(&qh, 1..=1, ())
                .ok();
            if ext.is_some() {
                // The capabilities arrive on binding.
                queue.roundtrip(&mut state).ok()?;
            }
            let compositor = ext
                .as_ref()
                .and_then(|_| globals.bind::<WlCompositor, _, _>(&qh, 1..=4, ()).ok());
            let kde = globals
                .bind::<OrgKdeKwinBlurManager, _, _>(&qh, 1..=1, ())
                .ok();
            Some(Shared {
                display: display as usize,
                conn,
                queue,
                state,
                compositor,
                ext,
                kde,
            })
        }
    }

    /// See [`super::request`].
    ///
    /// # Safety
    ///
    /// `display` is a live `wl_display` and `surface` a live `wl_surface`
    /// on it, both outliving what this returns; and `display` outlives
    /// every window made after this one (winit's, which lives as long as
    /// the event loop).
    pub(super) unsafe fn request(
        display: *mut c_void,
        surface: *mut c_void,
    ) -> Option<Box<dyn Any>> {
        let mut shared = SHARED.lock().unwrap_or_else(|e| e.into_inner());
        if shared
            .as_ref()
            .is_none_or(|s| s.display != display as usize)
        {
            // Another display than the one bound on: winit makes one
            // event loop per process, so this is not expected. What was
            // bound on the old one is left alone — its display may be gone.
            if let Some(old) = shared.take() {
                std::mem::forget(old);
            }
            // SAFETY: the caller's.
            *shared = unsafe { Shared::bind(display) };
        }
        let shared = shared.as_mut()?;
        let conn = shared.conn.clone();
        let qh = shared.queue.handle();
        // A capability that changed since, read off the socket by winit
        // onto this queue.
        let _ = shared.queue.dispatch_pending(&mut shared.state);
        // SAFETY: the caller's; the interface is the surface's.
        let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.cast()) }.ok()?;
        let surface = WlSurface::from_id(&conn, id).ok()?;
        if shared.state.blur
            && let (Some(manager), Some(compositor)) = (&shared.ext, &shared.compositor)
        {
            let effect = manager.get_background_effect(&surface, &qh, ());
            // The whole surface: the compositor clips the region to it.
            let region = compositor.create_region(&qh, ());
            region.add(0, 0, 1 << 16, 1 << 16);
            effect.set_blur_region(Some(&region));
            region.destroy();
            conn.flush().ok()?;
            return Some(Box::new(Kept {
                conn,
                effect: Effect::Ext(effect),
            }));
        }
        if let Some(manager) = &shared.kde {
            let blur = manager.create(&surface, &qh, ());
            // No region is the whole surface.
            blur.set_region(None);
            blur.commit();
            conn.flush().ok()?;
            return Some(Box::new(Kept {
                conn,
                effect: Effect::Kde(blur),
            }));
        }
        None
    }
}

mod x11 {
    use std::any::Any;

    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode};
    use x11rb::wrapper::ConnectionExt as _;

    /// See [`super::request`]: the property on `window`, where the window
    /// manager says it reads it.
    pub(super) fn request(window: u32) -> Option<Box<dyn Any>> {
        let (conn, screen) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        let atom = |name: &[u8]| Some(conn.intern_atom(false, name).ok()?.reply().ok()?.atom);
        let supported = atom(b"_NET_SUPPORTED")?;
        let blur = atom(b"_KDE_NET_WM_BLUR_BEHIND_REGION")?;
        let listed = conn
            .get_property(false, root, supported, AtomEnum::ATOM, 0, 4096)
            .ok()?
            .reply()
            .ok()?;
        if !listed.value32()?.any(|a| a == blur) {
            return None;
        }
        // An empty region is the whole window.
        conn.change_property32(PropMode::REPLACE, window, blur, AtomEnum::CARDINAL, &[])
            .ok()?;
        conn.flush().ok()?;
        // The property is the window's now; nothing to keep.
        Some(Box::new(()))
    }

    /// See [`super::composited`].
    pub(super) fn composited() -> bool {
        let Ok((conn, screen)) = x11rb::connect(None) else {
            return false;
        };
        let name = format!("_NET_WM_CM_S{screen}");
        let Some(atom) = conn
            .intern_atom(false, name.as_bytes())
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.atom)
        else {
            return false;
        };
        conn.get_selection_owner(atom)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|r| r.owner != x11rb::NONE)
    }
}
