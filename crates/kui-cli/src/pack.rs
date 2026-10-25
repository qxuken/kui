//! `kui pack` (ADR 0068, decisions 10–17): an app built for any target on
//! any machine. Nothing is compiled for a target — a runner app is Lua,
//! the same bytes everywhere — so a target's build is its prebuilt runner
//! with the app's archive added: appended to a Windows `exe` or a Linux
//! binary, a file inside a macOS `.app`, or an npm package of sources.

use std::path::{Path, PathBuf};

use crate::archive::{ZipWriter, write_tar_gz};
use crate::project::{self, Project};
use crate::release;

/// What `kui pack` was asked.
pub struct Opts {
    pub path: PathBuf,
    pub targets: Vec<String>,
    pub formats: Vec<String>,
    pub out: PathBuf,
    pub bytecode: bool,
    pub strip: bool,
    pub installer: Option<String>,
    pub runner: Option<PathBuf>,
    pub mirror: Option<String>,
}

/// The archive's own name for what `kui pack` learned.
pub const PACK_FILE: &str = "kui.pack.lua";
/// Where the app's files sit in the archive.
pub const APP_DIR: &str = "app/";
/// The entry a bytecode archive starts from: source, so the runner's
/// loader stays text-only, requiring the app's compiled entry.
pub const BYTECODE_ENTRY: &str = "__kui_entry.lua";

/// One file of the app, as it goes into the archive.
struct File {
    /// Relative to the app's folder, with `/`.
    name: String,
    data: Vec<u8>,
    mode: u32,
}

pub fn run(opts: &Opts) -> Result<i32, String> {
    let project = project::find(&opts.path)?;
    if !project.id_given {
        return Err(format!(
            "{}: kui pack needs an id in kui.config.lua — reverse DNS, as id = \"dev.example.{}\" — \
             it names the app's data folder and its macOS bundle",
            project.dir.display(),
            project.id
        ));
    }
    let version = project.version.clone().unwrap_or_else(|| "0.0.0".into());
    let runner_version = project.pin.clone().unwrap_or_else(|| crate::compat::VERSION.to_string());
    if opts.bytecode && runner_version != crate::compat::VERSION {
        return Err(format!(
            "--bytecode compiles with this runner's Lua, and the app pins kui {runner_version}: pack with \
             that runner (`kui upgrade {runner_version}`), or leave out --bytecode"
        ));
    }
    std::fs::create_dir_all(&opts.out).map_err(|e| format!("{}: {e}", opts.out.display()))?;
    let files = collect(&project)?;
    let slug = slug(&project.name);
    let mut written: Vec<PathBuf> = Vec::new();

    let formats: Vec<String> = if opts.formats.is_empty() { Vec::new() } else { opts.formats.clone() };
    if formats.iter().any(|f| f == "npm") {
        written.push(npm(&project, &files, &opts.out, &runner_version, &version)?);
    }
    if formats.iter().any(|f| f == "kui") {
        let archive = archive(&project, &files, opts, &runner_version, &version, 0)?;
        let path = opts.out.join(format!("{slug}-{version}.kui"));
        std::fs::write(&path, archive).map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(path);
    }
    let platform_formats = formats.iter().any(|f| matches!(f.as_str(), "exe" | "app" | "bin")) || formats.is_empty();
    if platform_formats {
        let targets = if opts.targets.is_empty() {
            vec![release::current_platform()
                .ok_or("this machine has no runner of its own to pack; name --target")?
                .to_string()]
        } else if opts.targets.iter().any(|t| t == "all") {
            release::PLATFORMS.iter().map(|s| s.to_string()).collect()
        } else {
            opts.targets.clone()
        };
        let icon = icon(&project)?;
        for t in &targets {
            let platform = release::check_platform(t)?;
            let format = default_format(platform);
            if !formats.is_empty() && !formats.iter().any(|f| f == format) {
                continue;
            }
            eprintln!("kui: packing {} for {platform} (kui {runner_version})", project.name);
            let runner = release::runner(&runner_version, platform, opts.mirror.as_deref(), opts.runner.as_deref())?;
            let path = match format {
                "exe" => exe(&project, &files, opts, runner, &runner_version, &version, &icon)?,
                "app" => app(&project, &files, opts, runner, &runner_version, &version, &icon)?,
                _ => bin(&project, &files, opts, runner, &runner_version, &version, platform, &icon)?,
            };
            written.push(path);
        }
    }
    if written.is_empty() {
        return Err(format!("nothing to pack: --format takes exe, app, bin, npm or kui (got {})", formats.join(", ")));
    }
    let sums: String = written
        .iter()
        .filter(|p| p.is_file())
        .filter_map(|p| {
            let bytes = std::fs::read(p).ok()?;
            Some(format!("{}  {}\n", release::sha256_hex(&bytes), p.file_name()?.to_string_lossy()))
        })
        .collect();
    std::fs::write(opts.out.join("SHA256SUMS"), &sums).map_err(|e| e.to_string())?;
    if let Some(base) = &opts.installer {
        installer(&project, base, &opts.out, &slug, &version)?;
    }
    for p in &written {
        println!("{}", p.display());
    }
    Ok(0)
}

