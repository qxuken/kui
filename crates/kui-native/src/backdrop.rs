//! The window's backdrop (backlog F126): what shows through where a frame
//! paints nothing, or paints with alpha — a material the OS draws behind
//! the window (macOS vibrancy, Windows 11's Mica and Acrylic), the bare
//! desktop, or nothing, which is the opaque window every app had before.
//!
//! Three parts, and each can come back with less than was asked:
//!
//! - **The window**, made see-through when it is created ([`attrs`]):
//!   winit's `with_transparent`, which on macOS is a non-opaque window
//!   with a clear background and on X11 an ARGB visual; on Windows a
//!   window with no GDI surface (`WS_EX_NOREDIRECTIONBITMAP`), whose
//!   only content is the swapchain DirectComposition presents.
//! - **The surface**, presented with alpha (`Renderer::new_in_with`):
//!   a CAMetalLayer that is not opaque, a D3D12 swapchain presented
//!   through DirectComposition (the device opened with
//!   `GpuOptions::transparent`), a Vulkan surface that offers a
//!   premultiplied or inherited mode. Where none is offered the window
//!   stays opaque, whatever else was done.
//! - **The material** ([`apply`]): an `NSVisualEffectView` under the
//!   content on macOS; `DWMWA_SYSTEMBACKDROP_TYPE` and the client area
//!   extended under the frame on Windows 11 (22H2 and later — Windows 10
//!   refuses the attribute, and the window is merely see-through there);
//!   none on Linux, where the window is merely see-through.
//!
//! What came of it is the pane's, and every frame's `env.window.backdrop`,
//! so a view draws its translucent sidebar only over something.

use std::sync::Arc;

use kui_core::Backdrop;
use winit::window::{Window, WindowAttributes};

/// The attributes a window that wants `backdrop` is created with: see-
/// through where the platform makes a window so at creation. Nothing for
/// an opaque one, so an app that never asked is created exactly as before.
pub(crate) fn attrs(attrs: WindowAttributes, backdrop: Backdrop) -> WindowAttributes {
    if !backdrop.is_translucent() {
        return attrs;
    }
    #[cfg(target_os = "windows")]
    {
        windows::attrs(attrs)
    }
    #[cfg(not(target_os = "windows"))]
    {
        attrs.with_transparent(true)
    }
}

/// Puts `backdrop` behind `window`, whose renderer has just been made —
/// asked to present with alpha when `backdrop` lets anything through
/// (`Renderer::new_in_with`) — and returns what is there now: the
/// backdrop asked for, `Transparent` where the platform has the
/// see-through window and not the material, or `Opaque` where the surface
/// would not take alpha. Called once, before the window is shown.
pub(crate) fn apply(
    window: &Arc<Window>,
    renderer: &kui_wgpu::Renderer,
    backdrop: Backdrop,
) -> Backdrop {
    if !backdrop.is_translucent() || !renderer.transparent() {
        // Nothing the frame paints with alpha would reach the screen as
        // alpha, so there is no backdrop to speak of.
        return Backdrop::Opaque;
    }
    if !backdrop.is_material() {
        return Backdrop::Transparent;
    }
    material(window, backdrop)
}

/// The material on macOS: an `NSVisualEffectView` the size of the content
/// view, under everything in it.
#[cfg(target_os = "macos")]
fn material(window: &Window, backdrop: Backdrop) -> Backdrop {
    if macos::install(window, backdrop) {
        backdrop
    } else {
        Backdrop::Transparent
    }
}

/// The material on Windows 11: the system backdrop, in a client area
/// extended under the frame so the backdrop is behind all of it. A
/// Windows that refuses the attribute (10, and 11 before 22H2) has no
/// material, and the window is see-through to the desktop.
#[cfg(target_os = "windows")]
fn material(window: &Window, backdrop: Backdrop) -> Backdrop {
    if windows::system_backdrop(window, backdrop) {
        backdrop
    } else {
        Backdrop::Transparent
    }
}

