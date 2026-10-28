//! The system's file manager and default apps (backlog F203): a file
//! shown in its folder, and a file, a folder or a URL opened with what
//! the system opens it with. The core asks for the first with
//! [`kui_core::MenuAction::ShowInFolder`] (the devtools' "open folder"),
//! which the runner applies here; an app calls either directly.
//!
//! Each starts the platform's own tool — `open` on macOS, `explorer` on
//! Windows, `xdg-open` elsewhere (a Linux desktop's file manager asked
//! over D-Bus first, to select) — with the path as an argument and no
//! shell in between, and returns once it has started: what the tool then
//! does is the system's. A thread waits on it, so no exited child is left
//! behind. Each tool starts as every child kui starts does
//! ([`crate::spawn`]): under kui's lock against macOS's pipes, which are
//! not born close-on-exec, or the host's way where it has one (backlog
//! F222, F223).

use std::path::Path;
use std::process::{Child, Command, Stdio};

/// Shows `path` in the system's file manager, selected: in Finder, in
/// Explorer, and on a Linux desktop in the file manager that answers
/// `org.freedesktop.FileManager1` (Nautilus, Dolphin, Thunar, Nemo) —
/// its folder opened by `xdg-open`, nothing selected, where none does.
/// An error, naming why, when the path does not exist or no tool would
/// start.
pub fn show_in_folder(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("{path}: no such file or folder"));
    }
    spawn_steps(show_steps(p)?)
}

/// Opens `target` with what the system opens it with: a file in its app,
/// a folder in the file manager — on macOS in Finder, so a bundle
/// (`X.app`) is shown as the folder it is rather than run — a URL
/// (`https:`, `mailto:`, …) in the browser or mail client. An error,
/// naming why, for a path that does not exist, a target the tool could
/// misread, or a tool that would not start.
pub fn open_external(target: &str) -> Result<(), String> {
    if target.is_empty() {
        return Err("nothing to open".to_string());
    }
    if !is_url(target) && !Path::new(target).exists() {
        return Err(format!("{target}: no such file or folder"));
    }
    spawn(open_command(target)?)
}

/// Opens the recording at `path` in kui-perf, the trace inspector (ADR
/// 0075): `kui perf <path>`, with the kui this process is when it is the
/// runner, else the `kui` on `PATH` (backlog F253). An error, naming why,
/// when the file does not exist or no kui would start — a host that
/// falls back shows the file with [`show_in_folder`].
pub fn open_recording(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("{path}: no such file"));
    }
    let p = std::path::absolute(p).map_err(|e| e.to_string())?;
    spawn_steps(recording_steps(&p))
}

/// The kuis that may open a recording, the running one first.
fn recording_steps(path: &Path) -> Vec<Step> {
    let mut kuis = Vec::new();
    if let Ok(me) = std::env::current_exe()
        && me.file_stem().is_some_and(|s| s == "kui")
    {
        kuis.push(me);
    }
    kuis.push(std::path::PathBuf::from("kui"));
    kuis.into_iter()
        .map(|k| {
            let mut c = Command::new(k);
            c.arg("perf").arg(not_an_option(path));
            plain(c)
        })
        .collect()
}

