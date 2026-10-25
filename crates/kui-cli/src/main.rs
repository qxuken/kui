//! `kui`: runs a Lua or Teal app with Kit and a standard library, and
//! nothing else to install (ADR 0064).
//!
//!   kui run [path] [--headless] [--watch] [--size WxH] [--appearance light|dark] [-- app args]
//!   kui check [path]
//!   kui new <dir>
//!   kui pack [path] [--target t,...] [--format f,...]
//!   kui upgrade [version]
//!   kui runners fetch <version> --to <dir>
//!
//! `kui <path>` is `kui run <path>`. A runner `kui pack` added an app to
//! runs that app: every argument is the app's, the runner's own come from
//! `KUI_FLAGS` (ADR 0068).

mod archive;
mod host;
mod icons;
mod compat;
mod kit;
mod modules;
mod ops;
mod pack;
mod packed;
mod pe;
mod paths;
mod project;
mod release;
mod rename;
mod teal;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
kui — run a Lua or Teal app with Kit and a standard library

usage:
  kui run [path] [options]   run the app at path (a .lua or .tl file, or a folder; default .)
  kui check [path]           type-check the app's .tl files
  kui new <dir>              start an app in dir
  kui types [path]           write editor declarations (LuaLS, Teal) into path/.kui/types
  kui pack [path] [options]  build the app for any platform (see kui pack --help)
  kui upgrade [version]      replace this kui with the release's (default: the latest)
  kui runners fetch <version> --to <dir>
                             download every platform's runner, for a mirror
  kui <path>                 the same as kui run <path>

run options:
  --watch        reload the app when its files change, keeping its model
  --headless     no window: for a server, a script, a test
  --size WxH     the window's size in logical px (default 1000x700)
  --devtools     open with the devtools panel (its Log tab has what the app printed)
  --no-rename    keep the name kui in the Dock (macOS names an app by its binary)
  --shot FILE    no window: write a frame to FILE (.png) and end
  --after MS     with --shot, how long to run first (default 800)
  --record MS    with --shot, record MS of frames into FILE (.gif, .png) instead of one
  --appearance light|dark
                 pin the light or dark appearance over the OS's (headless has none of its own)
  -- ARGS        what follows is the app's, in kui.args

An app defines view(env) and, to hear its events and kui.dispatch, update(msg).
";

struct Args {
    cmd: String,
    path: PathBuf,
    watch: bool,
    headless: bool,
    size: Option<(f64, f64)>,
    shot: Option<PathBuf>,
    after: f64,
    record: Option<f64>,
    /// After `--`: the app's own arguments.
    rest: Vec<String>,
    devtools: bool,
    no_rename: bool,
    appearance: kui_native::Appearance,
}

fn parse(argv: Vec<String>) -> Result<Args, String> {
    let mut it = argv.into_iter().peekable();
    let mut a = Args {
        cmd: "run".into(),
        path: PathBuf::from("."),
        watch: false,
        headless: false,
        size: None,
        shot: None,
        after: 800.0,
        record: None,
        rest: Vec::new(),
        devtools: false,
        no_rename: false,
        appearance: kui_native::Appearance::Unknown,
    };
    match it.peek().map(String::as_str) {
        Some("run" | "check" | "new" | "types") => a.cmd = it.next().unwrap(),
        Some("-h" | "--help" | "help") => {
            it.next();
            a.cmd = "help".into();
        }
        Some("-V" | "--version" | "version") => {
            it.next();
            a.cmd = "version".into();
        }
        _ => {}
    }
    let mut path = None;
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--" => {
                a.rest = it.by_ref().collect();
                break;
            }
            "--watch" => a.watch = true,
            "--devtools" => a.devtools = true,
            "--no-rename" => a.no_rename = true,
            "--headless" => a.headless = true,
            "--shot" => {
                a.shot = Some(PathBuf::from(it.next().ok_or("--shot needs a file, as out.png")?));
                a.headless = true;
            }
            "--record" => {
                let v = it.next().ok_or("--record needs milliseconds")?;
                a.record = Some(v.parse().map_err(|_| format!("--record: bad milliseconds {v}"))?);
            }
            "--after" => {
                let v = it.next().ok_or("--after needs milliseconds")?;
                a.after = v.parse().map_err(|_| format!("--after: bad milliseconds {v}"))?;
            }
            "--appearance" => {
                a.appearance = match it.next().as_deref() {
                    Some("light") => kui_native::Appearance::Light,
                    Some("dark") => kui_native::Appearance::Dark,
                    _ => return Err("--appearance needs light or dark".into()),
                };
            }
            "--size" => {
                let v = it.next().ok_or("--size needs WxH, as 1200x800")?;
                let (w, h) = v.split_once('x').ok_or("--size needs WxH, as 1200x800")?;
                a.size = Some((
                    w.parse().map_err(|_| format!("--size: bad width {w}"))?,
                    h.parse().map_err(|_| format!("--size: bad height {h}"))?,
                ));
            }
            "-h" | "--help" => a.cmd = "help".into(),
            other if other.starts_with("--") => return Err(format!("unknown option {other}\n\n{USAGE}")),
            other => {
                if path.replace(PathBuf::from(other)).is_some() {
                    return Err(format!("one path at a time\n\n{USAGE}"));
                }
            }
        }
    }
    if let Some(p) = path {
        a.path = p;
    }
    Ok(a)
}

