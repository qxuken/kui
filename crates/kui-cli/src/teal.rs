//! Teal, built in and optional (ADR 0064, decision 8): `require` finds a
//! `.tl` module and compiles it as it loads, an entry may be `main.tl`,
//! and `kui check` type-checks an app against kui's declarations, Kit's
//! and the runner's own. The compiler is the one file kui-lua vendors,
//! loaded only when a `.tl` file is actually found.

use std::path::{Path, PathBuf};

use mlua::Lua;

const TL: &str = include_str!("../../kui-lua/tests/tl/tl.lua");
const KUI_DECLS: &str = include_str!("../../../packages/lua/kui.d.tl");
const KIT_DECLS: &str = include_str!("../../../addons/kit/bindings/teal/kit/ui.d.tl");
const RUNNER_DECLS: &str = include_str!("lua/runner.d.tl");

/// The searcher, in Lua: a `.tl` beside the `.lua` path would be.
const SEARCHER: &str = r#"
local searchers = package.searchers
table.insert(searchers, 2, function(name)
  local path = package.searchpath(name, (package.path:gsub("%.lua", ".tl")))
  if not path or path:match("%.d%.tl$") then return nil end
  local tl = require("tl")
  local f = assert(io.open(path, "rb"))
  local src = f:read("a")
  f:close()
  local chunk, err = tl.load(src, "@" .. path, "t")
  if not chunk then error(err, 0) end
  return chunk, path
end)
"#;

/// Makes `require("tl")` the compiler and adds the `.tl` searcher.
pub fn install(lua: &Lua) -> mlua::Result<()> {
    let preload: mlua::Table = lua
        .globals()
        .get::<mlua::Table>("package")?
        .get("preload")?;
    preload.set(
        "tl",
        lua.create_function(|lua, ()| lua.load(TL).set_name("@tl.lua").call::<mlua::Value>(()))?,
    )?;
    lua.load(SEARCHER).set_name("@kui:teal").exec()
}

/// The Lua to run for `file`: its own text, or the text Teal compiles a
/// `.tl` file to. A syntax error is an error; a type error is `kui
/// check`'s to report, as `tl run` does.
pub fn source_for(file: &Path, source: &str) -> mlua::Result<String> {
    if file.extension().and_then(|e| e.to_str()) != Some("tl") {
        return Ok(source.to_string());
    }
    let lua = Lua::new();
    let tl: mlua::Table = lua.load(TL).set_name("@tl.lua").call(())?;
    let generate: mlua::Function = tl.get("gen")?;
    let (code, result): (Option<String>, mlua::Table) = generate.call(source)?;
    match code {
        Some(code) => Ok(code),
        None => {
            let errs: mlua::Table = result.get("syntax_errors")?;
            let mut out = Vec::new();
            for e in errs.sequence_values::<mlua::Table>() {
                let e = e?;
                out.push(format!(
                    "{}:{}:{}: {}",
                    file.display(),
                    e.get::<i64>("y")?,
                    e.get::<i64>("x")?,
                    e.get::<String>("msg")?
                ));
            }
            Err(mlua::Error::runtime(out.join("\n")))
        }
    }
}

/// `kui.d.tl` with the runner's API spliced into its `kui` record.
fn kui_decls() -> String {
    let start = KUI_DECLS.find("global type kui = record").unwrap_or(0);
    // The record's own `end`: the first one at the start of a line after it.
    match KUI_DECLS[start..].find("\nend\n") {
        Some(rel) => {
            let at = start + rel + 1;
            format!("{}{}\n{}", &KUI_DECLS[..at], RUNNER_DECLS, &KUI_DECLS[at..])
        }
        None => KUI_DECLS.to_string(),
    }
}

const KUI_META: &str = include_str!("../../../packages/lua/kui.lua");
const KIT_META: &str = include_str!("../../../addons/kit/bindings/lua/kit/ui.lua");
const RUNNER_META: &str = include_str!("lua/runner_meta.lua");

/// Writes every declaration an editor reads into `dir`: Teal's `kui.d.tl`
/// (with the runner's API) and `kit/ui.d.tl`, and lua-language-server's
/// `kui.lua`, `kui_runner.lua` and Kit's `kit/ui.lua` — each whole or not
/// at all, so a check reading the folder while another writes it never
/// reads one part written.
pub fn write_types(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("kit"))?;
    write_whole(&dir.join("kui.d.tl"), &kui_decls())?;
    write_whole(&dir.join("kit").join("ui.d.tl"), KIT_DECLS)?;
    write_whole(&dir.join("kui.lua"), KUI_META)?;
    write_whole(&dir.join("kui_runner.lua"), RUNNER_META)?;
    write_whole(&dir.join("kit").join("ui.lua"), KIT_META)?;
    Ok(())
}

