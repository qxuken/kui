//! The runner-app test controller (ADR 0072). File children never parse
//! terminal output as a protocol; their versioned results are files.
pub(crate) mod artifact;
mod drawing;
mod paths;
mod report;
mod session;
pub(crate) mod ui;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Options {
    pub root: PathBuf,
    pub output: PathBuf,
    pub filter: Option<String>,
    #[serde(default)]
    pub exact: bool,
    pub update: bool,
    pub system_fonts: bool,
    pub headed: bool,
    pub debug: bool,
    pub ui: bool,
    pub every_frame: bool,
    #[serde(default)]
    pub watch: bool,
    #[serde(default)]
    pub perf: bool,
    #[serde(default)]
    pub json: bool,
    pub timeout: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseResult {
    pub name: String,
    pub ok: bool,
    pub error: Option<String>,
    pub seconds: f64,
    pub file: String,
    pub line: u32,
}
#[derive(Default, Serialize, Deserialize)]
pub struct Results {
    pub version: u32,
    #[serde(default)]
    pub complete: bool,
    pub cases: Vec<CaseResult>,
}

static SUPERVISOR: std::sync::OnceLock<std::sync::Arc<crate::modules::Shared>> =
    std::sync::OnceLock::new();
fn interrupted() -> Option<i32> {
    SUPERVISOR.get().and_then(|s| *s.exit.lock().unwrap())
}
struct FileChild(std::process::Child);
impl std::ops::Deref for FileChild {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for FileChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for FileChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            crate::modules::process::kill_tree(self.0.id());
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

const USAGE: &str = "kui test [path] [--filter text] [--update] [--watch]\n  --headed          show automated execution\n  --debug           show execution and inspector, paused before actions\n  --ui              open the Kit test inspector\n  --system-fonts    use the machine's fonts\n  --every-frame     capture images at every frame\n  --perf            attach per-app performance recordings\n  --reporter junit|json  JUnit file or machine-readable JSON on stdout\n  --json            machine-readable report, with failure details and log paths\n  kui test report|output|events ARTIFACT [--json] [--file NAME] [--case NAME] [--step ID]\n  --output DIR      artifact folder (default test-results)\n  --timeout MS      real-time file timeout (default 30000; paused time excluded)\n";

pub fn command(argv: &[String]) -> Result<i32, String> {
    if argv
        .first()
        .is_some_and(|a| matches!(a.as_str(), "report" | "output" | "events"))
    {
        return report::command(argv);
    }
    if argv.iter().any(|a| a == "--help" || a == "-h") {
        print!("{USAGE}");
        return Ok(0);
    }
    let mut options = Options {
        root: PathBuf::from("."),
        timeout: 30_000.0,
        ..Default::default()
    };
    let mut watch = false;
    let mut junit = false;
    let mut path = false;
    let mut args = argv.iter();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--filter" => options.filter = Some(args.next().ok_or("--filter needs text")?.clone()),
            "--output" => {
                options.output = PathBuf::from(args.next().ok_or("--output needs a folder")?)
            }
            "--timeout" => {
                options.timeout = args
                    .next()
                    .ok_or("--timeout needs milliseconds")?
                    .parse()
                    .map_err(|_| "invalid timeout")?
            }
            "--reporter" => match args.next().map(String::as_str) {
                Some("junit") => junit = true,
                Some("json") => options.json = true,
                _ => return Err("--reporter expects junit or json".into()),
            },
            "--json" => options.json = true,
            "--update" => options.update = true,
            "--system-fonts" => options.system_fonts = true,
            "--headed" => options.headed = true,
            "--debug" => {
                options.debug = true;
                options.headed = true;
                options.ui = true;
            }
            "--ui" => options.ui = true,
            "--watch" => watch = true,
            "--every-frame" => options.every_frame = true,
            "--perf" => options.perf = true,
            a if a.starts_with('-') => return Err(format!("unknown test option {a}")),
            _ if !path => {
                options.root = PathBuf::from(a);
                path = true;
            }
            _ => return Err(format!("unexpected test argument {a}")),
        }
    }
    if !options.timeout.is_finite() || options.timeout <= 0.0 {
        return Err("timeout must be finite and positive".into());
    }
    if options.root.join("run.json").is_file() {
        if !options.ui {
            return Err("open a recorded run with --ui".into());
        }
        let run: serde_json::Value = serde_json::from_slice(
            &std::fs::read(options.root.join("run.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if run["version"] != 1 {
            return Err("unsupported test-run artifact version".into());
        }
        return ui::open_artifact(&options.root);
    }
    let requested = options.root.clone();
    let app_root = if requested.is_file() {
        let absolute = std::path::absolute(&requested).map_err(|e| e.to_string())?;
        absolute
            .ancestors()
            .skip(1)
            .find(|p| p.join("kui.config.lua").is_file() || p.join("kui.config.tl").is_file())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| requested.parent().unwrap_or(Path::new(".")).to_path_buf())
    } else {
        requested.clone()
    };
    let project = crate::project::find(&app_root)?;
    let selected = if options.root.is_file() {
        Some(crate::paths::canonical(&options.root).map_err(|e| e.to_string())?)
    } else {
        None
    };
    options.root = project.dir.clone();
    if options.output.as_os_str().is_empty() {
        options.output = project.dir.join("test-results");
    }
    options.output = std::path::absolute(&options.output).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&options.output).map_err(|e| e.to_string())?;
    artifact::atomic_json(&options.output.join("options.json"), &options)?;
    options.watch = watch;
    artifact::atomic_json(&options.output.join("options.json"), &options)?;
    if options.json && (options.ui || watch) {
        return Err(
            "--json is for a single headless/headed run; use test report for a live inspector"
                .into(),
        );
    }
    if options.ui {
        return ui::launch(options, selected, junit);
    }
    let signals = crate::modules::Shared::new(options.root.clone(), None);
    crate::modules::on_interrupt(signals.clone());
    let _ = SUPERVISOR.set(signals);
    let mut previous = source_stamp(&options.root, &options.output);
    loop {
        let code = match run(&options, selected.as_deref(), junit) {
            Ok(code) => code,
            Err(error) if options.json => {
                println!(
                    "{}",
                    serde_json::json!({"version":1,"status":"error","ok":false,"error":error})
                );
                return Ok(1);
            }
            Err(error) => return Err(error),
        };
        if let Some(code) = interrupted() {
            return Ok(code);
        }
        if !watch {
            return Ok(code);
        }
        println!("Watching {}", options.root.display());
        loop {
            std::thread::sleep(Duration::from_millis(200));
            if let Some(code) = interrupted() {
                return Ok(code);
            }
            let current = source_stamp(&options.root, &options.output);
            if current != previous {
                previous = current;
                break;
            }
        }
    }
}

