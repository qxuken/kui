//! The smoke round, one program for every platform (backlog AR4): what
//! `scripts/smoke-examples.sh`, `scripts/smoke-windows.ps1` and
//! `scripts/smoke-headless.sh` were, in the crate that already reads the
//! manifests and owns the `--headless` contract, so the round cannot drift
//! between a unix host and the Windows runner, which has no bash.
//!
//!     cargo run -p kui-devtools --bin smoke                    # windowed: every example, both bases
//!     cargo run -p kui-devtools --bin smoke -- --frames 300    # longer, for pacing bugs
//!     cargo run -p kui-devtools --bin smoke -- --only fragment,enter_exit
//!     cargo run -p kui-devtools --bin smoke -- --release       # the profile that ships
//!     cargo run -p kui-devtools --bin smoke -- --base light    # one base only
//!     cargo run -p kui-devtools --bin smoke -- --node          # the Node windows too
//!     cargo run -p kui-devtools --bin smoke -- --headless      # every headless drive, what CI runs
//!     cargo run -p kui-devtools --bin smoke -- --headless --list
//!
//! **The windowed round.** `KUI_SMOKE_FRAMES=n` (crates/kui/src/lib.rs)
//! makes the runner quit once the main window has presented n frames, so
//! an example is a self-terminating check: exit 0 means it drew n frames
//! and shut down, and anything else — a wgpu validation panic, a device
//! loss, a hang — is a failure with the stderr to read. A frame only
//! counts once a present has succeeded, so an example that opens a window
//! and never paints runs out the timeout rather than passing quietly.
//! Every example runs inside the devtools (ADR 0021), so `--light` and
//! `--dark` pin the theme base without the example knowing: each one is
//! opened twice, and a literal colour that reads on one base and not the
//! other is opened on both, every run. The dev profile honours the
//! variable unasked; a release build only with `--features smoke`, which
//! `--release` passes — an app you ship should not close its own window
//! over a variable in its environment. `--node` adds the Node windowed
//! examples on the same contract: it builds the addon in release with
//! kui-node's `smoke` feature, since that is the build `native.cjs` loads
//! first, then opens each entry of package.json's `kui.windowed`.
//!
//! **The headless round.** Every example that declares a self-check, run
//! with `--headless` and judged by its exit code (ADR 0021, decision 4):
//! each crate's `[package.metadata.kui] headless = [...]`, plus `npm run
//! smoke` for Node. An example listed without a drive exits 2 with "no
//! headless drive", so a name added before its drive goes red rather than
//! passing quietly. The C round is `cbuild --run`'s and stays
//! there: it builds what it runs.
//!
//! What neither covers: anything needing a human, and what was drawn.
//! `cargo test --workspace` checks the pixels, through the conformance
//! corpus; this checks that drawing them did not fail.
//!
//! Exit codes: 0 every example passed, 1 at least one failed, 2 a bad
//! flag or a missing name.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use kui_devtools::manifest;

struct Opts {
    headless: bool,
    list: bool,
    frames: u32,
    timeout: Duration,
    only: Vec<String>,
    release: bool,
    bases: Vec<String>,
    node: bool,
    build: bool,
}

