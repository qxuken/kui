use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::json;

use super::{File, Opts, collect};
use crate::{
    archive::{Entry, ZipReader},
    project::Project,
    release,
};

const APP_BUNDLE: &str = "app.kuiweb";

pub(super) fn pack(
    project: &Project,
    opts: &Opts,
    runner_version: &str,
    version: &str,
) -> Result<i32, String> {
    if opts.formats.iter().any(|format| format != "web")
        || opts.bytecode
        || opts.installer.is_some()
        || !opts.targets.is_empty()
    {
        return Err(
            "--format web is used on its own, without --target, --bytecode or --installer".into(),
        );
    }
    let requested: PathBuf = opts.out.components().collect();
    existing(&requested, true)?;
    fs::create_dir_all(&requested).map_err(|e| path_error(&requested, e))?;
    let out = crate::paths::canonical(&requested).map_err(|e| path_error(&requested, e))?;
    if project.dir.starts_with(&out) {
        return Err("the web output folder must be separate from the app's source folder".into());
    }

    let entry = project
        .entry
        .strip_prefix(&project.dir)
        .map_err(|_| "the entry is outside the app's folder")?;
    let entry = bundle_path(entry)?.replace('\\', "/");
    if !entry.ends_with(".lua") && !entry.ends_with(".tl") {
        return Err("the web runner needs a Lua or Teal entry".into());
    }
    let mut files = collect(project, Some(&out))?;
    let mut names = BTreeSet::new();
    for file in &mut files {
        prepare(project, file)?;
        if !names.insert(file.name.clone()) {
            return Err(format!(
                "{}: two app files have the same packed path",
                file.name
            ));
        }
    }
    for name in &names {
        for (offset, _) in name.match_indices('/') {
            if names.contains(&name[..offset]) {
                return Err(format!("{name}: an app file occupies its parent path"));
            }
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    let entry = lua_name(&entry);
    if !names.contains(&entry) {
        return Err(format!("{entry}: the entry is missing from the web bundle"));
    }

    let bytes = release::web_runner(
        runner_version,
        opts.mirror.as_deref(),
        opts.runner.as_deref(),
    )?;
    let runner = ZipReader::open(bytes.into()).map_err(|e| format!("web runner zip: {e}"))?;
    let members = runner_members(&runner)?;
    let runner_pin = members["runner-version.txt"]
        .read()
        .map_err(|e| format!("web runner version: {e}"))?;
    if String::from_utf8_lossy(&runner_pin).trim() != runner_version {
        return Err(format!(
            "web runner zip is for {}, but the app requires kui {runner_version}",
            String::from_utf8_lossy(&runner_pin).trim()
        ));
    }

    let bundle_files: Vec<_> = files
        .iter()
        .map(|file| (&file.name, file.data.len()))
        .collect();
    let header = serde_json::to_vec(&json!({
        "schema": 1,
        "entry": entry,
        "id": project.id,
        "name": project.name,
        "version": version,
        "runner": runner_version,
        "size": project.size,
        "minWidth": project.min_size.map(|size| size.0),
        "files": bundle_files,
    }))
    .map_err(|e| e.to_string())?;
    let header_len = u32::try_from(header.len()).map_err(|_| "web bundle manifest is too large")?;
    let stage = tempfile::Builder::new()
        .prefix(".kui-web-")
        .tempdir_in(&out)
        .map_err(|e| path_error(&out, e))?;
    let ready = stage.path().join("new");
    for (name, item) in &members {
        let bytes = item.read().map_err(|e| format!("{name}: {e}"))?;
        if item.is_dir() {
            if !bytes.is_empty() {
                return Err(format!("web runner directory has contents: {name}"));
            }
            continue;
        }
        write_staged(&ready.join(name), |file| file.write_all(&bytes))?;
    }
    write_staged(&ready.join(APP_BUNDLE), |file| {
        file.write_all(&header_len.to_le_bytes())?;
        file.write_all(&header)?;
        for app_file in &files {
            file.write_all(&app_file.data)?;
        }
        Ok(())
    })?;
    install(&out, stage, &members)?;
    println!("{}", out.join("index.html").display());
    Ok(0)
}

fn lua_name(name: &str) -> String {
    name.strip_suffix(".tl")
        .map_or_else(|| name.to_owned(), |stem| format!("{stem}.lua"))
}

pub(super) fn bundle_path(path: &Path) -> Result<&str, String> {
    let name = path
        .to_str()
        .ok_or_else(|| format!("{}: web bundle paths must be UTF-8", path.display()))?;
    if !cfg!(windows) && name.contains('\\') {
        return Err(format!(
            "{name}: web bundle paths cannot contain backslashes"
        ));
    }
    Ok(name)
}

fn prepare(project: &Project, file: &mut File) -> Result<(), String> {
    if !crate::archive::is_safe_name(&file.name)
        || file
            .name
            .split('/')
            .any(|part| part.is_empty() || part == ".")
    {
        return Err(format!("invalid web bundle path: {}", file.name));
    }
    if file.name.ends_with(".tl") && !file.name.ends_with(".d.tl") && file.name != "kui.config.tl" {
        let path = project.dir.join(&file.name);
        let text = std::str::from_utf8(&file.data).map_err(|e| format!("{}: {e}", file.name))?;
        file.data = crate::teal::source_for(&path, text)
            .map_err(|e| format!("{}: {e}", file.name))?
            .into_bytes();
        file.name = lua_name(&file.name);
    }
    Ok(())
}

/// The release runner has two top-level files and a kui/ subtree. Treat
/// path aliases as collisions so the same ZIP works on any host filesystem.
fn runner_members(runner: &ZipReader) -> Result<BTreeMap<&str, Entry<'_>>, String> {
    let mut members = BTreeMap::new();
    let mut paths = BTreeMap::new();
    for item in runner.entries() {
        let raw = item.name();
        if item.is_special() {
            return Err(format!("special file in web runner zip: {raw}"));
        }
        let dir = item.is_dir();
        let name = if dir {
            raw.strip_suffix('/').unwrap_or(raw)
        } else {
            raw
        };
        if !(name.starts_with("kui/")
            || dir && name == "kui"
            || !dir && matches!(name, "index.html" | "runner-version.txt"))
            || !name.split('/').all(portable_component)
        {
            return Err(format!("unexpected path in web runner zip: {raw}"));
        }
        for (offset, _) in name.match_indices('/') {
            check_member_path(&mut paths, &name[..offset], true)?;
        }
        check_member_path(&mut paths, name, dir)?;
        if members.insert(name, item).is_some() {
            return Err(format!("duplicate path in web runner zip: {name}"));
        }
    }
    for name in [
        "index.html",
        "runner-version.txt",
        "kui/runner.wasm",
        "kui/web-app.js",
    ] {
        if !members.get(name).is_some_and(|item| !item.is_dir()) {
            return Err(format!("web runner zip has no file {name}"));
        }
    }
    Ok(members)
}

fn portable_component(part: &str) -> bool {
    if part.is_empty()
        || part.ends_with('.')
        || !part
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return false;
    }
    let stem = part.split('.').next().unwrap().to_ascii_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit())
}

