//! The window's backdrop (backlog F126): what shows through where a frame
//! paints nothing, or paints with alpha — named for the effect: the desktop
//! as it is (`Transparent`), a live blur of what is behind the window
//! (`Blur`), the desktop's colour (`Tinted`), or nothing, the opaque window
//! every app had before. Which regions show it is the app's, by what it
//! paints with alpha; nothing here knows a sidebar from a toolbar.
//!
//! Each part can come back with less than was asked:
//!
//! - **The window**, made see-through when it is created ([`attrs`]):
//!   winit's `with_transparent` — on macOS a non-opaque window with a
//!   clear background, on X11 an ARGB visual — and on Windows a window
//!   with no GDI surface (`WS_EX_NOREDIRECTIONBITMAP`), whose only content
//!   is the swapchain DirectComposition presents, wherever
//!   `kui_wgpu::see_through_by_visual` says D3D12 will present it so
//!   (`WGPU_BACKEND` and `WGPU_DX12_PRESENTATION_SYSTEM` can say not;
//!   the window is then opaque and reported so).
//! - **The surface**, presented with alpha (`Renderer::new_in_with`): a
//!   CAMetalLayer that is not opaque, a D3D12 swapchain presented through
//!   DirectComposition, a Vulkan surface that offers a premultiplied or
//!   inherited mode. Where none is offered nothing shows through.
//! - **The effect** ([`apply`]). macOS: an `NSVisualEffectView` under the
//!   content, blending what is behind the window — the sidebar material
//!   for `Blur`, the window-background one (AppKit's wallpaper tinting)
//!   for `Tinted`; which material is kui's choice, not the API's. Windows
//!   11 (22H2 and later): `DWMWA_SYSTEMBACKDROP_TYPE`, Acrylic for `Blur`
//!   and Mica for `Tinted`, over a client area extended under the frame.
//!   Linux: `Blur` asked of the compositor (`mod linux_blur`).
//! - **The fallback**, where the OS has no effect to give — GNOME, a
//!   Linux compositor with no blur protocol, Windows 10: the wallpaper,
//!   read and blurred once on a thread and drawn by kui as the window's
//!   ground (`mod ground`), reported as `Tinted`. Whether the desktop
//!   names one is asked on the loop where that is one call (Windows,
//!   Plasma), so a window with none is `Opaque` from its first frame; on
//!   GNOME the thread finds it (`gsettings`), and the window is `Tinted`
//!   until the frame it says there is none. One that will not decode is
//!   `Opaque` from that frame everywhere (backlog RG150, RG154).
//!   `KUI_BACKDROP_EMULATE=1` takes that path for `Blur` and
//!   `Tinted` on Windows and Linux, to see it where the OS would do
//!   better (macOS reads no wallpaper, so it is `Opaque` there).
//!
//! What came of it is the pane's, and every frame's `env.window.backdrop`,
//! so a view paints its translucent regions only over something.

use std::any::Any;
use std::sync::Arc;

use kui_core::Backdrop;
use winit::window::{Window, WindowAttributes};

/// What [`apply`] got for a window.
pub(crate) struct Applied {
    /// What `env.window.backdrop` reports.
    pub got: Backdrop,
    /// The desktop's wallpaper kui draws as the window's ground, where the
    /// effect is kui's own (`mod ground`): the file the loop named, or one
    /// the thread finds, read on that thread once the pane exists; `got`
    /// is corrected to `Opaque` on the frame the thread finds none, or
    /// one that will not decode.
    pub ground: Option<crate::ground::Source>,
    /// What must live as long as the window for the effect to stay (a
    /// Linux compositor's blur objects).
    pub keep: Option<Box<dyn Any>>,
}

impl Applied {
    fn just(got: Backdrop) -> Self {
        Applied {
            got,
            ground: None,
            keep: None,
        }
    }