const USAGE: &str = "usage: smoke [--headless [--list]] [--frames N] [--timeout S] [--only a,b] \
                     [--release] [--base light,dark] [--node] [--no-build]";

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        headless: false,
        list: false,
        frames: 120,
        timeout: Duration::from_secs(60),
        only: Vec::new(),
        release: false,
        bases: vec!["light".into(), "dark".into()],
        node: false,
        build: true,
    };
    let mut i = 0;
    let value = |i: &mut usize, flag: &str| -> Result<String, String> {
        *i += 1;
        args.get(*i)
            .cloned()
            .ok_or_else(|| format!("{flag} needs a value"))
    };
    while i < args.len() {
        match args[i].as_str() {
            "--headless" => o.headless = true,
            "--windowed" => o.headless = false,
            "--list" => o.list = true,
            "--frames" => {
                o.frames = value(&mut i, "--frames")?
                    .parse()
                    .map_err(|e| format!("--frames: {e}"))?
            }
            "--timeout" => {
                o.timeout = Duration::from_secs(
                    value(&mut i, "--timeout")?
                        .parse()
                        .map_err(|e| format!("--timeout: {e}"))?,
                )
            }
            "--only" => {
                o.only = value(&mut i, "--only")?
                    .split(',')
                    .map(str::to_string)
                    .collect()
            }
            "--release" => o.release = true,
            "--base" => {
                o.bases = value(&mut i, "--base")?
                    .split(',')
                    .map(str::to_string)
                    .collect()
            }
            "--node" => o.node = true,
            "--no-build" => o.build = false,
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown flag {other}\n{USAGE}")),
        }
        i += 1;
    }
    for b in &o.bases {
        if b != "light" && b != "dark" {
            return Err(format!("--base: `{b}` is not light or dark"));
        }
    }
    Ok(o)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = match parse(&args) {
        Ok(o) => o,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };
    // Every path below is relative to the workspace root, wherever this
    // was started from.
    if let Err(e) = std::env::set_current_dir(manifest::root()) {
        eprintln!("cannot enter the workspace root: {e}");
        return ExitCode::from(2);
    }
    let failed = if opts.headless {
        headless(&opts)
    } else {
        windowed(&opts)
    };
    match failed {
        Ok(f) if f.is_empty() => ExitCode::SUCCESS,
        Ok(f) => {
            println!();
            println!("FAILED: {}", f.join(" "));
            ExitCode::from(1)
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(2)
        }
    }
}

// ---------------------------------------------------------------------------
// Running one thing

/// What one run came to.
enum Outcome {
    Ok { secs: f64, warnings: Vec<String> },
    Failed { code: i32, stderr: String },
    Hung,
}

/// Starts `cmd` with stdout to nobody (what an example prints is not what
/// is judged) and stderr kept, waits up to `timeout`, and kills a hang.
fn run(mut cmd: Command, timeout: Duration) -> Result<Outcome, String> {
    let start = Instant::now();
    let mut child = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start {:?}: {e}", cmd.get_program()))?;
    // Read stderr on a thread so a chatty example cannot fill the pipe and
    // block on it while the parent is waiting for it to exit.
    let mut stderr = child.stderr.take().expect("piped");
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break Some(status);
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let stderr = reader.join().unwrap_or_default();
    let Some(status) = status else {
        return Ok(Outcome::Hung);
    };
    if status.success() {
        // kui prints one line per misconfiguration a frame noticed. Not a
        // crash, but an example of all things should not produce them.
        let warnings = stderr
            .lines()
            .filter(|l| l.starts_with("kui: warning"))
            .map(str::to_string)
            .collect();
        Ok(Outcome::Ok {
            secs: start.elapsed().as_secs_f64(),
            warnings,
        })
    } else {
        Ok(Outcome::Failed {
            code: status.code().unwrap_or(-1),
            stderr,
        })
    }
}

fn indent(text: &str, lines: usize) {
    for l in text.lines().filter(|l| !l.trim().is_empty()).take(lines) {
        println!("                       {l}");
    }
}

/// Prints one row and says whether it passed.
fn report(label: &str, outcome: Outcome) -> bool {
    match outcome {
        Outcome::Ok { secs, warnings } => {
            let note = if warnings.is_empty() {
                String::new()
            } else {
                format!(" ({} warning(s))", warnings.len())
            };
            println!("  {label} ok       {secs:.2}s{note}");
            for w in &warnings {
                println!("                       {w}");
            }
            true
        }
        Outcome::Failed { code, stderr } => {
            println!("  {label} FAILED   exit {code}");
            indent(&stderr, 12);
            false
        }
        Outcome::Hung => {
            println!("  {label} HUNG     (killed at the timeout)");
            false
        }
    }
}

/// A `cargo` invocation, inherited output, judged by its exit.
fn cargo(args: &[&str]) -> Result<(), String> {
    let status = Command::new("cargo")
        .args(args)
        .status()
        .map_err(|e| format!("cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` failed", args.join(" ")))
    }
}

/// `npm`, which on Windows is a `.cmd` and has to be started through the
/// shell.
fn npm() -> Command {
    if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", "npm"]);
        c
    } else {
        Command::new("npm")
    }
}

// ---------------------------------------------------------------------------
// The windowed round

