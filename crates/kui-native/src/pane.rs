//! One window of the runner: the `Pane` — its OS surface, renderer, `Core`
//! and per-window input state — with the env sync that keeps its core's
//! facts current and the two small translations (appearance, cursor icon)
//! a pane makes between winit and the core. Split off `lib.rs` as a pure
//! move; the shell in `lib.rs` owns the list of panes.

use super::*;
use winit::keyboard::PhysicalKey;

/// Everything about a pane the driver owns and the app only reads: the
/// display's refresh rate, the four OS settings, whether assistive
/// technology is listening, and what kind of window this is.
///
/// Written here rather than only before a frame because a host that drives
/// its own loop runs its view before the first one — Node's `runWindowed`
/// renders as soon as the window exists — and a view that read these off a
/// core nothing had filled in got the defaults: appearance unknown, no
/// refresh rate, `custom_chrome` false in an app that draws its own
/// titlebar. It is also the frame the user actually sees in an app that
/// only redraws on input, since nothing re-runs a view that nothing asked
/// a question of.
pub(crate) fn sync_env(
    pane: &mut Pane,
    system: &system_env::Queried,
    pinned: SystemEnv,
    audio: AudioEnv,
) {
    let window = &pane.window;
    let chrome = pane.chrome;
    // Per-frame so it self-corrects when the window moves to another
    // monitor.
    pane.core.env.refresh_hz = window
        .current_monitor()
        .and_then(|m| m.refresh_rate_millihertz())
        .map(|mhz| mhz as f32 / 1000.0);
    // All four settings are event-driven rather than re-asked here: the
    // appearance arrives on `ThemeChanged` (and is read once when the
    // window is created), the other three were asked of the OS in `mod
    // system_env`. Unlike the refresh rate above there is nothing for a
    // per-frame read to self-correct against — a window does not drift
    // into another appearance the way it drifts onto another monitor —
    // and taking focus back re-asks both, which covers a change the
    // platform did not report.
    pane.core.env.system = system_reading(
        pane.appearance,
        system,
        pinned,
        // Not a setting but the same shape of fact: whether an
        // accessibility client has asked this window for its tree. The
        // bridge is the one place that knows, and a pane without one (the
        // `accesskit` feature off) cannot tell (backlog F48).
        pane.access
            .as_ref()
            .map_or(Assistive::Unknown, |b| b.assistive()),
    );
    pane.core.env.window = WindowEnv {
        id: pane.id,
        custom_chrome: chrome != Chrome::Native,
        maximized: window.is_maximized(),
        fullscreen: window.fullscreen().is_some(),
        // The runner's own record of the level it set — winit has no
        // getter, so a level the OS dropped afterwards is not seen (backlog
        // AR49) — and false on a platform without a level whatever was
        // asked of it.
        always_on_top: pane.applied_on_top && pane.level_supported,
        native_controls: pane.native_controls,
        backdrop: pane.backdrop,
    };
    // One device per app, so every pane reads the same state; per-frame
    // because the driver opens it off-thread and closes it when idle, and
    // both of those happen between frames.
    pane.core.env.audio = audio;
}

/// What a window's views read as `env.system`: the OS's four settings —
/// the appearance off the window, the other three from `system_env` —
/// with what the app pinned laid over them (`Launcher::system`).
/// Being applied here, in the write that happens before every frame,
/// is what makes the pin hold: a reading pushed into `core.env` from
/// anywhere else would be gone by the next `sync_env`, which is why a
/// window has no `set_env`. The fields the app left unknown are still the
/// OS's, so a real change to one of them still arrives — and the `system`
/// event that reports it carries the pin, since the event is the reading.
pub(crate) fn system_reading(
    appearance: Appearance,
    queried: &system_env::Queried,
    pinned: SystemEnv,
    assistive: Assistive,
) -> SystemEnv {
    pinned.over(SystemEnv {
        appearance,
        accent: queried.accent,
        motion: queried.motion,
        locale: queried.locale,
        assistive,
    })
}

/// A window's OS light/dark setting, as `env.system.appearance`.
///
/// winit answers on macOS and Windows and returns `None` on X11 and on
/// Wayland without an override — and `None` is `Unknown`, which is the
/// reading saying nobody could tell rather than a guessed `Light`.
pub(crate) fn appearance_of(window: &Window) -> Appearance {
    theme_appearance(window.theme())
}

/// The same, from the theme `WindowEvent::ThemeChanged` carries — the
/// event says what it changed to, so nothing has to ask again.
pub(crate) fn theme_appearance(theme: Option<winit::window::Theme>) -> Appearance {
    match theme {
        Some(winit::window::Theme::Light) => Appearance::Light,
        Some(winit::window::Theme::Dark) => Appearance::Dark,
        None => Appearance::Unknown,
    }
}