    /// The wallpaper drawn by kui, or the opaque window where the desktop
    /// names none. Whether it names one is asked here, on the event loop,
    /// where that is one call (`ground::known_path`: Windows, Plasma,
    /// macOS's never) — so a window with none to draw is `Opaque` from
    /// its first frame — and on the loader's thread where it is a process
    /// (GNOME's `gsettings`): there the window is `Tinted` until the
    /// thread answers, and `Opaque` from the frame it says there is none,
    /// the path a wallpaper that will not decode takes everywhere.
    fn emulated() -> Self {
        use crate::ground::Source;
        let source = match crate::ground::known_path() {
            Some(None) => return Applied::just(Backdrop::Opaque),
            Some(Some(path)) => Source::Load(path),
            None => Source::Find,
        };
        Applied {
            got: Backdrop::Tinted,
            ground: Some(source),
            keep: None,
        }
    }
}

/// `KUI_BACKDROP_EMULATE`: the wallpaper kui draws, for `Blur` and
/// `Tinted`, wherever kui can read one (not macOS).
fn emulate_forced() -> bool {
    std::env::var_os("KUI_BACKDROP_EMULATE").is_some_and(|v| !v.is_empty() && v != "0")
}

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
/// (`Renderer::new_in_with`) — and returns what came of it. Called once,
/// before the window is shown.
pub(crate) fn apply(
    window: &Arc<Window>,
    renderer: &kui_wgpu::Renderer,
    backdrop: Backdrop,
) -> Applied {
    match backdrop {
        Backdrop::Opaque => Applied::just(Backdrop::Opaque),
        Backdrop::Transparent => Applied::just(if see_through(window, renderer) {
            Backdrop::Transparent
        } else {
            // Nothing the frame paints with alpha would reach the screen
            // as alpha.
            Backdrop::Opaque
        }),
        Backdrop::Blur | Backdrop::Tinted if emulate_forced() => Applied::emulated(),
        Backdrop::Blur | Backdrop::Tinted => effect(window, renderer, backdrop),
    }
}

/// Whether the window's translucent pixels show what is behind it: the
/// surface takes alpha, and on X11 a compositor runs.
fn see_through(window: &Window, renderer: &kui_wgpu::Renderer) -> bool {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        renderer.transparent() && crate::linux_blur::composited(window)
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let _ = window;
        renderer.transparent()
    }
}

/// macOS: an `NSVisualEffectView` under the content, and the wallpaper
/// kui draws (none can be read there, so the opaque window) where the
/// window has no AppKit view to put one under.
#[cfg(target_os = "macos")]
fn effect(window: &Window, renderer: &kui_wgpu::Renderer, backdrop: Backdrop) -> Applied {
    if renderer.transparent() && macos::install(window, backdrop) {
        Applied::just(backdrop)
    } else {
        Applied::emulated()
    }
}

/// Windows 11: the system backdrop, in a client area extended under the
/// frame so it is behind all of it; a Windows that refuses the attribute
/// (10, and 11 before 22H2) gets the wallpaper kui draws.
#[cfg(target_os = "windows")]
fn effect(window: &Window, renderer: &kui_wgpu::Renderer, backdrop: Backdrop) -> Applied {
    if renderer.transparent() && windows::system_backdrop(window, backdrop) {
        Applied::just(backdrop)
    } else {
        Applied::emulated()
    }
}

/// Linux: `Blur` asked of the compositor where it can be; everything else
/// the wallpaper kui draws, `Tinted`.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn effect(window: &Window, renderer: &kui_wgpu::Renderer, backdrop: Backdrop) -> Applied {
    if backdrop == Backdrop::Blur
        && see_through(window, renderer)
        && let Some(keep) = crate::linux_blur::request(window)
    {
        return Applied {
            got: Backdrop::Blur,
            ground: None,
            keep: Some(keep),
        };
    }
    Applied::emulated()
}

