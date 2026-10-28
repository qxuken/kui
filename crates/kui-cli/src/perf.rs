//! `kui perf [RECORDING]` (ADR 0075, backlog F254): kui's trace inspector,
//! carried in the runner as Kit is. Its files (`tools/kui-perf`) are in the
//! binary; the first run of a version writes them into the runner's cache
//! (`<cache>/perf/<version>`), a later one finds them there as they are,
//! and the runner runs that folder as any app — the recording its
//! argument — so the inspector reads the format this kui writes.

use std::path::PathBuf;

use crate::paths;
use crate::teal::write_whole;

const FILES: &[(&str, &str)] = &[
    (
        "kui.config.lua",
        include_str!("../../../tools/kui-perf/kui.config.lua"),
    ),
    ("main.tl", include_str!("../../../tools/kui-perf/main.tl")),
    ("state.tl", include_str!("../../../tools/kui-perf/state.tl")),
    (
        "inspector.tl",
        include_str!("../../../tools/kui-perf/inspector.tl"),
    ),
];

/// The inspector's folder, its files written where they differ.
pub fn folder() -> Result<PathBuf, String> {
    let dir = paths::kui_cache()
        .join("perf")
        .join(env!("CARGO_PKG_VERSION"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, text) in FILES {
        let p = dir.join(name);
        write_whole(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file of `tools/kui-perf` the app loads is carried: a module
    /// added there and not here would be missing from `kui perf`.
    #[test]
    fn every_file_of_the_app_is_carried() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/kui-perf");
        let mut on_disk: Vec<String> = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tl") || n == "kui.config.lua")
            .collect();
        on_disk.sort();
        let mut carried: Vec<String> = FILES.iter().map(|(n, _)| n.to_string()).collect();
        carried.sort();
        assert_eq!(carried, on_disk);
    }
}