/// What a frame's `always_on_top` ask does to the window's level: the
/// level to set now, or `None` when the window already has it — so the
/// call reaches the OS once per change and never per frame. `applied` is
/// the runner's record of the level (`Pane::applied_on_top`), updated
/// here. A popup's level is its own: it opened `AlwaysOnTop` by
/// construction and stays there whatever its frame — or its owner's —
/// declares, since an owner pinned above everything with a dropdown
/// lowered under it is the visible defect a naive apply-to-every-window
/// would ship.
pub(crate) fn level_change(
    kind: WindowKind,
    want: bool,
    applied: &mut bool,
) -> Option<winit::window::WindowLevel> {
    if kind == WindowKind::Popup || want == *applied {
        return None;
    }
    *applied = want;
    Some(if want {
        winit::window::WindowLevel::AlwaysOnTop
    } else {
        winit::window::WindowLevel::Normal
    })
}

/// What a frame's `option_as_alt` ask does to the window:
/// the setting to hand winit now, or `None` when the window already has
/// it — so the call reaches the OS once per change and never per frame,
/// the level's rule. `applied` is the runner's record
/// (`Pane::applied_option_as_alt`), updated here. Every window starts at
/// winit's default, which is `None`, so an app that never declares it
/// never makes the call.
pub(crate) fn option_as_alt_change(
    want: kui_core::OptionAsAlt,
    applied: &mut kui_core::OptionAsAlt,
) -> Option<kui_core::OptionAsAlt> {
    if want == *applied {
        return None;
    }
    *applied = want;
    Some(want)
}

/// What a frame's `ime_off` ask does to the window: whether to turn the
/// input method off (`Some(true)`) or back on (`Some(false)`), or `None`
/// when the window already has it — once per change, never per frame, the
/// level's rule. `applied` is the runner's record (`Pane::applied_ime_off`),
/// updated here. Every window opens with its IME allowed, so an app that
/// never declares it never makes the call.
pub(crate) fn ime_off_change(want: bool, applied: &mut bool) -> Option<bool> {
    if want == *applied {
        return None;
    }
    *applied = want;
    Some(want)
}

/// Turns `window`'s input method off or back on. Off, a composition the
/// user had open is dropped first, so the IME's own window closes with
/// it; winit's `Ime::Disabled` then ends it in the view.
pub(crate) fn apply_ime_off(window: &Window, off: bool) {
    #[cfg(target_os = "macos")]
    if off {
        crate::macos_text_input::discard_marked_text(window);
    }
    window.set_ime_allowed(!off);
}

/// The held-key record behind `Pane::ime_off_held`: a press made with the
/// IME off is recorded, its release forgets it, a repeat changes nothing.
/// Returns whether the record emptied — the moment the IME may come back.
pub(crate) fn note_ime_key(
    held: &mut Vec<PhysicalKey>,
    ime_off: bool,
    key: PhysicalKey,
    pressed: bool,
    repeat: bool,
) -> bool {
    if pressed {
        if ime_off && !repeat && !held.contains(&key) {
            held.push(key);
        }
        return false;
    }
    let before = held.len();
    held.retain(|k| *k != key);
    before > 0 && held.is_empty()
}

/// Whether the Option key held for this press is Alt under the window's
/// applied setting: an Option down on a side it covers.
/// winit has already given such a press the layout's unmodified character
/// in place of the composed one; this is what keeps that character from
/// being typed as text too — an Option that is Alt types nothing, as
/// Control types nothing. `held` is which Options are down, left and
/// right, as winit's own reading says (`Pane::alt_held`) — the one its
/// rewrite of the press was decided on.
pub(crate) fn option_is_alt(applied: kui_core::OptionAsAlt, held: (bool, bool)) -> bool {
    use kui_core::KeyLocation::{Left, Right};
    (held.0 && applied.covers(Left)) || (held.1 && applied.covers(Right))
}

/// Whether the window's platform has a level to set: every backend winit
/// 0.30 supports here but Wayland, which the raw handle tells apart from
/// X11 on the one target that builds both.
pub(crate) fn level_supported(window: &Window) -> bool {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    !matches!(
        window.window_handle().map(|h| h.as_raw()),
        Ok(RawWindowHandle::Wayland(_))
    )
}

pub(crate) fn cursor_icon(shape: CursorShape) -> CursorIcon {
    match shape {
        CursorShape::Default => CursorIcon::Default,
        CursorShape::Text => CursorIcon::Text,
        CursorShape::Pointer => CursorIcon::Pointer,
        CursorShape::Grab => CursorIcon::Grab,
        CursorShape::Grabbing => CursorIcon::Grabbing,
        CursorShape::NotAllowed => CursorIcon::NotAllowed,
        CursorShape::EwResize => CursorIcon::EwResize,
        CursorShape::NsResize => CursorIcon::NsResize,
        CursorShape::NwseResize => CursorIcon::NwseResize,
        CursorShape::NeswResize => CursorIcon::NeswResize,
    }
}

