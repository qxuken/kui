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
/// `kui_run`'s app is three arguments and not a struct.
static ON_TEARDOWN: std::sync::Mutex<Option<TeardownFn>> = std::sync::Mutex::new(None);

impl kui_native::App for CApp {
    fn view(&mut self, ui: &mut kui_native::Ui<'_>) {
        // Hand the callback a context that borrows the runner's frame for the
        // duration of view(). `borrowing_in` rather than `borrowing`: the
        // `Ui` is what carries the runner's extensions, so a `kui_slot`
        // from the callback fills in place the way a Rust host's does.
        let mut shim = KuiCtx::borrowing_in(ui);
        (self.view)(self.user, &mut shim);
    }

    /// The window going for good, to the C host: once, with
    /// the `user` its `view` and `on_event` get, before `kui_run` returns
    /// or the process exits — which on macOS a Quit does without
    /// `kui_run` ever returning, so nothing after the call runs.
    fn teardown(&mut self) {
        if let Some(cb) = self.teardown {
            cb(self.user);
        }
    }

    fn on_event(&mut self, ev: kui_native::UiEvent) {
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

/// Sets what the next `kui_run` / `kui_run_with` calls as its window goes
/// for good (the close button, Quit from the menu or the dock, a
/// `KUI_CMD_CLOSE` on it): once, with the `user` the run's `view` and
/// `on_event` get, before `kui_run` returns or the process exits.
///
/// On macOS a Quit ends the process from inside the run, so this is the
/// only thing a host runs on Command-Q; nothing after `kui_run` does.
/// Nothing draws by then: save, and return. The last call before the run
/// wins, and a run takes it, so the next run starts with none.
#[unsafe(no_mangle)]
pub extern "C" fn kui_on_teardown(teardown: TeardownFn) {
    guard((), || {
        *ON_TEARDOWN.lock().unwrap_or_else(|e| e.into_inner()) = Some(teardown);
    })
}

/// What `kui_set_icon` asked for, for the next run to take.
#[derive(Debug, PartialEq)]
struct IconAsk {
    pixels: Option<(Vec<u8>, u32, u32)>,
    resource: Option<u16>,
}

/// The icon `kui_set_icon` set, for the next `kui_run` / `kui_run_with`
/// to take — a process setting, as `ON_TEARDOWN` is and for its reason.
static ICON: std::sync::Mutex<Option<IconAsk>> = std::sync::Mutex::new(None);

/// Reads `kui_set_icon`'s arguments, refusing what is not an icon with the
/// reason: pixels with a zero side, a side with no pixels, a size whose
/// byte count overflows, a resource id past 16 bits. `Ok(None)` is every
/// argument zero — no icon.
///
/// # Safety
/// `rgba`, when not null, points at `width * height * 4` readable bytes.
unsafe fn icon_ask_of(
    rgba: *const u8,
    width: u32,
    height: u32,
    resource: u32,
) -> Result<Option<IconAsk>, String> {
    let pixels = match (rgba.is_null(), width, height) {
        (true, 0, 0) => None,
        (false, w, h) if w > 0 && h > 0 => {
            let len = (w as usize)
                .checked_mul(h as usize)
                .and_then(|n| n.checked_mul(4))
                .filter(|&n| n <= isize::MAX as usize)
                .ok_or_else(|| format!("a {w}x{h} icon is too large to address"))?;
            // SAFETY: the caller's promise, `width * height * 4` bytes.
            Some((
                unsafe { std::slice::from_raw_parts(rgba, len) }.to_vec(),
                w,
                h,
            ))
        }
        (true, w, h) => return Err(format!("a {w}x{h} icon with NULL pixels")),
        (false, w, h) => return Err(format!("an icon is at least one pixel, not {w}x{h}")),
    };
    let resource = match resource {
        0 => None,
        r => Some(
            u16::try_from(r).map_err(|_| format!("an icon resource id is 1 to 65535, not {r}"))?,
        ),
    };
    Ok((pixels.is_some() || resource.is_some()).then_some(IconAsk { pixels, resource }))
}

/// Sets the icon every window of the next `kui_run` / `kui_run_with` is
/// created with.
///
/// `rgba` is `width` by `height` pixels, four bytes each, row by row from
/// the top left, alpha not premultiplied, copied. `resource`, on Windows,
/// is an icon resource in the executable (the `1 ICON "app.ico"` of its
/// `.rc`), which wins there. NULL, 0, 0 is no pixels and 0 no resource;
/// all four zero clears it. Windows shows the icon in the title bar,
/// Alt-Tab and the taskbar and X11 in the window manager's; macOS and
/// Wayland have no window icon. Returns false, with the reason on stderr,
/// for what is not an icon (a zero side with pixels, a side with none, a
/// resource past 65535) and keeps what was set before. The last call
/// before the run wins; a run takes it.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_icon(rgba: *const u8, width: u32, height: u32, resource: u32) -> bool {
    guard(false, || {
        // SAFETY: the header's contract for `rgba`.
        match unsafe { icon_ask_of(rgba, width, height, resource) } {
            Ok(ask) => {
                *ICON.lock().unwrap_or_else(|e| e.into_inner()) = ask;
                true
            }
            Err(why) => {
                eprintln!("kui: kui_set_icon: {why}");
                false
            }
        }
    })
}

/// Opens a window titled `title` and runs it to the end: `view(user, ctx)`
/// is called once per frame with a context to build into, and
/// `on_event(user, ev)` once per event, on this thread. Blocks until the
/// window closes; returns false if the event loop could not start. Same as
/// `kui_run_with(NULL, title, NULL, view, on_event, user)`.
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

/// `kui_run` with a window of the host's choosing and a context's
/// registrations.
///
/// `config` is the window (size and bounds, chrome, text antialiasing,
/// diagnostics), NULL for every default. `ctx`'s core becomes the
/// window's: the fonts, images, sounds, tokens, theme, devtools settings,
/// `kui_set_native_menus` and text-cache budget registered on it before
/// the call reach the window, and the handles it minted keep drawing
/// there. The extensions loaded into it with [`kui_ctx_add_extension`]
/// come along the same way, and the host's view declares slots with
/// [`kui_slot`] exactly as it would headless. What the host pushed with
/// [`kui_env_set_system`] becomes the window's pin over the OS's reading
/// (a zero field keeps following the OS). The context is left with a
/// fresh core and no extensions, still the caller's to free. A NULL
/// context with a NULL config is `kui_run`.
///
/// Returns false before any window opens, with the reason on stderr, for
/// a config word this build does not have (`chrome`, `text_aa` or
/// `diagnostics` past the last constant, or a size that is not a size).
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
        if let Some(icon) = ICON.lock().unwrap_or_else(|e| e.into_inner()).take() {
            if let Some((rgba, w, h)) = icon.pixels {
                launcher = match launcher.try_icon(rgba, w, h) {
                    Ok(l) => l,
                    Err(why) => {
                        eprintln!("kui: kui_run_with: {why}");
                        return false;
                    }
                };
            }
            if let Some(id) = icon.resource {
                launcher = launcher.icon_resource(id);
            }
        }
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
fn launcher_for(title: &str, options: RunOptions) -> kui_native::Launcher {
    let mut l = kui_native::app(title);
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
        KUI_CHROME_CUSTOM => kui_native::Chrome::Custom,
        KUI_CHROME_BORDERLESS => kui_native::Chrome::Borderless,
        _ => kui_native::Chrome::Native,
    });
    l = l.text_aa(match options.text_aa {
        KUI_TEXT_AA_GRAYSCALE => kui_native::TextAa::Grayscale,
        KUI_TEXT_AA_SUBPIXEL => kui_native::TextAa::Subpixel,
        _ => kui_native::TextAa::Auto,
    });
    if let Some(on) = options.diagnostics {
        l = l.diagnostics(on);
    }
    if let Some(frames) = options.frame_latency {
        l = l.frame_latency(frames);
    }
    l.backdrop(options.backdrop)
}