/// The format a target ships as.
pub fn default_format(platform: &str) -> &'static str {
    if platform.starts_with("win32") {
        "exe"
    } else if platform.starts_with("darwin") {
        "app"
    } else {
        "bin"
    }
}

/// A name for files: the app's name with anything odd turned into `-`.
fn slug(name: &str) -> String {
    project::default_id(name)
}

/// The app's files, as the archive takes them: everything under its folder
/// but `.git`, `node_modules`, `target`, hidden files, a top-level `test/`,
/// `.kui/` and what `pack.exclude` names.
fn collect(project: &Project) -> Result<Vec<File>, String> {
    let mut out = Vec::new();
    let mut err = None;
    project::walk(&project.dir, &mut |p| {
        let Ok(rel) = p.strip_prefix(&project.dir) else { return };
        let name = rel.to_string_lossy().replace('\\', "/");
        if name == "test" || name.starts_with("test/") || name.starts_with("dist/") {
            return;
        }
        if project.pack.exclude.iter().any(|g| project::glob(g, &name)) {
            return;
        }
        match std::fs::read(p) {
            Ok(data) => out.push(File { name, data, mode: mode_of(p) }),
            Err(e) => err = Some(format!("{}: {e}", p.display())),
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[cfg(unix)]
fn mode_of(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).map(|m| m.permissions().mode() & 0o777).unwrap_or(0o644)
}

#[cfg(not(unix))]
fn mode_of(_: &Path) -> u32 {
    0o644
}

/// The app's files as they go into the archive: as written, or — with
/// `--bytecode` — each `.lua` and `.tl` compiled (the manifest stays
/// source), plus a source entry that requires the compiled one.
fn prepared(project: &Project, files: &[File], opts: &Opts) -> Result<(Vec<File>, Option<String>), String> {
    if !opts.bytecode {
        return Ok((files.iter().map(|f| File { name: f.name.clone(), data: f.data.clone(), mode: f.mode }).collect(), None));
    }
    let lua = mlua::Lua::new();
    let mut out = Vec::new();
    for f in files {
        let is_config = f.name == "kui.config.lua" || f.name == "kui.config.tl";
        let code = f.name.ends_with(".lua") || (f.name.ends_with(".tl") && !f.name.ends_with(".d.tl"));
        if is_config || !code {
            out.push(File { name: f.name.clone(), data: f.data.clone(), mode: f.mode });
            continue;
        }
        let text = String::from_utf8(f.data.clone()).map_err(|_| format!("{}: not UTF-8", f.name))?;
        let path = project.dir.join(&f.name);
        let src = crate::teal::source_for(&path, &text).map_err(|e| format!("{}: {e}", f.name))?;
        let name = f.name.strip_suffix(".tl").map(|n| format!("{n}.lua")).unwrap_or_else(|| f.name.clone());
        let func = lua
            .load(&src)
            .set_name(format!("@{name}"))
            .into_function()
            .map_err(|e| format!("{}: {e}", f.name))?;
        out.push(File { name, data: func.dump(opts.strip), mode: f.mode });
    }
    let entry = project.entry.strip_prefix(&project.dir).map_err(|_| "the entry is outside the app's folder")?;
    let module = entry
        .with_extension("")
        .to_string_lossy()
        .replace(['/', '\\'], ".");
    out.push(File {
        name: BYTECODE_ENTRY.into(),
        data: format!("-- written by kui pack --bytecode: the app's entry, compiled\nrequire({module:?})\n").into_bytes(),
        mode: 0o644,
    });
    Ok((out, Some(BYTECODE_ENTRY.into())))
}

/// The archive: `kui.pack.lua` and the app under `app/`, its offsets
/// shifted by `base` — the runner's length when it is appended to one.
fn archive(
    project: &Project,
    files: &[File],
    opts: &Opts,
    runner_version: &str,
    version: &str,
    base: u64,
) -> Result<Vec<u8>, String> {
    let (files, entry) = prepared(project, files, opts)?;
    let mut hash_input = Vec::new();
    for f in &files {
        hash_input.extend_from_slice(f.name.as_bytes());
        hash_input.push(0);
        hash_input.extend_from_slice(release::sha256_hex(&f.data).as_bytes());
        hash_input.push(b'\n');
    }
    let hash = release::sha256_hex(&hash_input);
    let mut pack = format!(
        "-- written by kui pack: what packing learned (ADR 0068). The app reads it as kui.packed.\n\
         return {{\n  runner = {runner_version:?},\n  bytecode = {},\n  hash = {hash:?},\n  id = {:?},\n  name = {:?},\n  version = {version:?},\n",
        opts.bytecode, project.id, project.name
    );
    if let Some(e) = &entry {
        pack.push_str(&format!("  entry = {e:?},\n"));
    }
    pack.push_str("}\n");
    let mut zip = ZipWriter::new();
    zip.add_file(PACK_FILE, pack.as_bytes(), 0o644, true).map_err(|e| e.to_string())?;
    for f in &files {
        zip.add_file(&format!("{APP_DIR}{}", f.name), &f.data, f.mode, true)
            .map_err(|e| format!("{}: {e}", f.name))?;
    }
    zip.finish(base, format!("kui {runner_version} {hash}").as_bytes())
        .map_err(|e| e.to_string())
}

/// The app's icon at full size: its own PNG, else kui's.
fn icon(project: &Project) -> Result<crate::icons::Rgba, String> {
    match &project.icon {
        Some(p) => {
            let bytes = std::fs::read(p).map_err(|e| format!("icon {}: {e}", p.display()))?;
            let loaded = crate::icons::load_png(&bytes).map_err(|e| format!("icon {}: {e}", p.display()))?;
            for w in &loaded.warnings {
                eprintln!("kui: icon {}: {w}", p.display());
            }
            Ok(loaded.image)
        }
        None => Ok(crate::icons::default_icon()),
    }
}

/// Windows: the runner, its subsystem set, the archive appended.
#[allow(clippy::too_many_arguments)]
fn exe(
    project: &Project,
    files: &[File],
    opts: &Opts,
    mut runner: Vec<u8>,
    runner_version: &str,
    version: &str,
    _icon: &crate::icons::Rgba,
) -> Result<PathBuf, String> {
    crate::pe::set_subsystem(&mut runner, !project.pack.console)?;
    let zip = archive(project, files, opts, runner_version, version, runner.len() as u64)?;
    runner.extend_from_slice(&zip);
    let path = opts.out.join(format!("{}-{version}-win32-x64.exe", file_name(&project.name)));
    std::fs::write(&path, runner).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Linux: the runner with the archive appended, and its icon, in a
/// `.tar.gz`.
#[allow(clippy::too_many_arguments)]
fn bin(
    project: &Project,
    files: &[File],
    opts: &Opts,
    mut runner: Vec<u8>,
    runner_version: &str,
    version: &str,
    platform: &str,
    icon: &crate::icons::Rgba,
) -> Result<PathBuf, String> {
    let zip = archive(project, files, opts, runner_version, version, runner.len() as u64)?;
    runner.extend_from_slice(&zip);
    let slug = slug(&project.name);
    let png = crate::icons::png_of(icon, 512);
    let tar = write_tar_gz(&[(slug.as_str(), &runner, 0o755), (&format!("{slug}.png"), &png, 0o644)])
        .map_err(|e| e.to_string())?;
    let path = opts.out.join(format!("{slug}-{version}-{platform}.tar.gz"));
    std::fs::write(&path, tar).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// macOS: an `.app` — the runner as its executable, the archive as
/// `Resources/app.zip` (so the runner's signature still holds), the icon,
/// an `Info.plist` — shipped zipped. Packed on a Mac, the bundle is signed
/// ad hoc (`codesign --sign -`), which the runner's own signature does not
/// cover once it sits in a bundle; packed elsewhere it is not, and
/// `rcodesign` signs it (docs/releasing.md, ADR 0068 decision 16).
#[allow(clippy::too_many_arguments)]
fn app(
    project: &Project,
    files: &[File],
    opts: &Opts,
    runner: Vec<u8>,
    runner_version: &str,
    version: &str,
    icon: &crate::icons::Rgba,
) -> Result<PathBuf, String> {
    let name = file_name(&project.name);
    let archive = archive(project, files, opts, runner_version, version, 0)?;
    let build = opts.out.join(".build");
    let _ = std::fs::remove_dir_all(&build);
    let bundle = build.join(format!("{name}.app"));
    let contents = bundle.join("Contents");
    let put = |rel: &str, data: &[u8], mode: u32| -> Result<(), String> {
        let p = contents.join(rel);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&p, data).map_err(|e| format!("{}: {e}", p.display()))?;
        set_mode(&p, mode);
        Ok(())
    };
    put("Info.plist", info_plist(project, &name, version).as_bytes(), 0o644)?;
    put("PkgInfo", b"APPL????", 0o644)?;
    put(&format!("MacOS/{name}"), &runner, 0o755)?;
    put("Resources/app.zip", &archive, 0o644)?;
    put("Resources/AppIcon.icns", &crate::icons::icns(icon), 0o644)?;
    if cfg!(target_os = "macos") {
        let signed = std::process::Command::new("codesign")
            .args(["--force", "--sign", "-", "--timestamp=none"])
            .arg(&bundle)
            .output();
        match signed {
            Ok(o) if o.status.success() => {}
            Ok(o) => eprintln!("kui: codesign: {}", String::from_utf8_lossy(&o.stderr).trim()),
            Err(e) => eprintln!("kui: codesign not run ({e}); the .app is unsigned"),
        }
    }
    let mut zip = ZipWriter::new();
    zip_tree(&mut zip, &build, &bundle)?;
    let bytes = zip.finish(0, b"").map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&build);
    let path = opts.out.join(format!("{name}-{version}-darwin-arm64.app.zip"));
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Every folder and file under `dir`, named relative to `root`, modes kept.
fn zip_tree(zip: &mut ZipWriter, root: &Path, dir: &Path) -> Result<(), String> {
    let rel = |p: &Path| p.strip_prefix(root).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
    zip.add_dir(&rel(dir)).map_err(|e| e.to_string())?;
    let mut entries: Vec<_> = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            zip_tree(zip, root, &p)?;
        } else {
            let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            zip.add_file(&rel(&p), &data, mode_of(&p), true).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_mode(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_mode(_: &Path, _: u32) {}

/// A name for a bundle or an exe: the app's, without path separators.
fn file_name(name: &str) -> String {
    let n: String = name.chars().filter(|c| !matches!(c, '/' | '\\' | ':')).collect();
    if n.trim().is_empty() { "App".into() } else { n.trim().to_string() }
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn info_plist(project: &Project, exe: &str, version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>{name}</string>
  <key>CFBundleDisplayName</key><string>{name}</string>
  <key>CFBundleIdentifier</key><string>{id}</string>
  <key>CFBundleExecutable</key><string>{exe}</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>{version}</string>
  <key>CFBundleVersion</key><string>{version}</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
"#,
        name = xml(&project.name),
        id = xml(&project.id),
        exe = xml(exe),
        version = xml(version),
    )
}

/// npm: the app's sources (never bytecode), a `bin` shim, and an exact
/// dependency on the runner it pins — a package a company publishes to its
/// own registry.
fn npm(project: &Project, files: &[File], out: &Path, runner_version: &str, version: &str) -> Result<PathBuf, String> {
    let bin = project.id.rsplit('.').next().unwrap_or(&project.id).to_string();
    let name = project.pack.npm_name.clone().unwrap_or_else(|| bin.clone());
    let dir = out.join("npm").join(name.replace('/', "__"));
    let _ = std::fs::remove_dir_all(&dir);
    for f in files {
        let p = dir.join("app").join(&f.name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&p, &f.data).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    std::fs::create_dir_all(dir.join("bin")).map_err(|e| e.to_string())?;
    let shim = "#!/usr/bin/env node\n\
         // Written by kui pack --format npm: runs the app with the runner it pins.\n\
         'use strict';\n\
         const { spawnSync } = require('node:child_process');\n\
         const path = require('node:path');\n\
         const kui = require('@qxuken/kui-cli').path;\n\
         const app = path.join(__dirname, '..', 'app');\n\
         const r = spawnSync(kui, ['run', app, '--', ...process.argv.slice(2)], { stdio: 'inherit' });\n\
         if (r.error) { console.error(r.error.message); process.exit(1); }\n\
         process.exit(r.status === null ? 1 : r.status);\n";
    let shim_path = dir.join("bin").join(format!("{bin}.js"));
    std::fs::write(&shim_path, shim).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&shim_path, std::fs::Permissions::from_mode(0o755));
    }
    let pkg = format!(
        "{{\n  \"name\": {name:?},\n  \"version\": {version:?},\n  \"description\": {desc:?},\n  \
         \"bin\": {{ {bin:?}: \"bin/{bin}.js\" }},\n  \"files\": [\"bin/\", \"app/\"],\n  \
         \"dependencies\": {{ \"@qxuken/kui-cli\": {runner_version:?} }}\n}}\n",
        desc = format!("{}, on the kui runner", project.name),
    );
    std::fs::write(dir.join("package.json"), pkg).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// `--installer <base-url>`: an `install.sh` and an `install.ps1` that
/// fetch this app's build from where it is hosted, check its sum, and
/// install it in the user's own folders.
fn installer(project: &Project, base: &str, out: &Path, slug: &str, version: &str) -> Result<(), String> {
    let base = base.trim_end_matches('/');
    let name = file_name(&project.name);
    let sh = format!(
        r#"#!/bin/sh
# Installs {name} {version} (written by kui pack). Usage: curl -fsSL {base}/install.sh | sh
set -eu
BASE="{base}"
NAME="{name}"
SLUG="{slug}"
VERSION="{version}"
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) ASSET="$NAME-$VERSION-darwin-arm64.app.zip" ;;
  Linux-x86_64) ASSET="$SLUG-$VERSION-linux-x64.tar.gz" ;;
  Linux-aarch64|Linux-arm64) ASSET="$SLUG-$VERSION-linux-arm64.tar.gz" ;;
  *) echo "$NAME has no build for $(uname -s) $(uname -m)" >&2; exit 1 ;;
esac
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
curl -fsSL "$BASE/$ASSET" -o "$TMP/$ASSET"
curl -fsSL "$BASE/SHA256SUMS" -o "$TMP/SHA256SUMS"
WANT="$(grep " $ASSET\$" "$TMP/SHA256SUMS" | cut -d' ' -f1)"
if command -v sha256sum >/dev/null 2>&1; then GOT="$(sha256sum "$TMP/$ASSET" | cut -d' ' -f1)"; else GOT="$(shasum -a 256 "$TMP/$ASSET" | cut -d' ' -f1)"; fi
if [ -z "$WANT" ] || [ "$WANT" != "$GOT" ]; then echo "checksum mismatch for $ASSET; not installing" >&2; exit 1; fi
case "$ASSET" in
  *.app.zip)
    mkdir -p "$HOME/Applications"
    rm -rf "$HOME/Applications/$NAME.app"
    (cd "$HOME/Applications" && unzip -q "$TMP/$ASSET")
    echo "installed $HOME/Applications/$NAME.app"
    ;;
  *)
    mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/.local/share/icons/hicolor/512x512/apps"
    tar -xzf "$TMP/$ASSET" -C "$TMP"
    install -m 0755 "$TMP/$SLUG" "$HOME/.local/bin/$SLUG"
    install -m 0644 "$TMP/$SLUG.png" "$HOME/.local/share/icons/hicolor/512x512/apps/$SLUG.png"
    cat > "$HOME/.local/share/applications/$SLUG.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=$NAME
Exec=$HOME/.local/bin/$SLUG
Icon=$SLUG
Categories=Utility;
EOF
    echo "installed $HOME/.local/bin/$SLUG"
    case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) echo "add it to PATH: export PATH=\"\$HOME/.local/bin:\$PATH\"" ;; esac
    ;;
esac
"#
    );
    let ps1 = format!(
        r#"# Installs {name} {version} on Windows (written by kui pack).
# Usage: irm {base}/install.ps1 | iex
$ErrorActionPreference = 'Stop'
$Base = '{base}'
$Name = '{name}'
$Asset = "$Name-{version}-win32-x64.exe"
$Dir = Join-Path $env:LOCALAPPDATA "Programs\$Name"
$Tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $Tmp, $Dir | Out-Null
try {{
  Invoke-WebRequest -UseBasicParsing "$Base/$Asset" -OutFile (Join-Path $Tmp $Asset)
  $Sums = (Invoke-WebRequest -UseBasicParsing "$Base/SHA256SUMS").Content
  $Want = ($Sums -split "`n" | Where-Object {{ $_ -match "  $([regex]::Escape($Asset))$" }} | ForEach-Object {{ ($_ -split ' ')[0] }})
  $Got = (Get-FileHash -Algorithm SHA256 (Join-Path $Tmp $Asset)).Hash.ToLower()
  if (-not $Want -or $Want.ToLower() -ne $Got) {{ throw "checksum mismatch for $Asset; not installing" }}
  $Exe = Join-Path $Dir "$Name.exe"
  Move-Item -Force (Join-Path $Tmp $Asset) $Exe
  $Lnk = Join-Path ([Environment]::GetFolderPath('Programs')) "$Name.lnk"
  $Shell = New-Object -ComObject WScript.Shell
  $Link = $Shell.CreateShortcut($Lnk)
  $Link.TargetPath = $Exe
  $Link.Save()
  Write-Host "installed $Exe"
}} finally {{
  Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}}
"#
    );
    std::fs::write(out.join("install.sh"), sh).map_err(|e| e.to_string())?;
    std::fs::write(out.join("install.ps1"), ps1).map_err(|e| e.to_string())?;
    Ok(())
}
