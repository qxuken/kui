use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use serde_json::json;

use super::{File, Opts, collect};
use crate::{archive::ZipReader, project::Project, release};

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
    std::fs::create_dir_all(&opts.out).map_err(|e| format!("{}: {e}", opts.out.display()))?;
    let out =
        crate::paths::canonical(&opts.out).map_err(|e| format!("{}: {e}", opts.out.display()))?;
    if project.dir.starts_with(&out) {
        return Err("the web output folder must be separate from the app's source folder".into());
    }

    let entry = project
        .entry
        .strip_prefix(&project.dir)
        .map_err(|_| "the entry is outside the app's folder")?
        .to_str()
        .ok_or("the app entry path is not UTF-8")?
        .replace('\\', "/");
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
    let runner_pin = runner
        .get("runner-version.txt")
        .ok_or("web runner zip has no runner-version.txt")?
        .read()
        .map_err(|e| format!("web runner version: {e}"))?;
    if String::from_utf8_lossy(&runner_pin).trim() != runner_version {
        return Err(format!(
            "web runner zip is for {}, but the app requires kui {runner_version}",
            String::from_utf8_lossy(&runner_pin).trim()
        ));
    }
    for name in ["index.html", "kui/runner.wasm", "kui/web-app.js"] {
        if runner.get(name).is_none() {
            return Err(format!("web runner zip has no {name}"));
        }
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
    for item in runner.entries().filter(|item| !item.is_dir()) {
        let name = item.name();
        if name != "index.html" && name != "runner-version.txt" && !name.starts_with("kui/") {
            return Err(format!("unexpected path in web runner zip: {name}"));
        }
        let bytes = item.read().map_err(|e| format!("{name}: {e}"))?;
        write(&out.join(name), |file| file.write_all(&bytes))?;
    }
    write(&out.join("app.kuiweb"), |file| {
        file.write_all(&header_len.to_le_bytes())?;
        file.write_all(&header)?;
        for app_file in &files {
            file.write_all(&app_file.data)?;
        }
        Ok(())
    })?;
    println!("{}", out.join("index.html").display());
    Ok(0)
}

fn lua_name(name: &str) -> String {
    name.strip_suffix(".tl")
        .map_or_else(|| name.to_owned(), |stem| format!("{stem}.lua"))
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

fn write(
    path: &Path,
    f: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<(), String> {
    let result = (|| -> std::io::Result<()> {
        let dir = path.parent().expect("web output has a parent");
        std::fs::create_dir_all(dir)?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        f(tmp.as_file_mut())?;
        // The built site may be served by a different account.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tmp.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o644))?;
        }
        tmp.persist(path)?;
        Ok(())
    })();
    result.map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::ZipWriter;

    #[test]
    fn web_pack_stages_a_pinned_runner_and_a_repeatable_project_bundle() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("app");
        std::fs::create_dir_all(source.join("lib")).unwrap();
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
            std::fs::write(source.join(name), body).unwrap();
        }
        let runner = temp.path().join("web.zip");
        let mut zip = ZipWriter::new();
        for (name, bytes) in [
            ("runner-version.txt", b"0.1.0-alpha.79".as_slice()),
            ("index.html", b"<title>runner</title>"),
            ("kui/runner.wasm", b"wasm fixture"),
            ("kui/web-app.js", b"// runtime"),
        ] {
            zip.add_file(name, bytes, 0o644, false).unwrap();
        }
        std::fs::write(&runner, zip.finish(0, b"").unwrap()).unwrap();
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
        let project = crate::project::find(&source).unwrap();
        assert!(
            pack(&project, &opts, "0.1.0-alpha.78", "2.0.0")
                .unwrap_err()
                .contains("app requires kui")
        );
        for _ in 0..2 {
            pack(&project, &opts, "0.1.0-alpha.79", "2.0.0").unwrap();
            let bytes = std::fs::read(opts.out.join("app.kuiweb")).unwrap();
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
            assert_eq!(header["runner"], "0.1.0-alpha.79");
            let source = std::str::from_utf8(&bytes[4 + length..]).unwrap();
            assert!(source.contains("require(") && !source.contains("name: string"));
        }
    }
}
