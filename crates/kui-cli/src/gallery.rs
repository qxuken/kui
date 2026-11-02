//! `kui play` (ADR 0073): Kit's gallery, carried in the runner. Build.rs
//! bundles `addons/kit/app`; the first run of a version writes it into the
//! runner's cache, and the ordinary app runner opens that folder. The
//! repository app and the shipped gallery therefore stay one source.

use std::io::Read;
use std::path::PathBuf;

use crate::paths;
use crate::teal::write_whole;

const BUNDLE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/gallery.bin.gz"));

fn files() -> Result<Vec<(String, String)>, String> {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(BUNDLE)
        .read_to_end(&mut raw)
        .map_err(|e| format!("the embedded gallery cannot be read: {e}"))?;
    fn take<'a>(raw: &'a [u8], at: &mut usize) -> Result<&'a [u8], String> {
        let len_end = at
            .checked_add(4)
            .ok_or("the embedded gallery is truncated")?;
        let len_bytes: [u8; 4] = raw
            .get(*at..len_end)
            .ok_or("the embedded gallery is truncated")?
            .try_into()
            .unwrap();
        let end = len_end
            .checked_add(u32::from_le_bytes(len_bytes) as usize)
            .ok_or("the embedded gallery is too large")?;
        let part = raw
            .get(len_end..end)
            .ok_or("the embedded gallery is truncated")?;
        *at = end;
        Ok(part)
    }

    let mut at = 0usize;
    let mut files = Vec::new();
    while at < raw.len() {
        let name = String::from_utf8(take(&raw, &mut at)?.to_vec())
            .map_err(|_| "the embedded gallery has a non-UTF-8 path")?;
        let text = String::from_utf8(take(&raw, &mut at)?.to_vec())
            .map_err(|e| format!("the embedded gallery's {name} is not UTF-8: {e}"))?;
        files.push((name, text));
    }
    Ok(files)
}

/// The gallery's folder, its files written where they differ.
pub fn folder() -> Result<PathBuf, String> {
    let dir = paths::kui_cache()
        .join("gallery")
        .join(env!("CARGO_PKG_VERSION"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, text) in files()? {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        write_whole(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn app_files(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                app_files(root, &path, out);
            } else if path
                .extension()
                .is_some_and(|x| x == "lua" || x == "md" || x == "json")
                && path.file_name().is_some_and(|n| n != ".luarc.json")
            {
                out.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }

    #[test]
    fn every_file_of_the_app_is_carried_with_its_source() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/kit/app");
        let mut on_disk = Vec::new();
        app_files(&root, &root, &mut on_disk);
        on_disk.sort();
        let embedded = files().unwrap();
        let mut names: Vec<String> = embedded.iter().map(|(name, _)| name.clone()).collect();
        names.sort();
        assert_eq!(names, on_disk);
        for (name, text) in embedded {
            assert_eq!(text, std::fs::read_to_string(root.join(name)).unwrap());
        }
    }

    #[test]
    fn the_bundle_ends_at_a_file_boundary() {
        let files = files().unwrap();
        assert!(files.iter().any(|(name, _)| name == "main.lua"));
        assert!(files.iter().any(|(name, _)| name == "design/tokens.json"));
    }
}
