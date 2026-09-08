//! The windowed runner (winit + wgpu) driven through C callbacks.

use super::*;

// ---------------------------------------------------------------------------
// Windowed runner (winit + wgpu) via C callbacks

struct CApp {
    user: *mut c_void,
    view: ViewFn,
    on_event: Option<EventFn>,
}

impl kui::App for CApp {
    fn view(&mut self, ui: &mut kui::Ui<'_>) {
        // Hand the callback a context that borrows the runner's frame for the
        // duration of view(). `borrowing_in` rather than `borrowing`: the
        // `Ui` is what carries the runner's extensions, so a `kui_slot`
        // from the callback fills in place the way a Rust host's does.
        let mut shim = KuiCtx::borrowing_in(ui);
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
    kui_run_with(std::ptr::null_mut(), title, view, on_event, user)
}

/// `kui_run` with the extensions `ctx` loaded (ADR 0014): the window's
/// runner takes them, so the context is left with none and is still the
/// caller's to free. A NULL context is `kui_run`.
///
/// There is no loader here, on purpose. `kui_ctx_add_extension` is the
/// loader, with `kui_ctx_extension_error` for the reason a plugin was
/// refused; a second one taking paths would need a second error channel,
/// and the first version of this function had exactly that (stderr, and
/// a note advising to load into a context first to find out why).
///
/// The host's view declares slots with `kui_slot` exactly as it would
/// headless, and what an extension's nodes produce reaches the host as
/// replies carrying that extension's origin.
#[unsafe(no_mangle)]
pub extern "C" fn kui_run_with(
    ptr: *mut KuiCtx,
    title: KuiStr,
    view: ViewFn,
    on_event: EventFn,
    user: *mut c_void,
) -> bool {
    guard(false, || {
        let extensions = unsafe { ctx(ptr) }
            .map_or_else(Default::default, |c| std::mem::take(&mut c.extensions));
        let app = CApp {
            user,
            view,
            on_event: Some(on_event),
        };
        kui::app(&kstr(title))
            .with_extensions(extensions)
            .run(app)
            .is_ok()
    })
}