/// One window: the OS surface, its renderer, its `Core`, and every piece
/// of input state that belongs to a window rather than to the app — the
/// cursor, the click counter, the caret blink, the resize band, the cursor
/// icon last set, the accessibility adapter. The main window is the pane
/// with `WindowId::MAIN`; the rest are what frames declared.
pub(crate) struct Pane {
    pub(crate) id: WindowId,
    /// What kind of surface this is, from the `Open` that created it. A
    /// [`WindowKind::Popup`] is the one the runner treats differently after
    /// it is open: it takes the owner's keys while it is up, and Escape or
    /// a press anywhere else asks it to go away.
    pub(crate) kind: WindowKind,
    /// The window whose frame declared this one — itself for the main
    /// window, and for a popup the surface its anchor was measured against
    /// and whose keyboard it borrows.
    pub(crate) owner: WindowId,
    /// Whether it took OS focus when it opened. False is the popup default
    /// and what makes the borrowing necessary.
    pub(crate) activates: bool,
    /// Who draws this window's chrome: the launcher's choice for the
    /// app's windows, [`Chrome::Native`] for the devtools' own — which
    /// the core declares and nothing in the app draws a titlebar for.
    pub(crate) chrome: Chrome,
    /// What shows through this window, as the platform gave it
    /// (`backdrop::apply`, backlog F126): what `env.window.backdrop`
    /// reports, and whether a frame is cleared to nothing rather than to
    /// the theme's `bg`. `Opaque` for every window of an app that never
    /// asked, and for a popup and the devtools' window of one that did.
    pub(crate) backdrop: Backdrop,
    /// The wallpaper kui draws as the window's ground, where the backdrop
    /// is kui's own (`mod ground`); `None` everywhere else.
    pub(crate) ground: Option<crate::ground::Ground>,
    /// What keeps the OS's effect alive for as long as the window (a
    /// Linux compositor's blur objects).
    pub(crate) _backdrop_keep: Option<Box<dyn std::any::Any>>,
    /// The keep-out rect `env.window.native_controls` reports: measured
    /// from the window once it exists (`macos_chrome`), on the one
    /// platform whose custom chrome keeps controls of the OS's over the
    /// content. `None` everywhere else, and under native chrome.
    pub(crate) native_controls: Option<Rect>,
    /// The main window only: the minimum inner size last handed to the
    /// OS — the launcher's, plus what a docked devtools pane takes
    /// (`Core::devtools_inset`), re-applied when either changes.
    pub(crate) applied_min: Option<(f64, f64)>,
    /// A popup's anchor as its `Open` carried it, in the **owner's** logical
    /// coordinates: the rect an `onLayout` node reported. `popup_position`
    /// turned it into a screen position once; classifying the release needs it
    /// again, to tell a release on the field that opened the menu from a
    /// release on nothing. Zero-sized for a window that is not a popup, and
    /// so a rect nothing lands in.
    pub(crate) anchor: Rect,
    /// A non-activating popup only: whether the keyboard has already been
    /// asked back for since this window last took it. One request per
    /// acquisition — the ask is advisory, so repeating it every batch until
    /// the platform agrees would be a request storm.
    pub(crate) handed_back: bool,
    /// Whether the OS says *this* window holds the keyboard — what winit
    /// last reported, before the borrowing in [`Shell::settle_focus`] is
    /// applied. `core.env.focused`, which is what a view reads, is derived
    /// from every pane's copy of this and is not always the same answer.
    pub(crate) os_focused: bool,
    /// The OS light/dark setting for *this* window, read once when it was
    /// created and thereafter from `WindowEvent::ThemeChanged`, which
    /// carries the new theme. Per-window because it is: winit reports a
    /// theme per surface, and an app may override one.
    pub(crate) appearance: Appearance,
    pub(crate) core: Core,
    pub(crate) window: Arc<Window>,
    /// What draws into the window — `None` while the device is gone (a
    /// driver update, a GPU reset) and the next frame is to open a new
    /// one (`Shell::reopen_device`).
    pub(crate) renderer: Option<kui_wgpu::Renderer>,
    /// Frames in a row the surface refused for being configured wrong,
    /// each answered by configuring it to the window's size again; past
    /// `SURFACE_TRIES`, the device is reopened instead. Saturating: while
    /// that reopen waits out its second, every frame input asks for is
    /// refused again and counted, and the count has nowhere to go but
    /// back to zero when a frame lands or a new renderer is made.
    pub(crate) surface_tries: u8,
    /// This window's last frame asked for a new device (`Shell::
    /// reopen_owed`) and has not been given a renderer on one since.
    /// Nothing it draws can land until then, so an animation in it does
    /// not ask for frames — the reopen asks when it has made one.
    pub(crate) awaits_device: bool,
    /// Last title actually set on the window; views declare per frame and
    /// we only touch the window on change.
    pub(crate) applied_title: String,
    /// Whether the window's level is `AlwaysOnTop` as far as this runner
    /// has asked: the frame declares per frame, and
    /// [`level_change`] touches the window only when this differs. A popup
    /// opens at that level and stays there — its owner's ask never
    /// reaches it — so it starts true.
    pub(crate) applied_on_top: bool,
    /// Whether the platform has a window level at all. winit documents
    /// `set_window_level` as unsupported on Wayland and every level as "a
    /// hint to the OS"; a runner there still asks, harmlessly, and reports
    /// `env.window.always_on_top` false however often the app asks — which
    /// is the case the report exists for.
    pub(crate) level_supported: bool,
    /// Which Option keys are Alt on this window as far as this runner has
    /// told winit: the frame declares per frame, and
    /// [`option_as_alt_change`] touches the window only when this differs.
    /// Kept on every platform, called on macOS only.
    pub(crate) applied_option_as_alt: kui_core::OptionAsAlt,
    /// Whether this window's input method is off as far as this runner
    /// has told winit; [`ime_off_change`] touches the window only when
    /// the frame's ask differs.
    pub(crate) applied_ime_off: bool,
    /// Keys that went down while this window's IME was off and are still
    /// held. Turning the IME back on under one hands its repeats to an
    /// input method that never saw the press: on a Mac, press-and-hold
    /// opens its accent picker over them while winit still forwards each
    /// one as typed — a modal editor's held `i`, whose first press is the
    /// switch to insert mode, typed `iiii` under the picker. So the IME
    /// stays off until the last of these comes up, and a held key keeps
    /// repeating as it began.
    pub(crate) ime_off_held: Vec<PhysicalKey>,
    /// Whether the key target was last told of a composition still open —
    /// a non-empty preedit since the last commit or empty one — so the
    /// IME going away mid-composition ends it with an empty preedit, and
    /// going away with nothing open says nothing.
    pub(crate) preedit_open: bool,
    pub(crate) modifiers: ModifiersState,
    /// The modifier keys down in this window, by side — what a modifier
    /// key's release reads its own bit from while its twin is still held
    /// (`Pane::modifier_key`).
    pub(crate) modifier_keys_down: Vec<crate::keys::HeldModifier>,
    /// Which Option keys are down, left and right, from winit's
    /// `ModifiersChanged` — the device-dependent flags of the event on
    /// macOS, which winit rewrites a press under Option-as-Alt by, and
    /// which it sends before the first key after the window comes back
    /// with an Option already held. Mirrored to a popup
    /// borrowing the keyboard as `modifiers` is.
    pub(crate) alt_held: (bool, bool),
    /// Time of the last titlebar press, for double-click maximize. Not on
    /// a Mac, where the press's own click count says.
    #[cfg(not(target_os = "macos"))]
    pub(crate) last_titlebar_press: Option<std::time::Instant>,
    /// Last cursor position (logical px), for multi-click distance checks.
    pub(crate) cursor: Vec2,
    /// The last Force Touch stage this window reported (0 none, 1 a
    /// press, 2 a force click). Kept so only the *edge* into 2 counts:
    /// AppKit reports the whole ramp, many events a press.
    pub(crate) pressure_stage: i64,
    /// The trackpad swipe under way and the axis it keeps to (`mod
    /// axis_lock`).
    pub(crate) axis_lock: crate::axis_lock::AxisLock,
    /// Where the scroll gesture under way began, for the core's latch
    /// (`mod scroll_gesture`).
    pub(crate) scroll_gesture: crate::scroll_gesture::Gesture,
    /// The paths winit's per-file drag events built this batch, and
    /// whether the batch was a drop (`Some(true)`), a hover
    /// (`Some(false)`) or nothing (`None`) — dispatched at the batch's
    /// end. Unused where the platform override answers.
    pub(crate) file_drag: Vec<String>,
    pub(crate) file_drag_pending: Option<bool>,
    /// Last primary press: time, position, and its click count.
    pub(crate) last_click: Option<(std::time::Instant, Vec2, u8)>,
    /// Caret blink phase mirror + next toggle time; the clock lives here,
    /// the core only stores the visible flag.
    pub(crate) blink_visible: bool,
    pub(crate) blink_deadline: Option<std::time::Instant>,
    pub(crate) caret_stamp_seen: u64,
    /// Resize edge currently under the cursor (undecorated windows only).
    pub(crate) resize_edge: Option<ResizeDirection>,
    /// Cursor icon last set on the window, so a shape that did not change
    /// costs nothing.
    pub(crate) cursor_icon: CursorIcon,
    /// A frame the surface skipped, owed until one lands (`mod retry`):
    /// the window's first, and any after it that an occluded or timed-out
    /// acquire dropped. See the `Skip` arm of [`Shell::redraw`].
    pub(crate) retry: crate::retry::Retry,
    /// Whether the last redraw this pane was asked for was let wait for
    /// the host's answer (`Launcher::deferred_events`). It is what bounds
    /// the wait to a single frame: never two in a row, so no stream of
    /// input and no platform modal loop can stop this window painting.
    pub(crate) deferred_frame: bool,
    /// Why the runner asked this window for a frame since its last one
    /// began: handed to the core before the view runs,
    /// where it joins the input the core recorded itself. A `Cell`, since
    /// most of the places that ask hold the pane shared.
    pub(crate) cause: Causes,
    /// Whether a redraw draws now or waits for the display (`mod pacer`).
    pub(crate) pacer: crate::pacer::Pacer,
    /// The platform accessibility bridge.
    pub(crate) access: Option<access_bridge::Bridge>,
    /// Windows: the timer that keeps an animation running while the modal
    /// move/resize loop owns the message pump (`mod windows_anim`).
    #[cfg(target_os = "windows")]
    pub(crate) anim_timer: Option<windows_anim::AnimTimer>,
    /// Windows: answers WM_NCHITTEST from the frame's chrome regions, which
    /// enables snap layouts + native caption behavior over drawn controls.
    #[cfg(target_os = "windows")]
    pub(crate) nc: Option<windows_nc::NcHitTest>,
}

