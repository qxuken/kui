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
//! file, and winit ignores a window icon there.
//!
//! A macOS window has no icon of its own: the app has one, in the Dock
//! and ⌘Tab, which AppKit takes from the bundle's `.icns` — and a process
//! that is no bundle (a Node app run with `node`, a `cargo run`) shows its
//! executable's, Node's or the generic one. So on macOS the pixels are the
//! *app's* icon (backlog F181): `NSApp.applicationIconImage`, set once the
//! first window is open ([`AppIcon::show_in_dock`]), unless the main bundle
//! names an icon of its own, which wins the way Windows' resource does —
//! an `.icns` carries a frame drawn for each size, and the Finder shows it
//! whatever the Dock does. Only while the process runs: a tile kept in the
//! Dock is the executable's again once it quits. The name beside it can
//! not be set the same way — the menu bar, the Dock's label and
//! `NSRunningApplication` read the executable's however the process names
//! itself — so there is no such option.
//!
//! Which `.desktop` file is the app's is the launcher's
//! [`app_id`](crate::Launcher::app_id) (backlog F172): the Wayland
//! `app_id` and the X11 `WM_CLASS` every window is created with, which a
//! compositor or a window manager matches to the file's name or its
//! `StartupWMClass`. Without one a Wayland window has no `app_id` at all
//! and is drawn with the compositor's generic icon.

#[cfg(target_os = "macos")]
use std::sync::Arc;

use winit::window::{Icon, WindowAttributes};

/// The launcher's pixels, checked: winit's icon for the windows and, on
/// macOS, the pixels themselves for the Dock — winit keeps an icon's
/// pixels private, and its macOS backend does nothing with one.
#[derive(Clone, Debug)]
pub(crate) struct Pixels {
    window: Icon,
    #[cfg(target_os = "macos")]
    dock: DockPixels,
}

/// What `NSApp.applicationIconImage` is made from: RGBA, straight alpha.
#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
struct DockPixels {
    rgba: Arc<[u8]>,
    width: u32,
    height: u32,
}

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
    /// The app's icon in the Dock and ⌘Tab ([`AppIcon::show_in_dock`]).
    #[cfg(target_os = "macos")]
    dock: Option<DockPixels>,
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
pub(crate) fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Pixels, String> {
    if width == 0 || height == 0 {
        return Err(format!(
            "an icon is at least one pixel, not {width}x{height}"
        ));
    }
    #[cfg(target_os = "macos")]
    let dock = DockPixels {
        rgba: rgba.as_slice().into(),
        width,
        height,
    };
    Ok(Pixels {
        window: Icon::from_rgba(rgba, width, height).map_err(|e| format!("icon: {e}"))?,
        #[cfg(target_os = "macos")]
        dock,
    })
}