// ---------------------------------------------------------------------------
// The runner's image decoder (backlog F138)

/// One allocation handed to C: a word holding its own length in words, a
/// spare word, the pixels, then the delays on a word boundary — so
/// `kui_pixels_free` needs only the pixel pointer, and a `double *` into
/// it is aligned. `u64` words, because a `Vec<u8>`'s alignment is one.
fn hand_out(rgba: &[u8], delays: &[f64]) -> (*mut u8, *const f64) {
    let px_words = rgba.len().div_ceil(8);
    let words = 2 + px_words + delays.len();
    let mut block = vec![0u64; words];
    block[0] = words as u64;
    let base = block.as_mut_ptr();
    std::mem::forget(block);
    // SAFETY: `base` owns `words` words; the pixels fit in the `px_words`
    // after the first two and the delays in the rest, each written once.
    unsafe {
        let px = base.add(2).cast::<u8>();
        std::ptr::copy_nonoverlapping(rgba.as_ptr(), px, rgba.len());
        let d = base.add(2 + px_words).cast::<f64>();
        std::ptr::copy_nonoverlapping(delays.as_ptr(), d, delays.len());
        (px, d)
    }
}

/// # Safety
/// `bytes`, when not null, points at `len` readable bytes.
unsafe fn bytes_of<'a>(bytes: *const u8, len: usize) -> &'a [u8] {
    if bytes.is_null() || len == 0 {
        &[]
    } else {
        // SAFETY: the caller's promise.
        unsafe { std::slice::from_raw_parts(bytes, len) }
    }
}

