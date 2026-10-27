//! A context menu in a popup window of its own (backlog F210, ADR 0004
//! amended 2026-10-27).
//!
//! The core draws its context menu in the window, as a float that `fit`
//! keeps there — and a float cannot leave the window: a menu opened near
//! its edge is pushed back in over the point it opened at, and one taller
//! than the window is cut to it and scrolls (`layout::fit_menu_panel`).
//! Where the platform lets a borderless window be placed anywhere on the
//! display — Windows, macOS and X11 — the runner shows such a menu the way
//! the platform's own would be: in a non-activating popup window, past the
//! window's edges, held to the display's work area rather than to the
//! window.
//!
//! **Only a menu that does not fit.** A menu that shows whole at its
//! point stays a float, as ADR 0004 decision 11 keeps every float that
//! fits: it costs a frame where a popup costs a window, a swapchain, a
//! core and an access adapter, and it opens on the frame of the
//! right-click where a window takes the OS a moment to show. Whether it
//! fits is measured before anything is drawn — the popup's core lays the
//! menu out once at the work area's size (`measure`) — so a menu that
//! goes to a popup never shows in the window first.
//!
//! The menu stays the window's core's ([`Core::set_menu_in_popup`]): the
//! popup's core draws its rows with the same widget and says back what was
//! chosen ([`Core::take_menu_popup_answer`]), which the runner hands to the
//! window's core through `activate_menu_path`, as it does a macOS menu's.
//! A submenu is a further popup beside its row, owned by the popup it hangs
//! off, so the keyboard reaches the deepest one through `key_target` and a
//! press in the menu's own chain is not a press outside it.
//!
//! **Wayland** cannot place a top-level window, and winit 0.30 has no
//! `xdg_popup`, so there a menu stays in the window, cut and scrolling;
//! [`places_popups`] says which.

use super::*;

/// A pane that is a menu's popup (`Pane::menu_pane`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MenuPane {
    /// The window whose core holds the menu, and performs the row chosen.
    pub host: WindowId,
    /// The submenu this popup shows, as a path into the menu's rows:
    /// empty for the menu itself.
    pub path: Vec<usize>,
}

/// A rect in the display's frame — physical px on Windows and X11, points
/// on macOS (`retarget`'s module doc) — in `f64`, as winit positions windows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Area {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Whether a menu of `size` opened at `at` shows whole in a window of
/// `viewport` (all logical px): `fit` does not move it and layout does not
/// cut it. What stays in the window.
pub(crate) fn fits(at: Vec2, size: Size, viewport: Size) -> bool {
    let edge = widgets::MENU_EDGE;
    at.x >= 0.0
        && at.y >= 0.0
        && at.x + size.w <= viewport.w
        && at.y + size.h <= viewport.h
        && size.w <= viewport.w - 2.0 * edge
        && size.h < viewport.h - 2.0 * edge
}

/// Where a menu's popup of `size` opened at the point `at` goes, kept in
/// `work` less `edge` (all in the display's frame): its top-left at the
/// point; flipped to end at the point where it would run past the right
/// or the bottom and there is room the other way, as Windows places a
/// context menu; else pushed in against the far edge, which for a menu as
/// tall as the work area is its top — Part one's rule, against the display.
pub(crate) fn place_menu(at: (f64, f64), size: (f64, f64), work: Area, edge: f64) -> (f64, f64) {
    let (left, right) = (work.x + edge, work.x + work.w - edge);
    let (top, bottom) = (work.y + edge, work.y + work.h - edge);
    (
        flip_or_push(at.0, at.0, size.0, left, right),
        flip_or_push(at.1, at.1, size.1, top, bottom),
    )
}

