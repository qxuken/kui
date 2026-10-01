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
//!     cargo run -p kui-devtools --bin smoke -- --jobs 1           # one at a time
//!
//! **In parallel.** `--jobs N` runs N at once: by default eight windows,
//! and as many headless drives as there are cores. A window wholly
//! covered by another is `Occluded` on macOS and presents nothing, and
//! the round counts presents, so windows opened in the same place would
//! finish one after another however many ran; each job's windows open at
//! a place of their own instead (`KUI_WINDOW_AT`, cascaded by the job's
//! slot), overlapping but never covered. Rows print as runs finish.
//!
//! **The windowed round.** `KUI_SMOKE_FRAMES=n` (crates/kui-native/src/lib.rs)
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

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::sync::Mutex;
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
    /// `None` until `--jobs`: the round's own default.
    jobs: Option<usize>,
}

const USAGE: &str = "usage: smoke [--headless [--list]] [--frames N] [--timeout S] [--only a,b] \
                     [--release] [--base light,dark] [--node] [--no-build] [--jobs N]";

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
        jobs: None,
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
            "--jobs" | "-j" => {
                let n: usize = value(&mut i, "--jobs")?
                    .parse()
                    .map_err(|e| format!("--jobs: {e}"))?;
                o.jobs = Some(n.max(1));
            }
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

fn indent(out: &mut String, text: &str, lines: usize) {
    for l in text.lines().filter(|l| !l.trim().is_empty()).take(lines) {
        out.push_str(&format!("                       {l}\n"));
    }
}

/// Prints one row and says whether it passed. The row and what follows it
/// go out in one write, so rows of runs finishing together do not mix.
fn report(label: &str, outcome: Outcome) -> bool {
    let mut out = String::new();
    let passed = match outcome {
        Outcome::Ok { secs, warnings } => {
            let note = if warnings.is_empty() {
                String::new()
            } else {
                format!(" ({} warning(s))", warnings.len())
            };
            out.push_str(&format!("  {label} ok       {secs:.2}s{note}\n"));
            for w in &warnings {
                out.push_str(&format!("                       {w}\n"));
            }
            true
        }
        Outcome::Failed { code, stderr } => {
            out.push_str(&format!("  {label} FAILED   exit {code}\n"));
            indent(&mut out, &stderr, 12);
            false
        }
        Outcome::Hung => {
            out.push_str(&format!("  {label} HUNG     (killed at the timeout)\n"));
            false
        }
    };
    let _ = std::io::stdout().lock().write_all(out.as_bytes());
    passed
}

/// One run of a round: what the row says, and the command.
struct Job {
    label: String,
    /// The name `FAILED:` lists it under.
    name: String,
    cmd: Command,
    timeout: Duration,
}