/// Decodes PNG, JPEG, WebP or GIF bytes (an animated file's first frame)
/// to straight RGBA, `*width * *height * 4` bytes row by row from the top
/// left — what `kui_add_image`, `kui_update_image` and `kui_set_icon`
/// take. Free the pixels with `kui_pixels_free`. NULL, with the reason on
/// stderr and both sides 0, for bytes that are not an image or do not
/// decode.
#[unsafe(no_mangle)]
pub extern "C" fn kui_decode_image(
    bytes: *const u8,
    len: usize,
    width: *mut u32,
    height: *mut u32,
) -> *mut u8 {
    guard(std::ptr::null_mut(), || {
        let put = |w: u32, h: u32| unsafe {
            if !width.is_null() {
                *width = w;
            }
            if !height.is_null() {
                *height = h;
            }
        };
        put(0, 0);
        match kui_native::decode_image(unsafe { bytes_of(bytes, len) }) {
            Ok(p) => {
                put(p.width, p.height);
                hand_out(&p.rgba, &[]).0
            }
            Err(why) => {
                eprintln!("kui: kui_decode_image: {why}");
                std::ptr::null_mut()
            }
        }
    })
}

/// Decodes every frame of an animated GIF, PNG (APNG) or WebP, each the
/// whole `*width` by `*height` canvas: `*count` frames back to back,
/// `w * h * 4` bytes each, and `*delays` (inside the same allocation) the
/// seconds each shows. `*loops` is how many times the sequence plays, 0
/// for ever. A still image is one frame shown for ever (an infinite
/// delay). One `kui_pixels_free` frees pixels and delays. NULL, with the
/// reason on stderr and every out 0, for bytes that do not decode.
#[unsafe(no_mangle)]
pub extern "C" fn kui_decode_animation(
    bytes: *const u8,
    len: usize,
    width: *mut u32,
    height: *mut u32,
    count: *mut u32,
    loops: *mut u32,
    delays: *mut *const f64,
) -> *mut u8 {
    guard(std::ptr::null_mut(), || {
        let put = |w: u32, h: u32, n: u32, l: u32, d: *const f64| unsafe {
            for (p, v) in [(width, w), (height, h), (count, n), (loops, l)] {
                if !p.is_null() {
                    *p = v;
                }
            }
            if !delays.is_null() {
                *delays = d;
            }
        };
        put(0, 0, 0, 0, std::ptr::null());
        match kui_native::decode_animation(unsafe { bytes_of(bytes, len) }) {
            Ok(a) => {
                let rgba: Vec<u8> = a
                    .frames
                    .iter()
                    .flat_map(|f| f.rgba.iter().copied())
                    .collect();
                let ds: Vec<f64> = a.frames.iter().map(|f| f.delay).collect();
                let (px, d) = hand_out(&rgba, &ds);
                put(a.width, a.height, ds.len() as u32, a.loops.unwrap_or(0), d);
                px
            }
            Err(why) => {
                eprintln!("kui: kui_decode_animation: {why}");
                std::ptr::null_mut()
            }
        }
    })
}