/// Where a submenu's popup of `size` goes beside its `row` (the display's
/// frame): `pad` past the row's right edge with its first row level with
/// it, as in the window, or ending `pad` before the row's left edge where
/// the room on the right is short; pushed up where it would run past the
/// bottom, as both platforms do a submenu, and never above the top.
pub(crate) fn place_submenu(
    row: Area,
    size: (f64, f64),
    work: Area,
    edge: f64,
    pad: f64,
) -> (f64, f64) {
    let (left, right) = (work.x + edge, work.x + work.w - edge);
    let (top, bottom) = (work.y + edge, work.y + work.h - edge);
    let x = flip_or_push(row.x + row.w + pad, row.x - pad, size.0, left, right);
    let y = (row.y - pad).min(bottom - size.1).max(top);
    (x, y)
}

/// One axis: starting at `after` while that ends by `hi`; else ending at
/// `before` while that starts by `lo`; else ending at `hi`, and never
/// starting before `lo`.
fn flip_or_push(after: f64, before: f64, len: f64, lo: f64, hi: f64) -> f64 {
    if after + len <= hi {
        after.max(lo)
    } else if before - len >= lo {
        before - len
    } else {
        (hi - len).max(lo)
    }
}

/// Whether `window`'s platform lets the runner place a popup where it
/// likes: everywhere but Wayland, where a top-level window's position is
/// the compositor's and only an `xdg_popup`, which winit does not make,
/// is placed against its parent.
pub(crate) fn places_popups(window: &Window) -> bool {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    !matches!(
        window.window_handle().map(|h| h.as_raw()),
        Ok(RawWindowHandle::Wayland(_))
    )
}

/// The display's work area around the point `p` (the display's frame):
/// the monitor less the taskbar on Windows, less the menu bar and the
/// Dock on macOS, and the monitor itself on X11, which says nothing of
/// panels. `None` when the platform will not tell.
fn work_area(window: &Window, p: (f64, f64)) -> Option<Area> {
    #[cfg(target_os = "windows")]
    {
        let _ = window;
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
        };
        let at = POINT {
            x: p.0.round() as i32,
            y: p.1.round() as i32,
        };
        // SAFETY: plain Win32 calls; the struct is sized as the API asks.
        let monitor = unsafe { MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST) };
        let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
            return None;
        }
        let r = info.rcWork;
        Some(Area {
            x: r.left as f64,
            y: r.top as f64,
            w: (r.right - r.left) as f64,
            h: (r.bottom - r.top) as f64,
        })
    }
    #[cfg(target_os = "macos")]
    {
        let _ = p;
        let m = window.current_monitor()?;
        let s = m.scale_factor();
        let (pos, size) = (
            m.position().to_logical::<f64>(s),
            m.size().to_logical::<f64>(s),
        );
        let mut a = Area {
            x: pos.x,
            y: pos.y,
            w: size.width,
            h: size.height,
        };
        // AppKit's frames are the other way up, but the insets are not:
        // the menu bar is what the visible frame leaves at the top, the
        // Dock what it leaves at the bottom or a side.
        if let Some(screen) = crate::macos_chrome::ns_window(window).and_then(|w| w.screen()) {
            let (f, v) = (screen.frame(), screen.visibleFrame());
            let top = (f.origin.y + f.size.height) - (v.origin.y + v.size.height);
            let bottom = v.origin.y - f.origin.y;
            let left = v.origin.x - f.origin.x;
            let right = (f.origin.x + f.size.width) - (v.origin.x + v.size.width);
            a = Area {
                x: a.x + left,
                y: a.y + top,
                w: a.w - left - right,
                h: a.h - top - bottom,
            };
        }
        Some(a)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = p;
        let m = window.current_monitor()?;
        let (pos, size) = (m.position(), m.size());
        Some(Area {
            x: pos.x as f64,
            y: pos.y as f64,
            w: size.width as f64,
            h: size.height as f64,
        })
    }
}

