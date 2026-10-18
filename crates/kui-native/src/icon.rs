//! The picture the OS shows for the app's windows: the
//! launcher's [`icon`](crate::Launcher::icon) and
//! [`icon_resource`](crate::Launcher::icon_resource), made into icons once
//! and handed to every window as it is created: the pixels as winit's,
//! the resource as the executable's own `HICON`s (RG41).
//!
//! Where it is seen: on Windows the title bar, Alt-Tab and the taskbar —
//! a program's icon resource is what Explorer draws for the file, but
//! winit registers its window class with no icon, so a window shows the
//! platform's default until it is given one; on X11 the window manager's
//! `_NET_WM_ICON`. Wayland finds an app's icon through its `.desktop`
//! file and macOS through the bundle's `.icns` (the Dock's — a macOS
//! window has none of its own), and winit ignores a window icon on both.
//!
//! Which `.desktop` file is the app's is the launcher's
//! [`app_id`](crate::Launcher::app_id) (backlog F172): the Wayland
//! `app_id` and the X11 `WM_CLASS` every window is created with, which a
//! compositor or a window manager matches to the file's name or its
//! `StartupWMClass`. Without one a Wayland window has no `app_id` at all
//! and is drawn with the compositor's generic icon.

use winit::window::{Icon, WindowAttributes};

/// What every window of the app is created with; empty unless the
/// launcher was given an icon or an app id.
#[derive(Clone, Default)]
pub(crate) struct AppIcon {
    /// The Wayland `app_id` and X11 `WM_CLASS`, both halves
    /// ([`Launcher::app_id`](crate::Launcher::app_id)); nothing elsewhere.
    app_id: Option<String>,
    /// The title bar's — Windows' `ICON_SMALL` — and X11's one picture.
    window: Option<Icon>,
    /// Windows' `ICON_BIG`: Alt-Tab and the taskbar.
    #[cfg(target_os = "windows")]
    taskbar: Option<Icon>,
    /// The executable's icon resource, loaded from the process's own
    /// module and sent to each window once it exists ([`AppIcon::set_on`]).
    /// Not a winit icon: winit loads a resource from the module winit is
    /// linked into, which is the program only when kui is linked into it
    /// — under Node it is `kui_node.dll` and for a C host `kui_ffi.dll`,
    /// neither with a resource section, so their windows fell back to the
    /// pixels however plainly `node.exe` or the host carried the icon.
    #[cfg(target_os = "windows")]
    resource: Option<ResourceIcons>,
}

/// The small and the large icon loaded from the executable, each at the
/// size the system draws it — the frame of the `.ico` made for it rather
/// than one frame scaled. Loaded once and never freed: every window of the
/// process shows them until it ends.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct ResourceIcons {
    small: windows_sys::Win32::UI::WindowsAndMessaging::HICON,
    big: windows_sys::Win32::UI::WindowsAndMessaging::HICON,
}

/// The launcher's pixels as an icon, refused with the reason when they are
/// not `width` × `height` RGBA — what `Launcher::try_icon` returns.
pub(crate) fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Icon, String> {
    if width == 0 || height == 0 {
        return Err(format!(
            "an icon is at least one pixel, not {width}x{height}"
        ));
    }
    Icon::from_rgba(rgba, width, height).map_err(|e| format!("icon: {e}"))
}

impl AppIcon {
    /// The icons for `rgba` and, on Windows, the executable's icon
    /// resource `resource` — which wins there, since an `.ico` carries a
    /// frame drawn for each size and the title bar and the taskbar each
    /// load the one theirs is. A resource the executable does not have is
    /// said once on stderr and the pixels are used instead. Elsewhere the
    /// resource is nothing.
    pub(crate) fn new(rgba: Option<Icon>, resource: Option<u16>) -> Self {
        #[cfg(target_os = "windows")]
        if let Some(id) = resource {
            match from_resource(id) {
                Ok(icons) => {
                    return AppIcon {
                        app_id: None,
                        window: None,
                        taskbar: None,
                        resource: Some(icons),
                    };
                }
                Err(why) => eprintln!(
                    "kui: icon resource {id} is not in the executable ({why}); {}",
                    if rgba.is_some() {
                        "the icon's pixels are used"
                    } else {
                        "the windows keep the default icon"
                    }
                ),
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = resource;
        AppIcon {
            app_id: None,
            #[cfg(target_os = "windows")]
            taskbar: rgba.clone(),
            #[cfg(target_os = "windows")]
            resource: None,
            window: rgba,
        }
    }

    /// These icons under the app id `id`, for every window; `None` is
    /// none.
    pub(crate) fn with_app_id(mut self, id: Option<String>) -> Self {
        self.app_id = id;
        self
    }

    /// Gives `window` the executable's icons, on Windows, when the
    /// launcher named a resource the executable has; nothing otherwise
    /// (the pixels went in with [`AppIcon::apply`]). Called by each place
    /// a window is created, right after it is.
    pub(crate) fn set_on(&self, window: &winit::window::Window) {
        #[cfg(target_os = "windows")]
        if let Some(icons) = self.resource {
            use windows_sys::Win32::Foundation::HWND;
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                ICON_BIG, ICON_SMALL, SendMessageW, WM_SETICON,
            };
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let Some(RawWindowHandle::Win32(h)) = window.window_handle().ok().map(|h| h.as_raw())
            else {
                return;
            };
            let hwnd = h.hwnd.get() as HWND;
            // SAFETY: a window this thread made, and icons that live as
            // long as the process.
            unsafe {
                SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, icons.small);
                SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, icons.big);
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = window;
    }

    /// `attrs` with the icons and the app id set, for
    /// `Shell::window_attrs`.
    pub(crate) fn apply(&self, attrs: WindowAttributes) -> WindowAttributes {
        #[allow(unused_mut)]
        let mut attrs = attrs.with_window_icon(self.window.clone());
        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            attrs = attrs.with_taskbar_icon(self.taskbar.clone());
        }
        // One name in winit for both backends: Wayland's `app_id` is its
        // `general` half and X11's `WM_CLASS` both, instance then class.
        #[cfg(all(unix, not(target_os = "macos")))]
        if let Some(id) = &self.app_id {
            use winit::platform::x11::WindowAttributesExtX11;
            attrs = attrs.with_name(id.as_str(), id.as_str());
        }
        #[cfg(not(all(unix, not(target_os = "macos"))))]
        let _ = &self.app_id;
        attrs
    }
}

