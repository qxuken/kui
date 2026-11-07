//! Desktop file actions. Arguments stay separate from commands, including
//! editor preferences: filenames are never evaluated by a shell.
use std::path::Path;
use std::process::{Command, Stdio};

fn editor_args(editor: &str, path: &Path, line: u32) -> Result<Vec<String>, String> {
    let mut args = shlex::split(editor).ok_or("editor preference has unmatched quotes")?;
    let executable = args.first().ok_or("editor preference is empty")?;
    let name = Path::new(executable)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let path = path.to_string_lossy().into_owned();
    match name.as_ref() {
        "code" | "code-insiders" | "cursor" | "codium" => {
            args.extend(["--goto".into(), format!("{path}:{}", line.max(1))]);
        }
        "zed" | "subl" => args.push(format!("{path}:{}", line.max(1))),
        "vi" | "vim" | "nvim" | "emacs" | "nano" => {
            args.extend([format!("+{}", line.max(1)), path])
        }
        _ => args.push(path),
    }
    Ok(args)
}

pub fn command(action: &str, path: &Path, line: u32) -> Result<Command, String> {
    if !path.is_absolute() || !path.exists() {
        return Err(format!("file is unavailable: {}", path.display()));
    }
    if action == "editor" {
        if let Some(editor) = ["KUI_EDITOR", "VISUAL", "EDITOR"]
            .iter()
            .find_map(|key| std::env::var(key).ok().filter(|v| !v.trim().is_empty()))
        {
            let args = editor_args(&editor, path, line)?;
            let mut command = Command::new(&args[0]);
            command.args(&args[1..]);
            return Ok(command);
        }
        #[cfg(target_os = "macos")]
        {
            let mut c = Command::new("open");
            c.arg("-t").arg(path);
            Ok(c)
        }
        #[cfg(target_os = "windows")]
        {
            let mut c = Command::new("notepad.exe");
            c.arg(path);
            Ok(c)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let mut c = Command::new("xdg-open");
            c.arg(path);
            Ok(c)
        }
    } else if action == "reveal" {
        #[cfg(target_os = "macos")]
        {
            let mut c = Command::new("open");
            c.arg("-R").arg(path);
            Ok(c)
        }
        #[cfg(target_os = "windows")]
        {
            let mut c = Command::new("explorer.exe");
            c.arg(format!("/select,{}", path.display()));
            Ok(c)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let mut c = Command::new("xdg-open");
            c.arg(if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(path)
            });
            Ok(c)
        }
    } else {
        Err(format!("unknown file action: {action}"))
    }
}

pub fn spawn(
    mut command: Command,
    done: impl FnOnce(Result<(), String>) + Send + 'static,
) -> Result<(), String> {
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            format!("cannot launch file action: {e}. Set KUI_EDITOR to your editor command.")
        })?;
    std::thread::spawn(move || {
        done(
            child
                .wait_with_output()
                .map_err(|e| e.to_string())
                .and_then(|out| {
                    if out.status.success() {
                        Ok(())
                    } else {
                        Err(format!(
                            "file action exited {}: {}",
                            out.status,
                            String::from_utf8_lossy(&out.stderr).trim()
                        ))
                    }
                }),
        );
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_keeps_quoted_arguments_and_literal_filenames() {
        let path = Path::new("/tmp/a b/$(touch unsafe).lua");
        assert_eq!(
            editor_args("'code' --reuse-window", path, 42).unwrap(),
            [
                "code",
                "--reuse-window",
                "--goto",
                "/tmp/a b/$(touch unsafe).lua:42"
            ]
        );
        assert_eq!(
            editor_args("'/Applications/My Editor' --wait", path, 1).unwrap(),
            [
                "/Applications/My Editor",
                "--wait",
                "/tmp/a b/$(touch unsafe).lua"
            ]
        );
        assert!(editor_args("'bad", path, 1).is_err());
    }
    #[test]
    #[cfg(unix)]
    fn launcher_reports_nonzero_exit_and_preserves_path_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("test editor.sh");
        std::fs::write(
            &script,
            r#"printf '%s\n' "$@" >&2
exit 7
"#,
        )
        .unwrap();
        let mut command = Command::new("sh");
        command.arg(script).arg("/tmp/a b/$(touch unsafe).lua");
        let (send, recv) = std::sync::mpsc::channel();
        spawn(command, move |result| send.send(result).unwrap()).unwrap();
        let error = recv
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap()
            .unwrap_err();
        assert!(
            error.contains("exit status: 7") && error.contains("/tmp/a b/$(touch unsafe).lua"),
            "{error}"
        );
    }
}
