//! A packed macOS app starts under LaunchServices, without the environment
//! a terminal's login shell built. Discover it once in the background; the
//! first process the app starts waits if the shell is still starting.

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{CStr, OsString};
    use std::io::Read;
    use std::os::unix::ffi::OsStringExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::{Arc, Condvar, Mutex, OnceLock};
    use std::time::{Duration, Instant};

    const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
    const PATH_MARK: &[u8] = b"__KUI_PATH__";
    const SHELL_MARK: &[u8] = b"\0__KUI_SHELL__";

    #[derive(Clone, Default)]
    struct Environment {
        path: Option<OsString>,
        shell: Option<OsString>,
    }

    #[derive(Default)]
    struct State {
        done: bool,
        environment: Environment,
    }

    #[derive(Default)]
    struct Discovery {
        state: Mutex<State>,
        ready: Condvar,
    }

    static DISCOVERY: OnceLock<Arc<Discovery>> = OnceLock::new();

    pub fn start() {
        DISCOVERY.get_or_init(|| {
            let discovery = Arc::new(Discovery::default());
            let background = discovery.clone();
            std::thread::spawn(move || {
                let environment = discover();
                let mut state = background.state.lock().unwrap();
                state.environment = environment;
                state.done = true;
                background.ready.notify_all();
            });
            discovery
        });
    }

    pub fn apply(command: &mut Command) {
        let Some(discovery) = DISCOVERY.get() else {
            return;
        };
        let mut state = discovery.state.lock().unwrap();
        while !state.done {
            state = discovery.ready.wait(state).unwrap();
        }
        if let Some(path) = &state.environment.path {
            command.env("PATH", path);
        }
        if let Some(shell) = &state.environment.shell {
            command.env("SHELL", shell);
        }
    }

    fn discover() -> Environment {
        let shell = std::env::var_os("SHELL")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(login_shell);
        let mut environment = Environment {
            path: None,
            shell: shell.as_ref().map(|s| s.as_os_str().to_owned()),
        };
        let Some(shell) = shell else {
            return environment;
        };
        let Some(kind) = Kind::from_path(&shell) else {
            return environment;
        };
        if let Some(output) = probe_output(&shell, kind, PROBE_TIMEOUT) {
            let probed = parse(&output);
            if probed.path.as_ref().is_some_and(|p| !p.is_empty()) {
                environment.path = probed.path;
            }
            if probed.shell.as_ref().is_some_and(|s| !s.is_empty()) {
                environment.shell = probed.shell;
            }
        }
        environment
    }

    fn login_shell() -> Option<PathBuf> {
        let mut passwd: libc::passwd = unsafe { std::mem::zeroed() };
        let mut result = std::ptr::null_mut();
        let mut buffer = vec![0; 16 * 1024];
        // SAFETY: `passwd` and `buffer` are writable for the duration of the
        // call, and `result` is checked before the returned pointer is read.
        let code = unsafe {
            libc::getpwuid_r(
                libc::getuid(),
                &mut passwd,
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut result,
            )
        };
        if code != 0 || result.is_null() || passwd.pw_shell.is_null() {
            return None;
        }
        // SAFETY: a successful getpwuid_r returns a NUL-terminated shell in
        // `buffer`, which remains alive until after the string is copied.
        let shell = unsafe { CStr::from_ptr(passwd.pw_shell) };
        Some(PathBuf::from(OsString::from_vec(shell.to_bytes().to_vec())))
    }

    #[derive(Clone, Copy)]
    enum Kind {
        Posix,
        Nu,
        PowerShell,
    }

    impl Kind {
        fn from_path(path: &Path) -> Option<Self> {
            match path.file_stem()?.to_str()?.to_ascii_lowercase().as_str() {
                "zsh" | "bash" => Some(Self::Posix),
                "nu" => Some(Self::Nu),
                "pwsh" | "powershell" => Some(Self::PowerShell),
                _ => None,
            }
        }

        fn args(self) -> &'static [&'static str] {
            const POSIX_SCRIPT: &str =
                r#"printf '__KUI_PATH__%s\0__KUI_SHELL__%s\0' "$PATH" "$SHELL""#;
            const NU_SCRIPT: &str = r##"print -n $"__KUI_PATH__($env.PATH | str join ':')(char nul)__KUI_SHELL__($env.SHELL)(char nul)""##;
            const POWERSHELL_SCRIPT: &str = r#"[Console]::Out.Write('__KUI_PATH__' + $env:PATH + [char]0 + '__KUI_SHELL__' + $env:SHELL + [char]0)"#;
            match self {
                Self::Posix => &["-l", "-i", "-c", POSIX_SCRIPT],
                Self::Nu => &["--login", "--interactive", "-c", NU_SCRIPT],
                Self::PowerShell => &[
                    "-NoLogo",
                    "-Login",
                    "-Interactive",
                    "-Command",
                    POWERSHELL_SCRIPT,
                ],
            }
        }
    }

    fn probe_output(shell: &Path, kind: Kind, timeout: Duration) -> Option<Vec<u8>> {
        use std::os::fd::AsRawFd;

        let mut command = Command::new(shell);
        command
            .args(kind.args())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(home) = std::env::var_os("HOME") {
            command.current_dir(home);
        }
        let mut child = command.spawn().ok()?;
        let mut stdout = child.stdout.take()?;
        // Read while the shell runs: a noisy profile must not fill the pipe.
        // Nonblocking also covers a background process holding it open after
        // the shell exits.
        // SAFETY: `stdout` owns the descriptor, open until it drops.
        unsafe { libc::fcntl(stdout.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) };
        let mut output = Vec::new();
        let mut drain = || {
            let mut buffer = [0; 8192];
            loop {
                match stdout.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => output.extend_from_slice(&buffer[..n]),
                }
            }
        };
        let deadline = Instant::now() + timeout;
        loop {
            drain();
            match child.try_wait() {
                Ok(Some(status)) if status.success() => {
                    drain();
                    return Some(output);
                }
                Ok(Some(_)) | Err(_) => return None,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    }

    fn parse(output: &[u8]) -> Environment {
        let field = |mark: &[u8], start: usize| {
            let from = start + mark.len();
            let end = output[from..]
                .iter()
                .position(|b| *b == 0)
                .map(|i| from + i)
                .unwrap_or(output.len());
            OsString::from_vec(output[from..end].to_vec())
        };
        let Some(path) = find(output, PATH_MARK) else {
            return Environment::default();
        };
        let Some(shell) = find(output, SHELL_MARK).filter(|s| *s > path) else {
            return Environment::default();
        };
        Environment {
            path: Some(field(PATH_MARK, path)),
            shell: Some(field(SHELL_MARK, shell)),
        }
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn every_supported_shell_loads_login_and_interactive_configuration() {
            for (shell, kind) in [
                ("zsh", Kind::Posix),
                ("bash", Kind::Posix),
                ("nu", Kind::Nu),
                ("pwsh", Kind::PowerShell),
            ] {
                assert_eq!(
                    Kind::from_path(Path::new(shell)).map(|k| k as u8),
                    Some(kind as u8)
                );
                let args = kind.args();
                assert!(
                    args.iter()
                        .any(|a| matches!(*a, "-l" | "--login" | "-Login")),
                    "{shell} is not a login shell"
                );
                assert!(
                    args.iter()
                        .any(|a| matches!(*a, "-i" | "--interactive" | "-Interactive")),
                    "{shell} is not interactive"
                );
            }
            assert!(Kind::Nu.args().last().unwrap().contains("str join ':'"));
        }

        #[test]
        fn markers_ignore_noisy_shell_startup() {
            let env =
                parse(b"profile noise\n__KUI_PATH__/one:/two\0__KUI_SHELL__/bin/zsh\0trailing");
            assert_eq!(env.path, Some(OsString::from("/one:/two")));
            assert_eq!(env.shell, Some(OsString::from("/bin/zsh")));
        }

        #[test]
        fn a_probe_reads_the_shells_answer() {
            let dir = std::env::temp_dir().join(format!("kui-shell-env-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            use std::os::unix::fs::PermissionsExt;
            for (name, body) in [
                ("chatty", "printf 'startup noise\\n'"),
                ("leaves", "sleep 5 &"),
                (
                    "noisy",
                    "/usr/bin/yes startup-noise | /usr/bin/head -c 131072",
                ),
            ] {
                let shell = dir.join(name);
                std::fs::write(
                    &shell,
                    format!(
                        "#!/bin/sh\n{body}\nprintf '__KUI_PATH__/probe/bin\\0__KUI_SHELL__/bin/zsh\\0'\n"
                    ),
                )
                .unwrap();
                std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();
                let output = probe_output(&shell, Kind::Posix, Duration::from_secs(2)).unwrap();
                let env = parse(&output);
                assert_eq!(env.path, Some(OsString::from("/probe/bin")), "{name}");
                assert_eq!(env.shell, Some(OsString::from("/bin/zsh")), "{name}");
            }
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

#[cfg(target_os = "macos")]
pub use imp::{apply, start};

#[cfg(not(target_os = "macos"))]
pub fn start() {}

#[cfg(not(target_os = "macos"))]
pub fn apply(_command: &mut std::process::Command) {}