impl Pane {
    /// Whether this window's IME should be off now: the frame asked, or a
    /// key pressed while it was off is still down (`ime_off_held`).
    pub(crate) fn ime_off_wanted(&self) -> bool {
        self.core.ime_off() || !self.ime_off_held.is_empty()
    }

    /// Applies [`Self::ime_off_wanted`] to the window if it changed —
    /// for the moments between frames that can change it: the last held
    /// key coming up, the keyboard going away.
    pub(crate) fn sync_ime(&mut self) {
        if let Some(off) = ime_off_change(self.ime_off_wanted(), &mut self.applied_ime_off) {
            apply_ime_off(&self.window, off);
        }
    }

    /// Records a key event against [`Self::ime_off_held`], and gives the
    /// IME back when the last key held from under it comes up and the
    /// frame no longer asks for it off.
    pub(crate) fn note_ime_key(&mut self, key: PhysicalKey, pressed: bool, repeat: bool) {
        if note_ime_key(
            &mut self.ime_off_held,
            self.applied_ime_off,
            key,
            pressed,
            repeat,
        ) {
            self.sync_ime();
        }
    }

    /// Whether the window is minimized, on Windows: an animation there
    /// asks for no frames, where it built every one at the display's rate
    /// into a surface nobody could see (RG45). Restoring the window is a
    /// `WM_SIZE`, whose `Resized` asks for the frame that picks the
    /// animation up where its clock has got to. False elsewhere: macOS
    /// says a miniaturized window is covered (`Occluded`), and
    /// [`Self::animates_now`] reads that.
    pub(crate) fn minimized(&self) -> bool {
        cfg!(target_os = "windows") && self.window.is_minimized() == Some(true)
    }