/// A scheme before a colon (`https:`, `mailto:`, `file:`): a URL, not a
/// path. A Windows drive (`C:`) is one letter, so a path.
fn is_url(target: &str) -> bool {
    let Some((scheme, _)) = target.split_once(':') else {
        return false;
    };
    scheme.len() > 1
        && scheme
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// `path` in the platform's own separators: on Windows every `/` a `\`,
/// since Explorer reads `C:\Users\me\Notes/report.pdf` — a folder joined
/// to a Markdown link's path — as no path at all and opens Documents
/// (backlog F226, from Carnet). A verbatim path (`\\?\…`), where `/` is a
/// file name's character, is left as it is; elsewhere `/` is the
/// separator already.
fn native_separators(path: &Path) -> std::path::PathBuf {
    let s = path.to_string_lossy();
    if cfg!(windows) && s.contains('/') && !s.starts_with(r"\\?\") {
        std::path::PathBuf::from(s.replace('/', "\\"))
    } else {
        path.to_path_buf()
    }
}

/// A path the tool takes as a path, never as an option.
fn not_an_option(path: &Path) -> std::path::PathBuf {
    if path.to_string_lossy().starts_with('-') {
        Path::new(".").join(path)
    } else {
        path.to_path_buf()
    }
}

/// One way of asking: the tool, and whether its exit says it worked —
/// `dbus-send` fails when no file manager answers, and the next way is
/// tried; `explorer` exits 1 having shown the folder.
struct Step {
    command: Command,
    checked: bool,
}

fn plain(command: Command) -> Step {
    Step {
        command,
        checked: false,
    }
}

/// The ways to show `path` selected, in the order they are tried.
fn show_steps(path: &Path) -> Result<Vec<Step>, String> {
    if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg("-R").arg(not_an_option(path));
        Ok(vec![plain(c)])
    } else if cfg!(target_os = "windows") {
        // `explorer` reads `/select,` and the path as one argument, quoted
        // its own way: written raw, since a Windows path holds no `"`. Its
        // separators made `\` first: `absolute` (GetFullPathNameW) does
        // that too today, but the open path below has no `absolute` to
        // lean on, and both go through the one rule (backlog F226).
        let path = std::path::absolute(native_separators(path)).map_err(|e| e.to_string())?;
        let mut c = Command::new("explorer");
        raw_arg(&mut c, &format!("/select,\"{}\"", path.display()))?;
        Ok(vec![plain(c)])
    } else {
        // The desktop's file manager over D-Bus, which selects; a `,`
        // would split the array, so the URI has it as `%2C`.
        let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
        let mut dbus = Command::new("dbus-send");
        dbus.args([
            "--session",
            "--print-reply",
            "--dest=org.freedesktop.FileManager1",
            "--type=method_call",
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1.ShowItems",
        ])
        .arg(format!("array:string:{}", file_uri(&absolute)))
        .arg("string:");
        let dir = path
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut open = Command::new("xdg-open");
        open.arg(not_an_option(dir));
        Ok(vec![
            Step {
                command: dbus,
                checked: true,
            },
            plain(open),
        ])
    }
}

/// `file://` and the absolute path, every byte but the unreserved ones
/// and `/` written `%XX`.
fn file_uri(path: &Path) -> String {
    let mut out = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn open_command(target: &str) -> Result<Command, String> {
    let arg = if is_url(target) {
        std::path::PathBuf::from(target)
    } else {
        not_an_option(&native_separators(Path::new(target)))
    };
    if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        // A folder in Finder: `open` alone runs a bundle (`X.app`).
        if !is_url(target) && Path::new(target).is_dir() {
            c.args(["-a", "Finder"]);
        }
        c.arg(arg);
        Ok(c)
    } else if cfg!(target_os = "windows") {
        // Quoted whole, so a URL's `&` is not the end of it.
        let mut c = Command::new("explorer");
        raw_arg(&mut c, &format!("\"{}\"", arg.display()))?;
        Ok(c)
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(arg);
        Ok(c)
    }
}

/// `arg` exactly as written, which a quote inside would end early.
#[cfg(windows)]
fn raw_arg(c: &mut Command, arg: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    if arg.matches('"').count() > 2 {
        return Err(format!("{arg}: a quote in it"));
    }
    c.raw_arg(arg);
    Ok(())
}

#[cfg(not(windows))]
fn raw_arg(c: &mut Command, arg: &str) -> Result<(), String> {
    c.arg(arg);
    Ok(())
}

/// Starts it, and waits for it on a thread of its own. `explorer` exits
/// with 1 whether or not it did what it was asked, so starting is what
/// counts.
fn spawn(c: Command) -> Result<(), String> {
    spawn_steps(vec![plain(c)])
}

