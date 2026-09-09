//! The Win32 context menu: a popup `HMENU`, tracked where the press was
//! (`docs/adr/0017-selection-as-a-scope.md`, decision 5, step 3).
//!
//! Simpler than the macOS half in one way and identical in the other. The
//! way it is simpler: `TrackPopupMenu` with `TPM_RETURNCMD` returns the
//! chosen command id itself, so there is no target object, no action
//! selector and no second turn of the loop to collect an answer — one call
//! shows the menu and reports what happened.
//!
//! The way it is the same: that call runs a **modal message loop** and does
//! not return until the menu closes, so where it is called from matters.
//! It is called from `about_to_wait`, which is the shallowest handler the
//! runner has — the loop is about to park anyway — and never from inside a
//! `window_event`, where a nested loop would re-enter the event handler
//! with a `&mut` borrow already live.
//!
//! **Unverified.** No machine in this project's CI runs Windows, and this
//! was written on a Mac against the headers; it type-checks for
//! `x86_64-pc-windows-msvc` and nothing more. The failure it is most
//! likely to have is the one above — where the tracking call is made from
//! — and the fix for that is to move the call, not to rewrite the menu.

use kui_core::{MenuItem, MenuRole, Vec2};
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, MF_GRAYED, MF_SEPARATOR, MF_STRING, TPM_LEFTALIGN,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TPM_TOPALIGN, TrackPopupMenu,
};
use winit::window::Window;

/// What one tracked menu reported.
pub enum Tracked {
    /// The row at this index was chosen.
    Chosen(usize),
    /// The menu closed with nothing chosen.
    Dismissed,
    /// The menu could not be shown at all (no HWND, or Win32 refused), so
    /// the caller should fall back to the menu the core draws.
    Refused,
}

/// Shows `items` over `window` at `at` (logical viewport px) and blocks
/// until the menu closes.
///
/// Command ids are the row's index plus one, because zero is
/// `TrackPopupMenu`'s "nothing was chosen".
pub fn track(window: &Window, at: Vec2, items: &[MenuItem]) -> Tracked {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else {
        return Tracked::Refused;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return Tracked::Refused;
    };
    let hwnd = h.hwnd.get() as HWND;

    // SAFETY: every call below takes the menu this function owns and the
    // window handle winit just handed out, and the menu is destroyed on
    // every path out.
    unsafe {
        // An HMENU is an `isize` here, and zero is the failure.
        let menu = CreatePopupMenu();
        if menu == 0 {
            return Tracked::Refused;
        }
        for (i, item) in items.iter().enumerate() {
            if item.role == MenuRole::Separator {
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                continue;
            }
            // Win32 draws `&` as an access-key underline, so a label
            // holding one has to say it means the character.
            let text = wide(&item.text().replace('&', "&&"));
            let flags = MF_STRING | if item.enabled { 0 } else { MF_GRAYED };
            AppendMenuW(menu, flags, i + 1, text.as_ptr());
        }

        // Logical viewport px are client px; TrackPopupMenu wants screen.
        let scale = window.scale_factor();
        let mut p = POINT {
            x: (at.x as f64 * scale) as i32,
            y: (at.y as f64 * scale) as i32,
        };
        ClientToScreen(hwnd, &mut p);
        let chosen = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD | TPM_RIGHTBUTTON,
            p.x,
            p.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        match chosen {
            0 => Tracked::Dismissed,
            id => Tracked::Chosen(id as usize - 1),
        }
    }
}

/// A NUL-terminated UTF-16 copy, which is what every `*W` call takes.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
