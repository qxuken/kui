//! `kui`: runs a Lua or Teal app with Kit and a standard library, and
//! nothing else to install (ADR 0064).
//!
//!   kui run [path] [--headless] [--watch] [--size WxH]
//!   kui check [path]
//!   kui new <dir>
//!
//! `kui <path>` is `kui run <path>`.

mod host;
mod kit;
mod modules;
mod ops;
mod project;
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
  kui <path>                 the same as kui run <path>

run options:
  --watch        reload the app when its files change, keeping its model
  --headless     no window: for a server, a script, a test
  --size WxH     the window's size in logical px (default 1000x700)
  --shot FILE    no window: write a frame to FILE (.png) and end
  --after MS     with --shot, how long to run first (default 800)

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
}

fn parse() -> Result<Args, String> {
    let mut it = std::env::args().skip(1).peekable();
    let mut a = Args {
        cmd: "run".into(),
        path: PathBuf::from("."),
        watch: false,
        headless: false,
        size: None,
        shot: None,
        after: 800.0,
    };
    match it.peek().map(String::as_str) {
        Some("run" | "check" | "new" | "types") => a.cmd = it.next().unwrap(),
        Some("-h" | "--help" | "help") => a.cmd = "help".into(),
        Some("-V" | "--version" | "version") => a.cmd = "version".into(),
        _ => {}
    }
    let mut path = None;
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--watch" => a.watch = true,
            "--headless" => a.headless = true,
            "--shot" => {
                a.shot = Some(PathBuf::from(it.next().ok_or("--shot needs a file, as out.png")?));
                a.headless = true;
            }
            "--after" => {
                let v = it.next().ok_or("--after needs milliseconds")?;
                a.after = v.parse().map_err(|_| format!("--after: bad milliseconds {v}"))?;
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
    let args = match parse() {
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
        _ => run(&args),
    };
    match result {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(e) => {
            eprintln!("kui: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> Result<i32, String> {
    let project = project::find(&args.path)?;
    project::load_dotenv(&project.dir);
    let shared = modules::Shared::new(project.dir.clone(), None);
    let (ext, baseline, load_error) = host::load_app(&project, shared.clone(), args.watch)?;
    let lua = ext.lua().clone();
    let kit = kit::extension().map_err(|e| format!("Kit failed to load (a bug in the runner): {e}"))?;
    let watch = args.watch.then(|| host::Watch::start(&project, shared.clone()));
    let shot = args.shot.clone().map(|p| {
        let p = if p.is_absolute() { p } else { std::env::current_dir().unwrap_or_default().join(p) };
        host::Shot::new(p, args.after)
    });
    let runner = host::Runner::new(lua, shared.clone(), &project, baseline, watch, load_error, shot);
    let (w, h) = args.size.unwrap_or(project.size);
    let launcher = kui_native::app(&project.name)
        .extension_as("app", ext)
        .extension_as(kit::NAMESPACE, kit);
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