/// Which of `count` frames with these `delays` (seconds) shows `elapsed`
/// seconds after the animation started, playing `loops` times (0 for
/// ever); `*next`, when not NULL, is the seconds after the start when the
/// next frame is due — what to hand `kui_request_frame_at` with the start
/// added — and `INFINITY` once a finite animation has played out.
#[unsafe(no_mangle)]
pub extern "C" fn kui_animation_at(
    delays: *const f64,
    count: u32,
    loops: u32,
    elapsed: f64,
    next: *mut f64,
) -> u32 {
    guard(0, || {
        let ds: &[f64] = if delays.is_null() || count == 0 {
            &[]
        } else {
            // SAFETY: the caller's promise, `count` delays.
            unsafe { std::slice::from_raw_parts(delays, count as usize) }
        };
        let a = kui_native::Animation {
            width: 0,
            height: 0,
            frames: ds
                .iter()
                .map(|&delay| kui_native::AnimationFrame {
                    rgba: Vec::new(),
                    delay,
                })
                .collect(),
            loops: (loops != 0).then_some(loops),
        };
        let s = a.at(elapsed);
        if !next.is_null() {
            unsafe { *next = s.next };
        }
        s.index as u32
    })
}

/// Frees what `kui_decode_image` or `kui_decode_animation` returned; NULL
/// is nothing. Only those pointers: the length is kept just before them.
#[unsafe(no_mangle)]
pub extern "C" fn kui_pixels_free(pixels: *mut u8) {
    guard((), || {
        if pixels.is_null() {
            return;
        }
        // SAFETY: `pixels` is two words past the start of a `hand_out`
        // block, whose first word is its length in words.
        unsafe {
            let base = pixels.cast::<u64>().sub(2);
            let words = *base as usize;
            drop(Vec::from_raw_parts(base, words, words));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_icon_is_pixels_of_its_size_and_a_16_bit_resource() {
        let px = [7u8; 2 * 3 * 4];
        let ask = |rgba: *const u8, w, h, r| unsafe { icon_ask_of(rgba, w, h, r) };
        assert_eq!(ask(std::ptr::null(), 0, 0, 0), Ok(None), "all zero is none");
        assert_eq!(
            ask(px.as_ptr(), 2, 3, 1),
            Ok(Some(IconAsk {
                pixels: Some((px.to_vec(), 2, 3)),
                resource: Some(1)
            }))
        );
        assert_eq!(
            ask(std::ptr::null(), 0, 0, 65535),
            Ok(Some(IconAsk {
                pixels: None,
                resource: Some(65535)
            }))
        );
        assert!(
            ask(std::ptr::null(), 2, 3, 0)
                .unwrap_err()
                .contains("NULL pixels")
        );
        assert!(
            ask(px.as_ptr(), 0, 3, 0)
                .unwrap_err()
                .contains("at least one pixel")
        );
        assert!(
            ask(px.as_ptr(), 2, 3, 65536)
                .unwrap_err()
                .contains("1 to 65535")
        );
    }
}
