//! macOS names an app that is not a bundle by its binary — in the Dock,
//! ⌘Tab and the menu bar — so every runner app was "kui". A windowed run
//! relaunches from a clone of the binary named after the app: an APFS
//! clone costs no space (on another volume it is a plain copy), and it is
//! the same signed file. Elsewhere, under `--no-rename` or `rename =
//! false`, and after any failure, the app simply stays "kui".

#[cfg(target_os = "macos")]
pub fn relaunch_as(name: &str) {
    use std::os::unix::process::CommandExt;
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let safe: String = name
        .chars()
        .map(|c| {
            if c == '/' || c == ':' || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    if safe.is_empty() || exe.file_name().is_some_and(|f| f.to_string_lossy() == safe) {
        return;
    }
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    // One folder per binary and build, so a rebuilt runner is a new clone.
    let Ok(meta) = std::fs::metadata(&exe) else {
        return;
    };
    let stamp = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let mut h: u64 = 0xcbf29ce484222325;
    for b in format!("{}:{stamp}", exe.display()).bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    let dir = std::path::Path::new(&home)
        .join("Library/Caches/kui/apps")
        .join(format!("{h:016x}"));
    let clone = dir.join(&safe);
    if !clone.exists() {
        // Built aside and renamed into place: two apps started at once never
        // run a half-written file.
        let tmp = dir.join(format!(".{safe}.{}", std::process::id()));
        if std::fs::create_dir_all(&dir).is_err() || std::fs::copy(&exe, &tmp).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        if std::fs::rename(&tmp, &clone).is_err() {
            let _ = std::fs::remove_file(&tmp);
            if !clone.exists() {
                return;
            }
        }
    }
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    // `exec` returns only when it failed: run on as "kui".
    let _ = std::process::Command::new(&clone).args(args).exec();
}

#[cfg(not(target_os = "macos"))]
pub fn relaunch_as(_name: &str) {}