    /// Whether the window's animation asks for its next frame: it is
    /// animating, and where it could be seen — not waiting for a device
    /// (RG40), not minimized on Windows (RG45), and not dark, covered or
    /// minimized where the platform says so (`Occluded`, macOS and X11).
    /// A dark window's every frame is skipped by its surface, and nothing
    /// presented meant nothing paced the next: macOS drew 2 800 skipped
    /// frames a second from the minimize until it stopped delivering
    /// redraws. The frame that brings the window back
    /// ([`Self::came_back`]) picks the animation up where its clock has
    /// got to.
    pub(crate) fn animates_now(&self) -> bool {
        self.core.animating() && !self.awaits_device && !self.minimized() && !self.cause.is_dark()
    }

    /// Asks for a frame, and says why: `why` is among the
    /// reasons the frame is handed (`Core::frame_cause`).
    pub(crate) fn redraw_for(&self, why: FrameCause) {
        self.cause.note(why);
        self.window.request_redraw();
    }

    /// The window is back where it draws, from where it was dark
    /// ([`Causes::came_back`]): asks for the frame that says `why`.
    pub(crate) fn came_back(&self, why: FrameCause) {
        self.cause.came_back(why);
        self.window.request_redraw();
    }

    /// Whether a redraw draws now or waits for the display (`mod pacer`).
    pub(crate) fn admit_frame(&mut self) -> bool {
        let px = self.window.inner_size();
        self.pacer
            .admit(std::time::Instant::now(), (px.width, px.height))
    }

    /// Inner size in logical px plus the scale factor.
    pub(crate) fn size(&self) -> (Size, f32) {
        let scale = self.window.scale_factor() as f32;
        let size = self.window.inner_size();
        (
            Size::new(size.width as f32 / scale, size.height as f32 / scale),
            scale,
        )
    }

    /// This window as retargeting sees it (`retarget::Surface`): where its
    /// client area sits on the screen in **physical** pixels, its scale, and
    /// its logical size. `None` when the platform will not say where the
    /// window is — the same thing `popup_position` gives up on, and the same
    /// answer: leave the pointer where it was.
    pub(crate) fn surface(&self) -> Option<retarget::Surface> {
        let origin = self.window.inner_position().ok()?;
        let (size, scale) = self.size();
        // The common frame (`retarget`'s module doc): physical pixels
        // where the platform positions windows in them, points on macOS,
        // where `inner_position` is points times this window's own scale
        // and two windows on displays of different scale would otherwise
        // be in two frames (backlog AR33).
        if cfg!(target_os = "macos") {
            let origin = origin.to_logical::<f64>(scale as f64);
            return Some(retarget::Surface {
                origin: (origin.x, origin.y),
                scale: 1.0,
                size,
            });
        }
        Some(retarget::Surface {
            origin: (origin.x as f64, origin.y as f64),
            scale: scale as f64,
            size,
        })
    }

    /// Undecorated windows get no OS resize borders; the runner synthesizes
    /// them from a band inside the window edges (macOS custom chrome keeps
    /// native edge resizing, and on Windows the non-client subclass answers
    /// WM_NCHITTEST with real border codes instead, so neither synthesizes).
    pub(crate) fn synthesizes_resize(&self) -> bool {
        #[cfg(target_os = "windows")]
        if self.nc.is_some() {
            return false;
        }
        // A popup is not resizable at all (backlog AR32).
        self.chrome != Chrome::Native
            && self.kind != WindowKind::Popup
            && !cfg!(target_os = "macos")
    }