fn check_member_path<'a>(
    paths: &mut BTreeMap<String, (&'a str, bool)>,
    name: &'a str,
    dir: bool,
) -> Result<(), String> {
    let prior = paths
        .entry(name.to_ascii_lowercase())
        .or_insert((name, dir));
    if *prior != (name, dir) {
        return Err(format!(
            "conflicting paths in web runner zip: {} and {name}",
            prior.0
        ));
    }
    Ok(())
}

fn path_error(path: &Path, error: io::Error) -> String {
    format!("{}: {error}", path.display())
}

/// Inspect the path itself, so even a dangling symlink is rejected.
fn existing(path: &Path, dir: bool) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(format!(
            "{}: the web output path is a symlink",
            path.display()
        )),
        Ok(meta) if if dir { meta.is_dir() } else { meta.is_file() } => Ok(true),
        Ok(_) => {
            let kind = if dir { "directory" } else { "regular file" };
            Err(format!("{}: the web output needs a {kind}", path.display()))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(path_error(path, e)),
    }
}

fn write_staged(
    path: &Path,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> Result<(), String> {
    (|| -> io::Result<()> {
        fs::create_dir_all(path.parent().expect("staged output has a parent"))?;
        let mut file = fs::File::create_new(path)?;
        write(&mut file)?;
        // The built site may be served by a different account.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o644))?;
        }
        Ok(())
    })()
    .map_err(|e| path_error(path, e))
}

