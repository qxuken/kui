//! One window of the runner: the `Pane` — its OS surface, renderer, `Core`
//! and per-window input state — with the env sync that keeps its core's
//! facts current and the two small translations (appearance, cursor icon)
//! a pane makes between winit and the core. Split off `lib.rs` as a pure
//! move; the shell in `lib.rs` owns the list of panes.

use super::*;

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
/// a question of (backlog F39).
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
    };
    // One device per app, so every pane reads the same state; per-frame
    // because the driver opens it off-thread and closes it when idle, and
    // both of those happen between frames.
    pane.core.env.audio = audio;
}

/// What a window's views read as `env.system`: the OS's four settings —
/// the appearance off the window, the other three from `system_env` —
/// with what the app pinned laid over them (`Launcher::system`, backlog
/// F47). Being applied here, in the write that happens before every frame,
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
    /// turned it into a screen position once; ADR 0009 decision 4 needs it
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
    pub(crate) renderer: kui_wgpu::Renderer,
    /// Last title actually set on the window; views declare per frame and
    /// we only touch the window on change.
    pub(crate) applied_title: String,
    /// Whether the window's level is `AlwaysOnTop` as far as this runner
    /// has asked (backlog C30): the frame declares per frame, and
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
    pub(crate) modifiers: ModifiersState,
    /// Time of the last titlebar press, for double-click maximize.
    pub(crate) last_titlebar_press: Option<std::time::Instant>,
    /// Last cursor position (logical px), for multi-click distance checks.
    pub(crate) cursor: Vec2,
    /// The last Force Touch stage this window reported (0 none, 1 a
    /// press, 2 a force click). Kept so only the *edge* into 2 counts:
    /// AppKit reports the whole ramp, many events a press.
    pub(crate) pressure_stage: i64,
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
    /// Tries left at getting this window's *first* frame onto the screen,
    /// and when to make the next one — `None` once a frame has landed. See
    /// the `Skip` arm of [`Shell::redraw`].
    pub(crate) first_frame: Option<(u32, std::time::Instant)>,
    /// Whether the last redraw this pane was asked for was let wait for
    /// the host's answer (`Launcher::deferred_events`). It is what bounds
    /// the wait to a single frame: never two in a row, so no stream of
    /// input and no platform modal loop can stop this window painting.
    pub(crate) deferred_frame: bool,
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
    /// is an announcement said minutes after the thing it describes
    /// (`docs/adr/0008-live-regions-and-announcements.md`, decision 6).
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

    pub(crate) fn kmods(&self) -> KeyMods {
        KeyMods {
            shift: self.modifiers.shift_key(),
            ctrl: self.modifiers.control_key(),
            alt: self.modifiers.alt_key(),
            super_key: self.modifiers.super_key(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pin is laid over the OS's answer field by field: a launcher that
    /// pinned `motion` reads reduced motion whatever the machine says, and
    /// the appearance, accent and locale are still the OS's — so a real
    /// change to those still reaches the view (backlog F47).
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
    /// (backlog C30): a frame that keeps declaring what is applied costs
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
}