/// Shows a popup at once when it is shown, as a menu appears: Windows
/// fades a newly shown window in, which on a menu read as the menu being
/// slow to open (the user, trying F210). Set before the window is shown;
/// nothing elsewhere, where a borderless window appears as it is.
pub(crate) fn appear_at_once(window: &Window) {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::{BOOL, HWND, TRUE};
        use windows_sys::Win32::Graphics::Dwm::{
            DWMWA_TRANSITIONS_FORCEDISABLED, DwmSetWindowAttribute,
        };
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(h) = handle.as_raw() else {
            return;
        };
        let off: BOOL = TRUE;
        // SAFETY: a live window of this thread, and a BOOL the size the
        // attribute takes.
        unsafe {
            DwmSetWindowAttribute(
                h.hwnd.get() as HWND,
                DWMWA_TRANSITIONS_FORCEDISABLED as u32,
                (&off as *const BOOL).cast(),
                std::mem::size_of::<BOOL>() as u32,
            );
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
}

/// Hides a window from the screen while it stays shown to everything else
/// (DWM's cloak), or shows it again. A popup is cloaked from creation to
/// its first frame presented (`Pane::cloaked`): shown at once, with no
/// fade, Windows drew its empty surface white for the frames before kui's
/// first one landed (the user, trying F210). Answers whether the platform
/// cloaked it; nothing elsewhere.
pub(crate) fn cloak(window: &Window, on: bool) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::{BOOL, FALSE, HWND, TRUE};
        use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute};
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Ok(handle) = window.window_handle() else {
            return false;
        };
        let RawWindowHandle::Win32(h) = handle.as_raw() else {
            return false;
        };
        let value: BOOL = if on { TRUE } else { FALSE };
        // SAFETY: a live window of this thread, and a BOOL the size the
        // attribute takes.
        let hr = unsafe {
            DwmSetWindowAttribute(
                h.hwnd.get() as HWND,
                DWMWA_CLOAK as u32,
                (&value as *const BOOL).cast(),
                std::mem::size_of::<BOOL>() as u32,
            )
        };
        hr == 0
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, on);
        false
    }
}

/// Lays a menu's popup core out once at `room` less the margin above and
/// below — the room Part one's cut is against on the display — and answers
/// the size its panel took: the window to open.
fn measure(core: &mut Core, room: Size, scale: f32) -> Option<Size> {
    let edge = widgets::MENU_EDGE;
    let ui = core.frame(Size::new(room.w, (room.h - 2.0 * edge).max(0.0)), scale);
    ui.finish();
    core.menu_popup_size()
}