fn install(
    out: &Path,
    stage: tempfile::TempDir,
    members: &BTreeMap<&str, Entry<'_>>,
) -> Result<(), String> {
    // Keep the lock inode between invocations so waiting writers and new
    // writers always lock the same file.
    let lock_path = out.join(".kui-web.lock");
    existing(&lock_path, false)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|e| path_error(&lock_path, e))?;
    lock.lock().map_err(|e| path_error(&lock_path, e))?;

    let mut dirs = BTreeSet::new();
    for (name, item) in members {
        for (offset, _) in name.match_indices('/') {
            dirs.insert(&name[..offset]);
        }
        if item.is_dir() {
            dirs.insert(name);
        }
    }
    let dirs = dirs
        .into_iter()
        .map(|name| {
            let path = out.join(name);
            Ok((!existing(&path, true)?).then_some(path))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let names = members
        .iter()
        .filter_map(|(&name, item)| (!item.is_dir() && name != "index.html").then_some(name))
        .chain([APP_BUNDLE, "index.html"]);
    let destinations = names
        .map(|name| Ok((name, existing(&out.join(name), false)?)))
        .collect::<Result<Vec<_>, String>>()?;
    replace(out, stage, dirs.into_iter().flatten(), destinations)
}

fn replace(
    out: &Path,
    stage: tempfile::TempDir,
    dirs: impl Iterator<Item = PathBuf>,
    destinations: Vec<(&str, bool)>,
) -> Result<(), String> {
    let ready = stage.path().join("new");
    let old = stage.path().join("old");
    fs::create_dir(&old).map_err(|e| path_error(&old, e))?;
    let mut created = Vec::new();
    let mut changes = Vec::new();
    let result = (|| -> Result<(), String> {
        for dir in dirs {
            fs::create_dir(&dir).map_err(|e| path_error(&dir, e))?;
            created.push(dir);
        }
        for (name, existed) in destinations {
            let dest = out.join(name);
            let backup = if existed {
                let path = old.join(name);
                let parent = path.parent().expect("backup has a parent");
                fs::create_dir_all(parent).map_err(|e| path_error(parent, e))?;
                fs::rename(&dest, &path).map_err(|e| path_error(&dest, e))?;
                Some(path)
            } else {
                None
            };
            changes.push((dest, backup, false));
            let (dest, _, installed) = changes.last_mut().unwrap();
            fs::rename(ready.join(name), &dest).map_err(|e| path_error(dest, e))?;
            *installed = true;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut recovery_errors = Vec::new();
        for (dest, backup, installed) in changes.into_iter().rev() {
            let result = if let Some(backup) = backup {
                fs::rename(backup, &dest)
            } else if installed {
                fs::remove_file(&dest)
            } else {
                Ok(())
            };
            if let Err(e) = result {
                recovery_errors.push(path_error(&dest, e));
            }
        }
        for dir in created.into_iter().rev() {
            if let Err(e) = fs::remove_dir(&dir) {
                recovery_errors.push(path_error(&dir, e));
            }
        }
        if !recovery_errors.is_empty() {
            let recovery = stage.keep();
            return Err(format!(
                "{error}. Web output rollback failed: {}. Recovery files are in {}",
                recovery_errors.join(", "),
                recovery.display()
            ));
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::ZipWriter;

    const PIN: &str = "0.1.0-alpha.79";

    fn runner_zip(index: &[u8], extra: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = ZipWriter::new();
        zip.add_dir("kui/").unwrap();
        for (name, bytes) in [
            ("runner-version.txt", PIN.as_bytes()),
            ("index.html", index),
            ("kui/runner.wasm", b"wasm fixture"),
            ("kui/web-app.js", b"// runtime"),
        ]
        .into_iter()
        .chain(extra.iter().copied())
        {
            zip.add_file(name, bytes, 0o644, false).unwrap();
        }
        zip.finish(0, b"").unwrap()
    }

    fn fixture(root: &Path) -> (Project, Opts) {
        let source = root.join("app");
        fs::create_dir_all(source.join("lib")).unwrap();
        for (name, body) in [
            (
                "kui.config.lua",
                r#"return { id = "dev.kui.fixture", pack = { exclude = { "omit.txt" } } }"#,
            ),
            ("main.tl", "local name: string = require('lib.label')"),
            ("lib/label.tl", "return 'bundled'"),
            ("asset.txt", "asset"),
            ("omit.txt", "excluded"),
        ] {
            fs::write(source.join(name), body).unwrap();
        }
        let runner = root.join("web.zip");
        fs::write(&runner, runner_zip(b"<title>runner</title>", &[])).unwrap();
        let opts = Opts {
            path: source.clone(),
            targets: vec![],
            formats: vec!["web".into()],
            out: source.join("preview"),
            bytecode: false,
            strip: false,
            installer: None,
            runner: Some(runner),
            mirror: None,
        };
        (crate::project::find(&source).unwrap(), opts)
    }

    fn patch_zip(zip: &mut [u8], from: &[u8], to: &[u8]) {
        assert_eq!(from.len(), to.len());
        let mut replaced = false;
        for pos in 0..=zip.len() - from.len() {
            if &zip[pos..pos + from.len()] == from {
                zip[pos..pos + from.len()].copy_from_slice(to);
                replaced = true;
            }
        }
        assert!(replaced);
    }

    #[test]
    fn web_pack_stages_a_pinned_runner_and_a_repeatable_project_bundle() {
        let temp = tempfile::tempdir().unwrap();
        let (project, opts) = fixture(temp.path());
        assert!(
            pack(&project, &opts, "0.1.0-alpha.78", "2.0.0")
                .unwrap_err()
                .contains("app requires kui")
        );

        pack(&project, &opts, PIN, "2.0.0").unwrap();
        let bytes = fs::read(opts.out.join(APP_BUNDLE)).unwrap();
        for name in ["notes.txt", "kui/custom.txt"] {
            fs::write(opts.out.join(name), name).unwrap();
        }
        pack(&project, &opts, PIN, "2.0.0").unwrap();
        assert_eq!(fs::read(opts.out.join(APP_BUNDLE)).unwrap(), bytes);
        for name in ["notes.txt", "kui/custom.txt"] {
            assert_eq!(fs::read_to_string(opts.out.join(name)).unwrap(), name);
        }

        let length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let header: serde_json::Value = serde_json::from_slice(&bytes[4..4 + length]).unwrap();
        let names: Vec<_> = header["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file[0].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            ["asset.txt", "kui.config.lua", "lib/label.lua", "main.lua"]
        );
        assert_eq!(header["entry"], "main.lua");
        assert_eq!(header["runner"], PIN);
        let source = std::str::from_utf8(&bytes[4 + length..]).unwrap();
        assert!(source.contains("require(") && !source.contains("name: string"));
    }

    #[cfg(unix)]
    #[test]
    fn web_pack_rejects_asset_paths_it_cannot_preserve() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let temp = tempfile::tempdir().unwrap();
        let (project, opts) = fixture(temp.path());
        fs::write(project.dir.join(r"literal\asset.txt"), b"asset").unwrap();
        assert!(
            pack(&project, &opts, PIN, "2.0.0")
                .unwrap_err()
                .contains("web bundle paths cannot contain backslashes")
        );
        assert!(
            bundle_path(Path::new(OsStr::from_bytes(b"\xff.txt")))
                .unwrap_err()
                .contains("web bundle paths must be UTF-8")
        );
        assert!(!opts.out.join(APP_BUNDLE).exists());
    }

    #[test]
    fn web_pack_rejects_unsafe_runner_members_without_replacing_the_site() {
        let temp = tempfile::tempdir().unwrap();
        let (project, opts) = fixture(temp.path());
        pack(&project, &opts, PIN, "2.0.0").unwrap();
        let index = fs::read(opts.out.join("index.html")).unwrap();
        let bundle = fs::read(opts.out.join(APP_BUNDLE)).unwrap();
        let runner = opts.runner.as_ref().unwrap();

        let mut traversal = runner_zip(b"replacement", &[("kui/xx/xx/outside.txt", b"escape")]);
        patch_zip(&mut traversal, b"kui/xx/xx/", b"kui/../../");
        let mut duplicate = runner_zip(b"replacement", &[("kui/aliass.wasm", b"alias")]);
        patch_zip(&mut duplicate, b"kui/aliass.wasm", b"kui/runner.wasm");
        let mut corrupt = runner_zip(b"replacement", &[("kui/zz.txt", b"corrupt me")]);
        patch_zip(&mut corrupt, b"corrupt me", b"Corrupt me");
        let symlink_name = b"kui/link";
        let mut symlink = runner_zip(b"replacement", &[("kui/link", b"../target")]);
        let central_name = symlink
            .windows(symlink_name.len())
            .rposition(|name| name == symlink_name)
            .unwrap();
        // The high half of the central header's external attributes is the Unix mode.
        symlink[central_name - 6..central_name - 4].copy_from_slice(&0o120777_u16.to_le_bytes());
        for (zip, expected) in [
            (traversal, "unexpected path"),
            (duplicate, "duplicate path"),
            (corrupt, "CRC"),
            (symlink, "special file"),
            (
                runner_zip(b"replacement", &[("stray.txt", b"unexpected")]),
                "unexpected path",
            ),
            (
                runner_zip(b"replacement", &[("kui/Runner.wasm", b"alias")]),
                "conflicting paths",
            ),
            (
                runner_zip(b"replacement", &[("kui/runner.wasm/child", b"child")]),
                "conflicting paths",
            ),
        ] {
            fs::write(runner, zip).unwrap();
            let err = pack(&project, &opts, PIN, "2.1.0").unwrap_err();
            assert!(err.contains(expected), "{err}");
            assert_eq!(fs::read(opts.out.join("index.html")).unwrap(), index);
            assert_eq!(fs::read(opts.out.join(APP_BUNDLE)).unwrap(), bundle);
            assert!(!project.dir.join("outside.txt").exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn web_pack_does_not_follow_output_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let (project, opts) = fixture(temp.path());
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("runner.wasm"), b"untouched").unwrap();

        symlink(&outside, &opts.out).unwrap();
        assert!(
            pack(&project, &opts, PIN, "2.0.0")
                .unwrap_err()
                .contains("symlink")
        );
        fs::remove_file(&opts.out).unwrap();
        fs::create_dir(&opts.out).unwrap();
        symlink(&outside, opts.out.join("kui")).unwrap();
        assert!(
            pack(&project, &opts, PIN, "2.0.0")
                .unwrap_err()
                .contains("symlink")
        );
        fs::remove_file(opts.out.join("kui")).unwrap();
        fs::create_dir(opts.out.join("kui")).unwrap();
        symlink(
            outside.join("runner.wasm"),
            opts.out.join("kui/runner.wasm"),
        )
        .unwrap();
        assert!(
            pack(&project, &opts, PIN, "2.0.0")
                .unwrap_err()
                .contains("symlink")
        );

        assert_eq!(fs::read(outside.join("runner.wasm")).unwrap(), b"untouched");
        assert!(!outside.join("web-app.js").exists());
        assert!(!opts.out.join("index.html").exists());
    }

    #[test]
    fn failed_install_restores_overwritten_files_and_removes_new_paths() {
        let out = tempfile::tempdir().unwrap();
        let out = out.path();
        let stage = tempfile::tempdir_in(out).unwrap();
        fs::write(out.join("index.html"), b"old index").unwrap();
        fs::write(out.join(APP_BUNDLE), b"old bundle").unwrap();
        write_staged(&stage.path().join("new/index.html"), |file| {
            file.write_all(b"new index")
        })
        .unwrap();
        write_staged(&stage.path().join("new/kui/runner.wasm"), |file| {
            file.write_all(b"new runner")
        })
        .unwrap();
        let err = replace(
            out,
            stage,
            [out.join("kui")].into_iter(),
            vec![
                ("index.html", true),
                ("kui/runner.wasm", false),
                (APP_BUNDLE, true),
            ],
        )
        .unwrap_err();
        assert!(err.contains(APP_BUNDLE), "{err}");
        assert_eq!(fs::read(out.join("index.html")).unwrap(), b"old index");
        assert_eq!(fs::read(out.join(APP_BUNDLE)).unwrap(), b"old bundle");
        assert!(!out.join("kui").exists());
    }
}