#[cfg(target_os = "windows")]
mod windows {
    use kui_core::Backdrop;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMSBT_MAINWINDOW, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
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
    /// window on Windows 11 with a white at 45% reading as a flat grey.
    /// Only where the device will present through a visual
    /// (`kui_wgpu::see_through_by_visual`, which decides that too): a
    /// window with no surface of its own and a swapchain on its handle
    /// draws nothing. Where it will not, the window keeps its surface, the
    /// renderer is opaque, and [`super::apply`] reports what that leaves.
    pub(super) fn attrs(attrs: WindowAttributes) -> WindowAttributes {
        use winit::platform::windows::WindowAttributesExtWindows;
        if !kui_wgpu::see_through_by_visual() {
            return attrs;
        }
        attrs.with_no_redirection_bitmap(true)
    }

    /// The `DWM_SYSTEMBACKDROP_TYPE` an effect is: Acrylic, which blurs
    /// what is behind the window live, for `Blur`; Mica, which takes the
    /// wallpaper's colour and stays put, for `Tinted`.
    pub(super) fn backdrop_type(backdrop: Backdrop) -> Option<i32> {
        match backdrop {
            Backdrop::Blur => Some(DWMSBT_TRANSIENTWINDOW),
            Backdrop::Tinted => Some(DWMSBT_MAINWINDOW),
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
    use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
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

    /// The material an effect is: the sidebar's, the most see-through of
    /// AppKit's behind-window materials, for `Blur`; the window
    /// background's — what AppKit's wallpaper tinting draws behind a
    /// window, the desktop's colour more than its shapes — for `Tinted`.
    /// kui's choice: the API names the effect, never the material.
    pub(super) fn material(backdrop: Backdrop) -> Option<NSVisualEffectMaterial> {
        match backdrop {
            Backdrop::Blur => Some(NSVisualEffectMaterial::Sidebar),
            Backdrop::Tinted => Some(NSVisualEffectMaterial::WindowBackground),
            Backdrop::Opaque | Backdrop::Transparent => None,
        }
    }

    /// Adds the material under `window`'s content: an effect view the
    /// size of winit's content view, resized with it, blending what is
    /// behind the window and following its active state (dimmed in a
    /// window in the background). Its layer sits under the CAMetalLayer
    /// wgpu added to the same view — the order AppKit gives a subview's
    /// layer among sublayers it did not add is not one to rely on. False
    /// when the window has no AppKit view.
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
        for b in [Backdrop::Transparent, Backdrop::Blur, Backdrop::Tinted] {
            assert_eq!(
                attrs(Window::default_attributes(), b).transparent,
                !cfg!(target_os = "windows")
            );
        }
    }

    /// The fallback is honest: the wallpaper reads `Tinted` only where a
    /// wallpaper is being read to draw (and is corrected on the frame the
    /// thread finds none), and the opaque window where none can be — from
    /// the first frame, where the desktop could be asked on the loop
    /// (backlog RG154).
    #[test]
    fn the_fallback_reports_what_it_draws() {
        use crate::ground::Source;
        let got = Applied::emulated();
        assert_eq!(got.got == Backdrop::Tinted, got.ground.is_some());
        match crate::ground::known_path() {
            Some(None) => assert_eq!(got.got, Backdrop::Opaque),
            Some(Some(path)) => assert_eq!(got.ground, Some(Source::Load(path))),
            None => assert_eq!(got.ground, Some(Source::Find)),
        }
        if cfg!(target_os = "macos") {
            assert_eq!(got.got, Backdrop::Opaque, "never read on macOS");
        }
        if cfg!(target_os = "windows") {
            assert!(
                crate::ground::known_path().is_some(),
                "Windows answers on the loop"
            );
        }
        assert!(got.keep.is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn each_effect_is_a_system_backdrop_type() {
        use windows_sys::Win32::Graphics::Dwm::{DWMSBT_MAINWINDOW, DWMSBT_TRANSIENTWINDOW};
        assert_eq!(
            windows::backdrop_type(Backdrop::Blur),
            Some(DWMSBT_TRANSIENTWINDOW)
        );
        assert_eq!(
            windows::backdrop_type(Backdrop::Tinted),
            Some(DWMSBT_MAINWINDOW)
        );
        assert_eq!(windows::backdrop_type(Backdrop::Transparent), None);
        assert_eq!(windows::backdrop_type(Backdrop::Opaque), None);
    }
}