fn windowed(opts: &Opts) -> Result<Vec<String>, String> {
    let all = manifest::windowed();
    let examples = if opts.only.is_empty() {
        all
    } else {
        for w in &opts.only {
            if !all.contains(w) {
                return Err(format!("no such example: {w}"));
            }
        }
        opts.only.clone()
    };
    let (profile, dir): (&str, PathBuf) = if opts.release {
        ("release", "target/release/examples".into())
    } else {
        ("dev", "target/debug/examples".into())
    };
    if opts.build {
        println!("building {profile} examples...");
        let mut args = vec!["build", "--profile", profile, "-p", "kui", "--examples"];
        if opts.release {
            args.extend(["--features", "smoke"]);
        }
        cargo(&args)?;
    }
    println!();
    println!(
        "smoke: {} examples × ({}), {} frames each, {profile} profile",
        examples.len(),
        opts.bases.join(" "),
        opts.frames
    );
    println!();
    let mut failed = Vec::new();
    let exe_suffix = if cfg!(windows) { ".exe" } else { "" };
    for name in &examples {
        for base in &opts.bases {
            let label = format!("{name:<14} {base:<5}");
            let exe = dir.join(format!("{name}{exe_suffix}"));
            if !exe.exists() {
                println!("  {label} MISSING  {}", exe.display());
                failed.push(format!("{name}/{base}"));
                continue;
            }
            let mut cmd = Command::new(&exe);
            cmd.arg(format!("--{base}"))
                .env("KUI_SMOKE_FRAMES", opts.frames.to_string());
            if !report(&label, run(cmd, opts.timeout)?) {
                failed.push(format!("{name}/{base}"));
            }
        }
    }
    if opts.node {
        println!();
        println!("node: building the addon with kui-node's smoke feature, then the examples...");
        cargo(&[
            "build",
            "-p",
            "kui-node",
            "--release",
            "--features",
            "smoke",
        ])?;
        let status = npm()
            .args(["run", "build"])
            .current_dir("examples/node")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("npm: {e}"))?;
        if !status.success() {
            return Err("`npm run build` failed in examples/node".into());
        }
        for entry in manifest::node_roster("windowed") {
            let short = entry.rsplit('/').next().unwrap_or(&entry);
            for base in &opts.bases {
                let label = format!("{:<14} {base:<5}", format!("node:{short}"));
                let mut cmd = Command::new("node");
                cmd.arg(format!("examples/node/dist/{entry}.mjs"))
                    .arg(format!("--{base}"))
                    .env("KUI_SMOKE_FRAMES", opts.frames.to_string());
                if !report(&label, run(cmd, opts.timeout)?) {
                    failed.push(format!("node:{short}/{base}"));
                }
            }
        }
    }
    if failed.is_empty() {
        println!();
        println!(
            "all {} examples drew {} frames on each base and exited cleanly",
            examples.len(),
            opts.frames
        );
    }
    Ok(failed)
}

// ---------------------------------------------------------------------------
// The headless round

fn headless(opts: &Opts) -> Result<Vec<String>, String> {
    let pairs = manifest::headless();
    println!("the headless round:");
    for (krate, name) in &pairs {
        println!("  cargo run -p {krate} --example {name} -- --headless");
    }
    println!("  (cd examples/node && npm run smoke)");
    if opts.list {
        return Ok(Vec::new());
    }
    println!();
    if opts.build {
        // Build first, so a compile error is one message and not one per
        // example.
        let mut crates: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
        crates.sort();
        crates.dedup();
        for krate in crates {
            cargo(&["build", "-p", krate, "--examples"])?;
        }
    }
    let mut failed = Vec::new();
    for (krate, name) in &pairs {
        let label = format!("{krate:<12} {name:<14}");
        let mut cmd = Command::new("cargo");
        cmd.args([
            "run",
            "-q",
            "-p",
            krate,
            "--example",
            name,
            "--",
            "--headless",
        ]);
        // Under `--no-build` the first `cargo run` may be a compile; give
        // it the room the Node step gets rather than calling it hung.
        let room = opts.timeout.max(Duration::from_secs(300));
        if !report(&label, run(cmd, room)?) {
            failed.push(format!("{krate}/{name}"));
        }
    }
    let label = format!("{:<12} {:<14}", "node", "smoke");
    let mut cmd = npm();
    cmd.args(["run", "smoke"]).current_dir("examples/node");
    // `npm run build` inside it is a whole typecheck; give it the room.
    if !report(
        &label,
        run(cmd, opts.timeout.max(Duration::from_secs(300)))?,
    ) {
        failed.push("node/smoke".into());
    }
    if failed.is_empty() {
        println!();
        println!("the headless round passed");
    }
    Ok(failed)
}
