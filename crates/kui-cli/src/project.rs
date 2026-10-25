//! What `kui run <path>` runs (ADR 0064, decisions 1 and 10): a `.lua` or
//! `.tl` file, or a folder whose `kui.config.lua` (or `.tl`) names the
//! entry — else its `main.lua`, `main.tl` or `app.lua` — and the `.env`
//! files beside it.

use std::path::{Path, PathBuf};

use mlua::Lua;

/// The app to run.
pub struct Project {
    /// The app's folder: relative paths, `.env`, `require` start here.
    pub dir: PathBuf,
    pub entry: PathBuf,
    pub name: String,
    pub size: (f64, f64),
    /// The window's least size, logical px.
    pub min_size: Option<(f64, f64)>,
    /// `chrome = "custom"` draws the app's own titlebar; `"borderless"`
    /// none at all.
    pub chrome: kui_native::Chrome,
    /// `titlebar = "medium"` or `"tall"`: the macOS strip under custom
    /// chrome.
    pub titlebar: kui_native::Titlebar,
    /// A PNG for the Dock, the taskbar and the window, against the app's
    /// folder.
    pub icon: Option<PathBuf>,
    /// Open with the devtools panel.
    pub devtools: bool,
    /// Relaunch under the app's name on macOS (`rename = false` keeps "kui").
    pub rename: bool,
    /// Globs the watcher follows, beside the app's own `.lua` and `.tl`.
    pub watch_include: Vec<String>,
    pub watch_exclude: Vec<String>,
}

const ENTRIES: [&str; 3] = ["main.lua", "main.tl", "app.lua"];

pub fn find(path: &Path) -> Result<Project, String> {
    let path = if path.as_os_str().is_empty() { Path::new(".") } else { path };
    let path = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let (dir, mut entry) = if path.is_dir() {
        (path.clone(), None)
    } else {
        (
            path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(".")),
            Some(path.clone()),
        )
    };
    let mut p = Project {
        name: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "kui".into()),
        dir: dir.clone(),
        entry: PathBuf::new(),
        size: (1000.0, 700.0),
        min_size: None,
        chrome: kui_native::Chrome::Native,
        titlebar: kui_native::Titlebar::Standard,
        icon: None,
        devtools: false,
        rename: true,
        watch_include: Vec::new(),
        watch_exclude: Vec::new(),
    };
    if let Some(config) = ["kui.config.lua", "kui.config.tl"]
        .iter()
        .map(|n| dir.join(n))
        .find(|c| c.exists())
    {
        read_config(&config, &mut p, &mut entry)?;
    }
    p.entry = match entry {
        Some(e) => e,
        None => ENTRIES.iter().map(|n| dir.join(n)).find(|e| e.exists()).ok_or_else(|| {
            format!(
                "{}: no app here — expected {}, or a kui.config.lua naming an entry (start one with `kui new <dir>`)",
                dir.display(),
                ENTRIES.join(", ")
            )
        })?,
    };
    if entry_name_from_file(&p) {
        p.name = p.entry.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(p.name);
    }
    Ok(p)
}

/// A file given alone names the app after itself (`kui run tool.lua` is
/// "tool"); a folder's app is named for the folder.
fn entry_name_from_file(p: &Project) -> bool {
    !ENTRIES.iter().any(|n| p.entry.file_name().is_some_and(|f| f == *n))
}

fn read_config(path: &Path, p: &mut Project, entry: &mut Option<PathBuf>) -> Result<(), String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let source = crate::teal::source_for(path, &source).map_err(|e| e.to_string())?;
    let lua = Lua::new();
    let t: mlua::Table = lua
        .load(&source)
        .set_name(format!("@{}", path.display()))
        .eval()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut get = || -> mlua::Result<()> {
        if let Some(e) = t.get::<Option<String>>("entry")? {
            *entry = Some(p.dir.join(e));
        }
        if let Some(n) = t.get::<Option<String>>("name")? {
            p.name = n;
        }
        if let Some(s) = size(&t, "size")? {
            p.size = s;
        }
        p.min_size = size(&t, "min_size")?;
        if let Some(c) = t.get::<Option<String>>("chrome")? {
            p.chrome = match c.as_str() {
                "native" => kui_native::Chrome::Native,
                "custom" => kui_native::Chrome::Custom,
                "borderless" => kui_native::Chrome::Borderless,
                _ => return Err(mlua::Error::runtime(format!("chrome = {c:?}: native, custom or borderless"))),
            };
        }
        if let Some(b) = t.get::<Option<String>>("titlebar")? {
            p.titlebar = match b.as_str() {
                "standard" => kui_native::Titlebar::Standard,
                "medium" => kui_native::Titlebar::Medium,
                "tall" => kui_native::Titlebar::Tall,
                _ => return Err(mlua::Error::runtime(format!("titlebar = {b:?}: standard, medium or tall"))),
            };
        }
        p.devtools = t.get::<Option<bool>>("devtools")?.unwrap_or(false);
        p.rename = t.get::<Option<bool>>("rename")?.unwrap_or(true);
        if let Some(i) = t.get::<Option<String>>("icon")? {
            p.icon = Some(p.dir.join(i));
        }
        if let Some(w) = t.get::<Option<mlua::Table>>("watch")? {
            if let Some(i) = w.get::<Option<Vec<String>>>("include")? {
                p.watch_include = i;
            }
            if let Some(x) = w.get::<Option<Vec<String>>>("exclude")? {
                p.watch_exclude = x;
            }
        }
        Ok(())
    };
    get().map_err(|e| format!("{}: {e}", path.display()))
}

