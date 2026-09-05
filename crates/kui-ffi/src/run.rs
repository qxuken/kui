//! The windowed runner (winit + wgpu) driven through C callbacks.

use super::*;

// ---------------------------------------------------------------------------
// Windowed runner (winit + wgpu) via C callbacks

pub(crate) type ViewFn = extern "C" fn(user: *mut c_void, ctx: *mut KuiCtx);
pub(crate) type EventFn = extern "C" fn(user: *mut c_void, ev: *const KuiEvent);

struct CApp {
    user: *mut c_void,
    view: ViewFn,
    on_event: Option<EventFn>,
}

impl kui::App for CApp {
    fn view(&mut self, ui: &mut kui::Ui<'_>) {
        // Hand the callback a context that borrows the runner's Core for the
        // duration of view(); builder entry points only touch `core`.
        let mut shim = KuiCtx::borrowing(ui.core());
        (self.view)(self.user, &mut shim);
    }

    fn on_event(&mut self, ev: kui::UiEvent) {
        let Some(cb) = self.on_event else { return };
        let payload = KuiValue(ev.payload);
        // Library-allocated, so the full struct: `size` says how much of it
        // is meaningful, which is all of it.
        let out = KuiEvent {
            origin: ev.origin.0,
            key: ev.key.0,
            payload: &payload,
            window: ev.window.0,
            ..Default::default()
        };
        cb(self.user, &out);
    }
}

/// Runs a windowed app driven by C callbacks. Blocks until the window closes.
/// Returns false if the event loop could not start.
#[unsafe(no_mangle)]
pub extern "C" fn kui_run(
    title: KuiStr,
    view: ViewFn,
    on_event: EventFn,
    user: *mut c_void,
) -> bool {
    guard(false, || {
        let title = kstr(title).into_owned();
        let app = CApp {
            user,
            view,
            on_event: Some(on_event),
        };
        kui::run(&title, app, vec![]).is_ok()
    })
}