/// `text` at `path`, written beside it under a name of this call's and
/// renamed over it: a reader sees the file before or the file after.
/// Nothing written when it holds the text already, as a second check of
/// one version finds it.
pub(crate) fn write_whole(path: &Path, text: &str) -> std::io::Result<()> {
    if std::fs::read(path).is_ok_and(|b| b == text.as_bytes()) {
        return Ok(());
    }
    // The process and a count: two threads of one process — two checks,
    // or the tests — each write a part of their own, where one renamed
    // the other's away.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut part = path.as_os_str().to_owned();
    part.push(format!(".{}.{n}.part", std::process::id()));
    let part = PathBuf::from(part);
    std::fs::write(&part, text)?;
    // Windows refuses a rename over a file something holds open without
    // sharing its deletion — a scanner looking at the new file, or a
    // reader opening it that instant — as `PermissionDenied`, so the
    // rename is tried again, for 0.65 s at most; a file that holds the
    // text by then was written by another check (backlog F242: the test
    // below, once under a full parallel test run on Windows).
    let mut tries = 0;
    loop {
        match std::fs::rename(&part, path) {
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && tries < 25 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(2 * tries));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                if std::fs::read(path).is_ok_and(|b| b == text.as_bytes()) {
                    return Ok(());
                }
                return Err(e);
            }
            Ok(()) => return Ok(()),
        }
    }
}

/// Writes the declarations `kui check` reads into a cache folder of this
/// version's.
fn decls_dir() -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("kui-{}-types", env!("CARGO_PKG_VERSION")));
    write_types(&dir)?;
    Ok(dir)
}

/// One problem `kui check` found.
pub struct Problem {
    pub file: PathBuf,
    pub line: i64,
    pub col: i64,
    pub msg: String,
}

/// Type-checks every `.tl` file under `dir` (not `.d.tl`), against kui's,
/// Kit's and the runner's declarations.
pub fn check(dir: &Path, files: &[PathBuf]) -> Result<Vec<Problem>, String> {
    let types = decls_dir().map_err(|e| format!("writing the type declarations: {e}"))?;
    let lua = Lua::new();
    let run = || -> mlua::Result<Vec<Problem>> {
        let tl: mlua::Table = lua.load(TL).set_name("@tl.lua").call(())?;
        tl.set(
            "path",
            format!("{}/?.lua;{}/?.lua", types.display(), dir.display()),
        )?;
        let new_env: mlua::Function = tl.get("new_env")?;
        let check_string: mlua::Function = tl.get("check_string")?;
        let mut out = Vec::new();
        for file in files {
            let source = std::fs::read_to_string(file)
                .map_err(|e| mlua::Error::runtime(format!("{}: {e}", file.display())))?;
            let opts = lua.create_table()?;
            let defaults = lua.create_table()?;
            defaults.set("gen_target", "5.4")?;
            defaults.set("gen_compat", "off")?;
            opts.set("defaults", defaults)?;
            opts.set("predefined_modules", lua.create_sequence_from(["kui"])?)?;
            let env: mlua::Value = new_env.call(opts)?;
            let name = file.to_string_lossy().into_owned();
            let result: mlua::Table = check_string.call((source, env, name))?;
            for key in ["syntax_errors", "type_errors"] {
                let Ok(errs) = result.get::<mlua::Table>(key) else {
                    continue;
                };
                for e in errs.sequence_values::<mlua::Table>() {
                    let e = e?;
                    let at: Option<String> = e.get("filename")?;
                    out.push(Problem {
                        file: at.map(PathBuf::from).unwrap_or_else(|| file.clone()),
                        line: e.get::<Option<i64>>("y")?.unwrap_or(0),
                        col: e.get::<Option<i64>>("x")?.unwrap_or(0),
                        msg: e.get::<Option<String>>("msg")?.unwrap_or_default(),
                    });
                }
            }
        }
        Ok(out)
    };
    run().map_err(|e| e.to_string())
}

#[cfg(test)]
mod decls_tests {
    /// `kui check` runs read the declarations folder another run is
    /// writing — the tests run eight at once, an editor's check beside the
    /// command line's — and a reader never sees a file part written:
    /// read mid-write, `kui.d.tl` was empty and every name in it unknown
    /// (`unknown variable: kui`, PR #7's CI). The writer writes a stale
    /// folder each time, as a check of another version would — a folder
    /// of the test's own: the version's real one is what every other
    /// check reads, and a `kui check` in another test binary run beside
    /// this one found its `kui.d.tl` saying "stale".
    #[test]
    fn a_reader_never_sees_a_declaration_part_written() {
        let whole = super::kui_decls();
        let dir = std::env::temp_dir().join(format!("kui-decls-test-{}", std::process::id()));
        super::write_types(&dir).unwrap();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let writer = {
            let stop = stop.clone();
            let dir = dir.clone();
            std::thread::spawn(move || {
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    super::write_whole(&dir.join("kui.d.tl"), "stale").unwrap();
                    super::write_types(&dir).unwrap();
                }
            })
        };
        let file = dir.join("kui.d.tl");
        let mut torn = 0;
        for _ in 0..20_000 {
            if let Ok(read) = std::fs::read_to_string(&file)
                && read != whole
                && read != "stale"
            {
                torn += 1;
            }
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        writer.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(torn, 0, "reads of a part-written kui.d.tl");
    }
}
