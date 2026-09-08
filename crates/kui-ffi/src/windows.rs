//! The window: the facts a driver pushes (`kui_env_set_window`), the
//! declared set and the commands a driver drains (`docs/adr/0004`), and
//! the title.

use super::*;

/// Window facts for views to read (widgets::titlebar adapts to them).
/// `window` is which window this context draws — `KUI_WINDOW_MAIN`, or the
/// id an `Open` command carried — and is what every event it hands out
/// will say in `KuiEvent.window`. `controls_w/h > 0` describe the keep-out
/// rect of controls the OS draws over the content (macOS traffic lights),
/// anchored top-left.
#[unsafe(no_mangle)]
pub extern "C" fn kui_env_set_window(
    ptr: *mut KuiCtx,
    window: u32,
    custom_chrome: bool,
    maximized: bool,
    fullscreen: bool,
    controls_w: f32,
    controls_h: f32,
) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let win = &mut c.core().env.window;
            win.id = WindowId(window);
            win.custom_chrome = custom_chrome;
            win.maximized = maximized;
            win.fullscreen = fullscreen;
            win.native_controls = (controls_w > 0.0 && controls_h > 0.0)
                .then(|| Rect::new(0.0, 0.0, controls_w, controls_h));
        }
    });
}

/// Pops the next window command into `out`: what chrome nodes asked for
/// since the last drain, and the `KUI_CMD_OPEN` / `KUI_CMD_CLOSE` the
/// declared window set's diff decided at the last `kui_frame_finish`.
/// Returns false, writing nothing, when there is none — or when `out`'s
/// `size` is below the layout this library knows (start from
/// `KUI_WINDOW_COMMAND_INIT`), in which case the command stays queued.
/// Call after each input dispatch and each frame until it returns false,
/// and apply each to the real window it names.
#[unsafe(no_mangle)]
pub extern "C" fn kui_take_window_command(ptr: *mut KuiCtx, out: *mut KuiWindowCommand) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        if c.window_commands.is_empty() {
            let fresh = c.core().take_window_commands();
            c.window_commands.extend(fresh);
        }
        let Some(&cmd) = c.window_commands.front() else {
            return false;
        };
        if !write_out(out, window_command_to_c(cmd)) {
            return false;
        }
        c.window_commands.pop_front();
        true
    })
}

/// Declares that a window named `name` exists this frame (ADR 0004): it
/// opens on the first frame any window's frame declares it — `cfg` is
/// read then and never again, NULL meaning `KUI_WINDOW_CONFIG_INIT` — and
/// closes on the first frame none does. The `Open` / `Close` arrive
/// through `kui_take_window_command`; the app sees `{kind:"window",
/// phase:"opened"|"closed", name, id}` events. A window the user closed
/// (`kui_window_closed`) stays closed while still declared: stop declaring
/// it, then declare it again. Call between `kui_frame_begin` and
/// `kui_frame_finish`.
///
/// A `cfg` with `kind = KUI_WINDOW_KIND_POPUP` (start from
/// `KUI_WINDOW_POPUP_INIT`) declares a menu surface instead: borderless,
/// off the taskbar, owned by this window and closed with it, placed
/// against `cfg->anchor_*` in screen coordinates, and non-activating — so
/// the host opens it without focus and routes this window's keys to it.
/// Report a press outside it or an Escape with `kui_window_dismissed`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_window_declare(ptr: *mut KuiCtx, name: KuiStr, cfg: *const KuiWindowConfig) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let raw = unsafe { cfg.as_ref() };
            let cfg = window_config_of(raw);
            let name = kstr(name);
            // A kind this build does not have degrades to `Normal` (see
            // `window_config_of`) — but silently it would read as the popup
            // having shipped, so the degradation says so.
            if let Some(k) = raw
                .map(|c| c.kind)
                .filter(|k| *k != KUI_WINDOW_KIND_NORMAL && *k != KUI_WINDOW_KIND_POPUP)
            {
                c.core().warn(kui_core::diag::unknown_window_kind(&name, k));
            }
            c.core().declare_window(&name, cfg);
        }
    });
}

