//! Real HWND checks: run on an interactive Windows desktop. Winit needs
//! the main thread, hence this test's `harness = false`.
#[cfg(windows)]
fn main() {
    use kui_native::{App, Chrome, Color, NodeSpec, Rect, Ui, WindowConfig};
    use std::collections::HashMap;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible,
    };

    fn own_window(exclude: &[HWND]) -> HWND {
        struct Search<'a> {
            exclude: &'a [HWND],
            found: HWND,
        }
        unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> i32 {
            // SAFETY: enumeration is synchronous and Search lives for the call.
            let search = unsafe { &mut *(data as *mut Search<'_>) };
            let mut pid = 0;
            unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
            let mut class = [0u16; 64];
            let n = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
            // Exclude winit's event target and the IME's helper windows.
            if pid == std::process::id()
                && !search.exclude.contains(&hwnd)
                && String::from_utf16_lossy(&class[..n as usize]) == "Window Class"
            {
                search.found = hwnd;
                return 0;
            }
            1
        }
        let mut search = Search { exclude, found: 0 };
        unsafe { EnumWindows(Some(each), &mut search as *mut Search<'_> as LPARAM) };
        assert_ne!(search.found, 0, "the runner created an HWND");
        search.found
    }

    fn cloaked(hwnd: HWND) -> bool {
        let mut value: u32 = 0;
        let hr = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED as u32,
                (&mut value as *mut u32).cast(),
                std::mem::size_of::<u32>() as u32,
            )
        };
        assert_eq!(hr, 0, "DWM answered the cloak state");
        value != 0
    }

    #[derive(Default)]
    struct Probe {
        before_first_render: HashMap<String, (HWND, bool)>,
        popup: bool,
    }
    impl App for Probe {
        fn view(&mut self, ui: &mut Ui<'_>) {
            let name = ui.window_name().to_string();
            if !self.before_first_render.contains_key(&name) {
                let exclude: Vec<_> = self.before_first_render.values().map(|v| v.0).collect();
                let hwnd = own_window(&exclude);
                self.before_first_render.insert(
                    name.clone(),
                    (hwnd, unsafe { IsWindowVisible(hwnd) } == 0 || cloaked(hwnd)),
                );
            }
            if name == "main" {
                let config = if self.popup {
                    WindowConfig::popup(Rect::new(10.0, 10.0, 40.0, 20.0), 160.0, 100.0)
                } else {
                    WindowConfig::sized(160.0, 100.0)
                };
                ui.window("child", config);
            }
            ui.leaf(NodeSpec::column().fill().bg(Color::hex(0x202020ff)));
        }
    }

    for chrome in [Chrome::Native, Chrome::Custom, Chrome::Borderless] {
        for popup in [false, true] {
            let mut runner = kui_native::app("kui lifecycle probe")
                .size(320.0, 200.0)
                .chrome(chrome)
                .open(Probe {
                    popup,
                    ..Default::default()
                })
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while runner.app_mut().before_first_render.len() < 2
                && std::time::Instant::now() < deadline
            {
                runner.pump();
            }
            assert_eq!(
                runner.app_mut().before_first_render.len(),
                2,
                "both windows rendered"
            );
            let snapshots = runner.app_mut().before_first_render.clone();
            for (name, (hwnd, hidden)) in snapshots {
                assert!(
                    hidden,
                    "{chrome:?}/{name}: no empty surface before the first render"
                );
                while cloaked(hwnd) && std::time::Instant::now() < deadline {
                    runner.pump();
                }
                assert!(
                    !cloaked(hwnd),
                    "{chrome:?}/{name}: presented content becomes visible"
                );
                assert_ne!(unsafe { IsWindowVisible(hwnd) }, 0);
            }
            runner.request_exit();
            runner.pump();
            println!(
                "{chrome:?}, popup={popup}: both windows hidden before rendering, visible after presentation"
            );
        }
    }
}

#[cfg(not(windows))]
fn main() {}
