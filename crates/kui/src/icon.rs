//! The picture the OS shows for the app's windows (backlog F86): the
//! launcher's [`icon`](crate::Launcher::icon) and
//! [`icon_resource`](crate::Launcher::icon_resource), made into winit
//! icons once and handed to every window as it is created.
//!
//! Where it is seen: on Windows the title bar, Alt-Tab and the taskbar —
//! a program's icon resource is what Explorer draws for the file, but
//! winit registers its window class with no icon, so a window shows the
//! platform's default until it is given one; on X11 the window manager's
//! `_NET_WM_ICON`. Wayland finds an app's icon through its `.desktop`
//! file and macOS through the bundle's `.icns` (the Dock's — a macOS
//! window has none of its own), and winit ignores a window icon on both.

use winit::window::{Icon, WindowAttributes};

/// What every window of the app is created with; empty unless the
/// launcher was given an icon.
#[derive(Clone, Default)]
pub(crate) struct AppIcon {
    /// The title bar's — Windows' `ICON_SMALL` — and X11's one picture.
    window: Option<Icon>,
    /// Windows' `ICON_BIG`: Alt-Tab and the taskbar.
    #[cfg(target_os = "windows")]
    taskbar: Option<Icon>,
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
                Ok((window, taskbar)) => {
                    return AppIcon {
                        window: Some(window),
                        taskbar: Some(taskbar),
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
            #[cfg(target_os = "windows")]
            taskbar: rgba.clone(),
            window: rgba,
        }
    }

    /// `attrs` with the icons set, for `Shell::window_attrs`.
    pub(crate) fn apply(&self, attrs: WindowAttributes) -> WindowAttributes {
        #[allow(unused_mut)]
        let mut attrs = attrs.with_window_icon(self.window.clone());
        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            attrs = attrs.with_taskbar_icon(self.taskbar.clone());
        }
        attrs
    }
}

/// The small and the large icon from the executable's resource `id`, each
/// at the size the system draws it — the frame of the `.ico` made for it
/// rather than one frame scaled.
#[cfg(target_os = "windows")]
fn from_resource(id: u16) -> Result<(Icon, Icon), winit::window::BadIcon> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON, SM_CYSMICON};
    use winit::dpi::PhysicalSize;
    use winit::platform::windows::IconExtWindows;
    // SAFETY: a plain query with no pointers.
    let (w, h) = unsafe { (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON)) };
    let small = (w > 0 && h > 0).then(|| PhysicalSize::new(w as u32, h as u32));
    // `None` is the system's large size (`SM_CXICON`).
    Ok((
        Icon::from_resource(id, small)?,
        Icon::from_resource(id, None)?,
    ))
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
        }
    }
}
