//! Windows non-client integration for custom chrome. The runner mirrors
//! each frame's hit regions into shared state and subclasses the window so
//! WM_NCHITTEST can answer with real caption codes: the drag strip becomes
//! HTCAPTION (native drag, double-click maximize, Win+arrow snapping,
//! right-click system menu), the drawn buttons become HTMINBUTTON /
//! HTMAXBUTTON / HTCLOSE (which is what makes Windows 11 show snap layouts
//! over our maximize button), and the outer band becomes resize borders
//! with native cursors.
//!
//! Regions with those codes stop receiving client mouse messages, so the
//! subclass mirrors non-client mouse events back into the client stream:
//! moves for hover styling, button presses/releases so the core still
//! resolves them into `WindowCommand`s (the system's own default click
//! handling for the caption buttons is swallowed to avoid doubling up).

use std::sync::{Arc, Mutex};

use kui_core::{Rect, Vec2, WindowButton, WindowRole};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTCLOSE, HTLEFT,
    HTMAXBUTTON, HTMINBUTTON, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, PostMessageW, WM_DESTROY,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCHITTEST, WM_NCLBUTTONDBLCLK,
    WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE,
};

const SUBCLASS_ID: usize = 0x6b7569; // "kui"
const MK_LBUTTON: WPARAM = 0x0001;
/// Lives in a different windows-sys module (TrackMouseEvent's); inlined.
const WM_MOUSELEAVE: u32 = 0x02a3;
/// Must match the runner's RESIZE_BAND.
const BAND: f32 = 6.0;

/// Per-frame facts the hit-test callback reads. Updated on the event-loop
/// thread; the callback runs on the same thread, the mutex is just tidiness.
#[derive(Default)]
struct NcState {
    scale: f32,
    maximized: bool,
    /// Report resize borders around the edge (undecorated windows).
    resize_border: bool,
    /// All hit regions, paint order (topmost last), logical px. Non-chrome
    /// regions matter too: a tab drawn over the drag strip must win and
    /// stay HTCLIENT.
    regions: Vec<(Option<WindowRole>, Rect)>,
}

pub struct NcHitTest {
    state: Arc<Mutex<NcState>>,
}

impl NcHitTest {
    /// Subclasses the window; the state pointer handed to the subclass is
    /// released when the window is destroyed.
    pub fn install(window: &winit::window::Window, resize_border: bool) -> Option<Self> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let RawWindowHandle::Win32(h) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        let state = Arc::new(Mutex::new(NcState {
            scale: 1.0,
            resize_border,
            ..Default::default()
        }));
        let data = Arc::into_raw(state.clone()) as usize;
        let ok = unsafe {
            SetWindowSubclass(h.hwnd.get() as HWND, Some(subclass_proc), SUBCLASS_ID, data)
        };
        if ok == 0 {
            unsafe { drop(Arc::from_raw(data as *const Mutex<NcState>)) };
            return None;
        }
        Some(Self { state })
    }

    /// Publishes this frame's hit regions (paint order, logical px).
    pub fn update(
        &self,
        scale: f32,
        maximized: bool,
        regions: impl Iterator<Item = (Option<WindowRole>, Rect)>,
    ) {
        let mut s = self.state.lock().unwrap();
        s.scale = scale;
        s.maximized = maximized;
        s.regions.clear();
        s.regions.extend(regions);
    }
}

fn point_of(lparam: LPARAM) -> POINT {
    POINT { x: (lparam & 0xffff) as i16 as i32, y: ((lparam >> 16) & 0xffff) as i16 as i32 }
}

/// Re-posts a non-client mouse message (screen coords) as its client
/// equivalent so winit — which ignores NC mouse input — still sees it.
fn forward_as_client(hwnd: HWND, msg: u32, wparam: WPARAM, screen_lparam: LPARAM) {
    let mut p = point_of(screen_lparam);
    unsafe {
        ScreenToClient(hwnd, &mut p);
        let lparam = (((p.y as u32) << 16) | (p.x as u32 & 0xffff)) as i32 as LPARAM;
        PostMessageW(hwnd, msg, wparam, lparam);
    }
}

fn hit_code(state: &Mutex<NcState>, hwnd: HWND, screen_lparam: LPARAM) -> Option<u32> {
    let s = state.lock().ok()?;
    let mut p = point_of(screen_lparam);
    unsafe { ScreenToClient(hwnd, &mut p) };
    let scale = if s.scale > 0.0 { s.scale } else { 1.0 };
    let pt = Vec2::new(p.x as f32 / scale, p.y as f32 / scale);

    if s.resize_border && !s.maximized {
        let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe { GetClientRect(hwnd, &mut rc) };
        let w = (rc.right - rc.left) as f32 / scale;
        let h = (rc.bottom - rc.top) as f32 / scale;
        let (l, r) = (pt.x < BAND, pt.x > w - BAND);
        let (t, b) = (pt.y < BAND, pt.y > h - BAND);
        let code = match (l, r, t, b) {
            (true, _, true, _) => HTTOPLEFT,
            (_, true, true, _) => HTTOPRIGHT,
            (true, _, _, true) => HTBOTTOMLEFT,
            (_, true, _, true) => HTBOTTOMRIGHT,
            (true, ..) => HTLEFT,
            (_, true, ..) => HTRIGHT,
            (_, _, true, _) => HTTOP,
            (_, _, _, true) => HTBOTTOM,
            _ => 0,
        };
        if code != 0 {
            return Some(code);
        }
    }

    // Topmost region wins; interactive content drawn over the drag strip
    // stays client so it remains clickable.
    for (role, rect) in s.regions.iter().rev() {
        if rect.contains(pt) {
            return Some(match role {
                Some(WindowRole::Drag) => HTCAPTION,
                Some(WindowRole::Button(WindowButton::Close)) => HTCLOSE,
                Some(WindowRole::Button(WindowButton::Minimize)) => HTMINBUTTON,
                Some(WindowRole::Button(WindowButton::Maximize)) => HTMAXBUTTON,
                None => HTCLIENT,
            });
        }
    }
    None
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    let state = unsafe { &*(data as *const Mutex<NcState>) };
    match msg {
        WM_NCHITTEST => {
            let def = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
            if def != HTCLIENT as LRESULT {
                return def;
            }
            match hit_code(state, hwnd, lparam) {
                Some(code) => code as LRESULT,
                None => def,
            }
        }
        // Hover styling: mirror NC moves into the client stream (harmless
        // for HTCAPTION — dragging starts from WM_NCLBUTTONDOWN, not moves).
        WM_NCMOUSEMOVE => {
            forward_as_client(hwnd, WM_MOUSEMOVE, 0, lparam);
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_NCMOUSELEAVE => {
            unsafe { PostMessageW(hwnd, WM_MOUSELEAVE, 0, 0) };
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        // Caption-button clicks are ours: route them through the core so
        // they become WindowCommands; swallow the system's own handling.
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK
            if matches!(wparam as u32, HTCLOSE | HTMINBUTTON | HTMAXBUTTON) =>
        {
            forward_as_client(hwnd, WM_LBUTTONDOWN, MK_LBUTTON, lparam);
            0
        }
        WM_NCLBUTTONUP if matches!(wparam as u32, HTCLOSE | HTMINBUTTON | HTMAXBUTTON) => {
            forward_as_client(hwnd, WM_LBUTTONUP, 0, lparam);
            0
        }
        WM_DESTROY => {
            let res = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
            unsafe {
                RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
                drop(Arc::from_raw(data as *const Mutex<NcState>));
            }
            res
        }
        _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
    }
}