/// Runs `jobs` on `width` threads, each taking the next job when its last
/// is done, and returns the names of those that failed, in job order.
/// `place` gives a job's command what its slot (0..width) should add —
/// the windowed round's window position.
fn pool(
    jobs: Vec<Job>,
    width: usize,
    place: impl Fn(usize, &mut Command) + Sync,
) -> Result<Vec<String>, String> {
    let total = jobs.len();
    let queue = Mutex::new(jobs.into_iter().enumerate());
    let failed = Mutex::new(Vec::new());
    let error = Mutex::new(None);
    std::thread::scope(|scope| {
        for slot in 0..width.clamp(1, total.max(1)) {
            let (queue, failed, error, place) = (&queue, &failed, &error, &place);
            scope.spawn(move || {
                loop {
                    let Some((i, mut job)) = queue.lock().unwrap().next() else {
                        return;
                    };
                    place(slot, &mut job.cmd);
                    match run(job.cmd, job.timeout) {
                        Ok(outcome) => {
                            if !report(&job.label, outcome) {
                                failed.lock().unwrap().push((i, job.name));
                            }
                        }
                        Err(e) => {
                            *error.lock().unwrap() = Some(e);
                            return;
                        }
                    }
                }
            });
        }
    });
    if let Some(e) = error.into_inner().unwrap() {
        return Err(e);
    }
    let mut failed = failed.into_inner().unwrap();
    failed.sort();
    Ok(failed.into_iter().map(|(_, name)| name).collect())
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

/// Windows open at once by default (fewer on a machine with fewer
/// cores). On an M3 Pro the 98 windows of the round took 143 s one at a
/// time, 38 s four at a time and 24 s eight; eight keeps the cascade
/// (`place_window`) inside a laptop's screen.
const WINDOWS_AT_ONCE: usize = 8;

/// Where slot `slot` of `width` opens its windows: each slot a step down
/// and a step *left* of the one before. A window covers another only when
/// it starts both left of it and above it, and no two slots on this
/// diagonal stand that way, so whatever the examples' sizes and whichever
/// opened last, every window keeps a strip no other covers and none is
/// `Occluded` (a cascade down and right let a big window in the first slot
/// bury a small one in the second, and the buried one waited it out).
fn place_window(slot: usize, width: usize, cmd: &mut Command) {
    let x = 40 + 90 * (width - 1 - slot.min(width - 1));
    let y = 60 + 60 * slot;
    cmd.env("KUI_WINDOW_AT", format!("{x},{y}"));
}

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
        let mut args = vec![
            "build",
            "--profile",
            profile,
            "-p",
            "kui-native",
            "--examples",
        ];
        if opts.release {
            args.extend(["--features", "smoke"]);
        }
        cargo(&args)?;
    }
    let mut failed = Vec::new();
    let mut jobs = Vec::new();
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
            jobs.push(Job {
                label,
                name: format!("{name}/{base}"),
                cmd,
                timeout: opts.timeout,
            });
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
                jobs.push(Job {
                    label,
                    name: format!("node:{short}/{base}"),
                    cmd,
                    timeout: opts.timeout,
                });
            }
        }
    }
    let width = opts.jobs.unwrap_or_else(|| {
        std::thread::available_parallelism().map_or(1, |n| n.get().min(WINDOWS_AT_ONCE))
    });
    println!();
    println!(
        "smoke: {} examples × ({}){}, {} frames each, {profile} profile, {width} at a time",
        examples.len(),
        opts.bases.join(" "),
        if opts.node { " and Node's" } else { "" },
        opts.frames
    );
    println!();
    let started = Instant::now();
    // A lone window opens where the OS puts it, as it always has.
    failed.extend(pool(jobs, width, |slot, cmd| {
        if width > 1 {
            place_window(slot, width, cmd)
        }
    })?);
    if failed.is_empty() {
        println!();
        println!(
            "all {} examples drew {} frames on each base and exited cleanly ({:.1}s)",
            examples.len(),
            opts.frames,
            started.elapsed().as_secs_f64()
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
        // Build first, in one cargo, so a compile error is one message and
        // not one per example — and so every drive below is a binary to
        // start, not a `cargo run` to check for freshness.
        let mut crates: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
        crates.sort();
        crates.dedup();
        let mut args = vec!["build"];
        for krate in crates {
            args.extend(["-p", krate]);
        }
        args.push("--examples");
        cargo(&args)?;
    }
    let mut jobs = Vec::new();
    // Node's first: `npm run build` inside it is a whole typecheck, the
    // longest run of the round, so it starts while the drives fill in.
    let mut cmd = npm();
    cmd.args(["run", "smoke"]).current_dir("examples/node");
    jobs.push(Job {
        label: format!("{:<12} {:<14}", "node", "smoke"),
        name: "node/smoke".into(),
        cmd,
        timeout: opts.timeout.max(Duration::from_secs(300)),
    });
    let exe_suffix = if cfg!(windows) { ".exe" } else { "" };
    for (krate, name) in &pairs {
        let mut cmd = Command::new(format!("target/debug/examples/{name}{exe_suffix}"));
        cmd.arg("--headless");
        jobs.push(Job {
            label: format!("{krate:<12} {name:<14}"),
            name: format!("{krate}/{name}"),
            cmd,
            timeout: opts.timeout,
        });
    }
    let width = opts
        .jobs
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));
    let started = Instant::now();
    let failed = pool(jobs, width, |_, _| {})?;
    if failed.is_empty() {
        println!();
        println!(
            "the headless round passed ({:.1}s, {width} at a time)",
            started.elapsed().as_secs_f64()
        );
    }
    Ok(failed)
}