fn main() -> ExitCode {
    // A runner with an app packed into it runs that app.
    if let Some(packed) = packed::detect() {
        return exit(packed.and_then(run_packed));
    }
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match argv.first().map(String::as_str) {
        Some("pack") => return exit(pack_cmd(&argv[1..])),
        Some("upgrade") => return exit(upgrade(&argv[1..])),
        Some("runners") => return exit(runners(&argv[1..])),
        _ => {}
    }
    let args = match parse(argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("kui: {e}");
            return ExitCode::from(2);
        }
    };
    let result = match args.cmd.as_str() {
        "help" => {
            print!("{USAGE}");
            Ok(0)
        }
        "version" => {
            println!("kui {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        "check" => check(&args),
        "new" => new(&args),
        "types" => types(&args.path).map(|()| 0),
        // `kui run app.kui`: an archive `kui pack` wrote, run as packed.
        _ if args.path.extension().is_some_and(|e| e == "kui") && args.path.is_file() => {
            packed::from_file(&args.path).and_then(|p| {
                let mut a = args;
                a.path = p.dir.clone();
                a.no_rename = true;
                run(&a, Some(&p))
            })
        }
        _ => run(&args, None),
    };
    exit(result)
}

fn exit(result: Result<i32, String>) -> ExitCode {
    match result {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(e) => {
            eprintln!("kui: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A packed app: every argument is the app's, and the runner's own flags
/// come from `KUI_FLAGS` (ADR 0068, decision 19).
fn run_packed(p: packed::Packed) -> Result<i32, String> {
    let flags: Vec<String> = std::env::var("KUI_FLAGS")
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let mut args = parse(flags).map_err(|e| format!("KUI_FLAGS: {e}"))?;
    args.cmd = "run".into();
    args.path = p.dir.clone();
    args.rest = std::env::args().skip(1).collect();
    // A packed app names itself: its bundle on macOS, its file elsewhere.
    args.no_rename = true;
    run(&args, Some(&p))
}

fn run(args: &Args, packed: Option<&packed::Packed>) -> Result<i32, String> {
    let mut project = project::find(&args.path)?;
    if let Some(p) = packed {
        if let Some(entry) = &p.entry {
            project.entry = project.dir.join(entry);
        }
        project.devtools_reachable = project.pack.devtools || args.devtools;
    }
    if !args.headless && !args.no_rename && project.rename {
        rename::relaunch_as(&project.name);
    }
    project::load_dotenv(&project.dir);
    modules::set_args(args.rest.clone());
    modules::set_name(project.name.clone());
    modules::set_app(modules::AppInfo {
        id: project.id.clone(),
        version: project.version.clone(),
        data_dir: paths::data_dir(&project.id),
        cache_dir: paths::cache_dir(&project.id),
        packed: packed.map(|p| (p.runner.clone(), p.bytecode, p.hash.clone())),
    });
    // The log's file (ADR 0068, decision 20): `KUI_LOG=<path>`, `-` for
    // none beside stderr; a packed GUI app's under its data folder.
    match std::env::var("KUI_LOG").ok().filter(|v| !v.is_empty()) {
        Some(v) if v == "-" => {}
        Some(v) => modules::log::to_file(std::path::Path::new(&v)).map_err(|e| format!("KUI_LOG {v}: {e}"))?,
        None if packed.is_some() && !args.headless => {
            let file = paths::data_dir(&project.id)
                .join("logs")
                .join(format!("{}.log", project::default_id(&project.name)));
            if let Err(e) = modules::log::to_file(&file) {
                eprintln!("kui: the log file {}: {e}", file.display());
            }
        }
        None => {}
    }
    if let Some(w) = project.pin.as_deref().and_then(compat::warning) {
        eprintln!("kui: {w}");
    }
    modules::set_task_error_exits(args.headless);
    let shared = modules::Shared::new(project.dir.clone(), None);
    let loaded = host::load_app(&project, shared.clone(), args.watch);
    // The app ended before its window opened — `--help`, a bad flag, a
    // script that was only a script: no window, and no error for it.
    if let Some(code) = *shared.exit.lock().unwrap() {
        if let Err(e) = &loaded
            && !e.contains(modules::EXIT_MARK)
        {
            eprintln!("kui: {e}");
        }
        shared.children.kill_all();
        return Ok(code);
    }
    let (ext, baseline, load_error) = loaded?;
    let lua = ext.lua().clone();
    let kit = kit::extension().map_err(|e| format!("Kit failed to load (a bug in the runner): {e}"))?;
    let watch = args.watch.then(|| host::Watch::start(&project, shared.clone()));
    let shot = args.shot.clone().map(|p| {
        let p = if p.is_absolute() { p } else { std::env::current_dir().unwrap_or_default().join(p) };
        host::Shot::new(p, args.after).recording(args.record)
    });
    let runner = host::Runner::new(lua, shared.clone(), &project, baseline, watch, load_error, shot);
    let (w, h) = args.size.unwrap_or(project.size);
    let mut launcher = kui_native::app(&project.name)
        .chrome(project.chrome)
        .titlebar(project.titlebar)
        .extension_as("app", ext)
        .extension_as(kit::NAMESPACE, kit);
    if args.appearance != kui_native::Appearance::Unknown {
        launcher = launcher.system(kui_native::SystemEnv { appearance: args.appearance, ..Default::default() });
    }
    // The devtools are on in every runner app, hidden until F12 (or the
    // macOS View menu) brings them up at the Log tab (ADR 0068, decision
    // 21); `--devtools` or the manifest's `devtools` opens with them shown.
    if args.devtools || project.devtools {
        launcher = launcher.devtools(true);
    } else if project.devtools_reachable && !args.headless {
        launcher = launcher.setup_core(|core| {
            core.set_devtools(true);
            core.set_devtools_dock(kui_core::DevtoolsDock::Off);
        });
    }
    if project.devtools_reachable
        && let Some(f12) = kui_core::Accel::parse("f12")
    {
        launcher = launcher.devtools_key(f12);
    }
    if let Some((w, h)) = project.min_size {
        launcher = launcher.min_size(w, h);
    }
    if let Some(icon) = &project.icon {
        let bytes = std::fs::read(icon).map_err(|e| format!("icon {}: {e}", icon.display()))?;
        launcher = launcher.try_icon_bytes(&bytes).map_err(|e| format!("icon {}: {e}", icon.display()))?;
    }
    modules::on_interrupt(shared.clone());
    if args.headless {
        launcher.open_headless(runner, w as f32, h as f32, 1.0).run();
    } else {
        launcher.size(w, h).run(runner).map_err(|e| e.to_string())?;
    }
    Ok(shared.exit.lock().unwrap().unwrap_or(0))
}

fn check(args: &Args) -> Result<i32, String> {
    let project = project::find(&args.path)?;
    let files = project::teal_files(&project.dir);
    if files.is_empty() {
        println!("no .tl files under {}; plain .lua needs no check", project.dir.display());
        return Ok(0);
    }
    let problems = teal::check(&project.dir, &files)?;
    for p in &problems {
        let file = p.file.strip_prefix(&project.dir).unwrap_or(&p.file);
        println!("{}:{}:{}: {}", file.display(), p.line, p.col, p.msg);
    }
    if problems.is_empty() {
        println!("ok: {} file(s) checked", files.len());
        Ok(0)
    } else {
        println!("{} problem(s) in {} file(s)", problems.len(), files.len());
        Ok(1)
    }
}

const NEW_MAIN: &str = r#"-- Your app: `kui run` (or `kui run --watch`) in this folder.
local kit = require("kit.ui")

-- State is yours; init() runs once, and a --watch reload keeps `model`.
function init()
  return { count = 0, joke = nil }
end

-- Every event your nodes raise, and every kui.dispatch, arrives here.
function update(msg)
  if msg.kind == "add" then
    model.count = model.count + 1
  elseif msg.kind == "fetch" then
    -- A task: the request waits without stopping the window drawing.
    kui.spawn(function()
      local r = kui.http.get("https://icanhazdadjoke.com/", { headers = { accept = "text/plain" } })
      kui.dispatch({ kind = "joke", text = r.body })
    end)
  elseif msg.kind == "joke" then
    model.joke = msg.text
  end
end

function view(env)
  return column { width = "grow", height = "grow",
    kit.settings({ skin = "ledger" }),
    kit.toolbar({}, {
      kit.button({ label = "Add one", variant = "primary", on_press = { kind = "add" } }),
      kit.button({ label = "Fetch a joke", on_press = { kind = "fetch" } }),
    }),
    column { pad = 16, gap = 12,
      kit.stat({ label = "Count", value = tostring(model.count) }),
      model.joke and kit.code({ text = model.joke, wrap = true }) or nil,
    },
  }
end
"#;

fn new(args: &Args) -> Result<i32, String> {
    let dir = &args.path;
    if dir.as_os_str() == "." {
        return Err("kui new <dir>: name the folder to start the app in".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let main = dir.join("main.lua");
    if main.exists() {
        return Err(format!("{} exists; not overwriting it", main.display()));
    }
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "app".into());
    std::fs::write(&main, NEW_MAIN).map_err(|e| format!("{}: {e}", main.display()))?;
    std::fs::write(
        dir.join("kui.config.lua"),
        format!("return {{\n  name = {name:?},\n  entry = \"main.lua\",\n  size = {{ w = 900, h = 600 }},\n}}\n"),
    )
    .map_err(|e| e.to_string())?;
    types(dir)?;
    println!("started {}: kui run --watch {}", dir.display(), dir.display());
    Ok(0)
}

/// `kui types`: the declarations an editor reads, in `.kui/types`, and a
/// `.luarc.json` that points lua-language-server at them (left alone when
/// there is one).
fn types(dir: &std::path::Path) -> Result<(), String> {
    let types = dir.join(".kui").join("types");
    teal::write_types(&types).map_err(|e| format!("{}: {e}", types.display()))?;
    std::fs::write(dir.join(".kui").join(".gitignore"), "*\n").map_err(|e| e.to_string())?;
    let luarc = dir.join(".luarc.json");
    if !luarc.exists() {
        std::fs::write(
            &luarc,
            r#"{
  "runtime.version": "Lua 5.4",
  "workspace.library": [".kui/types"],
  "diagnostics.globals": ["view", "update", "init", "on_message", "model"]
}
"#,
        )
        .map_err(|e| format!("{}: {e}", luarc.display()))?;
    }
    // Teal finds `kui.d.tl` and `kit/ui.d.tl` through `include_dir`.
    let tlconfig = dir.join("tlconfig.lua");
    if !tlconfig.exists() {
        std::fs::write(
            &tlconfig,
            "return { include_dir = { \".kui/types\" }, global_env_def = \"kui\" }\n",
        )
        .map_err(|e| format!("{}: {e}", tlconfig.display()))?;
    }
    println!("wrote {} (LuaLS and Teal declarations)", types.display());
    Ok(())
}

const PACK_USAGE: &str = "\
kui pack — build the app for any platform (ADR 0068)

usage: kui pack [path] [options]

  --target T,...       linux-x64, linux-arm64, darwin-arm64, win32-x64, or all (default: this machine)
  --format F,...       exe (Windows), app (macOS), bin (Linux), npm, kui (the archive alone);
                       default: each target's own
  --out DIR            where the builds go (default: dist)
  --bytecode           compile the app's Lua to bytecode (sources by default)
  --strip              with --bytecode, drop the debug information
  --installer URL      also write install.sh and install.ps1 fetching the builds from URL
  --runner FILE        use this runner binary rather than the release's
  --runner-mirror URL  fetch runners from a mirror with the release's layout (also KUI_RUNNER_MIRROR)

The app needs an id in kui.config.lua. It packs the runner it pins (kui = \"<version>\"),
else this one; runners for other versions or platforms come from the release.
";

fn split_list(v: &str) -> Vec<String> {
    v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

fn pack_cmd(argv: &[String]) -> Result<i32, String> {
    let mut o = pack::Opts {
        path: PathBuf::from("."),
        targets: Vec::new(),
        formats: Vec::new(),
        out: PathBuf::from("dist"),
        bytecode: false,
        strip: false,
        installer: None,
        runner: None,
        mirror: None,
    };
    let mut it = argv.iter();
    let mut path = None;
    while let Some(a) = it.next() {
        let mut val = |name: &str| it.next().cloned().ok_or(format!("{name} needs a value\n\n{PACK_USAGE}"));
        match a.as_str() {
            "-h" | "--help" => {
                print!("{PACK_USAGE}");
                return Ok(0);
            }
            "--target" => o.targets.extend(split_list(&val("--target")?)),
            "--format" => o.formats.extend(split_list(&val("--format")?)),
            "--out" => o.out = PathBuf::from(val("--out")?),
            "--bytecode" => o.bytecode = true,
            "--strip" => o.strip = true,
            "--installer" => o.installer = Some(val("--installer")?),
            "--runner" => o.runner = Some(PathBuf::from(val("--runner")?)),
            "--runner-mirror" => o.mirror = Some(val("--runner-mirror")?),
            other if other.starts_with("--") => return Err(format!("unknown option {other}\n\n{PACK_USAGE}")),
            other => {
                if path.replace(PathBuf::from(other)).is_some() {
                    return Err(format!("one path at a time\n\n{PACK_USAGE}"));
                }
            }
        }
    }
    if let Some(p) = path {
        o.path = p;
    }
    if o.strip && !o.bytecode {
        return Err("--strip drops bytecode's debug information: it needs --bytecode".into());
    }
    o.mirror = release::mirror(o.mirror.as_deref());
    pack::run(&o)
}

/// `kui upgrade [version]`: this binary replaced by the release's.
fn upgrade(argv: &[String]) -> Result<i32, String> {
    let mut version = "latest".to_string();
    let mut mirror = None;
    let mut it = argv.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--mirror" => mirror = it.next().cloned(),
            "-h" | "--help" => {
                println!("kui upgrade [version] [--mirror URL]: replace this kui with the release's (default: the latest)");
                return Ok(0);
            }
            v => version = v.to_string(),
        }
    }
    let mirror = release::mirror(mirror.as_deref());
    let exe = std::env::current_exe().map_err(|e| format!("this kui's path: {e}"))?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let shown = exe.to_string_lossy().replace('\\', "/");
    if shown.contains("/node_modules/") {
        return Err(format!("this kui came with npm: npm i -g @qxuken/kui-cli@{version}"));
    }
    if shown.contains("/Cellar/") || shown.contains("/homebrew/") || shown.contains("/linuxbrew/") {
        return Err("this kui came with Homebrew: brew upgrade kui".into());
    }
    if shown.contains("/target/debug/") || shown.contains("/target/release/") {
        return Err(format!("this kui is a build from source ({}); cargo build it again", exe.display()));
    }
    let platform = release::current_platform().ok_or("the release has no runner for this machine")?;
    let version = release::resolve(&version, mirror.as_deref())?;
    if version == compat::VERSION {
        println!("kui {version} already");
        return Ok(0);
    }
    let base = release::base_url(mirror.as_deref(), &version);
    let sums = release::sums(&base)?;
    let asset = release::fetch_verified(&base, &release::asset_name(&version, platform), &sums)?;
    let bin = release::binary_from_asset(&asset, platform)?;
    let new = exe.with_extension("new");
    std::fs::write(&new, &bin).map_err(|e| format!("{}: {e}", new.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    // A running exe cannot be deleted on Windows, but it can be renamed.
    if cfg!(windows) {
        let old = exe.with_extension("old");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(&exe, &old).map_err(|e| format!("{}: {e}", exe.display()))?;
    }
    std::fs::rename(&new, &exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    println!("kui {} → {version} ({})", compat::VERSION, exe.display());
    Ok(0)
}

/// `kui runners fetch <version> --to <dir>`.
fn runners(argv: &[String]) -> Result<i32, String> {
    let usage = "kui runners fetch <version|latest> --to <dir> [--mirror URL]";
    let mut it = argv.iter();
    if it.next().map(String::as_str) != Some("fetch") {
        return Err(format!("usage: {usage}"));
    }
    let (mut version, mut to, mut mirror) = (None, None, None);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--to" => to = it.next().cloned(),
            "--mirror" => mirror = it.next().cloned(),
            v => version = Some(v.to_string()),
        }
    }
    let (Some(version), Some(to)) = (version, to) else {
        return Err(format!("usage: {usage}"));
    };
    let mirror = release::mirror(mirror.as_deref());
    release::fetch_all(&version, std::path::Path::new(&to), mirror.as_deref())?;
    Ok(0)
}