pub fn discover(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    crate::project::walk(root, &mut |p| {
        let name = p.file_name().unwrap_or_default().to_string_lossy();
        let rel = p.strip_prefix(root).unwrap_or(p);
        if rel.components().any(|c| {
            matches!(
                c.as_os_str().to_str(),
                Some("__snapshots__" | "test-results" | "fixtures")
            )
        }) {
            return;
        }
        let ext = p.extension().and_then(|x| x.to_str());
        if !matches!(ext, Some("lua" | "tl"))
            || name.ends_with(".d.tl")
            || name.starts_with("kui.config.")
        {
            return;
        }
        let under_test = rel.components().any(|c| c.as_os_str() == "test");
        if !under_test && !name.ends_with("_test.lua") && !name.ends_with("_test.tl") {
            return;
        }
        if ext == Some("lua") && p.with_extension("tl").is_file() {
            return;
        }
        files.push(p.to_path_buf());
    });
    files.sort();
    files
}
fn source_stamp(root: &Path, output: &Path) -> Vec<(PathBuf, std::time::SystemTime, u64)> {
    let mut out = Vec::new();
    crate::project::walk(root, &mut |p| {
        if p.starts_with(output)
            || p.components().any(|c| {
                matches!(
                    c.as_os_str().to_str(),
                    Some("__snapshots__" | "test-results")
                )
            })
        {
            return;
        }
        if let Ok(m) = p.metadata() {
            out.push((
                p.to_path_buf(),
                m.modified().unwrap_or(std::time::UNIX_EPOCH),
                m.len(),
            ));
        }
    });
    out.sort();
    out
}
pub fn run(options: &Options, selected: Option<&Path>, junit: bool) -> Result<i32, String> {
    let files = selected
        .map(|p| vec![p.to_path_buf()])
        .unwrap_or_else(|| discover(&options.root));
    if files.is_empty() {
        return Err(format!("no tests found in {}", options.root.display()));
    }
    let _ = std::fs::remove_file(options.output.join("cancel"));
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = format!(
        "{}-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let relative = format!("runs/{id}");
    let run_root = options.output.join(&relative);
    std::fs::create_dir_all(&run_root).map_err(|e| e.to_string())?;
    artifact::atomic_json(&run_root.join("options.json"), options)?;
    let manifest = |status: &str, failed: usize| serde_json::json!({"version":1,"id":id,"root":options.root,"status":status,"artifact_root":relative,"files":files,"failed":failed});
    artifact::atomic_json(&options.output.join("run.json"), &manifest("running", 0))?;
    let mut saved_manifest = manifest("running", 0);
    saved_manifest
        .as_object_mut()
        .unwrap()
        .remove("artifact_root");
    artifact::atomic_json(&run_root.join("run.json"), &saved_manifest)?;
    artifact::atomic_json(
        &options.output.join("results.json"),
        &Results {
            version: 1,
            ..Default::default()
        },
    )?;
    let mut results = Results {
        version: 1,
        ..Default::default()
    };
    for (i, file) in files.iter().enumerate() {
        let first_case = results.cases.len();
        if !options.json {
            println!(
                "RUN  {} ({}/{})",
                file.strip_prefix(&options.root).unwrap_or(file).display(),
                i + 1,
                files.len()
            );
        }
        if options.output.join("cancel").exists() || interrupted().is_some() {
            break;
        }
        let dir = run_root.join(format!("file-{i:04}"));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let tmp = tempfile::Builder::new()
            .prefix("kui-test-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        let log = std::fs::File::create(dir.join("output.log")).map_err(|e| e.to_string())?;
        let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        if options.perf {
            command.env_remove("KUI_PERF_TRACE");
        }
        let mut child = FileChild(
            command
                .arg("__test-file")
                .arg(run_root.join("options.json"))
                .arg(file)
                .arg(&dir)
                .arg(tmp.path())
                .env("TZ", "UTC")
                .env("LC_ALL", "C")
                .env("LANG", "C")
                .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
                .stderr(Stdio::from(log))
                .spawn()
                .map_err(|e| e.to_string())?,
        );
        let mut elapsed = Duration::ZERO;
        let mut last = Instant::now();
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            let now = Instant::now();
            let paused = std::fs::read_to_string(dir.join("status.json"))
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .is_some_and(|v| v["paused"] == true);
            if !paused {
                elapsed += now - last;
            }
            last = now;
            if interrupted().is_some()
                || options.output.join("cancel").exists()
                || elapsed.as_secs_f64() * 1000.0 > options.timeout
            {
                timed_out = interrupted().is_none() && !options.output.join("cancel").exists();
                crate::modules::process::kill_tree(child.id());
                let _ = child.kill();
                break child.wait().map_err(|e| e.to_string())?;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let retained = std::fs::read(dir.join("retained.json"))
            .ok()
            .and_then(|v| serde_json::from_slice::<Vec<PathBuf>>(&v).ok())
            .unwrap_or_default();
        if retained.iter().any(|p| p.starts_with(tmp.path())) {
            crate::context::cleanup_except(tmp.path(), &retained);
            if !options.json {
                println!("Retained temporary resources: {}", tmp.path().display());
            }
            let _ = tmp.keep();
        }
        let child_results = std::fs::read(dir.join("results.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Results>(&b).ok());
        let complete = child_results
            .as_ref()
            .is_some_and(|r| r.version == 1 && r.complete);
        let reported_failure = child_results
            .as_ref()
            .is_some_and(|r| r.complete && r.cases.iter().any(|c| !c.ok));
        if let Some(child_results) = child_results {
            results.cases.extend(child_results.cases);
        }
        if (!status.success() || !complete)
            && !reported_failure
            && interrupted().is_none()
            && !options.output.join("cancel").exists()
        {
            let log = std::fs::read_to_string(dir.join("output.log")).unwrap_or_default();
            let failure = CaseResult {
                name: file
                    .strip_prefix(&options.root)
                    .unwrap_or(file)
                    .display()
                    .to_string(),
                ok: false,
                error: Some(if timed_out {
                    format!("test file timed out after {} ms", options.timeout)
                } else {
                    format!(
                        "test file exited with {status}{}\n{log}",
                        if complete {
                            ""
                        } else {
                            " with incomplete results"
                        }
                    )
                }),
                seconds: elapsed.as_secs_f64(),
                file: file.display().to_string(),
                line: 1,
            };
            if let Some(case) = results.cases[first_case..].iter_mut().rev().find(|c| !c.ok) {
                case.error = Some(format!(
                    "{}\n{}",
                    case.error.take().unwrap_or_default(),
                    failure.error.unwrap_or_default()
                ));
            } else {
                results.cases.push(failure);
            }
        }
        artifact::atomic_json(&options.output.join("results.json"), &results)?;
        artifact::atomic_json(&run_root.join("results.json"), &results)?;
        if !options.json {
            for case in &results.cases[first_case..] {
                println!(
                    "  {}  {} ({:.3}s)",
                    if case.ok { "PASS" } else { "FAIL" },
                    case.name,
                    case.seconds
                );
                if let Some(err) = &case.error {
                    println!("{}:{}\n{err}", case.file, case.line);
                }
            }
        }
        if !options.json && results.cases[first_case..].iter().any(|c| !c.ok) {
            println!("Output: {}", dir.join("output.log").display());
        }
    }
    let failed = results.cases.iter().filter(|c| !c.ok).count();
    if !options.json {
        println!("\n{} passed, {failed} failed", results.cases.len() - failed);
        let mut slow = results.cases.iter().collect::<Vec<_>>();
        slow.sort_by(|a, b| b.seconds.total_cmp(&a.seconds));
        for c in slow.iter().take(5) {
            println!("  {:.3}s  {}", c.seconds, c.name);
        }
    }
    let cancelled = interrupted().is_some() || options.output.join("cancel").exists();
    let unmatched = results.cases.is_empty() && !cancelled;
    results.complete = !cancelled;
    artifact::atomic_json(&options.output.join("results.json"), &results)?;
    if junit {
        artifact::junit(&options.output.join("junit.xml"), &results)?;
    }
    artifact::atomic_json(&run_root.join("results.json"), &results)?;
    let status = if cancelled {
        "cancelled"
    } else if unmatched {
        "error"
    } else if failed > 0 {
        "failed"
    } else {
        "passed"
    };
    let mut finished = manifest(status, failed);
    if unmatched {
        finished["error"] = serde_json::json!("no test cases matched");
    }
    artifact::atomic_json(&options.output.join("run.json"), &finished)?;
    let mut saved = finished;
    saved.as_object_mut().unwrap().remove("artifact_root");
    artifact::atomic_json(&run_root.join("run.json"), &saved)?;
    let report = report::read(&options.output)?;
    artifact::atomic_json(&options.output.join("report.json"), &report)?;
    artifact::atomic_json(&run_root.join("report.json"), &report)?;
    if options.json {
        println!(
            "{}",
            serde_json::to_string(&report).map_err(|e| e.to_string())?
        );
    } else {
        println!("Artifacts: {}", run_root.display());
        println!("Report: {}", options.output.join("report.json").display());
    }
    Ok(interrupted().unwrap_or(if cancelled {
        130
    } else if failed == 0 && !unmatched {
        0
    } else {
        1
    }))
}
pub fn child(argv: &[String]) -> Result<i32, String> {
    if argv.len() != 4 {
        return Err("invalid internal test invocation".into());
    }
    let options: Options =
        serde_json::from_slice(&std::fs::read(&argv[0]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    session::run_file(
        options,
        PathBuf::from(&argv[1]),
        PathBuf::from(&argv[2]),
        PathBuf::from(&argv[3]),
    )
}