impl AppIcon {
    /// The icons for `rgba` and, on Windows, the executable's icon
    /// resource `resource` — which wins there, since an `.ico` carries a
    /// frame drawn for each size and the title bar and the taskbar each
    /// load the one theirs is. A resource the executable does not have is
    /// said once on stderr and the pixels are used instead. Elsewhere the
    /// resource is nothing.
    pub(crate) fn new(rgba: Option<Pixels>, resource: Option<u16>) -> Self {
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
            taskbar: rgba.as_ref().map(|p| p.window.clone()),
            #[cfg(target_os = "windows")]
            resource: None,
            #[cfg(target_os = "macos")]
            dock: rgba.as_ref().map(|p| p.dock.clone()),
            window: rgba.map(|p| p.window),
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

    /// Makes the pixels the app's icon in the Dock and ⌘Tab, on macOS,
    /// unless the main bundle names an icon of its own; nothing without
    /// pixels, so a second runner with none leaves the first's. Called
    /// once a runner's first window is open: the app has finished
    /// launching by then, and is a regular app with a Dock tile.
    pub(crate) fn show_in_dock(&self) {
        #[cfg(target_os = "macos")]
        if let Some(px) = &self.dock
            && !bundle_has_icon()
        {
            set_dock_icon(px);
        }
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

/// Whether the main bundle's `Info.plist` names the app's icon — a
/// `CFBundleIconFile` (an `.icns`) or a `CFBundleIconName` (an asset
/// catalog's). A process that is no bundle has no `Info.plist`, unless
/// one is linked into its `__info_plist` section, and names none.
#[cfg(target_os = "macos")]
fn bundle_has_icon() -> bool {
    use objc2_foundation::{NSBundle, NSString};
    let bundle = NSBundle::mainBundle();
    ["CFBundleIconFile", "CFBundleIconName"].iter().any(|key| {
        bundle
            .objectForInfoDictionaryKey(&NSString::from_str(key))
            .is_some()
    })
}

/// `px` as `NSApp.applicationIconImage`, which the Dock scales to its
/// tile and ⌘Tab to its own.
#[cfg(target_os = "macos")]
fn set_dock_icon(px: &DockPixels) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    let (Some(mtm), Some(image)) = (MainThreadMarker::new(), dock_image(px)) else {
        return;
    };
    // SAFETY: an image, not nil, on the main thread.
    unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
}

/// `px` as an image of one bitmap rep, as many points as it has pixels;
/// `None` when AppKit will not make the rep.
#[cfg(target_os = "macos")]
fn dock_image(px: &DockPixels) -> Option<objc2::rc::Retained<objc2_app_kit::NSImage>> {
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSBitmapFormat, NSBitmapImageRep, NSDeviceRGBColorSpace, NSImage};
    use objc2_foundation::NSSize;
    let (w, h) = (px.width as isize, px.height as isize);
    // SAFETY: no planes, so AppKit allocates the buffer itself — four
    // 8-bit samples a pixel, `w * 4` bytes a row, straight alpha, as the
    // launcher's pixels are.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            w,
            h,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            NSBitmapFormat::AlphaNonpremultiplied,
            w * 4,
            32,
        )
    }?;
    let data = rep.bitmapData();
    if data.is_null() || rep.bytesPerRow() != w * 4 {
        return None;
    }
    // SAFETY: the rep's own buffer, `bytesPerRow * h` bytes — the length
    // of the launcher's pixels, which `from_rgba` checked.
    unsafe { std::ptr::copy_nonoverlapping(px.rgba.as_ptr(), data, px.rgba.len()) };
    let image = NSImage::initWithSize(
        NSImage::alloc(),
        NSSize::new(px.width as f64, px.height as f64),
    );
    image.addRepresentation(&rep);
    Some(image)
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

    /// On macOS the pixels are the app's Dock icon (backlog F181): kept
    /// whole beside winit's icon, and made into an image of that many
    /// points whose one rep holds them byte for byte, straight alpha. A
    /// test binary is no bundle, so it names no icon to keep instead.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_pixels_are_the_dock_s_icon_on_macos() {
        use objc2_app_kit::NSBitmapImageRep;
        let rgba: Vec<u8> = (0..3 * 2 * 4).map(|i| (i * 10) as u8).collect();
        let set = AppIcon::new(Some(from_rgba(rgba.clone(), 3, 2).unwrap()), None);
        let px = set.dock.as_ref().expect("the dock's pixels");
        assert_eq!((&px.rgba[..], px.width, px.height), (&rgba[..], 3, 2));
        assert!(AppIcon::new(None, None).dock.is_none());
        assert!(!bundle_has_icon());

        let image = dock_image(px).expect("an image");
        let size = image.size();
        assert_eq!((size.width, size.height), (3.0, 2.0));
        let reps = image.representations();
        assert_eq!(reps.len(), 1);
        let rep = reps
            .objectAtIndex(0)
            .downcast::<NSBitmapImageRep>()
            .unwrap();
        assert_eq!((rep.pixelsWide(), rep.pixelsHigh()), (3, 2));
        // SAFETY: the rep's buffer, 3 × 2 pixels of four bytes, unpadded
        // (`dock_image` refuses a rep that pads its rows).
        let bytes = unsafe { std::slice::from_raw_parts(rep.bitmapData(), rgba.len()) };
        assert_eq!(bytes, &rgba[..]);
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