/// The small and the large icon from the executable's resource `id`: the
/// process's own module (`GetModuleHandleW(NULL)`), whatever module kui is
/// linked into.
#[cfg(target_os = "windows")]
fn from_resource(id: u16) -> Result<ResourceIcons, std::io::Error> {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, SM_CXICON, SM_CXSMICON,
        SM_CYICON, SM_CYSMICON,
    };
    // SAFETY: NULL names the executable; the id is `MAKEINTRESOURCEW`'s
    // integer in a pointer, never read through; the sizes are queries.
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        let load = |w: i32, h: i32| {
            let icon = LoadImageW(
                module,
                id as usize as *const u16,
                IMAGE_ICON,
                w,
                h,
                LR_DEFAULTCOLOR,
            );
            if icon == 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(icon)
            }
        };
        Ok(ResourceIcons {
            small: load(GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON))?,
            big: load(GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CYICON))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_that_are_not_the_size_are_refused_with_the_reason() {
        assert!(from_rgba(vec![0; 2 * 2 * 4], 2, 2).is_ok());
        let short = from_rgba(vec![0; 2 * 2 * 4 - 4], 2, 2).unwrap_err();
        assert!(
            short.contains("2x2") || short.contains("dimensions"),
            "{short}"
        );
        let ragged = from_rgba(vec![0; 7], 1, 1).unwrap_err();
        assert!(ragged.contains("divisible by 4"), "{ragged}");
        let empty = from_rgba(Vec::new(), 0, 0).unwrap_err();
        assert!(empty.contains("at least one pixel"), "{empty}");
    }

    #[test]
    fn every_window_is_created_with_the_launcher_s_icon_and_none_without() {
        let attrs = || winit::window::Window::default_attributes();
        assert!(AppIcon::default().apply(attrs()).window_icon.is_none());
        let icon = from_rgba(vec![255; 4 * 4 * 4], 4, 4).unwrap();
        // A resource is Windows'; elsewhere the pixels are the icon whatever
        // id came with them.
        #[cfg(not(target_os = "windows"))]
        {
            let set = AppIcon::new(Some(icon), Some(1));
            assert!(set.apply(attrs()).window_icon.is_some());
            assert!(
                set.apply(attrs()).window_icon.is_some(),
                "and again for the next window"
            );
            assert!(
                AppIcon::new(None, Some(1))
                    .apply(attrs())
                    .window_icon
                    .is_none()
            );
        }
        #[cfg(target_os = "windows")]
        {
            // A test binary links no icon resource, so id 1 falls back.
            let set = AppIcon::new(Some(icon), Some(1));
            assert!(set.apply(attrs()).window_icon.is_some());
            assert!(set.resource.is_none());
            // Looked up in the executable — this test binary — and not in
            // a module of kui's (RG41): refused with the OS's reason.
            let why = from_resource(1)
                .err()
                .expect("no resource in a test binary");
            assert!(why.raw_os_error().is_some(), "{why}");
        }
    }

    /// The app id reaches every window's attributes as winit's name, both
    /// halves, and no id leaves winit's default (backlog F172). winit keeps
    /// the name crate-private, so its `Debug` is what the test can read.
    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn every_window_is_created_under_the_app_id() {
        let attrs = || winit::window::Window::default_attributes();
        let named = AppIcon::default().with_app_id(Some("kawoosh".into()));
        for _ in 0..2 {
            let dbg = format!("{:?}", named.apply(attrs()));
            assert!(
                dbg.contains(
                    r#"name: Some(ApplicationName { general: "kawoosh", instance: "kawoosh" })"#
                ),
                "{dbg}"
            );
        }
        let dbg = format!("{:?}", AppIcon::default().apply(attrs()));
        assert!(dbg.contains("name: None"), "{dbg}");
    }
}
