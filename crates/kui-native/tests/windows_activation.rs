//! Cloaking an opening window must preserve its requested activation once
//! content is presented. Run on an interactive Windows desktop.
#[cfg(windows)]
fn main() {
    use kui_native::{App, Color, NodeSpec, Ui, Waker, WindowCommand, WindowId};
    use std::{
        process::{Command, Stdio},
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, GetForegroundWindow};

    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--child") {
        struct Child;
        impl App for Child {
            fn view(&mut self, ui: &mut Ui<'_>) {
                ui.leaf(NodeSpec::column().fill().bg(Color::hex(0x303030ff)));
            }
        }
        let mut child = kui_native::app("kui startup activation child")
            .size(240.0, 160.0)
            .open(Child)
            .unwrap();
        let until = Instant::now() + Duration::from_millis(300);
        while Instant::now() < until {
            child.pump_until(until);
        }
        let title: Vec<u16> = "kui startup activation child\0".encode_utf16().collect();
        let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        assert_ne!(hwnd, 0);
        assert_eq!(
            unsafe { GetForegroundWindow() },
            hwnd,
            "the newly presented window must activate without unrelated input"
        );
        assert_eq!(
            std::fs::read_to_string(&args[2]).unwrap(),
            "false",
            "the previous window received the focus loss"
        );
        child.request_exit();
        child.pump();
        println!("presented window activated without unrelated input");
        return;
    }

    struct Probe {
        state: std::path::PathBuf,
        output: Arc<Mutex<Option<std::process::Output>>>,
    }
    impl App for Probe {
        fn setup(&mut self, waker: Waker) {
            let state = self.state.clone();
            let output = self.output.clone();
            std::thread::spawn(move || {
                let title: Vec<u16> = "kui startup activation parent\0".encode_utf16().collect();
                loop {
                    if unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) } != 0 {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                std::thread::sleep(Duration::from_millis(500));
                let child = Command::new(std::env::current_exe().unwrap())
                    .arg("--child")
                    .arg(state)
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                *output.lock().unwrap() = Some(child.wait_with_output().unwrap());
                waker.wake();
            });
        }
        fn view(&mut self, ui: &mut Ui<'_>) {
            std::fs::write(&self.state, ui.env().focused.to_string()).unwrap();
            ui.leaf(NodeSpec::column().fill().bg(Color::hex(0x202020ff)));
            if self.output.lock().unwrap().is_some() {
                ui.window_command(WindowCommand::Close(WindowId::MAIN));
            }
        }
    }
    let state = tempfile::NamedTempFile::new().unwrap();
    let output = Arc::new(Mutex::new(None));
    kui_native::app("kui startup activation parent")
        .custom_titlebar()
        .size(320.0, 200.0)
        .run(Probe {
            state: state.path().to_owned(),
            output: output.clone(),
        })
        .unwrap();
    let output = output.lock().unwrap().take().unwrap();
    println!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "opening must preserve requested activation"
    );
}

#[cfg(not(windows))]
fn main() {}