/// `{ w = 900, h = 600 }` or `{ 900, 600 }` under `name`.
fn size(t: &mlua::Table, name: &str) -> mlua::Result<Option<(f64, f64)>> {
    let Some(s) = t.get::<Option<mlua::Table>>(name)? else { return Ok(None) };
    let w = s.get::<Option<f64>>("w")?.or(s.get::<Option<f64>>(1)?);
    let h = s.get::<Option<f64>>("h")?.or(s.get::<Option<f64>>(2)?);
    Ok(w.zip(h))
}

/// Loads `.env`, then `.env.local`, from `dir` into the process's
/// environment; a variable already set wins.
pub fn load_dotenv(dir: &Path) {
    for name in [".env", ".env.local"] {
        let Ok(text) = std::fs::read_to_string(dir.join(name)) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            let Some((k, v)) = line.split_once('=') else { continue };
            let k = k.trim();
            let mut v = v.trim();
            if (v.starts_with('"') && v.ends_with('"') && v.len() >= 2)
                || (v.starts_with('\'') && v.ends_with('\'') && v.len() >= 2)
            {
                v = &v[1..v.len() - 1];
            } else if let Some(i) = v.find(" #") {
                v = v[..i].trim_end();
            }
            if std::env::var_os(k).is_none() {
                // SAFETY: before the app's threads exist; the runner sets
                // the environment once, on its own thread, at start.
                unsafe { std::env::set_var(k, v) };
            }
        }
    }
}

/// Puts `dir` first on `package.path`, so `require "lib.store"` finds
/// `dir/lib/store.lua` (or `.tl`).
pub fn add_path(lua: &Lua, dir: &Path) -> mlua::Result<()> {
    let package: mlua::Table = lua.globals().get("package")?;
    let rest: String = package.get("path")?;
    let d = dir.to_string_lossy();
    package.set("path", format!("{d}/?.lua;{d}/?/init.lua;{rest}"))
}

/// Every `.tl` under `dir` (not `.d.tl`), for `kui check`; hidden folders
/// and `node_modules` skipped.
pub fn teal_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, &mut |p| {
        let name = p.to_string_lossy();
        if name.ends_with(".tl") && !name.ends_with(".d.tl") {
            out.push(p.to_path_buf());
        }
    });
    out.sort();
    out
}

/// Calls `f` for every file under `dir`.
pub fn walk(dir: &Path, f: &mut dyn FnMut(&Path)) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }
        if p.is_dir() {
            walk(&p, f);
        } else {
            f(&p);
        }
    }
}

/// `*` within a name, `**` across folders, matched against the path's end.
pub fn glob(pattern: &str, path: &str) -> bool {
    fn go(p: &[u8], s: &[u8]) -> bool {
        match p.first() {
            None => s.is_empty(),
            Some(b'*') if p.get(1) == Some(&b'*') => (0..=s.len()).any(|i| go(&p[2..], &s[i..])),
            Some(b'*') => (0..=s.len()).take_while(|&i| i == 0 || s[i - 1] != b'/').any(|i| go(&p[1..], &s[i..])),
            Some(c) => s.first() == Some(c) && go(&p[1..], &s[1..]),
        }
    }
    let p = pattern.as_bytes();
    let s = path.as_bytes();
    (0..=s.len()).filter(|&i| i == 0 || s[i - 1] == b'/').any(|i| go(p, &s[i..]))
}