/// The first of `steps` that starts, started; an error naming why the
/// last could not when none does — `dbus-send` not installed is
/// `xdg-open`'s turn. The wait, and the next way when a checked one
/// says it failed, on a thread of its own.
fn spawn_steps(steps: Vec<Step>) -> Result<(), String> {
    let mut steps = steps.into_iter();
    let first = start_first(&mut steps, "nothing to start".to_string())?;
    let _ = std::thread::Builder::new()
        .name("kui-system-open".into())
        .spawn(move || {
            let mut tried = first;
            loop {
                let (checked, mut child) = tried;
                let worked = child.wait().is_ok_and(|s| s.success());
                if worked || !checked {
                    return;
                }
                match start_first(&mut steps, String::new()) {
                    Ok(next) => tried = next,
                    Err(_) => return,
                }
            }
        });
    Ok(())
}

fn start_first(
    steps: &mut impl Iterator<Item = Step>,
    mut why: String,
) -> Result<(bool, Child), String> {
    for mut step in steps {
        let program = step.command.get_program().to_string_lossy().into_owned();
        step.command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match crate::spawn::spawn(&mut step.command) {
            Ok(child) => return Ok((step.checked, child)),
            Err(e) => why = format!("{program}: {e}"),
        }
    }
    Err(why)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(c: &Command) -> (String, Vec<String>) {
        (
            c.get_program().to_string_lossy().into_owned(),
            c.get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect(),
        )
    }

    #[test]
    fn a_url_is_told_from_a_path() {
        for url in [
            "https://example.com/a?b=1&c=2",
            "mailto:a@b.c",
            "file:///tmp/x",
        ] {
            assert!(is_url(url), "{url}");
        }
        for path in ["C:\\Users\\a.png", "/tmp/a.png", "a.png", "./a:b", "-x"] {
            assert!(!is_url(path), "{path}");
        }
    }

    #[test]
    fn a_path_is_never_an_option() {
        let (_, a) = args(&open_command("-rf").unwrap());
        assert!(a.iter().all(|a| !a.starts_with('-') || a == "-R"), "{a:?}");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_asks_finder_to_select_it() {
        let (p, a) = args(&show_steps(Path::new("/tmp/kui shot.png")).unwrap()[0].command);
        assert_eq!(
            (p.as_str(), a),
            ("open", vec!["-R".into(), "/tmp/kui shot.png".into()])
        );
        let (p, a) = args(&open_command("https://example.com/?a=1&b=2").unwrap());
        assert_eq!(
            (p.as_str(), a),
            ("open", vec!["https://example.com/?a=1&b=2".into()])
        );
    }

    /// A folder opens in Finder, a bundle as the folder it is (backlog
    /// F222): `open` alone runs `X.app`.
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_opens_a_folder_in_finder() {
        let dir = std::env::temp_dir().join("kui F222 dir.app");
        std::fs::create_dir_all(&dir).unwrap();
        let d = dir.to_string_lossy().into_owned();
        let (p, a) = args(&open_command(&d).unwrap());
        assert_eq!(
            (p.as_str(), a),
            ("open", vec!["-a".into(), "Finder".into(), d])
        );
        let _ = std::fs::remove_dir(&dir);
    }

    /// A Linux desktop's file manager is asked to select it over D-Bus,
    /// the path whole as a URI, and `xdg-open` on its folder is the way
    /// when none answers (backlog F222).
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    #[test]
    fn linux_asks_the_file_manager_to_select_it_then_opens_its_folder() {
        let steps = show_steps(Path::new("/a b/c,d.png")).unwrap();
        assert_eq!(steps.len(), 2);
        assert!(steps[0].checked && !steps[1].checked);
        let (p, a) = args(&steps[0].command);
        assert_eq!(p, "dbus-send");
        assert!(
            a.iter().any(|a| a.ends_with("FileManager1.ShowItems")),
            "{a:?}"
        );
        assert!(
            a.contains(&"array:string:file:///a%20b/c%2Cd.png".to_string()),
            "{a:?}"
        );
        assert_eq!(
            args(&steps[1].command),
            ("xdg-open".into(), vec!["/a b".into()])
        );
    }

    /// A path with a `/` among its `\` reaches Explorer with `\` alone —
    /// what an app gets joining its folder to a Markdown link (backlog
    /// F226, from Carnet) — for opening and for showing it; a URL keeps
    /// its own.
    #[cfg(windows)]
    #[test]
    fn windows_paths_reach_explorer_with_backslashes() {
        let mixed = r"C:\Users\me\Carnet/attachments/report.pdf";
        let whole = r"C:\Users\me\Carnet\attachments\report.pdf";
        let (p, a) = args(&open_command(mixed).unwrap());
        assert_eq!((p.as_str(), a), ("explorer", vec![format!("\"{whole}\"")]));
        let (p, a) = args(&show_steps(Path::new(mixed)).unwrap()[0].command);
        assert_eq!(
            (p.as_str(), a),
            ("explorer", vec![format!("/select,\"{whole}\"")])
        );
        let url = "https://example.com/a/b";
        assert_eq!(
            args(&open_command(url).unwrap()).1,
            vec![format!("\"{url}\"")]
        );
    }

    #[test]
    fn a_file_uri_keeps_the_path_whole() {
        assert_eq!(
            file_uri(Path::new("/a b/c,d&e;$(f)~_.x")),
            "file:///a%20b/c%2Cd%26e%3B%24%28f%29~_.x"
        );
    }

    /// The tools start through the host's spawner once it sets one
    /// (backlog F222): a host's spawn lock covers kui's children too.
    #[test]
    fn a_host_s_spawner_starts_the_tools() {
        use crate::spawn::set_spawner;
        use std::sync::Arc;
        let _one = crate::spawn::TEST_SPAWNER
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let file = std::env::temp_dir().join("kui-f220-spawner.txt");
        std::fs::write(&file, "x").unwrap();
        let seen = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let s = seen.clone();
        set_spawner(move |c| {
            s.lock()
                .unwrap()
                .push(c.get_program().to_string_lossy().into_owned());
            // Something harmless in its place, so no window opens.
            let harmless = if cfg!(windows) { "cmd" } else { "true" };
            let mut t = Command::new(harmless);
            if cfg!(windows) {
                t.args(["/c", "exit", "0"]);
            }
            t.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            t.spawn()
        });
        show_in_folder(&file.to_string_lossy()).unwrap();
        open_external("https://example.com/").unwrap();
        crate::spawn::reset_spawner();
        let seen = seen.lock().unwrap().clone();
        let first = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(windows) {
            "explorer"
        } else {
            "dbus-send"
        };
        assert_eq!(seen.first().map(String::as_str), Some(first), "{seen:?}");
        assert!(seen.len() >= 2, "{seen:?}");
        let _ = std::fs::remove_file(&file);
    }

    /// `kui perf <path>`: the running kui when this is the runner, else
    /// the one on `PATH` — a test binary is not `kui`, so that one alone.
    #[test]
    fn a_recording_opens_with_kui_perf() {
        let path = std::path::absolute("a.kuitrace").unwrap();
        let steps = recording_steps(&path);
        assert_eq!(steps.len(), 1);
        let (p, a) = args(&steps[0].command);
        assert_eq!(p, "kui");
        assert_eq!(a, ["perf".to_string(), path.to_string_lossy().into_owned()]);
        assert!(open_recording("/no/such/kui/file.kuitrace").is_err());
    }

    #[test]
    fn a_missing_path_says_so_and_starts_nothing() {
        let e = show_in_folder("/no/such/kui/file.png").unwrap_err();
        assert!(e.contains("no such file"), "{e}");
        assert!(open_external("/no/such/kui/file.png").is_err());
        assert!(open_external("").is_err());
    }
}