impl DynShell<'_> {
    /// Keeps the menus' popups with the menus: what a popup said (a row,
    /// Escape, the Left arrow) taken and done, the popups of a menu that
    /// closed or changed closed, a menu just opened that does not fit its
    /// window put in one, each popup's open submenu given its own, and the
    /// window's look carried into them. Run after each input, each frame
    /// and each turn of the loop; a turn with no menu open costs a walk of
    /// the panes.
    pub(super) fn pump_menu_popups(&mut self, event_loop: &ActiveEventLoop) {
        self.take_menu_answers(event_loop);
        self.drop_stale_menu_popups();
        self.open_menu_popups(event_loop);
        self.sync_submenu_popups(event_loop);
        self.sync_menu_look();
    }

    /// The menu pane of `host`'s menu showing `path`, if one is open.
    fn menu_pane_at(&self, host: WindowId, path: &[usize]) -> Option<usize> {
        self.panes.iter().position(|p| {
            p.menu_pane
                .as_ref()
                .is_some_and(|m| m.host == host && m.path == path)
        })
    }

    /// What each popup said since the last pump.
    fn take_menu_answers(&mut self, event_loop: &ActiveEventLoop) {
        let mut i = 0;
        while i < self.panes.len() {
            let Some(mp) = self.panes[i].menu_pane.clone() else {
                i += 1;
                continue;
            };
            let Some(answer) = self.panes[i].core.take_menu_popup_answer() else {
                i += 1;
                continue;
            };
            match answer {
                MenuPopupAnswer::Chose(row) => {
                    let path: Vec<usize> = mp.path.iter().copied().chain(row).collect();
                    self.close_menu_popups(mp.host, 0);
                    self.choose_from_popup(event_loop, mp.host, &path);
                }
                MenuPopupAnswer::Back if !mp.path.is_empty() => {
                    self.close_menu_popups(mp.host, mp.path.len());
                    if let Some(j) = self.menu_pane_at(mp.host, &mp.path[..mp.path.len() - 1]) {
                        self.panes[j].core.close_menu_popup_submenu();
                        self.panes[j].redraw_for(FrameCause::MENU);
                    }
                }
                MenuPopupAnswer::Back | MenuPopupAnswer::Dismissed => {
                    self.dismiss_menu_popups(mp.host);
                }
            }
            // The panes moved under the walk: start it again. Each answer
            // is taken once, so this ends.
            i = 0;
        }
    }

    /// A row chosen in a popup, performed by the window whose menu it is —
    /// exactly as a macOS menu's row is (`pump_native_menu`). A row the
    /// core refuses closes the menu: its popup is gone either way.
    fn choose_from_popup(&mut self, event_loop: &ActiveEventLoop, host: WindowId, path: &[usize]) {
        let Some(i) = self.pane_of(host) else { return };
        let core = &mut self.panes[i].core;
        let events = core.activate_menu_path(path).unwrap_or_else(|| {
            core.close_menu();
            Vec::new()
        });
        let reached_app = self.route_events(events);
        self.owe_for(reached_app);
        self.apply_menu_actions(event_loop, i);
        self.apply_window_commands(event_loop);
        if let Some(i) = self.pane_of(host) {
            self.panes[i].redraw_for(FrameCause::MENU);
        }
    }

    /// Escape in the menu's popup, a press outside every popup of it, or
    /// the app losing the keyboard: the menu closes, as the in-window
    /// menu's `dismiss` closes it.
    pub(super) fn dismiss_menu_popups(&mut self, host: WindowId) {
        self.close_menu_popups(host, 0);
        if let Some(i) = self.pane_of(host) {
            self.panes[i].core.close_menu();
            self.panes[i].redraw_for(FrameCause::MENU);
        }
    }

    /// Closes the popups of `host`'s menu at submenu depth `depth` and
    /// deeper (0 is all of them), deepest first, the keyboard and the
    /// accessibility focus handed back down the chain. The popups' cores
    /// say nothing to the app on the way out: they never declared a thing.
    pub(super) fn close_menu_popups(&mut self, host: WindowId, depth: usize) {
        loop {
            let deepest = self
                .panes
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    let m = p.menu_pane.as_ref()?;
                    (m.host == host && m.path.len() >= depth).then_some((i, m.path.len()))
                })
                .max_by_key(|&(_, len)| len);
            let Some((i, _)) = deepest else { break };
            let id = self.panes[i].id;
            let owner = self.panes[i].owner;
            let had_keyboard = self.panes[i].os_focused;
            self.armed.retain(|a| a.id != id);
            #[cfg(target_os = "macos")]
            {
                macos_text_input::detach(&self.panes[i].window);
                macos_drop::detach(&self.panes[i].window);
                macos_key::release(&self.panes[i].window);
            }
            self.panes.remove(i);
            if let Some(j) = self.pane_of(owner) {
                if had_keyboard {
                    self.panes[j].os_focused = true;
                    self.panes[j].window.focus_window();
                }
                // Back to the popup it hangs off, or to the window while
                // the app has the keyboard; one that lost it hears so
                // from the OS.
                if self.panes[j].menu_pane.is_some() || self.panes[j].os_focused {
                    self.lend_access_focus(None, Some(j));
                }
                self.panes[j].redraw_for(FrameCause::MENU);
            }
        }
        self.settle_focus();
    }

    /// The popups of a menu that closed, or was replaced by another (which
    /// waits to be placed again), and of a window that is gone.
    fn drop_stale_menu_popups(&mut self) {
        let hosts: Vec<WindowId> = self
            .panes
            .iter()
            .filter_map(|p| p.menu_pane.as_ref().map(|m| m.host))
            .collect();
        for host in hosts {
            let stale = self
                .pane_of(host)
                .is_none_or(|i| !self.panes[i].core.menu_in_popup());
            if stale {
                self.close_menu_popups(host, 0);
            }
        }
    }

    /// A menu a window's core opened and is waiting to be placed
    /// (`Core::menu_awaits_host`): measured, and shown in a popup when it
    /// does not fit the window, in the window when it does.
    fn open_menu_popups(&mut self, event_loop: &ActiveEventLoop) {
        for i in 0..self.panes.len() {
            let p = &self.panes[i];
            if p.menu_pane.is_some() || p.core.native_menus() || !p.core.menu_awaits_host() {
                continue;
            }
            let menu = p.core.menu().expect("awaits, so open").clone();
            // A popup opened is pushed behind every pane, so `i` holds.
            let popup = self.show_menu_popup(event_loop, i, &menu);
            self.panes[i].core.set_menu_in_popup(popup);
            self.panes[i].redraw_for(FrameCause::MENU);
        }
    }

    /// A popup for the menu window `i`'s core has open, if it does not fit
    /// the window and the platform can place one: answers whether it
    /// opened.
    fn show_menu_popup(&mut self, event_loop: &ActiveEventLoop, i: usize, menu: &Menu) -> bool {
        let pane = &self.panes[i];
        if !places_popups(&pane.window) {
            return false;
        }
        let Some(owner) = pane.surface() else {
            return false;
        };
        let at = owner.to_screen(menu.at);
        let Some(work) = work_area(&pane.window, at) else {
            return false;
        };
        let unit = owner.scale;
        let room = Size::new((work.w / unit) as f32, (work.h / unit) as f32);
        let scale = pane.size().1;
        let mut core = self.menu_core(i);
        core.open_menu_popup(menu.items.clone(), menu.max_size, room, false);
        let Some(size) = measure(&mut core, room, scale) else {
            return false;
        };
        if fits(menu.at, size, owner.size) {
            return false;
        }
        let edge = widgets::MENU_EDGE as f64 * unit;
        let pos = place_menu(at, (size.w as f64 * unit, size.h as f64 * unit), work, edge);
        let mp = MenuPane {
            host: self.panes[i].id,
            path: Vec::new(),
        };
        let anchor = Rect::new(menu.at.x, menu.at.y, 0.0, 0.0);
        let owner = self.panes[i].id;
        self.open_menu_pane(event_loop, mp, owner, core, size, pos, anchor)
    }

    /// Each popup whose row has its submenu open, given a popup for it;
    /// a popup whose row closed, or moved to another, closed.
    fn sync_submenu_popups(&mut self, event_loop: &ActiveEventLoop) {
        let mut chain: Vec<(WindowId, MenuPane)> = self
            .panes
            .iter()
            .filter_map(|p| Some((p.id, p.menu_pane.clone()?)))
            .collect();
        chain.sort_by_key(|(_, m)| m.path.len());
        for (id, mp) in chain {
            let Some(i) = self.pane_of(id) else { continue };
            let want = self.panes[i].core.menu_popup_submenu();
            let child = self.panes.iter().find_map(|p| {
                let m = p.menu_pane.as_ref()?;
                (m.host == mp.host
                    && m.path.len() == mp.path.len() + 1
                    && m.path.starts_with(&mp.path))
                .then(|| *m.path.last().expect("one longer"))
            });
            match (want, child) {
                (None, None) => {}
                (Some(w), Some(row)) if w.row == row => {}
                (want, child) => {
                    if child.is_some() {
                        self.close_menu_popups(mp.host, mp.path.len() + 1);
                    }
                    if let Some(w) = want
                        && let Some(i) = self.pane_of(id)
                    {
                        self.open_submenu_popup(event_loop, i, &mp, w);
                    }
                }
            }
        }
    }

    /// A popup for the submenu of the row `w` of popup `i`, beside the row.
    fn open_submenu_popup(
        &mut self,
        event_loop: &ActiveEventLoop,
        i: usize,
        mp: &MenuPane,
        w: MenuPopupSubmenu,
    ) {
        let Some(h) = self.pane_of(mp.host) else {
            return;
        };
        let path: Vec<usize> = mp.path.iter().copied().chain([w.row]).collect();
        let Some(menu) = self.panes[h].core.menu() else {
            return;
        };
        // A submenu is held to what its menu is, as in the window.
        let max_size = menu.max_size;
        let Some(items) = MenuItem::at_path(&menu.items, &path)
            .filter(|item| item.enabled && item.has_submenu())
            .map(|item| item.submenu.clone())
        else {
            return;
        };
        let parent = &self.panes[i];
        let Some(surface) = parent.surface() else {
            return;
        };
        let unit = surface.scale;
        let tl = surface.to_screen(Vec2::new(w.rect.x, w.rect.y));
        let row = Area {
            x: tl.0,
            y: tl.1,
            w: w.rect.w as f64 * unit,
            h: w.rect.h as f64 * unit,
        };
        let Some(work) = work_area(&parent.window, tl) else {
            return;
        };
        let room = Size::new((work.w / unit) as f32, (work.h / unit) as f32);
        let scale = parent.size().1;
        let parent_id = parent.id;
        let mut core = self.menu_core(h);
        core.open_menu_popup(items, max_size, room, true);
        if w.keyboard {
            core.menu_popup_by_keyboard();
        }
        let Some(size) = measure(&mut core, room, scale) else {
            return;
        };
        let pos = place_submenu(
            row,
            (size.w as f64 * unit, size.h as f64 * unit),
            work,
            widgets::MENU_EDGE as f64 * unit,
            widgets::MENU_PANEL_PAD as f64 * unit,
        );
        let sub = MenuPane {
            host: mp.host,
            path,
        };
        self.open_menu_pane(event_loop, sub, parent_id, core, size, pos, w.rect);
    }

    /// A core for a popup of window `host`'s menu: the session's fonts and
    /// images, and the window's palette, sizes and feel.
    fn menu_core(&self, host: usize) -> Core {
        let from = &self.panes[host].core;
        let mut core = Core::new_in(&self.session);
        core.set_diagnostics(self.diagnostics);
        core.set_subpixel_text(self.subpixel);
        core.set_opacity_groups(true);
        core.set_smooth_wheel(from.smooth_wheel());
        core.set_wheel_feel(from.wheel_feel());
        core.set_edge_scroll_feel(from.edge_scroll_feel());
        core.set_theme(*from.theme());
        core.set_metrics(*from.metrics());
        core
    }

    /// The window's palette and sizes carried into its menu's popups when
    /// they change while one is up — the OS going dark under an open menu.
    fn sync_menu_look(&mut self) {
        for i in 0..self.panes.len() {
            let Some(host) = self.panes[i].menu_pane.as_ref().map(|m| m.host) else {
                continue;
            };
            let Some(h) = self.pane_of(host) else {
                continue;
            };
            let (theme, metrics) = (*self.panes[h].core.theme(), *self.panes[h].core.metrics());
            let core = &mut self.panes[i].core;
            if *core.theme() != theme || *core.metrics() != metrics {
                core.set_theme(theme);
                core.set_metrics(metrics);
                self.panes[i].redraw_for(FrameCause::APPEARANCE);
            }
        }
    }

    /// Opens a menu's popup: a borderless, non-activating window of `size`
    /// (logical px) at `pos` (the display's frame) over `core`, owned by
    /// `owner` — the window the menu is in, or the popup a submenu hangs
    /// off — with `anchor` the row or point it hangs from in the owner's
    /// coordinates. Answers whether it opened.
    #[allow(
        clippy::too_many_arguments,
        reason = "two private call sites, each argument a distinct fact about the popup"
    )]
    fn open_menu_pane(
        &mut self,
        event_loop: &ActiveEventLoop,
        mp: MenuPane,
        owner: WindowId,
        mut core: Core,
        size: Size,
        pos: (f64, f64),
        anchor: Rect,
    ) -> bool {
        let id = self.session.host_window_id();
        let mut attrs = self
            .window_attrs(
                "",
                (size.w as f64, size.h as f64),
                Chrome::Borderless,
                Backdrop::Opaque,
            )
            .with_active(false)
            .with_resizable(false)
            .with_window_level(winit::window::WindowLevel::AlwaysOnTop);
        // In the display's frame, as `popup_position` places an app's
        // popup: a logical size or position would be resolved against the
        // monitor the window is created on, which need not be its owner's
        // (backlog AR33).
        if cfg!(target_os = "macos") {
            attrs = attrs.with_position(LogicalPosition::new(pos.0, pos.1));
        } else {
            let scale = self
                .pane_of(owner)
                .map_or(1.0, |j| self.panes[j].window.scale_factor());
            attrs = attrs
                .with_position(winit::dpi::PhysicalPosition::new(pos.0, pos.1))
                .with_inner_size(winit::dpi::PhysicalSize::new(
                    (size.w as f64 * scale).ceil() as u32,
                    (size.h as f64 * scale).ceil() as u32,
                ));
        }
        #[cfg(target_os = "windows")]
        {
            // A menu's corners and shadow, as Windows 11 draws its own.
            use winit::platform::windows::{CornerPreference, WindowAttributesExtWindows};
            attrs = attrs
                .with_skip_taskbar(true)
                .with_undecorated_shadow(true)
                .with_corner_preference(CornerPreference::RoundSmall);
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            use winit::platform::x11::{WindowAttributesExtX11, WindowType};
            attrs = attrs.with_x11_window_type(vec![WindowType::PopupMenu]);
        }
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(err) => {
                eprintln!("kui: cannot open a menu's window: {err}");
                return false;
            }
        };
        self.icon.set_on(&window);
        appear_at_once(&window);
        let cloaked = cloak(&window, true);
        #[cfg(target_os = "macos")]
        macos_menu::exclude_from_windows_menu(&window);
        let Some(gpu) = self.gpu.clone() else {
            return false;
        };
        let px = window.inner_size();
        let renderer =
            match kui_wgpu::Renderer::new_in_with(&gpu, window.clone(), px.width, px.height, false)
            {
                Ok(mut r) => {
                    r.set_frame_latency(self.frame_latency);
                    r
                }
                Err(err) => {
                    eprintln!("kui: cannot draw a menu's window: {err}");
                    return false;
                }
            };
        let applied = crate::backdrop::apply(&window, &renderer, Backdrop::Opaque);
        #[cfg(target_os = "macos")]
        macos_key::refuse_key(&window);
        core.env.window.id = id;
        let config = WindowConfig::popup(anchor, size.w, size.h);
        self.push_pane(
            event_loop,
            id,
            config,
            owner,
            Chrome::Borderless,
            core,
            window,
            renderer,
            applied,
        );
        let Some(i) = self.pane_of(id) else {
            return false;
        };
        // Its menu is drawn here whatever the platform's own menus are:
        // it is the window that *is* the drawn menu.
        self.panes[i].core.set_native_menus(false);
        self.panes[i].core.set_menu_popups(false);
        self.panes[i].core.set_lookup_available(false);
        self.panes[i].menu_pane = Some(mp);
        self.panes[i].cloaked = cloaked;
        // Opened under a held button — a select opened on its press — it
        // joins that press, as an app's popup does (ADR 0009 decision 1).
        if self.primary_down.is_some() {
            self.armed.push(Armed { id, inside: false });
        }
        // The keyboard stays with the window the menu is in; the runner
        // hands its keys down the chain (`key_target`).
        let owner_i = self.pane_of(owner);
        if let Some(j) = owner_i {
            self.panes[j].window.focus_window();
        }
        self.lend_access_focus(owner_i, Some(i));
        true
    }

    /// Moves the accessibility focus from pane `from` to pane `to`, either
    /// optional: a screen reader follows the window it is told has focus,
    /// and a menu's popup never takes the OS's (the keyboard is lent to
    /// it). Told on opening a popup and on closing one, so the rows are
    /// read while the menu is up and the window is again once it is gone.
    fn lend_access_focus(&mut self, from: Option<usize>, to: Option<usize>) {
        for (i, focused) in [(from, false), (to, true)] {
            let Some(i) = i else { continue };
            let pane = &mut self.panes[i];
            if let Some(bridge) = &mut pane.access {
                bridge.process_event(&pane.window, &WinitWindowEvent::Focused(focused));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Area = Area {
        x: 0.0,
        y: 0.0,
        w: 1000.0,
        h: 600.0,
    };

    #[test]
    fn a_menu_that_shows_whole_at_its_point_fits() {
        let vp = Size::new(400.0, 300.0);
        assert!(fits(Vec2::new(20.0, 20.0), Size::new(200.0, 100.0), vp));
        // Past the right edge, `fit` would move it in.
        assert!(!fits(Vec2::new(300.0, 20.0), Size::new(200.0, 100.0), vp));
        // Past the bottom, the same.
        assert!(!fits(Vec2::new(20.0, 250.0), Size::new(200.0, 100.0), vp));
        // Taller than the window less its margins, layout would cut it.
        assert!(!fits(Vec2::new(0.0, 0.0), Size::new(200.0, 290.0), vp));
        // Wider than the window less its margins, its rows would be cut.
        assert!(!fits(Vec2::new(0.0, 0.0), Size::new(395.0, 100.0), vp));
    }

    #[test]
    fn a_menus_popup_opens_at_the_point_and_flips_where_it_would_run_off() {
        let size = (200.0, 300.0);
        assert_eq!(place_menu((100.0, 100.0), size, WORK, 8.0), (100.0, 100.0));
        // Near the right edge: ends at the point.
        assert_eq!(place_menu((900.0, 100.0), size, WORK, 8.0), (700.0, 100.0));
        // Near the bottom: above the point.
        assert_eq!(place_menu((100.0, 500.0), size, WORK, 8.0), (100.0, 200.0));
    }

    /// Too tall for either side of the point: pushed in, which for a menu
    /// as tall as the work area less its margins is the top of that room.
    #[test]
    fn a_menus_popup_too_tall_for_either_side_goes_as_high_as_it_can() {
        let size = (200.0, WORK.h - 16.0);
        assert_eq!(place_menu((100.0, 300.0), size, WORK, 8.0).1, 8.0);
        let size = (200.0, 400.0);
        assert_eq!(
            place_menu((100.0, 300.0), size, WORK, 8.0).1,
            WORK.h - 8.0 - 400.0,
            "a shorter one ends at the room's bottom"
        );
    }

    #[test]
    fn a_submenus_popup_opens_beside_its_row_and_flips_at_the_edge() {
        let row = Area {
            x: 100.0,
            y: 200.0,
            w: 180.0,
            h: 24.0,
        };
        let size = (200.0, 100.0);
        assert_eq!(
            place_submenu(row, size, WORK, 8.0, 4.0),
            (284.0, 196.0),
            "past the row's right edge, first row level with it"
        );
        let row = Area { x: 700.0, ..row };
        assert_eq!(
            place_submenu(row, size, WORK, 8.0, 4.0).0,
            496.0,
            "before the row's left edge where the right is short"
        );
        let row = Area { y: 560.0, ..row };
        assert_eq!(
            place_submenu(row, size, WORK, 8.0, 4.0).1,
            WORK.h - 8.0 - 100.0,
            "pushed up at the bottom"
        );
    }
}
