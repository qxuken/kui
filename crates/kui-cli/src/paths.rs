//! Where an app keeps what it writes (ADR 0068, decision 9): a folder of
//! its own for data and one for caches, named by the app's `id`, in the
//! place each system expects — the same under `kui run` and packed, so an
//! app never invents `~/.local/share/…` and a packed app, whose own files
//! are read-only, has somewhere to write. And the runner's own cache, for
//! the runners `kui pack` downloads.

use std::path::PathBuf;

fn home() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The base every app's data folder sits in.
fn data_base() -> PathBuf {
    if cfg!(target_os = "macos") {
        home().join("Library/Application Support")
    } else if cfg!(windows) {
        env_dir("APPDATA").unwrap_or_else(|| home().join("AppData/Roaming"))
    } else {
        env_dir("XDG_DATA_HOME").unwrap_or_else(|| home().join(".local/share"))
    }
}

/// The base every app's cache folder sits in.
fn cache_base() -> PathBuf {
    if cfg!(target_os = "macos") {
        home().join("Library/Caches")
    } else if cfg!(windows) {
        env_dir("LOCALAPPDATA").unwrap_or_else(|| home().join("AppData/Local"))
    } else {
        env_dir("XDG_CACHE_HOME").unwrap_or_else(|| home().join(".cache"))
    }
}

/// `kui.data_dir`: the app's own data folder.
pub fn data_dir(id: &str) -> PathBuf {
    data_base().join(id)
}

/// `kui.cache_dir`: the app's own cache folder. On Windows a `cache`
/// folder under the app's local one, which is where its data would go if
/// it were not roaming.
pub fn cache_dir(id: &str) -> PathBuf {
    if cfg!(windows) { cache_base().join(id).join("cache") } else { cache_base().join(id) }
}

/// The runner's own cache: downloaded runners, extracted packed apps.
pub fn kui_cache() -> PathBuf {
    env_dir("KUI_CACHE_DIR").unwrap_or_else(|| cache_base().join("kui"))
}