    pub(crate) fn resize_edge_at(&self, p: Vec2) -> Option<ResizeDirection> {
        let w = &self.window;
        if w.is_maximized() || w.fullscreen().is_some() {
            return None;
        }
        let (size, _) = self.size();
        let (sw, sh) = (size.w, size.h);
        let (l, r) = (p.x < RESIZE_BAND, p.x > sw - RESIZE_BAND);
        let (t, b) = (p.y < RESIZE_BAND, p.y > sh - RESIZE_BAND);
        Some(match (l, r, t, b) {
            (true, _, true, _) => ResizeDirection::NorthWest,
            (_, true, true, _) => ResizeDirection::NorthEast,
            (true, _, _, true) => ResizeDirection::SouthWest,
            (_, true, _, true) => ResizeDirection::SouthEast,
            (true, ..) => ResizeDirection::West,
            (_, true, ..) => ResizeDirection::East,
            (_, _, true, _) => ResizeDirection::North,
            (_, _, _, true) => ResizeDirection::South,
            _ => return None,
        })
    }

    /// Applies the pointer shape the core derived for this frame, with the
    /// synthesized resize band on top: the band is the runner's own edge,
    /// invisible to the core, and a press there resizes the window rather
    /// than reaching the UI, so what it says wins. Only touches the window
    /// when the answer changes.
    pub(crate) fn apply_cursor(&mut self) {
        let icon = match self.resize_edge {
            Some(ResizeDirection::West | ResizeDirection::East) => CursorIcon::EwResize,
            Some(ResizeDirection::North | ResizeDirection::South) => CursorIcon::NsResize,
            Some(ResizeDirection::NorthWest | ResizeDirection::SouthEast) => CursorIcon::NwseResize,
            Some(ResizeDirection::NorthEast | ResizeDirection::SouthWest) => CursorIcon::NeswResize,
            None => cursor_icon(self.core.cursor_shape()),
        };
        if icon == self.cursor_icon {
            return;
        }
        self.cursor_icon = icon;
        self.window.set_cursor(icon);
    }

    /// Hands the frame's access tree to the platform when assistive
    /// technology is attached and the tree changed, and whatever the frame
    /// asked to say (`ui.announce`) with it.
    ///
    /// The queue is drained **every frame, attached or not**, and what a
    /// silent window cannot deliver is dropped here: an announcement kept
    /// is an announcement said minutes after the thing it describes.
    pub(crate) fn publish_access(&mut self) {
        let said = self.core.take_announcements();
        let Some(bridge) = self.access.as_mut() else {
            return;
        };
        if !bridge.active() {
            return;
        }
        let scale = self.window.scale_factor() as f32;
        bridge.publish(self.core.access_tree(), scale, &said);
    }

    /// The platform primary shortcut modifier (Cmd on macOS, Ctrl elsewhere).
    pub(crate) fn primary(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.modifiers.super_key()
        } else {
            self.modifiers.control_key()
        }
    }

    /// `mods` for a key event, with a modifier key's own bit set to
    /// the state after it (`keys::modifier_after`).
    pub(crate) fn modifier_key(
        &mut self,
        code: kui_core::KeyCode,
        at: kui_core::KeyCode,
        location: kui_core::KeyLocation,
        pressed: bool,
        mods: KeyMods,
    ) -> KeyMods {
        crate::keys::modifier_after(
            &mut self.modifier_keys_down,
            code,
            at,
            location,
            pressed,
            mods,
        )
    }

    pub(crate) fn kmods(&self) -> KeyMods {
        KeyMods {
            shift: self.modifiers.shift_key(),
            ctrl: self.modifiers.control_key(),
            alt: self.modifiers.alt_key(),
            super_key: self.modifiers.super_key(),
        }
    }
}

/// Why the runner asked a window for a frame since its last one began
///, and what of that it asked while the window could not
/// draw. `Cell`s, since most of the places that ask hold
/// the pane shared.
#[derive(Default)]
pub(crate) struct Causes {
    noted: std::cell::Cell<FrameCause>,
    /// Between [`Self::went_dark`] and [`Self::came_back`]: the window is
    /// covered, minimized or hidden.
    dark: std::cell::Cell<bool>,
    /// What was noted while dark that no frame has taken. The platform
    /// delivers no redraw to a minimized window, so these are for frames
    /// that never came.
    noted_dark: std::cell::Cell<FrameCause>,
}

impl Causes {
    pub(crate) fn new(first: FrameCause) -> Causes {
        let c = Causes::default();
        c.noted.set(first);
        c
    }

    /// `why` is among the reasons the next frame is handed.
    pub(crate) fn note(&self, why: FrameCause) {
        self.noted.set(self.noted.get() | why);
        if self.dark.get() {
            self.noted_dark.set(self.noted_dark.get() | why);
        }
    }

    /// The reasons for the frame beginning now, and none left over.
    pub(crate) fn take(&self) -> FrameCause {
        self.noted_dark.take();
        self.noted.take()
    }

