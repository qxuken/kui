//! A packed app at run time (ADR 0068, decisions 18–21): the runner finds
//! the archive `kui pack` added — `Contents/Resources/app.zip` in a macOS
//! bundle, or appended to the binary itself — and runs the app from it.
//!
//! The archive is extracted once into the runner's cache, under its hash,
//! and run from there. ADR 0068 reads the app's files from the archive in
//! place; the core still opens an `svg`'s or an `image`'s `src` by path
//! itself (the core-purity plan's "core I/O asks" moves that), so a folder
//! is what every reader can use today. What the folder keeps of the
//! decision is that it is read-only to the app: `kui.fs` refuses to write
//! into it and names `kui.data_dir` instead.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::archive::ZipReader;
use crate::pack::{APP_DIR, PACK_FILE};

/// What a packed app is.
pub struct Packed {
    /// The extracted app's folder.
    pub dir: PathBuf,
    pub runner: String,
    pub bytecode: bool,
    pub hash: String,
    /// The entry `kui pack --bytecode` wrote, when it did.
    pub entry: Option<String>,
}

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// The packed app's own folder, when this runner is running one: read-only
/// to `kui.fs`, and the one place bytecode may load from.
pub fn dir() -> Option<&'static Path> {
    DIR.get().map(PathBuf::as_path)
}

/// This runner's archive, if it carries one.
fn find() -> Option<ZipReader> {
    let exe = std::env::current_exe().ok()?;
    if cfg!(target_os = "macos") {
        let res = exe.parent()?.parent()?.join("Resources").join("app.zip");
        if res.is_file() {
            return ZipReader::open_file(&res).ok();
        }
    }
    ZipReader::find_appended(&exe).ok().flatten()
}

/// The packed app this runner carries: `None` for a plain `kui`, an error
/// for an archive it cannot run.
pub fn detect() -> Option<Result<Packed, String>> {
    let zip = find()?;
    let pack = zip.get(PACK_FILE)?;
    Some(open(&zip, &pack.read().ok()?))
}

/// A `.kui` archive given to `kui run`: the same app, run the same way.
pub fn from_file(path: &Path) -> Result<Packed, String> {
    let zip = ZipReader::open_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pack = zip
        .get(PACK_FILE)
        .ok_or_else(|| format!("{}: not a kui archive (no {PACK_FILE})", path.display()))?;
    let bytes = pack
        .read()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    open(&zip, &bytes)
}

/// Reads `kui.pack.lua`, checks it, and extracts the app once.
fn open(zip: &ZipReader, pack: &[u8]) -> Result<Packed, String> {
    let lua = mlua::Lua::new();
    let t: mlua::Table = lua
        .load(pack)
        .set_mode(mlua::chunk::ChunkMode::Text)
        .set_name("@kui.pack.lua")
        .eval()
        .map_err(|e| format!("this app's {PACK_FILE}: {e}"))?;
    let get = |k: &str| t.get::<Option<String>>(k).ok().flatten();
    let runner = get("runner").unwrap_or_default();
    let bytecode = t
        .get::<Option<bool>>("bytecode")
        .ok()
        .flatten()
        .unwrap_or(false);
    let hash = get("hash").ok_or_else(|| format!("this app's {PACK_FILE} names no hash"))?;
    if bytecode && runner != crate::compat::VERSION {
        return Err(format!(
            "this app was packed with bytecode for kui {runner}, and this is kui {}: pack it again",
            crate::compat::VERSION
        ));
    }
    let dir = extract(zip, &hash)?;
    // Canonical, as the app's own paths are (`project::find`), so the
    // read-only and bytecode checks compare like with like.
    let dir = crate::paths::canonical(&dir).unwrap_or(dir);
    let _ = DIR.set(dir.clone());
    Ok(Packed {
        dir,
        runner,
        bytecode,
        hash,
        entry: get("entry"),
    })
}

/// `<kui cache>/apps/<hash>/`, written once: into a temporary folder that
/// is renamed into place, so a run interrupted halfway leaves nothing a
/// later run would take for whole.
fn extract(zip: &ZipReader, hash: &str) -> Result<PathBuf, String> {
    let short: String = hash.chars().take(32).collect();
    let root = crate::paths::kui_cache().join("apps").join(&short);
    let app = root.join("app");
    if root.join(".complete").is_file() {
        return Ok(app);
    }
    let tmp = root.with_extension(format!("part-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    for entry in zip.entries() {
        let name = entry.name();
        let Some(rel) = name.strip_prefix(APP_DIR) else {
            continue;
        };
        if rel.is_empty() || !crate::archive::is_safe_name(rel) {
            continue;
        }
        let path = tmp.join("app").join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let data = entry.read().map_err(|e| format!("{name}: {e}"))?;
        std::fs::write(&path, data).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    std::fs::write(tmp.join(".complete"), hash).map_err(|e| e.to_string())?;
    if std::fs::rename(&tmp, &root).is_err() {
        // Another run got there first — take its folder — or a folder a
        // run left half-written is in the way: clear it, once.
        if !root.join(".complete").is_file() {
            let _ = std::fs::remove_dir_all(&root);
            std::fs::rename(&tmp, &root).map_err(|e| format!("{}: {e}", root.display()))?;
        } else {
            let _ = std::fs::remove_dir_all(&tmp);
        }
    }
    Ok(app)
}