/// Asks the driver to resize `window` to `w`x`h` logical px. A request and
/// not a declaration: `kui_window_declare`'s config is read on the opening
/// edge only, because the user owns a window's size once it exists, so this
/// is the only way an app moves a live window's size. It is queued the way
/// `kui_reveal` queues a scroll and comes back out of the host's own
/// `kui_take_window_command` as `KUI_CMD_SET_SIZE`, carrying `window` and
/// the size in `width`/`height`, for the host to apply; a headless host
/// that never drains ignores it. `window` is the id events carry
/// (`KuiEvent.window`), `KUI_WINDOW_MAIN` for the launcher's. The size the
/// window actually becomes arrives as the ordinary `resize` event.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_window_size(ptr: *mut KuiCtx, window: u32, w: f32, h: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_window_size(WindowId(window), Size::new(w, h));
        }
    });
}

/// Asks the driver to give `window` keyboard focus; queued and drained the
/// same way, as `KUI_CMD_FOCUS`. Advisory, like every focus request an app
/// makes of a window manager: whether it was granted shows up through
/// `kui_env_set`'s `focused` on the frames that follow, not as a reply here.
#[unsafe(no_mangle)]
pub extern "C" fn kui_focus_window(ptr: *mut KuiCtx, window: u32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().focus_window(WindowId(window));
        }
    });
}

/// Which window this context draws: `env.window.id`, as
/// `kui_env_set_window` set it — `KUI_WINDOW_MAIN` for the launcher's, and
/// in a `kui_run` view callback the id of the window being drawn.
#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_window(ptr: *mut KuiCtx) -> u32 {
    guard(0, || match unsafe { ctx(ptr) } {
        Some(c) => c.core().env.window.id.0,
        None => 0,
    })
}

/// The name of the window this context draws: "main" for the launcher's,
/// else the name the declaration that opened it used. Borrowed until the
/// next call on the same context. False only on a bad context.
#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_window_name(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        if out.is_null() {
            return false;
        }
        let name = c.core().window_name();
        unsafe {
            *out = KuiStr {
                ptr: name.as_ptr(),
                len: name.len(),
            };
        }
        c.last_window_name = Some(name);
        true
    })
}

/// A host reports that window `id` was asked to go away: a press landed
/// outside it (`KUI_DISMISS_OUTSIDE`), or Escape reached it
/// (`KUI_DISMISS_ESCAPE`). The app gets `{kind:"dismiss", reason, name,
/// id}` from `kui_poll_event` and **nothing closes** — exactly what a
/// `modal` node's dismissal does, one level up: only the app can stop
/// declaring the window, and it does that on the frame it decides to. So a
/// dropdown that graduates from a modal float to a popup window changes
/// its declaration and keeps its handler.
///
/// This is a host call and not something the core notices, because neither
/// fact is the frame's: a press outside a window lands in another surface,
/// and a non-activating popup is never the window the OS hands keys to.
/// Nothing happens for a window the session has not opened.
#[unsafe(no_mangle)]
pub extern "C" fn kui_window_dismissed(ptr: *mut KuiCtx, id: u32, reason: u32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let reason = match reason {
                KUI_DISMISS_ESCAPE => DismissReason::Escape,
                _ => DismissReason::Outside,
            };
            c.core().dismiss_window(WindowId(id), reason);
            let evs = c.core().take_pending_events();
            c.absorb(evs);
        }
    });
}

/// A host reports that the OS closed window `id` — its close button, the
/// window manager. The window stays closed while its name is still
/// declared (see `kui_window_declare`), whatever only it declared closes
/// with it, and the app gets `{kind:"window", phase:"closed", name, id}`
/// from `kui_poll_event`. Nothing happens for `KUI_WINDOW_MAIN` or for a
/// window already closed by the diff.
#[unsafe(no_mangle)]
pub extern "C" fn kui_window_closed(ptr: *mut KuiCtx, id: u32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().window_closed(WindowId(id));
            let evs = c.core().take_pending_events();
            c.absorb(evs);
        }
    });
}

/// Declares this frame's window title (cleared each kui_frame_begin).
#[unsafe(no_mangle)]
pub extern "C" fn kui_window_title(ptr: *mut KuiCtx, title: KuiStr) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let title = kstr(title).into_owned();
            c.core().set_window_title(&title);
        }
    });
}

/// The title declared this frame, if any — for hosts driving their own
/// window: diff and apply after kui_frame_finish. The view is valid until
/// the next kui_frame_begin.
#[unsafe(no_mangle)]
pub extern "C" fn kui_window_title_get(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
    guard(false, || {
        let (Some(c), Some(out)) = (unsafe { ctx(ptr) }, unsafe { out.as_mut() }) else {
            return false;
        };
        let Some(t) = c.core().window_title() else {
            return false;
        };
        *out = KuiStr {
            ptr: t.as_ptr(),
            len: t.len(),
        };
        true
    })
}