    /// The window went where it may not draw: covered, minimized or
    /// hidden.
    pub(crate) fn went_dark(&self) {
        self.dark.set(true);
    }

    pub(crate) fn is_dark(&self) -> bool {
        self.dark.get()
    }

    /// The window is back where it draws, and the next frame says `why`.
    /// What was noted while it was dark and no frame took is left out: a
    /// minimized window noted `elsewhere`, `wake` or `appearance` for
    /// frames that never came, and the restore frame reported them from
    /// long before. A frame that ran while it was covered — macOS builds
    /// them through the minimize animation, and the surface skips — took
    /// what was noted before it, as any frame does, and what was noted
    /// before the window went dark stays.
    pub(crate) fn came_back(&self, why: FrameCause) {
        let stale = self.noted_dark.take().bits();
        self.dark.set(false);
        self.noted
            .set(FrameCause::from_bits(self.noted.get().bits() & !stale) | why);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frame that brings a window back names the return, not what was
    /// asked while it could not draw; a frame that ran in
    /// the dark took what came before it, and what was asked before the
    /// window went dark stays asked.
    #[test]
    fn the_frame_back_from_the_dark_names_the_return() {
        let c = Causes::new(FrameCause::FIRST);
        assert_eq!(c.take(), FrameCause::FIRST);
        // Minimized with a key unanswered; another window's input and the
        // appearance reach it there, and no frame comes.
        c.note(FrameCause::KEY);
        c.went_dark();
        c.note(FrameCause::ELSEWHERE);
        c.note(FrameCause::APPEARANCE);
        c.came_back(FrameCause::OCCLUSION);
        assert_eq!(c.take(), FrameCause::KEY | FrameCause::OCCLUSION);
        // A frame through the minimize animation takes what was noted
        // before it; only what came after is left out.
        c.went_dark();
        c.note(FrameCause::WAKE);
        assert_eq!(c.take(), FrameCause::WAKE);
        c.note(FrameCause::ELSEWHERE);
        c.came_back(FrameCause::OCCLUSION);
        assert_eq!(c.take(), FrameCause::OCCLUSION);
        // Back in the light, a reason is kept as before.
        assert!(!c.is_dark());
        c.note(FrameCause::HOST);
        c.note(FrameCause::HOST);
        assert_eq!(c.take(), FrameCause::HOST);
    }

    /// The pin is laid over the OS's answer field by field: a launcher that
    /// pinned `motion` reads reduced motion whatever the machine says, and
    /// the appearance, accent and locale are still the OS's — so a real
    /// change to those still reaches the view.
    #[test]
    fn a_pinned_field_beats_the_query_and_the_rest_stay_the_oss() {
        let queried = system_env::Queried {
            accent: Some(Color::hex(0x0a84ffff)),
            motion: MotionPref::Full,
            locale: Locale::new("en-US"),
        };
        let none = SystemEnv::default();
        // Nothing pinned: the reading is the query plus the window's
        // appearance, as before the pin existed.
        assert_eq!(
            system_reading(Appearance::Light, &queried, none, Assistive::Unknown),
            SystemEnv {
                appearance: Appearance::Light,
                accent: queried.accent,
                motion: MotionPref::Full,
                locale: queried.locale,
                assistive: Assistive::Unknown,
            }
        );
        // The bridge's reading rides the query, and the pin cannot say
        // anything about it: a pin on `assistive` is meaningless and stays
        // Unknown, so the bridge's answer is what the view reads (F48
        // over F47).
        assert_eq!(
            system_reading(Appearance::Light, &queried, none, Assistive::Listening).assistive,
            Assistive::Listening
        );
        let less_motion = SystemEnv {
            motion: MotionPref::Reduced,
            ..Default::default()
        };
        let read = system_reading(Appearance::Light, &queried, less_motion, Assistive::Unknown);
        assert_eq!(
            read.motion,
            MotionPref::Reduced,
            "the pin wins over a real Full"
        );
        assert_eq!(read.appearance, Appearance::Light);
        assert_eq!(read.accent, queried.accent);
        assert_eq!(read.locale, queried.locale);
        // The OS moved on an unpinned field: the reading follows it, and
        // the pin is still there.
        let dark = system_reading(Appearance::Dark, &queried, less_motion, Assistive::Unknown);
        assert_eq!(dark.appearance, Appearance::Dark);
        assert_eq!(dark.motion, MotionPref::Reduced);
        // A pin on a field the platform cannot answer is the reading.
        let mute = system_env::Queried::default();
        assert_eq!(
            system_reading(Appearance::Unknown, &mute, less_motion, Assistive::Unknown).motion,
            MotionPref::Reduced
        );
    }

    /// The level reaches the OS once per change and never per frame
    ///: a frame that keeps declaring what is applied costs
    /// nothing, a frame that stops declaring lowers the window, and a
    /// popup's level is never touched whatever its frame says.
    #[test]
    fn a_level_is_applied_on_change_and_never_for_a_popup() {
        use winit::window::WindowLevel;
        let mut applied = false;
        assert_eq!(
            level_change(WindowKind::Normal, true, &mut applied),
            Some(WindowLevel::AlwaysOnTop)
        );
        assert!(applied);
        // The next frames declare the same thing: no call.
        assert_eq!(level_change(WindowKind::Normal, true, &mut applied), None);
        assert_eq!(level_change(WindowKind::Normal, true, &mut applied), None);
        // A frame that stops declaring it is the lowering.
        assert_eq!(
            level_change(WindowKind::Normal, false, &mut applied),
            Some(WindowLevel::Normal)
        );
        assert!(!applied);
        assert_eq!(level_change(WindowKind::Normal, false, &mut applied), None);

        // A popup opened on top and stays there: its record is untouched.
        let mut popup = true;
        assert_eq!(level_change(WindowKind::Popup, false, &mut popup), None);
        assert_eq!(level_change(WindowKind::Popup, true, &mut popup), None);
        assert!(popup);
    }

    /// Option as Alt reaches winit once per change and never per frame
    ///: a window starts at winit's default and an app that
    /// never declares it never makes the call; a frame that keeps
    /// declaring what is applied costs nothing; a frame that stops
    /// declaring it gives the Option keys back.
    #[test]
    fn option_as_alt_is_applied_on_change() {
        use kui_core::OptionAsAlt::{Both, Left, None as Off};
        let mut applied = Off;
        assert_eq!(option_as_alt_change(Off, &mut applied), None);
        assert_eq!(option_as_alt_change(Left, &mut applied), Some(Left));
        assert_eq!(applied, Left);
        assert_eq!(option_as_alt_change(Left, &mut applied), None);
        assert_eq!(option_as_alt_change(Left, &mut applied), None);
        assert_eq!(option_as_alt_change(Both, &mut applied), Some(Both));
        assert_eq!(option_as_alt_change(Off, &mut applied), Some(Off));
        assert_eq!(applied, Off);
        assert_eq!(option_as_alt_change(Off, &mut applied), None);
    }

    /// The input method reaches winit once per change and never per
    /// frame: a window opens with it allowed and an app that never asks
    /// never makes the call; a mode that keeps asking costs nothing; the
    /// frame that stops asking turns it back on.
    #[test]
    fn ime_off_is_applied_on_change() {
        let mut applied = false;
        assert_eq!(ime_off_change(false, &mut applied), None);
        assert_eq!(ime_off_change(true, &mut applied), Some(true));
        assert!(applied);
        assert_eq!(ime_off_change(true, &mut applied), None);
        assert_eq!(ime_off_change(true, &mut applied), None);
        assert_eq!(ime_off_change(false, &mut applied), Some(false));
        assert!(!applied);
        assert_eq!(ime_off_change(false, &mut applied), None);
    }

    /// A key that went down with the IME off holds it off until it comes
    /// up: recorded on its press, kept through its repeats, forgotten on
    /// its release — and the release of the last one is the moment the
    /// IME may come back. A press made with the IME on is never recorded.
    #[test]
    fn a_key_pressed_with_the_ime_off_holds_it_off_until_released() {
        use winit::keyboard::{KeyCode, PhysicalKey::Code};
        let mut held = Vec::new();
        assert!(!note_ime_key(
            &mut held,
            true,
            Code(KeyCode::KeyI),
            true,
            false
        ));
        assert_eq!(held, [Code(KeyCode::KeyI)]);
        assert!(!note_ime_key(
            &mut held,
            false,
            Code(KeyCode::KeyI),
            true,
            true
        ));
        assert!(!note_ime_key(
            &mut held,
            true,
            Code(KeyCode::ShiftLeft),
            true,
            false
        ));
        assert!(!note_ime_key(
            &mut held,
            true,
            Code(KeyCode::ShiftLeft),
            false,
            false
        ));
        assert_eq!(
            held,
            [Code(KeyCode::KeyI)],
            "a repeat records nothing twice"
        );
        assert!(note_ime_key(
            &mut held,
            false,
            Code(KeyCode::KeyI),
            false,
            false
        ));
        assert!(held.is_empty());
        // With the IME on, a press is the IME's own business.
        assert!(!note_ime_key(
            &mut held,
            false,
            Code(KeyCode::KeyE),
            true,
            false
        ));
        assert!(!note_ime_key(
            &mut held,
            false,
            Code(KeyCode::KeyE),
            false,
            false
        ));
        assert!(held.is_empty());
    }

    /// Which held Option makes a press Alt: the side the
    /// setting names, both under `Both`, neither under `None` — so the
    /// right Option still types `ü`'s accent while the left one is Alt.
    #[test]
    fn only_a_covered_option_is_alt() {
        use kui_core::OptionAsAlt;
        let (left, right, neither) = ((true, false), (false, true), (false, false));
        assert!(option_is_alt(OptionAsAlt::Left, left));
        assert!(!option_is_alt(OptionAsAlt::Left, right));
        assert!(option_is_alt(OptionAsAlt::Right, right));
        assert!(option_is_alt(OptionAsAlt::Both, left));
        assert!(option_is_alt(OptionAsAlt::Both, right));
        assert!(!option_is_alt(OptionAsAlt::None, left));
        assert!(!option_is_alt(OptionAsAlt::None, (true, true)));
        assert!(!option_is_alt(OptionAsAlt::Both, neither));
    }
}
