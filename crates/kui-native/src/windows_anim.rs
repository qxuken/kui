//! The clock an animation needs while Windows owns the message loop
//! (backlog W3).
//!
//! Moving or resizing a window runs inside `DefWindowProc`'s own modal loop
//! (`WM_ENTERSIZEMOVE` … `WM_EXITSIZEMOVE`), and for its duration the
//! process's loop is that one, not winit's: `about_to_wait` — where the
//! runner asks an animating pane for its next frame — never runs, so a
//! transition froze for as long as the title bar was held. Measured on
//! Windows 11 / winit 0.30.13 on 2026-09-08: 0 frames in a 2 s hold.
//!
//! The modal loop does dispatch `WM_PAINT`, so the first fix asked for the
//! next frame from `RedrawRequested` itself. That works and costs too much:
//! each answer blocks on vsync *inside* the modal loop, and the coalesced
//! `WM_MOUSEMOVE` behind it waits that long, so a dragged window followed
//! the pointer in 18 px steps 8 ms apart where a still one manages 5 px
//! steps 2 ms apart — the lurch that made this worth measuring.
//!
//! A timer is the mechanism that has neither problem. `SetTimer` with a
//! `TIMERPROC` is dispatched by `DispatchMessage` wherever the loop is
//! running, including in there, and `WM_TIMER` is a generated message like
//! `WM_PAINT`: real input outranks it. So the frames come at the timer's
//! pace and the drag keeps the rest of the loop. `RDW_INTERNALPAINT` is
//! what the callback does with its turn — invalidate nothing, just ask for
//! the paint winit turns into `RedrawRequested` — because the callback runs
//! with no access to the runner and needs none.
//!
//! It is armed while the pane animates and killed when it settles, so a
//! still app posts nothing. Outside the modal loop it is redundant with
//! `about_to_wait`'s own request, which is coalesced into the same
//! `WM_PAINT`; the cost of that is a timer message every
//! [`INTERVAL_MS`] that changes nothing.

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::{RDW_INTERNALPAINT, RedrawWindow};
use windows_sys::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

/// Per window, and not shared with any other timer this process sets.
const TIMER_ID: usize = 0x6b7561; // "kua"

/// How often the modal loop is asked for a frame. `USER_TIMER_MINIMUM` is
/// 10 ms, so this is as fast as a timer goes: ~100 fps for the hold, and
/// the rest of the loop's time left to the drag it is competing with.
const INTERVAL_MS: u32 = 10;

/// The redraw pacer for one window. Dropping it kills the timer.
pub struct AnimTimer {
    hwnd: HWND,
    armed: bool,
}

// The HWND is only ever touched from the event-loop thread that made it.
unsafe impl Send for AnimTimer {}

impl AnimTimer {
    pub fn new(window: &winit::window::Window) -> Option<Self> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let RawWindowHandle::Win32(h) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        Some(Self {
            hwnd: h.hwnd.get() as HWND,
            armed: false,
        })
    }

    /// Arms the timer while `animating`, kills it once. Cheap to call every
    /// frame: the state is mirrored here and the OS is only told on change.
    pub fn set(&mut self, animating: bool) {
        if animating == self.armed {
            return;
        }
        self.armed = animating;
        unsafe {
            if animating {
                SetTimer(self.hwnd, TIMER_ID, INTERVAL_MS, Some(tick));
            } else {
                KillTimer(self.hwnd, TIMER_ID);
            }
        }
    }
}

impl Drop for AnimTimer {
    fn drop(&mut self) {
        if self.armed {
            unsafe { KillTimer(self.hwnd, TIMER_ID) };
        }
    }
}

/// Dispatched wherever the message loop is, which is the whole point.
unsafe extern "system" fn tick(hwnd: HWND, _msg: u32, _id: usize, _time: u32) {
    unsafe { RedrawWindow(hwnd, std::ptr::null(), 0, RDW_INTERNALPAINT) };
}
