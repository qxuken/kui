//! The system's file manager and default apps (backlog F203): a file
//! shown in its folder, and a file, a folder or a URL opened with what
//! the system opens it with. The core asks for the first with
//! [`kui_core::MenuAction::ShowInFolder`] (the devtools' "open folder"),
//! which the runner applies here; an app calls either directly.
//!
//! Each starts the platform's own tool — `open` on macOS, `explorer` on
//! Windows, `xdg-open` elsewhere — with the path as an argument and no
//! shell in between, and returns once it has started: what the tool then
//! does is the system's. A thread waits on it, so no exited child is left
//! behind.

use std::path::Path;
use std::process::Command;

/// Shows `path` in the system's file manager: selected in Finder or
/// Explorer, its folder opened elsewhere (a Linux file manager is not
/// asked to select). An error, naming why, when the path does not exist
/// or the tool would not start.
pub fn show_in_folder(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("{path}: no such file or folder"));
    }
    spawn(show_command(p)?)
}

/// Opens `target` with what the system opens it with: a file in its app,
/// a folder in the file manager, a URL (`https:`, `mailto:`, …) in the
/// browser or mail client. An error, naming why, for a path that does not
/// exist, a target the tool could misread, or a tool that would not
/// start.
pub fn open_external(target: &str) -> Result<(), String> {
    if target.is_empty() {
        return Err("nothing to open".to_string());
    }
    if !is_url(target) && !Path::new(target).exists() {
        return Err(format!("{target}: no such file or folder"));
    }
    spawn(open_command(target)?)
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

/// A path the tool takes as a path, never as an option.
fn not_an_option(path: &Path) -> std::path::PathBuf {
    if path.to_string_lossy().starts_with('-') {
        Path::new(".").join(path)
    } else {
        path.to_path_buf()
    }
}

fn show_command(path: &Path) -> Result<Command, String> {
    if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg("-R").arg(not_an_option(path));
        Ok(c)
    } else if cfg!(target_os = "windows") {
        // `explorer` reads `/select,` and the path as one argument, quoted
        // its own way: written raw, since a Windows path holds no `"`.
        let path = std::path::absolute(path).map_err(|e| e.to_string())?;
        let mut c = Command::new("explorer");
        raw_arg(&mut c, &format!("/select,\"{}\"", path.display()))?;
        Ok(c)
    } else {
        let dir = path
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut c = Command::new("xdg-open");
        c.arg(not_an_option(dir));
        Ok(c)
    }
}

fn open_command(target: &str) -> Result<Command, String> {
    let arg = if is_url(target) {
        std::path::PathBuf::from(target)
    } else {
        not_an_option(Path::new(target))
    };
    if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
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
fn spawn(mut c: Command) -> Result<(), String> {
    let program = c.get_program().to_string_lossy().into_owned();
    let mut child = c.spawn().map_err(|e| format!("{program}: {e}"))?;
    let _ = std::thread::Builder::new()
        .name("kui-system-open".into())
        .spawn(move || {
            let _ = child.wait();
        });
    Ok(())
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
        let (p, a) = args(&show_command(Path::new("/tmp/kui shot.png")).unwrap());
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

    #[test]
    fn a_missing_path_says_so_and_starts_nothing() {
        let e = show_in_folder("/no/such/kui/file.png").unwrap_err();
        assert!(e.contains("no such file"), "{e}");
        assert!(open_external("/no/such/kui/file.png").is_err());
        assert!(open_external("").is_err());
    }
}
