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

/// `kui_run` with extensions loaded into the window's own core
/// (ADR 0014). `paths` and `namespaces` are parallel arrays of `count`
/// entries: the shared library to load, and the word that fronts every
/// slot name it fills. `namespaces` may be NULL, and any entry in it may
/// be empty, to take each extension's own `kui_ext_name`.
///
/// Returns false without opening a window if any of them will not load or
/// two want one namespace — the reason goes to stderr, because there is no
/// context to hang it on and a window drawn silently without the panel the
/// host asked for would be worse. `kui_ctx_add_extension` on a standalone
/// context is how to inspect a failure before committing to a window.
///
/// The host's view declares slots with `kui_slot` exactly as it would
/// headless, and what an extension's nodes produce reaches the host as
/// replies carrying that extension's origin.
///
/// # Safety
/// Every library named runs in this process. Loading one is trusting it
/// exactly as much as linking it would be.
#[unsafe(no_mangle)]
pub extern "C" fn kui_run_with(
    title: KuiStr,
    view: ViewFn,
    on_event: EventFn,
    user: *mut c_void,
    paths: *const KuiStr,
    namespaces: *const KuiStr,
    count: usize,
) -> bool {
    guard(false, || {
        if count > 0 && paths.is_null() {
            eprintln!("kui: kui_run_with: {count} extensions asked for and no paths");
            return false;
        }
        let mut launcher = kui::app(&kstr(title));
        for i in 0..count {
            let path = kstr(unsafe { *paths.add(i) }).into_owned();
            // SAFETY: the caller's, and the doc comment says so.
            let ext = match unsafe { crate::CExtension::open(&path) } {
                Ok(ext) => ext,
                Err(err) => {
                    eprintln!("kui: {err}");
                    return false;
                }
            };
            let ns = match unsafe { namespaces.as_ref() } {
                Some(_) => kstr(unsafe { *namespaces.add(i) }).into_owned(),
                None => String::new(),
            };
            let ns = if ns.is_empty() {
                kui_core::Extension::name(&ext).to_owned()
            } else {
                ns
            };
            // The refusing form, not `extension_as`: these paths came from
            // outside the program, so a collision is a message and not a
            // panic.
            launcher = match launcher.try_extension_as(ns, ext) {
                Ok(l) => l,
                Err(err) => {
                    eprintln!("kui: {err}");
                    return false;
                }
            };
        }
        let app = CApp {
            user,
            view,
            on_event: Some(on_event),
        };
        launcher.run(app).is_ok()
    })
}
