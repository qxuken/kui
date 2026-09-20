//! The windowed runner (winit + wgpu) driven through C callbacks.

use super::*;

// ---------------------------------------------------------------------------
// Windowed runner (winit + wgpu) via C callbacks

struct CApp {
    user: *mut c_void,
    view: ViewFn,
    on_event: Option<EventFn>,
    /// What `kui_on_teardown` set before the run, taken as the run began.
    teardown: Option<TeardownFn>,
}

/// The teardown callback `kui_on_teardown` set, for the next `kui_run` /
/// `kui_run_with` on any thread to take: a process setting, since
/// `kui_run`'s app is three arguments and not a struct (backlog RG1).
static ON_TEARDOWN: std::sync::Mutex<Option<TeardownFn>> = std::sync::Mutex::new(None);

impl kui::App for CApp {
    fn view(&mut self, ui: &mut kui::Ui<'_>) {
        // Hand the callback a context that borrows the runner's frame for the
        // duration of view(). `borrowing_in` rather than `borrowing`: the
        // `Ui` is what carries the runner's extensions, so a `kui_slot`
        // from the callback fills in place the way a Rust host's does.
        let mut shim = KuiCtx::borrowing_in(ui);
        (self.view)(self.user, &mut shim);
    }

    /// The window going for good (backlog F74), to the C host: once, with
    /// the `user` its `view` and `on_event` get, before `kui_run` returns
    /// or the process exits — which on macOS a Quit does without
    /// `kui_run` ever returning, so nothing after the call runs.
    fn teardown(&mut self) {
        if let Some(cb) = self.teardown {
            cb(self.user);
        }
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
            slot: ev.slot.map_or(0, |k| k.0),
            ..Default::default()
        };
        cb(self.user, &out);
    }
}

/// What the next `kui_run` / `kui_run_with` calls as its window goes for
/// good — the close button, Quit from the menu or the dock, a
/// `KUI_WINDOW_CLOSE` command on it — once, with the `user` the run's
/// `view` and `on_event` get, before `kui_run` returns or the process
/// exits (backlog RG1; the Rust `App::teardown` of F74). On macOS a Quit
/// ends the process from inside the run, so this is the only thing a
/// host runs on ⌘Q: nothing after `kui_run` does. The last call before
/// the run wins; a run takes it, so the next run starts with none.
/// Nothing draws by then, and the context the callback might reach is
/// the window's, not the host's: save, and return. A free function
/// rather than a field: `kui_run`'s app is three arguments and not a
/// struct, and `KuiRunConfig` is the window,
/// so a field there would reach `kui_run_with` alone and cost the [in]
/// bump AR50 asks for.
#[unsafe(no_mangle)]
pub extern "C" fn kui_on_teardown(teardown: TeardownFn) {
    guard((), || {
        *ON_TEARDOWN.lock().unwrap_or_else(|e| e.into_inner()) = Some(teardown);
    })
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
    kui_run_with(
        std::ptr::null_mut(),
        title,
        std::ptr::null(),
        view,
        on_event,
        user,
    )
}

/// `kui_run` with a window of the host's choosing and the context's
/// registrations (backlog AR27): `config` is the window — its size and
/// bounds, its chrome, the text antialiasing, the diagnostics — read the
/// way `kui_window_declare` reads its own, NULL for every default; and
/// `ctx`'s core becomes the window's, so the fonts, images, sounds,
/// tokens, theme, devtools doors, `kui_set_native_menus` and text-cache
/// budget the host registered on it before the call reach the window,
/// and the handles it minted keep drawing there. The context is left
/// with a fresh core and no extensions, still the caller's to free. A
/// NULL context with a NULL config is `kui_run`.
///
/// A config word this build does not have — a `chrome`, `text_aa` or
/// `diagnostics` past the last constant, a size that is not a size —
/// returns false before any window opens, with the reason on stderr: a
/// window opened native when asked for the custom chrome would draw its
/// titlebar under the OS's, and the host had no way to hear that.
///
/// The extensions loaded into the context (ADR 0014) come along the same
/// way: the window's runner takes them. There is no loader here, on
/// purpose. `kui_ctx_add_extension` is the loader, with
/// `kui_ctx_extension_error` for the reason a plugin was refused; a
/// second one taking paths would need a second error channel, and the
/// first version of this function had exactly that (stderr, and a note
/// advising to load into a context first to find out why).
///
/// The host's view declares slots with `kui_slot` exactly as it would
/// headless, and what an extension's nodes produce reaches the host as
/// replies carrying that extension's origin.
///
/// The context's `env.system` comes along too, as the window's pin: what
/// the host pushed with `kui_env_set_system` before handing the context
/// here is laid over the OS's reading before every frame (backlog F47),
/// so `kui_env_set_system(ctx, 0, 0, KUI_MOTION_REDUCED, empty)` and then
/// `kui_run_with(ctx, ..)` opens the window as a user who asked for less
/// motion sees it, and the zero fields keep following the OS. A context
/// never told anything pins nothing — zero is unknown is not pinned — so
/// `kui_run` is unchanged. This is the launcher door C has: the ffi's
/// headless setter is sticky by nature (a C host is its own frame driver
/// there), and under `kui_run` the runner is, which is why a pin has to
/// ride in with the context rather than be pushed at a window.
#[unsafe(no_mangle)]
pub extern "C" fn kui_run_with(
    ptr: *mut KuiCtx,
    title: KuiStr,
    config: *const KuiRunConfig,
    view: ViewFn,
    on_event: EventFn,
    user: *mut c_void,
) -> bool {
    guard(false, || {
        let options = match run_options_of(unsafe { config.as_ref() }) {
            Ok(o) => o,
            Err(why) => {
                eprintln!("kui: kui_run_with: {why}");
                return false;
            }
        };
        let mut launcher = launcher_for(&kstr(title), options);
        if let Some(c) = unsafe { ctx(ptr) } {
            launcher = launcher
                .with_extensions(std::mem::take(&mut c.extensions))
                .system(c.core().env.system);
            if let Some(core) = c.take_core() {
                launcher = launcher.core(*core);
            }
        }
        let app = CApp {
            user,
            view,
            on_event: Some(on_event),
            teardown: ON_TEARDOWN.lock().unwrap_or_else(|e| e.into_inner()).take(),
        };
        launcher.run(app).is_ok()
    })
}

/// The launcher `options` describe: each field that was set is one
/// builder call, a default is none, so `kui_run` builds exactly what it
/// always did.
fn launcher_for(title: &str, options: RunOptions) -> kui::Launcher {
    let mut l = kui::app(title);
    if let Some((w, h)) = options.size {
        l = l.size(w, h);
    }
    if let Some((w, h)) = options.min_size {
        l = l.min_size(w, h);
    }
    if let Some((w, h)) = options.max_size {
        l = l.max_size(w, h);
    }
    l = l.chrome(match options.chrome {
        KUI_CHROME_CUSTOM => kui::Chrome::Custom,
        KUI_CHROME_BORDERLESS => kui::Chrome::Borderless,
        _ => kui::Chrome::Native,
    });
    l = l.text_aa(match options.text_aa {
        KUI_TEXT_AA_GRAYSCALE => kui::TextAa::Grayscale,
        KUI_TEXT_AA_SUBPIXEL => kui::TextAa::Subpixel,
        _ => kui::TextAa::Auto,
    });
    if let Some(on) = options.diagnostics {
        l = l.diagnostics(on);
    }
    l
}