/// Elsewhere there is no material to ask for: the see-through window is
/// the whole of it.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn material(_window: &Window, _backdrop: Backdrop) -> Backdrop {
    Backdrop::Transparent
}

#[cfg(target_os = "windows")]
mod windows {
    use kui_core::Backdrop;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMSBT_MAINWINDOW, DWMSBT_TABBEDWINDOW, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
        DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
    };
    use windows_sys::Win32::UI::Controls::MARGINS;
    use winit::window::{Window, WindowAttributes};

    /// A translucent window on Windows has no GDI surface at all
    /// (`WS_EX_NOREDIRECTIONBITMAP`): the frame is the D3D12 swapchain
    /// presented through DirectComposition, alone, so where it is
    /// transparent DWM shows what is behind the window — the system
    /// backdrop, or the desktop. With a redirection surface under the
    /// swapchain its pixels showed instead: black, opaque, measured in a
    /// window on Windows 11 with the sidebar's white at 45% reading as a
    /// flat grey. Not where `WGPU_BACKEND` names another backend: only
    /// D3D12 presents through a visual, and a window with no surface of
    /// its own and a swapchain on its handle draws nothing.
    pub(super) fn attrs(attrs: WindowAttributes) -> WindowAttributes {
        use winit::platform::windows::WindowAttributesExtWindows;
        if std::env::var_os("WGPU_BACKEND").is_some() {
            return attrs;
        }
        attrs.with_no_redirection_bitmap(true)
    }

    /// The `DWM_SYSTEMBACKDROP_TYPE` a backdrop is: Mica for a window's,
    /// Mica Alt (the tabbed-window backdrop, the more tinted one) for a
    /// sidebar's, Acrylic for a menu's.
    pub(super) fn backdrop_type(backdrop: Backdrop) -> Option<i32> {
        match backdrop {
            Backdrop::Window => Some(DWMSBT_MAINWINDOW),
            Backdrop::Sidebar => Some(DWMSBT_TABBEDWINDOW),
            Backdrop::Transient => Some(DWMSBT_TRANSIENTWINDOW),
            Backdrop::Opaque | Backdrop::Transparent => None,
        }
    }

    /// Asks DWM for the backdrop behind `window` and extends the frame
    /// over the whole client area, so it is behind every pixel. Whether
    /// DWM took the attribute: an `HRESULT` failure on a Windows without
    /// system backdrops, where nothing is extended either.
    pub(super) fn system_backdrop(window: &Window, backdrop: Backdrop) -> bool {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Some(kind) = backdrop_type(backdrop) else {
            return false;
        };
        let Ok(handle) = window.window_handle() else {
            return false;
        };
        let RawWindowHandle::Win32(h) = handle.as_raw() else {
            return false;
        };
        let hwnd = h.hwnd.get() as HWND;
        // SAFETY: a live window's handle, on the thread that owns it; the
        // attribute is an `i32` read for the call's length only.
        let hr = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE as u32,
                (&kind as *const i32).cast(),
                size_of::<i32>() as u32,
            )
        };
        if hr < 0 {
            return false;
        }
        // -1 on every side: "sheet of glass", the whole client area is
        // frame, and DWM draws the backdrop behind all of it.
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        // SAFETY: as above; `margins` outlives the call.
        let hr = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };
        hr >= 0
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use kui_core::Backdrop;
    use objc2::rc::Retained;
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
    use objc2_app_kit::{
        NSAutoresizingMaskOptions, NSResponder, NSView, NSVisualEffectBlendingMode,
        NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
    };
    use objc2_foundation::{NSObject, NSPoint, NSRect};
    use winit::window::Window;

    define_class!(
        // SAFETY:
        // - `NSVisualEffectView` has no subclassing requirements beyond
        //   its designated initializer, which `new` calls.
        // - `BackdropView` does not implement `Drop`.
        #[unsafe(super(NSVisualEffectView, NSView, NSResponder, NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "KuiBackdropView"]
        struct BackdropView;

        impl BackdropView {
            /// Never the view a point lands on: it is a subview of winit's
            /// content view, and a subview answers `hitTest:` before the
            /// view it sits in — so without this every click, drag and
            /// cursor update would land on the material and not on the
            /// view that turns them into winit's events.
            #[unsafe(method(hitTest:))]
            fn hit_test(&self, _point: NSPoint) -> *mut NSView {
                std::ptr::null_mut()
            }
        }
    );

    impl BackdropView {
        fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(());
            // SAFETY: `initWithFrame:` is NSView's designated initializer.
            unsafe { msg_send![super(this), initWithFrame: frame] }
        }
    }

    /// The material a backdrop is: the under-window background for a
    /// window's, the sidebar's for a sidebar, the popover's for a menu.
    pub(super) fn material(backdrop: Backdrop) -> Option<NSVisualEffectMaterial> {
        match backdrop {
            Backdrop::Window => Some(NSVisualEffectMaterial::UnderWindowBackground),
            Backdrop::Sidebar => Some(NSVisualEffectMaterial::Sidebar),
            Backdrop::Transient => Some(NSVisualEffectMaterial::Popover),
            Backdrop::Opaque | Backdrop::Transparent => None,
        }
    }

    /// Adds the material under `window`'s content: an effect view the
    /// size of winit's content view, resized with it, blending what is
    /// behind the window and following its active state (dimmed in a
    /// window in the background, as every sidebar is). Its layer sits
    /// under the CAMetalLayer wgpu added to the same view — the order
    /// AppKit gives a subview's layer among sublayers it did not add is
    /// not one to rely on. False when the window has no AppKit view.
    pub(super) fn install(window: &Window, backdrop: Backdrop) -> bool {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Some(material) = material(backdrop) else {
            return false;
        };
        let Some(mtm) = MainThreadMarker::new() else {
            return false;
        };
        let Ok(handle) = window.window_handle() else {
            return false;
        };
        let RawWindowHandle::AppKit(h) = handle.as_raw() else {
            return false;
        };
        // SAFETY: a live winit content view, on the event loop's thread.
        let Some(view) = (unsafe { Retained::retain(h.ns_view.as_ptr().cast::<NSView>()) }) else {
            return false;
        };
        let effect = BackdropView::new(mtm, view.bounds());
        effect.setMaterial(material);
        effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        effect.setState(NSVisualEffectState::FollowsWindowActiveState);
        effect.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        effect.setWantsLayer(true);
        view.addSubview_positioned_relativeTo(&effect, NSWindowOrderingMode::Below, None);
        if let Some(layer) = effect.layer() {
            layer.setZPosition(-1.0);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An app that never asked is created exactly as before: no attribute
    /// is touched for an opaque window.
    #[test]
    fn an_opaque_window_is_created_as_it_always_was() {
        let plain = Window::default_attributes();
        let same = attrs(Window::default_attributes(), Backdrop::Opaque);
        assert_eq!(same.transparent, plain.transparent);
        assert!(!same.transparent);
        // See-through from creation, by winit's flag everywhere but
        // Windows, where the window is one with no GDI surface instead
        // (whose flag the attributes keep to themselves).
        for b in [Backdrop::Transparent, Backdrop::Sidebar] {
            assert_eq!(
                attrs(Window::default_attributes(), b).transparent,
                !cfg!(target_os = "windows")
            );
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn each_material_is_a_system_backdrop_type() {
        use windows_sys::Win32::Graphics::Dwm::{
            DWMSBT_MAINWINDOW, DWMSBT_TABBEDWINDOW, DWMSBT_TRANSIENTWINDOW,
        };
        assert_eq!(
            windows::backdrop_type(Backdrop::Window),
            Some(DWMSBT_MAINWINDOW)
        );
        assert_eq!(
            windows::backdrop_type(Backdrop::Sidebar),
            Some(DWMSBT_TABBEDWINDOW)
        );
        assert_eq!(
            windows::backdrop_type(Backdrop::Transient),
            Some(DWMSBT_TRANSIENTWINDOW)
        );
        assert_eq!(windows::backdrop_type(Backdrop::Transparent), None);
        assert_eq!(windows::backdrop_type(Backdrop::Opaque), None);
    }
}
