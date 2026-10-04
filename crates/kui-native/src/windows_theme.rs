//! The OS's light/dark switch, seen while the app runs.
//!
//! winit reads a window's appearance with uxtheme's `ShouldAppsUseDarkMode`
//! (ordinal 132) when the window is made and again on every
//! `WM_SETTINGCHANGE`, and sends `ThemeChanged` when the answer moved. The
//! answer comes from a policy uxtheme caches per process, and nothing in the
//! message's default handling refreshes it: the cache is refreshed by
//! `RefreshImmersiveColorPolicyState` (ordinal 104), which the process has
//! to call itself — win32-darkmode, which winit's code follows, calls it on
//! the `ImmersiveColorSet` setting change, and winit does not. So the switch
//! in Settings reached winit as the answer it already had, no
//! `ThemeChanged` went out, and a window kept the base it opened with until
//! the app was started again (seen 2026-09-30 in kawoosh, light after the
//! OS went dark). Focus coming back did not help: it re-reads
//! `Window::theme`, which on Windows is winit's record of the last answer.
//! Measured on Windows 11 26200 the same day, four switches in a window:
//! on every `ImmersiveColorSet` the policy answered the old base before
//! the refresh and the new one after it.
//!
//! The subclass runs before winit's window procedure, so refreshing the
//! policy there, on `ImmersiveColorSet`, is what makes winit's own check
//! see the new answer — and winit then does the rest: `ThemeChanged`, and
//! the window's dark-mode attributes set to match. The refresh is
//! process-wide and cheap; each window refreshes it on its own copy of the
//! broadcast, since the order the windows get it in is the OS's.
//!
//! Both ordinals are the ones winit and win32-darkmode use, and exist from
//! Windows 10 1809 (build 17763), the build winit's dark mode starts at;
//! below it the ordinal names another function, so nothing is called.
//!
//! The same subclass hears `WM_FONTCHANGE`, the broadcast that follows a
//! font installed or removed, and hands it to `mod system_fonts`.

use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{WM_DESTROY, WM_FONTCHANGE, WM_SETTINGCHANGE};

const SUBCLASS_ID: usize = 0x6b7574; // "kut"

/// The first build whose uxtheme has the ordinal (Windows 10 1809).
const FIRST_BUILD: u32 = 17763;

/// Subclasses the window so a light/dark switch reaches winit. `false` when
/// the window is not a Win32 one or the subclass was refused; the window
/// then keeps the appearance it opened with, as every window did before.
pub fn install(window: &winit::window::Window) -> bool {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else {
        return false;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return false;
    };
    unsafe { SetWindowSubclass(h.hwnd.get() as HWND, Some(subclass_proc), SUBCLASS_ID, 0) != 0 }
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        WM_SETTINGCHANGE if is_color_scheme_change(lparam) => {
            if let Some(refresh) = refresh_policy() {
                unsafe { refresh() };
            }
        }
        // A font added or removed system-wide (`mod system_fonts`): the
        // same subclass, since every top-level window gets the broadcast
        // and this is where they are heard.
        WM_FONTCHANGE => crate::system_fonts::changed(),
        WM_DESTROY => unsafe {
            RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
        },
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Whether the setting that changed is the colour scheme: the broadcast's
/// `lParam` names the area, `ImmersiveColorSet` for the light/dark switch
/// (and the accent), compared without case as win32-darkmode does.
fn is_color_scheme_change(lparam: LPARAM) -> bool {
    if lparam == 0 {
        return false;
    }
    let area = lparam as *const u16;
    let mut len = 0;
    // Bounded: the area names are short, and a longer one is not this.
    while len < 64 && unsafe { *area.add(len) } != 0 {
        len += 1;
    }
    let area = unsafe { std::slice::from_raw_parts(area, len) };
    String::from_utf16(area).is_ok_and(|a| a.eq_ignore_ascii_case("ImmersiveColorSet"))
}

type RefreshImmersiveColorPolicyState = unsafe extern "system" fn();

/// uxtheme's `RefreshImmersiveColorPolicyState`, looked up once; `None`
/// before 1809 or without uxtheme.
fn refresh_policy() -> Option<RefreshImmersiveColorPolicyState> {
    static REFRESH: OnceLock<Option<RefreshImmersiveColorPolicyState>> = OnceLock::new();
    *REFRESH.get_or_init(|| {
        if windows_build()? < FIRST_BUILD {
            return None;
        }
        let module = unsafe { LoadLibraryA(c"uxtheme.dll".as_ptr().cast()) };
        if module == 0 {
            return None;
        }
        const ORDINAL: usize = 104;
        let f = unsafe { GetProcAddress(module, ORDINAL as *const u8) }?;
        Some(unsafe {
            std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                RefreshImmersiveColorPolicyState,
            >(f)
        })
    })
}

/// The Windows 10/11 build number, from `RtlGetVersion` (which, unlike
/// `GetVersionEx`, is not shimmed to the manifest's version).
fn windows_build() -> Option<u32> {
    #[repr(C)]
    struct OsVersionInfoW {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        csd: [u16; 128],
    }
    type RtlGetVersion = unsafe extern "system" fn(*mut OsVersionInfoW) -> i32;
    let module = unsafe { LoadLibraryA(c"ntdll.dll".as_ptr().cast()) };
    if module == 0 {
        return None;
    }
    let f = unsafe { GetProcAddress(module, c"RtlGetVersion".as_ptr().cast()) }?;
    let f =
        unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, RtlGetVersion>(f) };
    let mut info = OsVersionInfoW {
        size: size_of::<OsVersionInfoW>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform: 0,
        csd: [0; 128],
    };
    (unsafe { f(&mut info) } >= 0 && info.major == 10).then_some(info.build)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    #[test]
    fn only_the_colour_scheme_is_a_colour_scheme_change() {
        let set = wide("ImmersiveColorSet");
        assert!(is_color_scheme_change(set.as_ptr() as LPARAM));
        let lower = wide("immersivecolorset");
        assert!(is_color_scheme_change(lower.as_ptr() as LPARAM));
        let other = wide("intl");
        assert!(!is_color_scheme_change(other.as_ptr() as LPARAM));
        assert!(!is_color_scheme_change(0));
    }

    #[test]
    fn the_refresh_is_found_on_this_build() {
        // Every Windows the suite runs on is past 1809.
        assert!(windows_build().is_some_and(|b| b >= FIRST_BUILD));
        assert!(refresh_policy().is_some());
    }
}
